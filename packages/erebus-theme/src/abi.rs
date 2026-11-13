use std::{alloc::Layout, slice};

use crate::primitives::Primitive;

pub fn alloc(len: u32) -> u32 {
    match Layout::from_size_align(len as usize, align_of::<u32>()) {
        Ok(layout) if layout.size() > 0 => unsafe { std::alloc::alloc(layout) as usize as u32 },
        _ => 0,
    }
}

pub unsafe fn dealloc(ptr: u32, len: u32) {
    if ptr == 0 {
        return;
    }

    unsafe {
        let layout = Layout::from_size_align_unchecked(len as usize, align_of::<u32>());
        std::alloc::dealloc(ptr as usize as *mut u8, layout);
    }
}

pub unsafe fn apply<T: Primitive>(ptr: u32, len: u32, mut f: impl FnMut(&mut T)) -> bool {
    if ptr == 0 || len == 0 {
        return false;
    }

    let bytes = unsafe { slice::from_raw_parts_mut(ptr as usize as *mut u8, len as usize) };

    let (head, items, tail) = unsafe { bytes.align_to_mut::<T>() };
    if !head.is_empty() || !tail.is_empty() {
        return false;
    }

    let mut changed = false;
    for item in items {
        let before = *item;
        f(item);
        changed |= *item != before;
    }

    changed
}
