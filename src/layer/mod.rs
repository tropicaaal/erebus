//! Implementation of the vulkan layer.
//!
//! Go read:
//! - <https://docs.vulkan.org/guide/latest/layers.html>
//! - <https://renderdoc.org/vulkan-layer-guide.html>

pub mod hooks;
pub mod vk;

use std::{
    collections::HashMap,
    ffi::{CStr, c_char},
    mem::transmute,
    ptr,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

use crate::{ExampleTheme, scene};

use fxhash::FxHashMap;
use vk::{
    PFN_vkBindBufferMemory, PFN_vkBindBufferMemory2, PFN_vkCmdCopyBuffer, PFN_vkCmdCopyBuffer2,
    PFN_vkDestroyBuffer, PFN_vkDestroyDevice, PFN_vkFlushMappedMemoryRanges, PFN_vkFreeMemory,
    PFN_vkGetDeviceProcAddr, PFN_vkGetInstanceProcAddr, PFN_vkMapMemory, PFN_vkUnmapMemory,
    PFN_vkVoidFunction, VK_ERROR_INITIALIZATION_FAILED, VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
    VK_SUCCESS, VK_WHOLE_SIZE, VkBuffer, VkDevice, VkDeviceMemory, VkDeviceSize, VkInstance,
    VkMappedMemoryRange, VkNegotiateLayerInterface, VkResult,
};

/// staging uploads larger than this are never primitive batches
const MAX_PRIMITIVE_UPLOAD_BYTES: VkDeviceSize = 64 << 20;

/// What the loader's layer chain gave us for one `VkInstance`.
struct Instance {
    handle: VkInstance,
    vkGetInstanceProcAddr: PFN_vkGetInstanceProcAddr,
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

/// Result of a `vkMapMemory` call.
struct MappedMemory {
    addr: usize,
    offset: VkDeviceSize,
    size: VkDeviceSize,
}

/// Result of a `vkBindBufferMemory` call.
struct BufferBinding {
    memory: VkDeviceMemory,
    offset: VkDeviceSize,
}

struct Device {
    handle: VkDevice,
    dispatch: DeviceDispatch,
    mapped: HashMap<VkDeviceMemory, MappedMemory>,
    bound: HashMap<VkBuffer, BufferBinding>,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    unsafe fn apply_theme_to_buffer(
        &self,
        buffer: VkBuffer,
        offset: VkDeviceSize,
        size: VkDeviceSize,
    ) {
        if size == 0 || size > MAX_PRIMITIVE_UPLOAD_BYTES {
            return;
        }

        // the buffer must be bound to VkDeviceMemory on the GPU
        let Some(binding) = self.bound.get(&buffer) else {
            return;
        };
        // the buffer must be mapped to CPU-visible memory
        let Some(mapping) = self.mapped.get(&binding.memory) else {
            return;
        };

        // compute the address of the buffer's mapped region in main memory
        let mapped_addr = {
            // absolute address of the data within the VkDeviceMemory mapping
            let memory_offset = binding.offset + offset;

            // the region must contain the entire buffer
            let mapping_contains_range = memory_offset >= mapping.offset
                && (mapping.size == VK_WHOLE_SIZE
                    || memory_offset + size <= mapping.offset + mapping.size);
            if !mapping_contains_range {
                return;
            }

            (mapping.addr as u64 + (memory_offset - mapping.offset)) as *mut u8
        };

        // now we try and identify the memory as an array of one of GPUI's primitives
        // and run the theme over every element.
        let bytes = unsafe { std::slice::from_raw_parts_mut(mapped_addr, size as usize) };
        let Some(changed) = scene::apply_theme(bytes, &mut ExampleTheme {}) else {
            return;
        };

        // make the change visible to the GPU if the theme modified any bytes
        if changed && mapping.offset == 0 {
            unsafe {
                (self.dispatch.vkFlushMappedMemoryRanges)(
                    self.handle as VkDevice,
                    1,
                    &VkMappedMemoryRange {
                        sType: VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE,
                        pNext: ptr::null(),
                        memory: binding.memory,
                        offset: 0,
                        size: VK_WHOLE_SIZE,
                    },
                )
            };
        }
    }
}

#[derive(Default)]
struct State {
    instances: FxHashMap<DispatchKey, Instance>,
    devices: FxHashMap<DispatchKey, Device>,
}

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

static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Obtains a [`Device`] from any dispatchable handle associated to it.
unsafe fn with_device<T, R>(handle: *mut T, f: impl FnOnce(&mut Device) -> R) -> R {
    let key = unsafe { DispatchKey::from_raw_handle(handle) };

    let mut state = state();

    f(state
        .devices
        .get_mut(&key)
        .expect("Vulkan call on a device this layer did not create"))
}

/// Obtains an [`Instance`] from any dispatchable handle associated to it.
unsafe fn with_instance<T, R>(handle: *mut T, f: impl FnOnce(&mut Instance) -> R) -> R {
    let key = unsafe { DispatchKey::from_raw_handle(handle) };

    let mut state = state();

    f(state
        .instances
        .get_mut(&key)
        .expect("Vulkan call on an instance this layer did not create"))
}

/// Reinterprets a `PFN_vkVoidFunction` as its real function pointer type.
///
/// SAFETY: same contract as [`std::mem::transmute`].
unsafe fn cast_pfn<F: Copy>(function: PFN_vkVoidFunction) -> Option<F> {
    // basically doing transmute's work for it
    const { assert!(size_of::<F>() == size_of::<usize>()) };

    function.map(|f| unsafe { std::mem::transmute_copy(&f) })
}

//
// MARK: Layer entrypoints
//

/// this is the first thing that vulkan will call when loading our
/// layer, where the loader version is negotiated and we either proceed
/// or don't based on that.
///
/// From there, we pass it our own pointers to [`vkGetInstanceProcAddr`] and
/// [`vkGetDeviceProcAddr`] which will allow hooking further functions
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkNegotiateLoaderLayerInterfaceVersion(
    pVersionStruct: *mut VkNegotiateLayerInterface,
) -> VkResult {
    if pVersionStruct.is_null() {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    let negotiate = unsafe { &mut *pVersionStruct };

    // version 1 uses JSON which is some bullshit
    negotiate.loaderLayerInterfaceVersion = negotiate.loaderLayerInterfaceVersion.min(2);
    negotiate.pfnGetInstanceProcAddr = Some(vkGetInstanceProcAddr);
    negotiate.pfnGetDeviceProcAddr = Some(vkGetDeviceProcAddr);
    negotiate.pfnGetPhysicalDeviceProcAddr = None; // nothing here matters for our purposes

    VK_SUCCESS
}

/// this is how vulkan loaders get access to their function pointers,
/// along with [`vkGetDeviceProcAddr`] and `vkGetPhysicalDeviceProcAddr`.
///
/// this specific function can be used for retrieving all vulkan commands,
#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetInstanceProcAddr(
    instance: VkInstance,
    pName: *const c_char,
) -> PFN_vkVoidFunction {
    let hooked: PFN_vkVoidFunction = match unsafe { CStr::from_ptr(pName) }.to_bytes() {
        b"vkGetInstanceProcAddr" => Some(unsafe { transmute(vkGetInstanceProcAddr as *const ()) }),
        b"vkGetDeviceProcAddr" => Some(unsafe { transmute(vkGetDeviceProcAddr as *const ()) }),
        b"vkCreateInstance" => Some(unsafe { transmute(hooks::vkCreateInstance as *const ()) }),
        b"vkDestroyInstance" => Some(unsafe { transmute(hooks::vkDestroyInstance as *const ()) }),
        b"vkCreateDevice" => Some(unsafe { transmute(hooks::vkCreateDevice as *const ()) }),
        _ => None,
    };
    if hooked.is_some() || instance.is_null() {
        return hooked;
    }
    let next = state()
        .instances
        .get(&unsafe { DispatchKey::from_raw_handle(instance) })?
        .vkGetInstanceProcAddr;
    unsafe { next(instance, pName) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetDeviceProcAddr(
    device: VkDevice,
    pName: *const c_char,
) -> PFN_vkVoidFunction {
    let dispatch = state()
        .devices
        .get(&unsafe { DispatchKey::from_raw_handle(device) })?
        .dispatch;

    // Only intercept what the next layer actually provides.
    let hooked: PFN_vkVoidFunction = unsafe {
        match CStr::from_ptr(pName).to_bytes() {
            b"vkGetDeviceProcAddr" => Some(transmute(vkGetDeviceProcAddr as *const ())),
            b"vkDestroyDevice" => Some(transmute(hooks::vkDestroyDevice as *const ())),
            b"vkMapMemory" => Some(transmute(hooks::vkMapMemory as *const ())),
            b"vkUnmapMemory" => Some(transmute(hooks::vkUnmapMemory as *const ())),
            b"vkFreeMemory" => Some(transmute(hooks::vkFreeMemory as *const ())),
            b"vkBindBufferMemory" => Some(transmute(hooks::vkBindBufferMemory as *const ())),
            b"vkDestroyBuffer" => Some(transmute(hooks::vkDestroyBuffer as *const ())),
            b"vkCmdCopyBuffer" => Some(transmute(hooks::vkCmdCopyBuffer as *const ())),
            b"vkBindBufferMemory2" | b"vkBindBufferMemory2KHR"
                if dispatch.vkBindBufferMemory2.is_some() =>
            {
                Some(transmute(hooks::vkBindBufferMemory2 as *const ()))
            }
            b"vkCmdCopyBuffer2" | b"vkCmdCopyBuffer2KHR" if dispatch.vkCmdCopyBuffer2.is_some() => {
                Some(transmute(hooks::vkCmdCopyBuffer2 as *const ()))
            }
            _ => None,
        }
    };
    hooked.or_else(|| unsafe { (dispatch.vkGetDeviceProcAddr)(device, pName) })
}
