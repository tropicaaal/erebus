use std::{alloc::Layout, cell::RefCell, collections::HashMap, slice, thread::LocalKey};

use crate::{
    Theme,
    primitives::Primitive,
    settings::{SettingValue, Settings},
};

pub const ABI_VERSION: u32 = 1;

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

pub fn with_theme<T: Theme, R>(
    instance: &'static LocalKey<RefCell<Option<T>>>,
    f: impl FnOnce(&mut T) -> R,
) -> R {
    instance.with_borrow_mut(|theme| {
        let theme = theme.get_or_insert_with(|| {
            T::init(&Settings::resolve(T::MANIFEST.settings, &HashMap::new()))
        });
        f(theme)
    })
}

pub fn manifest<T: Theme>() -> u64 {
    let Ok(bytes) = postcard::to_allocvec(&T::MANIFEST) else {
        return 0;
    };
    let Ok(len) = u32::try_from(bytes.len()) else {
        return 0;
    };

    let ptr = alloc(len);
    if ptr == 0 {
        return 0;
    }

    unsafe { slice::from_raw_parts_mut(ptr as usize as *mut u8, len as usize) }
        .copy_from_slice(&bytes);

    (u64::from(ptr) << 32) | u64::from(len)
}

pub unsafe fn init<T: Theme>(
    instance: &'static LocalKey<RefCell<Option<T>>>,
    ptr: u32,
    len: u32,
) -> u32 {
    let bytes: &[u8] = if ptr == 0 || len == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(ptr as usize as *const u8, len as usize) }
    };

    let values = if bytes.is_empty() {
        HashMap::new()
    } else {
        match postcard::from_bytes::<HashMap<String, SettingValue>>(bytes) {
            Ok(values) => values,
            Err(_) => return 1,
        }
    };

    let settings = Settings::resolve(T::MANIFEST.settings, &values);
    instance.with_borrow_mut(|theme| *theme = Some(T::init(&settings)));

    0
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
