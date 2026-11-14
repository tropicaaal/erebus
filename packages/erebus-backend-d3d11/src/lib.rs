#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

pub mod d3d11;
pub mod hooks;
pub mod shadow;
pub mod vtable;

use std::{
    ffi::{CStr, c_void},
    sync::{LazyLock, OnceLock},
};

use erebus_engine::{ThemeCtx, ThemeModule};
use windows_sys::{
    Win32::{
        Foundation::{E_FAIL, FARPROC, HMODULE},
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryW},
            SystemInformation::GetSystemDirectoryW,
        },
    },
    core::HRESULT,
};

/// stores HMODULE cast to usize because rust doesn't like raw pointers in statics
static ORIGINAL_D3D11: OnceLock<usize> = OnceLock::new();

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

/// Instantiate the theme.
fn new_theme_ctx() -> Option<ThemeCtx> {
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

    Some(ctx)
}

fn find_orig_proc(name: &CStr) -> FARPROC {
    // LoadLibrary from system32 so we don't load ourselves
    let module = *ORIGINAL_D3D11.get_or_init(|| unsafe {
        let mut buf = [0u16; 260]; // utf16

        // should be C:\Windows\System32
        let len = GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as _) as usize;
        if len == 0 || len >= buf.len() {
            return 0;
        }

        let mut path = buf[..len].to_vec();
        path.extend("\\d3d11.dll\0".encode_utf16());

        LoadLibraryW(path.as_ptr()) as _
    });

    // this should maybe be a panic
    if module == 0 {
        return None;
    }

    unsafe { GetProcAddress(module as HMODULE, name.as_ptr().cast()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D11CreateDevice(
    pAdapter: *mut c_void,
    DriverType: u32,
    Software: HMODULE,
    Flags: u32,
    pFeatureLevels: *const u32,
    FeatureLevels: u32,
    SDKVersion: u32,
    ppDevice: *mut *mut c_void,
    pFeatureLevel: *mut u32,
    ppImmediateContext: *mut *mut c_void,
) -> HRESULT {
    let Some(proc) = find_orig_proc(c"D3D11CreateDevice") else {
        return E_FAIL;
    };

    let orig_D3D11CreateDevice: d3d11::PFN_D3D11_CREATE_DEVICE =
        unsafe { std::mem::transmute(proc) };

    let result = unsafe {
        orig_D3D11CreateDevice(
            pAdapter,
            DriverType,
            Software,
            Flags,
            pFeatureLevels,
            FeatureLevels,
            SDKVersion,
            ppDevice,
            pFeatureLevel,
            ppImmediateContext,
        )
    };

    if result >= 0 {
        unsafe {
            let device = if ppDevice.is_null() {
                std::ptr::null_mut()
            } else {
                *ppDevice
            };
            let context = if ppImmediateContext.is_null() {
                std::ptr::null_mut()
            } else {
                *ppImmediateContext
            };
            hooks::device_create(device, context);
        };
    }
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn D3D11CreateDeviceAndSwapChain(
    pAdapter: *mut c_void,
    DriverType: u32,
    Software: HMODULE,
    Flags: u32,
    pFeatureLevels: *const u32,
    FeatureLevels: u32,
    SDKVersion: u32,
    pSwapChainDesc: *const c_void,
    ppSwapChain: *mut *mut c_void,
    ppDevice: *mut *mut c_void,
    pFeatureLevel: *mut u32,
    ppImmediateContext: *mut *mut c_void,
) -> HRESULT {
    let Some(proc) = find_orig_proc(c"D3D11CreateDeviceAndSwapChain") else {
        return E_FAIL;
    };

    let orig_D3D11CreateDeviceAndSwapChain: d3d11::PFN_D3D11_CREATE_DEVICE_AND_SWAP_CHAIN =
        unsafe { std::mem::transmute(proc) };

    let result = unsafe {
        orig_D3D11CreateDeviceAndSwapChain(
            pAdapter,
            DriverType,
            Software,
            Flags,
            pFeatureLevels,
            FeatureLevels,
            SDKVersion,
            pSwapChainDesc,
            ppSwapChain,
            ppDevice,
            pFeatureLevel,
            ppImmediateContext,
        )
    };

    if result >= 0 {
        unsafe {
            let device = if ppDevice.is_null() {
                std::ptr::null_mut()
            } else {
                *ppDevice
            };
            let context = if ppImmediateContext.is_null() {
                std::ptr::null_mut()
            } else {
                *ppImmediateContext
            };

            hooks::device_create(device, context);
        };
    }

    result
}
