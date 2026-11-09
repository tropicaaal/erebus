//! Hooked vulkan functions
//!
//! Each is named after the Vulkan function it replaces and calls the next layer's version.
//! 
//! Our goal here is:
//! - Track all created devices and instances so that device dispatch tables can be cached.
//! - Track

use std::collections::HashMap;

use crate::layer::{cast_pfn, with_instance};

use super::{
    BufferBinding, Device, DeviceDispatch, DispatchKey, Instance, MappedMemory, state, with_device,
};

use super::vk::{
    PFN_vkCreateDevice, PFN_vkCreateInstance, PFN_vkDestroyInstance,
    VK_ERROR_INITIALIZATION_FAILED, VK_LAYER_LINK_INFO,
    VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO, VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO,
    VK_SUCCESS, VkAllocationCallbacks, VkBaseInStructure, VkBindBufferMemoryInfo, VkBuffer,
    VkBufferCopy, VkCommandBuffer, VkCopyBufferInfo2, VkDevice, VkDeviceCreateInfo, VkDeviceMemory,
    VkDeviceSize, VkInstance, VkInstanceCreateInfo, VkLayerDeviceCreateInfo,
    VkLayerInstanceCreateInfo, VkMemoryMapFlags, VkPhysicalDevice, VkResult, VkStructureType,
};

/// funds the next
unsafe fn find_in_pnext<T>(
    mut node: *const core::ffi::c_void,
    sType: VkStructureType,
    is_link_info: impl Fn(&T) -> bool,
) -> *mut T {
    while !node.is_null() {
        let base = unsafe { &*(node as *const VkBaseInStructure) };
        if base.sType == sType && is_link_info(unsafe { &*(node as *const T) }) {
            return node as *mut T;
        }
        node = base.pNext as *const core::ffi::c_void;
    }
    core::ptr::null_mut()
}

//
// MARK: Device/instance tracking
//

pub unsafe extern "system" fn vkCreateInstance(
    pCreateInfo: *const VkInstanceCreateInfo,
    pAllocator: *const VkAllocationCallbacks,
    pInstance: *mut VkInstance,
) -> VkResult {
    unsafe {
        // aliirght, so main annoying thing here is finding the next layer's vkCreateInstance pointer.
        // since we aren't working with device handles, this isn't nearly as nice since aren't

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

            state().instances.insert(
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
        with_instance(instance, |instance| {
            let next_vkDestroyInstance = cast_pfn::<PFN_vkDestroyInstance>((instance.vkGetInstanceProcAddr)(
                instance.handle,
                c"vkDestroyInstance".as_ptr(),
            ))
            .unwrap();

            next_vkDestroyInstance(instance.handle, pAllocator)
        });

        state()
            .instances
            .remove(&DispatchKey::from_raw_handle(instance));
    }
}

pub unsafe extern "system" fn vkCreateDevice(
    physicalDevice: VkPhysicalDevice,
    pCreateInfo: *const VkDeviceCreateInfo,
    pAllocator: *const VkAllocationCallbacks,
    pDevice: *mut VkDevice,
) -> VkResult {
    unsafe {
        let link_info = find_in_pnext::<VkLayerDeviceCreateInfo>(
            (*pCreateInfo).pNext,
            VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO,
            |info| info.function == VK_LAYER_LINK_INFO,
        );

        // A physical device shares its instance's dispatch key.
        let instance = state()
            .instances
            .get(&DispatchKey::from_raw_handle(physicalDevice))
            .map(|instance| instance.handle as VkInstance);

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

            state().devices.insert(
                DispatchKey::from_raw_handle(device),
                Device {
                    handle: device,
                    dispatch,
                    mapped: HashMap::new(),
                    bound: HashMap::new(),
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
        let key = DispatchKey::from_raw_handle(device);
        let next = with_device(device, |device| device.dispatch);

        (next.vkDestroyDevice)(device, pAllocator);

        state().devices.remove(&key);
    }
}

//
// MARK: Memory map tracking
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
        with_device(device, |device| {
            let result =
                (device.dispatch.vkMapMemory)(device.handle, memory, offset, size, flags, ppData);

            if result == VK_SUCCESS {
                device.mapped.insert(
                    memory,
                    MappedMemory {
                        addr: *ppData as usize,
                        offset,
                        size,
                    },
                );
            }

            result
        })
    }
}

pub unsafe extern "system" fn vkUnmapMemory(device: VkDevice, memory: VkDeviceMemory) {
    let next = unsafe {
        with_device(device, |device| {
            device.mapped.remove(&memory);
            device.dispatch
        })
    };
    unsafe { (next.vkUnmapMemory)(device, memory) }
}

pub unsafe extern "system" fn vkFreeMemory(
    device: VkDevice,
    memory: VkDeviceMemory,
    pAllocator: *const VkAllocationCallbacks,
) {
    let next = unsafe {
        with_device(device, |device| {
            device.mapped.remove(&memory);
            device.bound.retain(|_, binding| binding.memory != memory);
            device.dispatch
        })
    };
    unsafe { (next.vkFreeMemory)(device, memory, pAllocator) }
}

pub unsafe extern "system" fn vkBindBufferMemory(
    device: VkDevice,
    buffer: VkBuffer,
    memory: VkDeviceMemory,
    memoryOffset: VkDeviceSize,
) -> VkResult {
    unsafe {
        with_device(device, |device| {
            let result =
                (device.dispatch.vkBindBufferMemory)(device.handle, buffer, memory, memoryOffset);

            if result == VK_SUCCESS {
                device.bound.insert(
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
        with_device(device, |device| {
            let result = (device.dispatch.vkBindBufferMemory2.unwrap())(
                device.handle,
                bindInfoCount,
                pBindInfos,
            );

            // update bindings if successful
            if result == VK_SUCCESS {
                for info in std::slice::from_raw_parts(pBindInfos, bindInfoCount as usize) {
                    device.bound.insert(
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
    let next = unsafe {
        with_device(device, |device| {
            device.bound.remove(&buffer);
            device.dispatch
        })
    };
    unsafe { (next.vkDestroyBuffer)(device, buffer, pAllocator) }
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
    let regions = unsafe { std::slice::from_raw_parts(pRegions, regionCount as usize) };
    let next = unsafe {
        with_device(commandBuffer, |device| {
            for region in regions {
                device.apply_theme_to_buffer(srcBuffer, region.srcOffset, region.size);
            }
            device.dispatch
        })
    };
    unsafe { (next.vkCmdCopyBuffer)(commandBuffer, srcBuffer, dstBuffer, regionCount, pRegions) }
}

pub unsafe extern "system" fn vkCmdCopyBuffer2(
    commandBuffer: VkCommandBuffer,
    pCopyBufferInfo: *const VkCopyBufferInfo2,
) {
    unsafe {
        let info = &*pCopyBufferInfo;
        let regions = std::slice::from_raw_parts(info.pRegions, info.regionCount as usize);

        let next = with_device(commandBuffer, |device| {
            for region in regions {
                device.apply_theme_to_buffer(info.srcBuffer, region.srcOffset, region.size);
            }

            device.dispatch
        });

        let next_vkCmdCopyBuffer2 = next
            .vkCmdCopyBuffer2
            .expect("hooked without being available");

        next_vkCmdCopyBuffer2(commandBuffer, pCopyBufferInfo)
    }
}
