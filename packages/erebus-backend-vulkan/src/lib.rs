//! Implementation of the vulkan layer.
//!
//! Go read:
//! - <https://docs.vulkan.org/guide/latest/layers.html>
//! - <https://renderdoc.org/vulkan-layer-guide.html>

#![allow(non_snake_case)]

pub mod hooks;
pub mod vk;

use std::{
    ffi::c_void,
    ops::Range,
    ptr,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use erebus_engine::{ThemeCtx, ThemeModule};

use fxhash::FxHashMap;
use vk::{
    PFN_vkBindBufferMemory, PFN_vkBindBufferMemory2, PFN_vkCmdCopyBuffer, PFN_vkCmdCopyBuffer2,
    PFN_vkDestroyBuffer, PFN_vkDestroyDevice, PFN_vkFlushMappedMemoryRanges, PFN_vkFreeMemory,
    PFN_vkGetDeviceProcAddr, PFN_vkGetInstanceProcAddr, PFN_vkMapMemory, PFN_vkUnmapMemory,
    PFN_vkVoidFunction, VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE, VK_WHOLE_SIZE, VkBuffer, VkDevice,
    VkDeviceMemory, VkDeviceSize, VkInstance, VkMappedMemoryRange,
};

/// Dispatchable handles (VkInstance, VkPhysicalDevice, VkDevice, VkCommandBuffer, etc...)
/// start with a pointer into the loader's dispatch table. This is shared by everything
/// created from the same instance/device, which makes it useful for a hashmap key by
/// instance/device pair.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct DispatchKey(usize);

impl DispatchKey {
    pub unsafe fn from_raw_handle<T>(handle: *mut T) -> Self {
        Self(unsafe { *(handle as *const usize) })
    }
}

/// Reinterprets a `PFN_vkVoidFunction` as its real function pointer type.
///
/// SAFETY: same contract as [`std::mem::transmute`].
unsafe fn cast_pfn<F: Copy>(function: PFN_vkVoidFunction) -> Option<F> {
    // basically doing transmute's work for it
    const { assert!(size_of::<F>() == size_of::<usize>()) };

    function.map(|f| unsafe { std::mem::transmute_copy(&f) })
}

/// staging uploads larger than this are never primitive batches
const MAX_PRIMITIVE_UPLOAD_BYTES: VkDeviceSize = 64 << 20;

/// What the loader's layer chain gave us for one `VkInstance`.
#[derive(Clone, Copy)]
struct Instance {
    handle: VkInstance,
    vkGetInstanceProcAddr: PFN_vkGetInstanceProcAddr,
}

impl Instance {
    /// Gets a copy of an [`Instance`] from any dispatchable handle associated to it.
    unsafe fn try_from_handle<T>(handle: *mut T) -> Option<Self> {
        let key = unsafe { DispatchKey::from_raw_handle(handle) };
        instances().get(&key).copied()
    }
}

unsafe impl Send for Instance {}
unsafe impl Sync for Instance {}

/// The next layer's (or driver's if its the end of the chain) entry points for one `VkDevice`.
#[derive(Clone, Copy)]
struct DeviceDispatch {
    vkGetDeviceProcAddr: PFN_vkGetDeviceProcAddr,
    vkDestroyDevice: PFN_vkDestroyDevice,
    vkMapMemory: PFN_vkMapMemory,
    vkUnmapMemory: PFN_vkUnmapMemory,
    vkFreeMemory: PFN_vkFreeMemory,
    vkFlushMappedMemoryRanges: PFN_vkFlushMappedMemoryRanges,
    vkBindBufferMemory: PFN_vkBindBufferMemory,
    vkDestroyBuffer: PFN_vkDestroyBuffer,
    vkCmdCopyBuffer: PFN_vkCmdCopyBuffer,
    // optional to VK_KHR_bind_memory2
    vkBindBufferMemory2: Option<PFN_vkBindBufferMemory2>,
    // optional to VK_KHR_copy_commands2
    vkCmdCopyBuffer2: Option<PFN_vkCmdCopyBuffer2>,
}

impl DeviceDispatch {
    /// Gets a copy of a device's dispatch table, or `None` if the layer doesn't know the device.
    unsafe fn try_from_handle(handle: VkDevice) -> Option<Self> {
        let key = unsafe { DispatchKey::from_raw_handle(handle) };
        devices().get(&key).map(|device| device.dispatch)
    }

    unsafe fn load(vkGetDeviceProcAddr: PFN_vkGetDeviceProcAddr, device: VkDevice) -> Self {
        unsafe {
            Self {
                vkGetDeviceProcAddr,
                vkDestroyDevice: cast_pfn(vkGetDeviceProcAddr(device, c"vkDestroyDevice".as_ptr()))
                    .expect("missing function from driver"),
                vkMapMemory: cast_pfn(vkGetDeviceProcAddr(device, c"vkMapMemory".as_ptr()))
                    .expect("missing function from driver"),
                vkUnmapMemory: cast_pfn(vkGetDeviceProcAddr(device, c"vkUnmapMemory".as_ptr()))
                    .expect("missing function from driver"),
                vkFreeMemory: cast_pfn(vkGetDeviceProcAddr(device, c"vkFreeMemory".as_ptr()))
                    .expect("missing function from driver"),
                vkFlushMappedMemoryRanges: cast_pfn(vkGetDeviceProcAddr(
                    device,
                    c"vkFlushMappedMemoryRanges".as_ptr(),
                ))
                .expect("missing function from driver"),
                vkBindBufferMemory: cast_pfn(vkGetDeviceProcAddr(
                    device,
                    c"vkBindBufferMemory".as_ptr(),
                ))
                .expect("missing function from driver"),
                vkDestroyBuffer: cast_pfn(vkGetDeviceProcAddr(device, c"vkDestroyBuffer".as_ptr()))
                    .expect("missing function from driver"),
                vkCmdCopyBuffer: cast_pfn(vkGetDeviceProcAddr(device, c"vkCmdCopyBuffer".as_ptr()))
                    .expect("missing function from driver"),
                vkBindBufferMemory2: cast_pfn(
                    vkGetDeviceProcAddr(device, c"vkBindBufferMemory2".as_ptr()).or_else(|| {
                        vkGetDeviceProcAddr(device, c"vkBindBufferMemory2KHR".as_ptr())
                    }),
                ),
                vkCmdCopyBuffer2: cast_pfn(
                    vkGetDeviceProcAddr(device, c"vkCmdCopyBuffer2".as_ptr())
                        .or_else(|| vkGetDeviceProcAddr(device, c"vkCmdCopyBuffer2KHR".as_ptr())),
                ),
            }
        }
    }
}

struct Device {
    handle: VkDevice,

    /// dispatch table containing the next layer's vulkan commands
    dispatch: DeviceDispatch,

    /// tracked buffers and mapped memory for this VkDevice
    tracked: Mutex<DeviceTracked>,

    /// The wasm theme instance for this device, or `None` if no theme is configured (or it
    /// failed to load or initialize), in which case the layer just passes everything through.
    ///
    /// Behind a mutex because the hooks only get `&Device`, and running the theme needs `&mut`.
    ctx: Option<Mutex<ThemeCtx>>,
}
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

#[derive(Default)]
struct DeviceTracked {
    /// device memory allocations currently mapped to application memory
    mapped: FxHashMap<VkDeviceMemory, MappedMemory>,

    /// which buffers are bound to device memory.
    ///
    /// entries are removed by `vkDestroyBuffer`. they are deliberately *not* swept when
    /// memory is freed: a buffer can't legally be used after its memory is gone, and the
    /// sweep would be O(all buffers) on every free.
    bound: FxHashMap<VkBuffer, BufferBinding>,
}

/// Block of device memory mapped into the app's address space by `vkMapMemory`.
///
/// see <https://docs.vulkan.org/refpages/latest/refpages/source/vkMapMemory.html>
struct MappedMemory {
    /// application address of the byte at `self.offset` into the allocation
    addr: *mut c_void,

    /// start of the mapped window relative to the start of the allocation
    offset: VkDeviceSize,

    /// end of the mapped window (exclusive) relative to the start of the allocation.
    ///
    /// this is `VK_WHOLE_SIZE` if the app mapped with `VK_WHOLE_SIZE`
    end: VkDeviceSize,
}

/// Bound VkBuffer to device memory from a `vkBindBufferMemory` call.
struct BufferBinding {
    memory: VkDeviceMemory,
    offset: VkDeviceSize,
}

/// VkPhysicalDeviceLimits::nonCoherentAtomSize reports the alignment/block size of a cache
/// operation to GPU memory. When doing cache operations, we need to align our address ranges
/// to this value. Rather than querying Vulakn for this, we're just using the maximum value
/// that the spec allows.
///
/// see: <https://docs.vulkan.org/refpages/latest/refpages/source/Required_Limits.html>
const FLUSH_ALIGNMENT: VkDeviceSize = 256;

/// computes the `(offset, size)` to pass to `vkFlushMappedMemoryRanges` so that it covers
/// the provided range in the mapping.
fn compute_flush_range(
    range: Range<VkDeviceSize>,
    mapping: &MappedMemory,
) -> (VkDeviceSize, VkDeviceSize) {
    let offset = (range.start & !(FLUSH_ALIGNMENT - 1)).max(mapping.offset);
    let aligned_end = range.end.saturating_add(FLUSH_ALIGNMENT - 1) & !(FLUSH_ALIGNMENT - 1);

    let size = if mapping.end != VK_WHOLE_SIZE && aligned_end < mapping.end {
        aligned_end - offset
    } else {
        // todo: maybe track allocation ranges so we don't automatically assume the mapping
        // needs to be flushed to VK_WHOLE_SIZE? at that point we might be micro-optimizing
        // way too much tbh
        VK_WHOLE_SIZE
    };

    (offset, size)
}

/// This is the compiled theme.
static THEME_MODULE: LazyLock<Option<ThemeModule>> = LazyLock::new(|| {
    let path = std::env::var_os("EREBUS_THEME")?;

    match ThemeModule::from_file(&path) {
        Ok(module) => Some(module),
        Err(err) => {
            log::error!("failed to load theme {path:?}: {err}");
            None
        }
    }
});

impl Device {
    /// Instantiates and initializes the configured theme for a new device, if there is one.
    fn new_theme_ctx() -> Option<Mutex<ThemeCtx>> {
        let mut ctx = match THEME_MODULE.as_ref()?.instantiate() {
            Ok(ctx) => ctx,
            Err(err) => {
                log::error!("failed to instantiate theme: {err}");
                return None;
            }
        };

        if let Err(err) = ctx.init_with_default_settings() {
            log::error!("failed to initialize theme: {err}");
            return None;
        }

        Some(Mutex::new(ctx))
    }

    fn tracked(&self) -> MutexGuard<'_, DeviceTracked> {
        self.tracked.lock().unwrap_or_else(PoisonError::into_inner)
    }

    unsafe fn apply_theme_to_regions(
        &self,
        buffer: VkBuffer,
        regions: impl Iterator<Item = (VkDeviceSize, VkDeviceSize)>,
    ) {
        let Some(ctx) = &self.ctx else {
            // no theme loaded
            return;
        };

        let tracked = self.tracked();

        // the buffer must be bound to VkDeviceMemory on the GPU
        let Some(binding) = tracked.bound.get(&buffer) else {
            return;
        };
        // the buffer must be mapped to CPU-visible memory
        let Some(mapping) = tracked.mapped.get(&binding.memory) else {
            return;
        };

        // only take the theme lock once we know there's something mapped to theme, so copies
        // we don't care about (most of them) never contend on it.
        let mut theme = ctx.lock().unwrap_or_else(PoisonError::into_inner);

        // allocation-relative span of bytes that the theme modified
        let mut dirty: Option<Range<VkDeviceSize>> = None;

        for (offset, size) in regions {
            if size == 0 || size > MAX_PRIMITIVE_UPLOAD_BYTES {
                continue;
            }

            // where the region sits within the VkDeviceMemory allocation
            let Some(start) = binding.offset.checked_add(offset) else {
                continue;
            };
            let Some(end) = start.checked_add(size) else {
                continue;
            };

            // the entire region must be actually mapped
            if start < mapping.offset || end > mapping.end {
                continue;
            }

            // pointer to the mapping
            let ptr = unsafe {
                mapping
                    .addr
                    .cast::<u8>()
                    .add((start - mapping.offset) as usize)
            };

            // now we try and identify the memory as an array of one of GPUI's primitives
            // and run the theme over every element.
            let bytes = unsafe { std::slice::from_raw_parts_mut(ptr, size as usize) };
            if theme.apply(bytes) == Some(true) {
                // if we modified bytes, update the region of memory we've modified and might need to flush
                dirty = Some(match dirty {
                    Some(Range {
                        start: dirty_start,
                        end: dirty_end,
                    }) => dirty_start.min(start)..dirty_end.max(end),
                    None => start..end,
                });
            }
        }

        // make any changes we made visible to the GPU
        let Some(dirty_range) = dirty else { return };
        let (offset, size) = compute_flush_range(dirty_range, mapping);

        unsafe {
            (self.dispatch.vkFlushMappedMemoryRanges)(
                self.handle,
                1,
                &VkMappedMemoryRange {
                    sType: VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
                    pNext: ptr::null(),
                    memory: binding.memory,
                    offset,
                    size,
                },
            )
        };
    }

    /// Runs `f` on the [`Device`] associated with any of its dispatchable handles.
    ///
    /// This holds the device registry's read lock for the duration of `f`, so `f`
    /// must be non-reentrant.
    unsafe fn with<T, R>(handle: *mut T, f: impl FnOnce(&Self) -> R) -> R {
        let key = unsafe { DispatchKey::from_raw_handle(handle) };

        f(devices()
            .get(&key)
            .expect("Vulkan call on a device this layer did not create"))
    }
}

static INSTANCE_REGISTRY: LazyLock<RwLock<FxHashMap<DispatchKey, Instance>>> =
    LazyLock::new(RwLock::default);
static DEVICE_REGISTRY: LazyLock<RwLock<FxHashMap<DispatchKey, Device>>> =
    LazyLock::new(RwLock::default);

fn instances() -> RwLockReadGuard<'static, FxHashMap<DispatchKey, Instance>> {
    INSTANCE_REGISTRY
        .read()
        .unwrap_or_else(PoisonError::into_inner)
}
fn instances_mut() -> RwLockWriteGuard<'static, FxHashMap<DispatchKey, Instance>> {
    INSTANCE_REGISTRY
        .write()
        .unwrap_or_else(PoisonError::into_inner)
}
fn devices() -> RwLockReadGuard<'static, FxHashMap<DispatchKey, Device>> {
    DEVICE_REGISTRY
        .read()
        .unwrap_or_else(PoisonError::into_inner)
}
fn devices_mut() -> RwLockWriteGuard<'static, FxHashMap<DispatchKey, Device>> {
    DEVICE_REGISTRY
        .write()
        .unwrap_or_else(PoisonError::into_inner)
}
