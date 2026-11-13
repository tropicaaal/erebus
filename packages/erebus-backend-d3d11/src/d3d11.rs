//! d3d11 ffi

#![allow(non_camel_case_types)]

use windows_sys::{Win32::Foundation::HMODULE, core::HRESULT};

pub const D3D11_RESOURCE_DIMENSION_BUFFER: u32 = 1;
pub const D3D11_USAGE_DYNAMIC: u32 = 2;
pub const D3D11_BIND_SHADER_RESOURCE: u32 = 0x8;
pub const D3D11_CPU_ACCESS_WRITE: u32 = 0x10000;
pub const D3D11_RESOURCE_MISC_BUFFER_STRUCTURED: u32 = 0x40;
pub const D3D11_MAP_WRITE_DISCARD: u32 = 4;

pub type PFN_D3D11_CREATE_DEVICE = unsafe extern "system" fn(
    pAdapter: *mut core::ffi::c_void,
    DriverType: u32,
    Software: HMODULE,
    Flags: u32,
    pFeatureLevels: *const u32,
    FeatureLevels: u32,
    SDKVersion: u32,
    ppDevice: *mut *mut core::ffi::c_void,
    pFeatureLevel: *mut u32,
    ppImmediateContext: *mut *mut core::ffi::c_void,
) -> HRESULT;

pub type PFN_D3D11_CREATE_DEVICE_AND_SWAP_CHAIN = unsafe extern "system" fn(
    pAdapter: *mut core::ffi::c_void,
    DriverType: u32,
    Software: HMODULE,
    Flags: u32,
    pFeatureLevels: *const u32,
    FeatureLevels: u32,
    SDKVersion: u32,
    pSwapChainDesc: *const core::ffi::c_void,
    ppSwapChain: *mut *mut core::ffi::c_void,
    ppDevice: *mut *mut core::ffi::c_void,
    pFeatureLevel: *mut u32,
    ppImmediateContext: *mut *mut core::ffi::c_void,
) -> HRESULT;

#[repr(C)]
pub struct D3D11_BUFFER_DESC {
    pub ByteWidth: u32,
    pub Usage: u32,
    pub BindFlags: u32,
    pub CPUAccessFlags: u32,
    pub MiscFlags: u32,
    pub StructureByteStride: u32,
}

#[repr(C)]
pub struct D3D11_MAPPED_SUBRESOURCE {
    pub pData: *mut core::ffi::c_void,
    pub RowPitch: u32,
    pub DepthPitch: u32,
}
