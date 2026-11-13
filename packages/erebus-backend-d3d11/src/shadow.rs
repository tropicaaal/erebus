//! When GPUI maps a primitive buffer we give it back a pointer to "shadow buffer" instead
//! of the actual mapped GPU memory.
//!
//! We do this because theming in place on the mapped GPU memory is impractical on DX11. Buffers
//! get mapped to [write-combined] pages because GPUI maps its buffers with [`D3D11_MAP_WRITE_DISCARD`],
//! so reading it is uncached and requires flushing any pending writes. This is *really* bad for us since:
//!
//! - We have to do effectively a linear search on that span of memory to find the boundaries of our
//!   primitives in [`written_len`].
//!
//! - Themes need to read the primitives.
//!
//! So instead we give it this fake buffer and on Unmap, [`ShadowBuffer::flush`] applies the theme then copies
//! it into the real mapping.
//!
//! [write-combined]: https://en.wikipedia.org/wiki/Write_combining

use erebus_engine::ThemeCtx;
use erebus_theme::{MonochromeSprite, PolychromeSprite, Quad, Shadow, SubpixelSprite, Underline};

pub fn is_primitive_stride(stride: usize) -> bool {
    stride != 0
        && stride % 4 == 0
        && [
            size_of::<Quad>(),
            size_of::<Shadow>(),
            size_of::<Underline>(),
            size_of::<MonochromeSprite>(),
            size_of::<SubpixelSprite>(),
            size_of::<PolychromeSprite>(),
        ]
        .contains(&stride)
}

pub struct ShadowBuf {
    pub data: Box<[u64]>,
    pub len: usize,
}

impl ShadowBuf {
    pub const MAX_LEN: u32 = 256 << 20;
    pub const MAX_IDLE_SHADOWS: usize = 32;

    pub fn new(len: usize) -> Self {
        Self {
            data: vec![0; len.div_ceil(8)].into_boxed_slice(),
            len,
        }
    }

    /// Commit buffer to mapped GPU memory.
    pub unsafe fn flush(&mut self, stride: usize, theme: &mut ThemeCtx, dst: *mut u8) -> usize {
        let used = written_len(
            unsafe { std::slice::from_raw_parts(self.data.as_ptr().cast(), self.len) },
            stride,
        );

        if used == 0 {
            return 0;
        }

        let bytes =
            unsafe { std::slice::from_raw_parts_mut(self.data.as_mut_ptr().cast::<u8>(), used) };

        if is_primitive_stride(stride) {
            _ = theme.apply(bytes);
        }

        // SAFETY: caller guarantees that for dst, used <= len.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, used) };
        bytes.fill(0);

        used
    }
}

/// Run a linear search to determine the length of bytes GPU wrote to the shadowbuf.
pub fn written_len(data: &[u8], stride: usize) -> usize {
    if stride == 0 {
        return 0;
    }

    // SAFETY: any bit pattern is a valid u64.
    let (head, words, tail) = unsafe { data.align_to::<u64>() };

    let last = if let Some(i) = tail.iter().rposition(|&b| b != 0) {
        Some(head.len() + words.len() * 8 + i)
    } else if let Some(w) = words.iter().rposition(|&w| w != 0) {
        let base = head.len() + w * 8;
        data[base..base + 8]
            .iter()
            .rposition(|&b| b != 0)
            .map(|i| base + i)
    } else {
        head.iter().rposition(|&b| b != 0)
    };

    match last {
        Some(i) => ((i + 1).div_ceil(stride) * stride).min(data.len() / stride * stride),
        None => 0,
    }
}
