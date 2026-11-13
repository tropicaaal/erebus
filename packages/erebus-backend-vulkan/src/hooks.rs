//! Hooked vulkan functions
//!
//! Each is named after the Vulkan function it replaces and calls the next layer's version.
//!
//! Our goal here is:
//! - Track all created devices and instances.
//! - Track which VkBuffers are bound to which device memory, and which device memory
//!   is mapped to application memory.
//! - Whenever a VkBuffer gets copied to GPU-local VRAM, intercept that, find out if it's
//!   mapped to application memory and where, scan that mapping for GPUI primitives, and
//!   apply the theme to it.

use std::ffi::CStr;
use std::mem::transmute;

use crate::vk::{PFN_vkVoidFunction, VkNegotiateLayerInterface};

use super::{
    BufferBinding, Device, DeviceDispatch, DispatchKey, Instance, MappedMemory, cast_pfn,
    devices_mut, instances, instances_mut,
};

use super::vk::{
    PFN_vkCreateDevice, PFN_vkCreateInstance, PFN_vkDestroyInstance,
    VK_ERROR_INITIALIZATION_FAILED, VK_LAYER_LINK_INFO,
    VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO, VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO,
    VK_SUCCESS, VK_WHOLE_SIZE, VkAllocationCallbacks, VkBaseInStructure, VkBindBufferMemoryInfo,
    VkBuffer, VkBufferCopy, VkCommandBuffer, VkCopyBufferInfo2, VkDevice, VkDeviceCreateInfo,
    VkDeviceMemory, VkDeviceSize, VkInstance, VkInstanceCreateInfo, VkLayerDeviceCreateInfo,
    VkLayerInstanceCreateInfo, VkMemoryMapFlags, VkPhysicalDevice, VkResult, VkStructureType,
};

//
// MARK: Layer entrypoint
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
    pName: *const core::ffi::c_char,
) -> PFN_vkVoidFunction {
    let hooked: PFN_vkVoidFunction = match unsafe { CStr::from_ptr(pName) }.to_bytes() {
        b"vkGetInstanceProcAddr" => Some(unsafe { transmute(vkGetInstanceProcAddr as *const ()) }),
        b"vkGetDeviceProcAddr" => Some(unsafe { transmute(vkGetDeviceProcAddr as *const ()) }),
        b"vkCreateInstance" => Some(unsafe { transmute(vkCreateInstance as *const ()) }),
        b"vkDestroyInstance" => Some(unsafe { transmute(vkDestroyInstance as *const ()) }),
        b"vkCreateDevice" => Some(unsafe { transmute(vkCreateDevice as *const ()) }),
        _ => None,
    };

    if hooked.is_some() || instance.is_null() {
        return hooked;
    }

    let next_vkGetInstanceProcAddr =
        unsafe { Instance::try_from_handle(instance) }?.vkGetInstanceProcAddr;

    unsafe { next_vkGetInstanceProcAddr(instance, pName) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn vkGetDeviceProcAddr(
    device: VkDevice,
    pName: *const core::ffi::c_char,
) -> PFN_vkVoidFunction {
    let dispatch = unsafe { DeviceDispatch::try_from_handle(device) }?;

    // Only intercept what the next layer actually provides.
    let hooked: PFN_vkVoidFunction = unsafe {
        match CStr::from_ptr(pName).to_bytes() {
            b"vkGetDeviceProcAddr" => Some(transmute(vkGetDeviceProcAddr as *const ())),
            b"vkDestroyDevice" => Some(transmute(vkDestroyDevice as *const ())),
            b"vkMapMemory" => Some(transmute(vkMapMemory as *const ())),
            b"vkUnmapMemory" => Some(transmute(vkUnmapMemory as *const ())),
            b"vkFreeMemory" => Some(transmute(vkFreeMemory as *const ())),
            b"vkBindBufferMemory" => Some(transmute(vkBindBufferMemory as *const ())),
            b"vkDestroyBuffer" => Some(transmute(vkDestroyBuffer as *const ())),
            b"vkCmdCopyBuffer" => Some(transmute(vkCmdCopyBuffer as *const ())),
            b"vkBindBufferMemory2" | b"vkBindBufferMemory2KHR"
                if dispatch.vkBindBufferMemory2.is_some() =>
            {
                Some(transmute(vkBindBufferMemory2 as *const ()))
            }
            b"vkCmdCopyBuffer2" | b"vkCmdCopyBuffer2KHR" if dispatch.vkCmdCopyBuffer2.is_some() => {
                Some(transmute(vkCmdCopyBuffer2 as *const ()))
            }
            _ => None,
        }
    };

    hooked.or_else(|| unsafe { (dispatch.vkGetDeviceProcAddr)(device, pName) })
}

//
// MARK: Device/instance tracking
//

/// instance/device creation functions get passed a linked list of of internal data
/// in [`VkInstanceCreateInfo`]/[`VkDeviceCreateInfo`] that lets us obtain the next
/// layer's `vkGetInstanceProcAddr` that we need to fill out dispatch tables in later
/// hooks.
///
/// This function simply searches the linked list for whatever stuff we need specified
/// by the `filter` callback.
unsafe fn find_in_pnext<T>(
    mut node: *const core::ffi::c_void,
    sType: VkStructureType,
    filter: impl Fn(&T) -> bool,
) -> *mut T {
    while !node.is_null() {
        let base = unsafe { &*(node as *const VkBaseInStructure) };
        if base.sType == sType && filter(unsafe { &*(node as *const T) }) {
            return node as *mut T;
        }
        node = base.pNext as *const core::ffi::c_void;
    }

    core::ptr::null_mut()
}

pub unsafe extern "system" fn vkCreateInstance(
    pCreateInfo: *const VkInstanceCreateInfo,
    pAllocator: *const VkAllocationCallbacks,
    pInstance: *mut VkInstance,
) -> VkResult {
    unsafe {
        // aliirght, so main annoying thing here is finding the next layer's vkGetInstanceProcAddr pointer.
        // since we aren't working with device handles, this isn't nearly as nice since we don't have the
        // dispatch table stored and such. so, walk pNext for the next layer's `VkLayerInstanceCreateInfo`.
        //
        // from there we can access the link info and eventually arrive at our bestie
        // `pfnNextGetInstanceProcAddr`. yay!

        let link_info = find_in_pnext::<VkLayerInstanceCreateInfo>(
            (*pCreateInfo).pNext,
            VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO,
            |info| info.function == VK_LAYER_LINK_INFO,
        );

        if link_info.is_null() {
            return VK_ERROR_INITIALIZATION_FAILED;
        }

        let link = (*link_info).u.pLayerInfo;
        let next_vkGetInstanceProcAddr = (*link).pfnNextGetInstanceProcAddr;

        // advance the chain for the next layer
        (*link_info).u.pLayerInfo = (*link).pNext;

        let Some(next_vkCreateInstance) = cast_pfn::<PFN_vkCreateInstance>(
            next_vkGetInstanceProcAddr(core::ptr::null_mut(), c"vkCreateInstance".as_ptr()),
        ) else {
            return VK_ERROR_INITIALIZATION_FAILED;
        };

        let result = next_vkCreateInstance(pCreateInfo, pAllocator, pInstance);

        if result == VK_SUCCESS {
            let instance = *pInstance;

            instances_mut().insert(
                DispatchKey::from_raw_handle(instance),
                Instance {
                    handle: instance,
                    vkGetInstanceProcAddr: next_vkGetInstanceProcAddr,
                },
            );
        }

        result
    }
}

pub unsafe extern "system" fn vkDestroyInstance(
    instance: VkInstance,
    pAllocator: *const VkAllocationCallbacks,
) {
    unsafe {
        // note that we want to remove from the instance registry before
        // calling next_vkDestroyInstance, since we could race condition
        // and have a new instance made with the same dispatch key.
        let Some(removed) = instances_mut().remove(&DispatchKey::from_raw_handle(instance)) else {
            return;
        };

        let next_vkDestroyInstance = cast_pfn::<PFN_vkDestroyInstance>((removed
            .vkGetInstanceProcAddr)(
            removed.handle,
            c"vkDestroyInstance".as_ptr(),
        ))
        .unwrap();

        next_vkDestroyInstance(removed.handle, pAllocator)
    }
}

pub unsafe extern "system" fn vkCreateDevice(
    physicalDevice: VkPhysicalDevice,
    pCreateInfo: *const VkDeviceCreateInfo,
    pAllocator: *const VkAllocationCallbacks,
    pDevice: *mut VkDevice,
) -> VkResult {
    unsafe {
        // most of this is the same deal as `vkCreateInstance`, except we use
        // `pfnNextGetInstanceProcAddr` to build our dispatch table for hooks
        // to call into the next layer.

        let link_info = find_in_pnext::<VkLayerDeviceCreateInfo>(
            (*pCreateInfo).pNext,
            VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO,
            |info| info.function == VK_LAYER_LINK_INFO,
        );

        let instance = instances()
            .get(&DispatchKey::from_raw_handle(physicalDevice))
            .map(|instance| instance.handle);

        let (false, Some(instance)) = (link_info.is_null(), instance) else {
            return VK_ERROR_INITIALIZATION_FAILED;
        };

        let link = (*link_info).u.pLayerInfo;
        let next_vkGetInstanceProcAddr = (*link).pfnNextGetInstanceProcAddr;
        let next_vkGetDeviceProcAddr = (*link).pfnNextGetDeviceProcAddr;
        (*link_info).u.pLayerInfo = (*link).pNext;

        let Some(next_vkCreateDevice) = cast_pfn::<PFN_vkCreateDevice>(next_vkGetInstanceProcAddr(
            instance,
            c"vkCreateDevice".as_ptr(),
        )) else {
            return VK_ERROR_INITIALIZATION_FAILED;
        };

        let result = next_vkCreateDevice(physicalDevice, pCreateInfo, pAllocator, pDevice);

        if result == VK_SUCCESS {
            let device = *pDevice;
            let dispatch = DeviceDispatch::load(next_vkGetDeviceProcAddr, device);

            // instantiate before taking the registry's write lock: loading and compiling the
            // theme on first use is slow and would stall every hook in the meantime.
            let ctx = Device::new_theme_ctx();

            devices_mut().insert(
                DispatchKey::from_raw_handle(device),
                Device {
                    handle: device,
                    dispatch,
                    tracked: Default::default(),
                    ctx,
                },
            );
        }

        result
    }
}

pub unsafe extern "system" fn vkDestroyDevice(
    device: VkDevice,
    pAllocator: *const VkAllocationCallbacks,
) {
    unsafe {
        // see the comment in vkDestroyInstance. same applies here
        let Some(removed) = devices_mut().remove(&DispatchKey::from_raw_handle(device)) else {
            return;
        };

        (removed.dispatch.vkDestroyDevice)(device, pAllocator);
    }
}

//
// MARK: Memory tracking
//

pub unsafe extern "system" fn vkMapMemory(
    device: VkDevice,
    memory: VkDeviceMemory,
    offset: VkDeviceSize,
    size: VkDeviceSize,
    flags: VkMemoryMapFlags,
    ppData: *mut *mut core::ffi::c_void,
) -> VkResult {
    unsafe {
        Device::with(device, |device| {
            let result =
                (device.dispatch.vkMapMemory)(device.handle, memory, offset, size, flags, ppData);

            if result == VK_SUCCESS {
                device.tracked().mapped.insert(
                    memory,
                    MappedMemory {
                        addr: *ppData,
                        offset,
                        end: if size == VK_WHOLE_SIZE {
                            VK_WHOLE_SIZE
                        } else {
                            offset.saturating_add(size)
                        },
                    },
                );
            }

            result
        })
    }
}

pub unsafe extern "system" fn vkUnmapMemory(device: VkDevice, memory: VkDeviceMemory) {
    let next_vkUnmapMemory = unsafe {
        Device::with(device, |device| {
            device.tracked().mapped.remove(&memory);
            device.dispatch.vkUnmapMemory
        })
    };

    unsafe { next_vkUnmapMemory(device, memory) }
}

pub unsafe extern "system" fn vkFreeMemory(
    device: VkDevice,
    memory: VkDeviceMemory,
    pAllocator: *const VkAllocationCallbacks,
) {
    let next_vkFreeMemory = unsafe {
        Device::with(device, |device| {
            // freeing mapped memory implicitly unmaps it
            device.tracked().mapped.remove(&memory);
            device.dispatch.vkFreeMemory
        })
    };

    unsafe { next_vkFreeMemory(device, memory, pAllocator) }
}

pub unsafe extern "system" fn vkBindBufferMemory(
    device: VkDevice,
    buffer: VkBuffer,
    memory: VkDeviceMemory,
    memoryOffset: VkDeviceSize,
) -> VkResult {
    unsafe {
        Device::with(device, |device| {
            let result =
                (device.dispatch.vkBindBufferMemory)(device.handle, buffer, memory, memoryOffset);

            if result == VK_SUCCESS {
                device.tracked().bound.insert(
                    buffer,
                    BufferBinding {
                        memory,
                        offset: memoryOffset,
                    },
                );
            }

            result
        })
    }
}

pub unsafe extern "system" fn vkBindBufferMemory2(
    device: VkDevice,
    bindInfoCount: u32,
    pBindInfos: *const VkBindBufferMemoryInfo,
) -> VkResult {
    unsafe {
        Device::with(device, |device| {
            let result = (device.dispatch.vkBindBufferMemory2.unwrap())(
                device.handle,
                bindInfoCount,
                pBindInfos,
            );

            // update bindings if successful
            if result == VK_SUCCESS && bindInfoCount > 0 {
                let mut tracked = device.tracked();
                for info in std::slice::from_raw_parts(pBindInfos, bindInfoCount as usize) {
                    tracked.bound.insert(
                        info.buffer,
                        BufferBinding {
                            memory: info.memory,
                            offset: info.memoryOffset,
                        },
                    );
                }
            }

            result
        })
    }
}

pub unsafe extern "system" fn vkDestroyBuffer(
    device: VkDevice,
    buffer: VkBuffer,
    pAllocator: *const VkAllocationCallbacks,
) {
    let next_vkDestroyBuffer = unsafe {
        Device::with(device, |device| {
            device.tracked().bound.remove(&buffer);
            device.dispatch.vkDestroyBuffer
        })
    };

    unsafe { next_vkDestroyBuffer(device, buffer, pAllocator) }
}

//
// MARK: Theming
//
// Okay this is the fun part. We hook vkCmdCopyBuffer and vkCmdCopyBuffer2 which
// is invoked by wgpu when GPUI uploads a region of memory to the GPU with [`queue.write_buffer`].
// From there, we can intercept the source data, look for paint operations, modify them, then forward
// it to the next layer.
//
// [`queue.write_buffer`]: https://docs.rs/wgpu/latest/wgpu/struct.Queue.html#method.write_buffer
//

pub unsafe extern "system" fn vkCmdCopyBuffer(
    commandBuffer: VkCommandBuffer,
    srcBuffer: VkBuffer,
    dstBuffer: VkBuffer,
    regionCount: u32,
    pRegions: *const VkBufferCopy,
) {
    let next_vkCmdCopyBuffer = unsafe {
        Device::with(commandBuffer, |device| {
            if regionCount > 0 {
                let regions = std::slice::from_raw_parts(pRegions, regionCount as usize);

                device.apply_theme_to_regions(
                    srcBuffer,
                    regions.iter().map(|region| (region.srcOffset, region.size)),
                );
            }
            device.dispatch.vkCmdCopyBuffer
        })
    };

    unsafe { next_vkCmdCopyBuffer(commandBuffer, srcBuffer, dstBuffer, regionCount, pRegions) }
}

pub unsafe extern "system" fn vkCmdCopyBuffer2(
    commandBuffer: VkCommandBuffer,
    pCopyBufferInfo: *const VkCopyBufferInfo2,
) {
    unsafe {
        let info = &*pCopyBufferInfo;

        let next_vkCmdCopyBuffer2 = Device::with(commandBuffer, |device| {
            if info.regionCount > 0 {
                let regions = std::slice::from_raw_parts(info.pRegions, info.regionCount as usize);
                device.apply_theme_to_regions(
                    info.srcBuffer,
                    regions.iter().map(|region| (region.srcOffset, region.size)),
                );
            }
            device
                .dispatch
                .vkCmdCopyBuffer2
                .expect("hooked without being available")
        });

        next_vkCmdCopyBuffer2(commandBuffer, pCopyBufferInfo)
    }
}
