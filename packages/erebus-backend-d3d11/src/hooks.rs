//! Hooks into D3D11 functions.

use std::{collections::hash_map, ffi::c_void, mem::transmute};

use crate::{
    d3d11::{
        D3D11_BIND_SHADER_RESOURCE, D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_WRITE,
        D3D11_MAP_WRITE_DISCARD, D3D11_MAPPED_SUBRESOURCE, D3D11_RESOURCE_DIMENSION_BUFFER,
        D3D11_RESOURCE_MISC_BUFFER_STRUCTURED, D3D11_USAGE_DYNAMIC,
    },
    shadow::{ShadowBuf, is_primitive_stride},
    vtable::{
        Originals, TrackedShadow, VT_BUFFER_GET_DESC, VT_CTX_MAP, VT_CTX_UNMAP, VT_RES_GET_TYPE,
    },
};
use windows_sys::{Win32::Foundation::E_FAIL, core::HRESULT};

use crate::vtable::{self, originals_for, tracked};

/// runs after D3D11CreateDevice/D3D11CreateDeviceAndSwapChain whenever a new device is created.
///
/// Given a new ID3D11DeviceContext, we patch its vtable so we can run our own code on Map/Unmap to track memory
pub unsafe fn device_create(_device: *mut core::ffi::c_void, context: *mut core::ffi::c_void) {
    if context.is_null() {
        return;
    }

    let vtable = unsafe {
        *(context as *const *mut *const c_void) // my head hurts
    };
    let mut guard = tracked();
    let state = guard.as_mut().unwrap();

    // Try to instantiate the theme, if we can't do that then we don't
    // track this device at all.
    if state.theme.is_none() {
        let Some(ctx) = crate::new_theme_ctx() else {
            return;
        };
        state.theme = Some(ctx);
    }

    let hash_map::Entry::Vacant(entry) = state.originals.entry(vtable.addr()) else {
        return;
    };

    // patch Map and Unmap to call our hook instead
    unsafe {
        let originals = Originals {
            map: transmute(*vtable.add(VT_CTX_MAP)),
            unmap: transmute(*vtable.add(VT_CTX_UNMAP)),
        };

        vtable::patch_entry(vtable.add(VT_CTX_MAP), Map as *const c_void);
        vtable::patch_entry(vtable.add(VT_CTX_UNMAP), Unmap as *const c_void);

        entry.insert(originals);
    }

    // TODO: might be necessary to handle a case where context is null, but device is passed? doesnt seem to matter in pratice tho?
}

/// runs instead of ID3D11DeviceContext->vptr->Map
///
/// See <https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-map>
unsafe extern "system" fn Map(
    vptr: *mut c_void,
    resource: *mut c_void,
    subresource: u32,
    map_type: u32,
    map_flags: u32,
    mapped: *mut D3D11_MAPPED_SUBRESOURCE,
) -> HRESULT {
    let Some(original) = originals_for(vptr) else {
        return E_FAIL; // unreachable: we only hook vtables we recorded
    };

    unsafe {
        // we only care about stuff mapped from this function:
        //
        // <https://github.com/zed-industries/zed/blob/main/crates/gpui_windows/src/directx_renderer.rs#L1637>
        let layout = if subresource == 0
            && map_type == D3D11_MAP_WRITE_DISCARD
            && !mapped.is_null()
            && !resource.is_null()
        {
            primitive_buffer_layout(resource)
        } else {
            None
        };

        let result = (original.map)(vptr, resource, subresource, map_type, map_flags, mapped);
        if result < 0 || (*mapped).pData.is_null() {
            return result;
        }
        let Some((byte_width, stride)) = layout else {
            return result;
        };

        let mapping_addr = (*mapped).pData.cast::<u8>();
        let mut guard = tracked();
        let state = guard.as_mut().unwrap();

        // keep the table from growing forever as GPUI swaps in bigger buffers
        if state.shadows.len() > ShadowBuf::MAX_IDLE_SHADOWS {
            state
                .shadows
                .retain(|_, shadow| !shadow.real_mapping_addr.is_null());
        }

        let shadow = state
            .shadows
            .entry(resource as usize)
            .or_insert_with(|| TrackedShadow {
                buffer: ShadowBuf::new(byte_width),
                stride,
                real_mapping_addr: core::ptr::null_mut(),
            });

        if !shadow.real_mapping_addr.is_null() {
            return result;
        }

        if shadow.buffer.len != byte_width || shadow.stride != stride {
            // address reused by a different buffer
            *shadow = TrackedShadow {
                buffer: ShadowBuf::new(byte_width),
                stride,
                real_mapping_addr: core::ptr::null_mut(),
            };
        }

        shadow.real_mapping_addr = mapping_addr;
        (*mapped).pData = shadow.buffer.data.as_mut_ptr().cast();
        result
    }
}

/// runs instead of ID3D11DeviceContext->vptr->Unmap
///
/// See <https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-unmap>
unsafe extern "system" fn Unmap(this: *mut c_void, resource: *mut c_void, subresource: u32) {
    let Some(original) = originals_for(this) else {
        return;
    };

    if subresource == 0 {
        let mut guard = tracked();
        let state = guard.as_mut().unwrap();

        if let (Some(shadow), Some(theme)) = (
            state.shadows.get_mut(&(resource as usize)),
            state.theme.as_mut(),
        ) {
            if !shadow.real_mapping_addr.is_null() {
                let real = std::mem::replace(&mut shadow.real_mapping_addr, core::ptr::null_mut());
                let stride = shadow.stride;

                _ = unsafe { shadow.buffer.flush(stride, theme, real) };
            }
        }
    }

    unsafe { (original.unmap)(this, resource, subresource) }
}

/// If resource contains GPUI primitives, calculate its (byte_width, stride)`.
///
/// This is deliberately strict: `DYNAMIC` + `SHADER_RESOURCE` + `STRUCTURED` + CPU write,
/// with a stride equal to one of our primitive sizes. Everything else (constant buffers,
/// path vertex data, staging readbacks, ...) goes straight to the driver.
unsafe fn primitive_buffer_layout(resource: *mut c_void) -> Option<(usize, usize)> {
    unsafe {
        let GetType: unsafe extern "system" fn(*mut c_void, *mut u32) =
            transmute(vtable::entry(resource, VT_RES_GET_TYPE));
        let mut dimension = 0;
        GetType(resource, &mut dimension);
        if dimension != D3D11_RESOURCE_DIMENSION_BUFFER {
            return None;
        }

        let GetDesc: unsafe extern "system" fn(*mut c_void, *mut D3D11_BUFFER_DESC) =
            transmute(vtable::entry(resource, VT_BUFFER_GET_DESC));
        let mut desc: D3D11_BUFFER_DESC = std::mem::zeroed();
        GetDesc(resource, &mut desc);

        let stride = desc.StructureByteStride as usize;

        (desc.Usage == D3D11_USAGE_DYNAMIC
            && desc.BindFlags & D3D11_BIND_SHADER_RESOURCE != 0
            && desc.CPUAccessFlags & D3D11_CPU_ACCESS_WRITE != 0
            && desc.MiscFlags & D3D11_RESOURCE_MISC_BUFFER_STRUCTURED != 0
            && desc.ByteWidth <= ShadowBuf::MAX_LEN
            && is_primitive_stride(stride)
            && desc.ByteWidth as usize % stride == 0)
            .then_some((desc.ByteWidth as usize, stride))
    }
}
