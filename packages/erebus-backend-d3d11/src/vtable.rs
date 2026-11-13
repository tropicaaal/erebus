//! Stuff for patching vtables on COM objects.

use std::ffi::c_void;

use std::sync::{Mutex, MutexGuard, PoisonError};

use erebus_engine::ThemeCtx;
use fxhash::FxHashMap;
use windows_sys::core::HRESULT;

use crate::{d3d11::D3D11_MAPPED_SUBRESOURCE, shadow::ShadowBuf};

use windows_sys::Win32::System::Memory::{PAGE_READWRITE, VirtualProtect};

// indicies on vtable for various types
pub const VT_CTX_MAP: usize = 14;
pub const VT_CTX_UNMAP: usize = 15;
pub const VT_RES_GET_TYPE: usize = 7;
pub const VT_BUFFER_GET_DESC: usize = 10;

/// set a vtable entry's value to a different function pointer
pub unsafe fn patch_entry(p_entry: *mut *const c_void, value: *const c_void) {
    let mut old = 0u32;
    unsafe {
        // vtables of system COM objects are normally readonly
        if VirtualProtect(p_entry.cast(), size_of::<usize>(), PAGE_READWRITE, &mut old) != 0 {
            *p_entry = value;
            VirtualProtect(p_entry.cast(), size_of::<usize>(), old, &mut old);
        }
    }
}

/// read a vtable function pointer entry at a given index in a COM object
pub unsafe fn entry(object: *mut c_void, index: usize) -> *const c_void {
    unsafe {
        let vtable = *(object as *const *const *const c_void);
        *vtable.add(index)
    }
}

/// The functions we replaced, so we can still call them.
#[derive(Clone, Copy)]
pub struct Originals {
    pub map: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        resource: *mut core::ffi::c_void,
        subresource: u32,
        map_type: u32,
        map_flags: u32,
        mapped: *mut D3D11_MAPPED_SUBRESOURCE,
    ) -> HRESULT,
    pub unmap: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        resource: *mut core::ffi::c_void,
        subresource: u32,
    ),
}

pub struct TrackedShadow {
    pub buffer: ShadowBuf,
    pub stride: usize,
    pub real_mapping_addr: *mut u8,
}

pub struct Tracked {
    /// keyed by vtable address
    pub originals: FxHashMap<usize, Originals>,

    /// keyed by ID3D11Resource*
    pub shadows: FxHashMap<usize, TrackedShadow>,

    /// The loaded theme.
    pub theme: Option<ThemeCtx>,
}
unsafe impl Send for Tracked {}

static TRACKED: Mutex<Option<Tracked>> = Mutex::new(None);

pub fn tracked() -> MutexGuard<'static, Option<Tracked>> {
    let mut guard = TRACKED.lock().unwrap_or_else(PoisonError::into_inner);

    guard.get_or_insert_with(|| Tracked {
        originals: FxHashMap::default(),
        shadows: FxHashMap::default(),
        theme: None,
    });

    guard
}

pub fn originals_for(context: *mut core::ffi::c_void) -> Option<Originals> {
    let vtable = unsafe { *(context as *const usize) };
    tracked().as_ref()?.originals.get(&vtable).copied()
}
