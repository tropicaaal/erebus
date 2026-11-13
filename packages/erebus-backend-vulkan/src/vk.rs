//! vulkan ffi

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::ffi::{c_char, c_void};

pub type VkResult = i32;
pub type VkStructureType = i32;
pub type VkDeviceSize = u64;
pub type VkFlags = u32;
pub type VkInstanceCreateFlags = VkFlags;
pub type VkDeviceCreateFlags = VkFlags;
pub type VkMemoryMapFlags = VkFlags;

pub const VK_SUCCESS: VkResult = 0;
pub const VK_ERROR_INITIALIZATION_FAILED: VkResult = -3;
pub const VK_WHOLE_SIZE: VkDeviceSize = !0;

pub const VK_STRUCTURE_TYPE_MAPPED_MEMORY_RANGE: VkStructureType = 6;
pub const VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO: VkStructureType = 47;
pub const VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO: VkStructureType = 48;

#[repr(C)]
pub struct VkInstance_T {
    _private: [u8; 0],
}
#[repr(C)]
pub struct VkPhysicalDevice_T {
    _private: [u8; 0],
}
#[repr(C)]
pub struct VkDevice_T {
    _private: [u8; 0],
}
#[repr(C)]
pub struct VkCommandBuffer_T {
    _private: [u8; 0],
}
pub type VkInstance = *mut VkInstance_T;
pub type VkPhysicalDevice = *mut VkPhysicalDevice_T;
pub type VkDevice = *mut VkDevice_T;
pub type VkCommandBuffer = *mut VkCommandBuffer_T;

pub type VkBuffer = u64;
pub type VkDeviceMemory = u64;

#[repr(C)]
pub struct VkBaseInStructure {
    pub sType: VkStructureType,
    pub pNext: *const VkBaseInStructure,
}

#[repr(C)]
pub struct VkAllocationCallbacks {
    _private: [u8; 0],
}
#[repr(C)]
pub struct VkDeviceQueueCreateInfo {
    _private: [u8; 0],
}
#[repr(C)]
pub struct VkPhysicalDeviceFeatures {
    _private: [u8; 0],
}

#[repr(C)]
pub struct VkApplicationInfo {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub pApplicationName: *const c_char,
    pub applicationVersion: u32,
    pub pEngineName: *const c_char,
    pub engineVersion: u32,
    pub apiVersion: u32,
}

#[repr(C)]
pub struct VkInstanceCreateInfo {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub flags: VkInstanceCreateFlags,
    pub pApplicationInfo: *const VkApplicationInfo,
    pub enabledLayerCount: u32,
    pub ppEnabledLayerNames: *const *const c_char,
    pub enabledExtensionCount: u32,
    pub ppEnabledExtensionNames: *const *const c_char,
}

#[repr(C)]
pub struct VkDeviceCreateInfo {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub flags: VkDeviceCreateFlags,
    pub queueCreateInfoCount: u32,
    pub pQueueCreateInfos: *const VkDeviceQueueCreateInfo,
    pub enabledLayerCount: u32,
    pub ppEnabledLayerNames: *const *const c_char,
    pub enabledExtensionCount: u32,
    pub ppEnabledExtensionNames: *const *const c_char,
    pub pEnabledFeatures: *const VkPhysicalDeviceFeatures,
}

#[repr(C)]
pub struct VkMappedMemoryRange {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub memory: VkDeviceMemory,
    pub offset: VkDeviceSize,
    pub size: VkDeviceSize,
}

#[repr(C)]
pub struct VkBindBufferMemoryInfo {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub buffer: VkBuffer,
    pub memory: VkDeviceMemory,
    pub memoryOffset: VkDeviceSize,
}

#[repr(C)]
pub struct VkBufferCopy {
    pub srcOffset: VkDeviceSize,
    pub dstOffset: VkDeviceSize,
    pub size: VkDeviceSize,
}

#[repr(C)]
pub struct VkBufferCopy2 {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub srcOffset: VkDeviceSize,
    pub dstOffset: VkDeviceSize,
    pub size: VkDeviceSize,
}

#[repr(C)]
pub struct VkCopyBufferInfo2 {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub srcBuffer: VkBuffer,
    pub dstBuffer: VkBuffer,
    pub regionCount: u32,
    pub pRegions: *const VkBufferCopy2,
}

pub type PFN_vkVoidFunction = Option<unsafe extern "system" fn()>;
pub type PFN_vkGetInstanceProcAddr =
    unsafe extern "system" fn(instance: VkInstance, pName: *const c_char) -> PFN_vkVoidFunction;
pub type PFN_vkGetDeviceProcAddr =
    unsafe extern "system" fn(device: VkDevice, pName: *const c_char) -> PFN_vkVoidFunction;
pub type PFN_GetPhysicalDeviceProcAddr =
    unsafe extern "system" fn(instance: VkInstance, pName: *const c_char) -> PFN_vkVoidFunction;

pub type PFN_vkCreateInstance = unsafe extern "system" fn(
    pCreateInfo: *const VkInstanceCreateInfo,
    pAllocator: *const VkAllocationCallbacks,
    pInstance: *mut VkInstance,
) -> VkResult;
pub type PFN_vkDestroyInstance =
    unsafe extern "system" fn(instance: VkInstance, pAllocator: *const VkAllocationCallbacks);
pub type PFN_vkCreateDevice = unsafe extern "system" fn(
    physicalDevice: VkPhysicalDevice,
    pCreateInfo: *const VkDeviceCreateInfo,
    pAllocator: *const VkAllocationCallbacks,
    pDevice: *mut VkDevice,
) -> VkResult;
pub type PFN_vkDestroyDevice =
    unsafe extern "system" fn(device: VkDevice, pAllocator: *const VkAllocationCallbacks);

pub type PFN_vkMapMemory = unsafe extern "system" fn(
    device: VkDevice,
    memory: VkDeviceMemory,
    offset: VkDeviceSize,
    size: VkDeviceSize,
    flags: VkMemoryMapFlags,
    ppData: *mut *mut c_void,
) -> VkResult;
pub type PFN_vkUnmapMemory = unsafe extern "system" fn(device: VkDevice, memory: VkDeviceMemory);
pub type PFN_vkFreeMemory = unsafe extern "system" fn(
    device: VkDevice,
    memory: VkDeviceMemory,
    pAllocator: *const VkAllocationCallbacks,
);
pub type PFN_vkFlushMappedMemoryRanges = unsafe extern "system" fn(
    device: VkDevice,
    memoryRangeCount: u32,
    pMemoryRanges: *const VkMappedMemoryRange,
) -> VkResult;
pub type PFN_vkBindBufferMemory = unsafe extern "system" fn(
    device: VkDevice,
    buffer: VkBuffer,
    memory: VkDeviceMemory,
    memoryOffset: VkDeviceSize,
) -> VkResult;
pub type PFN_vkBindBufferMemory2 = unsafe extern "system" fn(
    device: VkDevice,
    bindInfoCount: u32,
    pBindInfos: *const VkBindBufferMemoryInfo,
) -> VkResult;
pub type PFN_vkDestroyBuffer = unsafe extern "system" fn(
    device: VkDevice,
    buffer: VkBuffer,
    pAllocator: *const VkAllocationCallbacks,
);
pub type PFN_vkCmdCopyBuffer = unsafe extern "system" fn(
    commandBuffer: VkCommandBuffer,
    srcBuffer: VkBuffer,
    dstBuffer: VkBuffer,
    regionCount: u32,
    pRegions: *const VkBufferCopy,
);
pub type PFN_vkCmdCopyBuffer2 = unsafe extern "system" fn(
    commandBuffer: VkCommandBuffer,
    pCopyBufferInfo: *const VkCopyBufferInfo2,
);

pub type VkNegotiateLayerStructType = u32;
pub const LAYER_NEGOTIATE_INTERFACE_STRUCT: VkNegotiateLayerStructType = 1;

#[repr(C)]
pub struct VkNegotiateLayerInterface {
    pub sType: VkNegotiateLayerStructType,
    pub pNext: *mut c_void,
    pub loaderLayerInterfaceVersion: u32,
    pub pfnGetInstanceProcAddr: Option<PFN_vkGetInstanceProcAddr>,
    pub pfnGetDeviceProcAddr: Option<PFN_vkGetDeviceProcAddr>,
    pub pfnGetPhysicalDeviceProcAddr: Option<PFN_GetPhysicalDeviceProcAddr>,
}

pub type VkLayerFunction = u32;
pub const VK_LAYER_LINK_INFO: VkLayerFunction = 0;
pub const VK_LOADER_DATA_CALLBACK: VkLayerFunction = 1;
pub const VK_LOADER_LAYER_CREATE_DEVICE_CALLBACK: VkLayerFunction = 2;
pub const VK_LOADER_FEATURES: VkLayerFunction = 3;

#[repr(C)]
pub struct VkLayerInstanceLink {
    pub pNext: *mut VkLayerInstanceLink,
    pub pfnNextGetInstanceProcAddr: PFN_vkGetInstanceProcAddr,
    pub pfnNextGetPhysicalDeviceProcAddr: Option<PFN_GetPhysicalDeviceProcAddr>,
}

#[repr(C)]
pub struct VkLayerDeviceLink {
    pub pNext: *mut VkLayerDeviceLink,
    pub pfnNextGetInstanceProcAddr: PFN_vkGetInstanceProcAddr,
    pub pfnNextGetDeviceProcAddr: PFN_vkGetDeviceProcAddr,
}

pub type PFN_vkSetInstanceLoaderData =
    unsafe extern "system" fn(instance: VkInstance, object: *mut c_void) -> VkResult;
pub type PFN_vkSetDeviceLoaderData =
    unsafe extern "system" fn(device: VkDevice, object: *mut c_void) -> VkResult;

#[repr(C)]
#[derive(Clone, Copy)]
pub union VkLayerInstanceCreateInfo_u {
    pub pLayerInfo: *mut VkLayerInstanceLink,
    pub pfnSetInstanceLoaderData: PFN_vkSetInstanceLoaderData,
    pub loaderFeatures: u32,
}

#[repr(C)]
pub struct VkLayerInstanceCreateInfo {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub function: VkLayerFunction,
    pub u: VkLayerInstanceCreateInfo_u,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union VkLayerDeviceCreateInfo_u {
    pub pLayerInfo: *mut VkLayerDeviceLink,
    pub pfnSetDeviceLoaderData: PFN_vkSetDeviceLoaderData,
}

#[repr(C)]
pub struct VkLayerDeviceCreateInfo {
    pub sType: VkStructureType,
    pub pNext: *const c_void,
    pub function: VkLayerFunction,
    pub u: VkLayerDeviceCreateInfo_u,
}
