#![allow(non_camel_case_types)]

//! d3d11 ffi

use windows_sys::{Win32::Foundation::HMODULE, core::HRESULT};

pub type PFN_D3D11_CREATE_DEVICE = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    u32,
    HMODULE,
    u32,
    *const u32,
    u32,
    u32,
    *mut *mut core::ffi::c_void,
    *mut u32,
    *mut *mut core::ffi::c_void,
) -> HRESULT;

pub type PFN_D3D11_CREATE_DEVICE_AND_SWAP_CHAIN = unsafe extern "system" fn(
    *mut core::ffi::c_void,
    u32,
    HMODULE,
    u32,
    *const u32,
    u32,
    u32,
    *const core::ffi::c_void,
    *mut *mut core::ffi::c_void,
    *mut *mut core::ffi::c_void,
    *mut u32,
    *mut *mut core::ffi::c_void,
) -> HRESULT;
