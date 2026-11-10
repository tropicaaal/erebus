//! GPUI Scene Primitives
//!
//! This module re-creates most `#[repr(C)]` primitive structs and subtypes from
//! <https://github.com/zed-industries/zed/blob/main/crates/gpui/src/scene.rs>.
//!
//! Every type here must exactly match the layout of GPUI's struct layouts.

// Portions of this file are derived from the zed editor, which is licensed
// under the Apache License, Version 2.0.
//
// Copyright 2022 - 2025 Zed Industries, Inc.
//
// <https://github.com/zed-industries/zed/blob/main/LICENSE-APACHE>

use crate::theme::Theme;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsla {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl Hsla {
    pub const WHITE: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 1.0,
    };

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::from_rgba(r, g, b, 255)
    }

    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        let r = r as f32 / 255.0;
        let g = g as f32 / 255.0;
        let b = b as f32 / 255.0;
        let a = a as f32 / 255.0;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let l = (max + min) / 2.0;

        let s = if delta == 0.0 {
            0.0
        } else {
            delta / (1.0 - (2.0 * l - 1.0).abs())
        };

        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            let h = (g - b) / delta;
            (if h < 0.0 { h + 6.0 } else { h }) / 6.0
        } else if max == g {
            ((b - r) / delta + 2.0) / 6.0
        } else {
            ((r - g) / delta + 4.0) / 6.0
        };

        Hsla { h, s, l, a }
    }

    pub fn approx_eq(self, other: Hsla) -> bool {
        const EPSILON: f32 = 1.0 / 512.0;
        (self.h - other.h).abs() < EPSILON
            && (self.s - other.s).abs() < EPSILON
            && (self.l - other.l).abs() < EPSILON
            && (self.a - other.a).abs() < EPSILON
    }

    fn looks_valid(&self) -> bool {
        all_in_range(&[self.h, self.s, self.l, self.a], 0.0, 1.0)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub origin: Point,
    pub size: Size,
}

impl Bounds {
    fn looks_valid(&self) -> bool {
        all_in_range(&[self.origin.x, self.origin.y], -1e6, 1e6)
            && all_in_range(&[self.size.width, self.size.height], 0.0, 1e6)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContentMask {
    pub bounds: Bounds,
}

impl ContentMask {
    fn looks_valid(&self) -> bool {
        let b = &self.bounds;
        all_in_range(
            &[b.origin.x, b.origin.y, b.size.width, b.size.height],
            -1e6,
            1e6,
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl Corners {
    fn looks_valid(&self) -> bool {
        all_in_range(
            &[
                self.top_left,
                self.top_right,
                self.bottom_right,
                self.bottom_left,
            ],
            0.0,
            1e6,
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    fn looks_valid(&self) -> bool {
        all_in_range(&[self.top, self.right, self.bottom, self.left], 0.0, 1e6)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BackgroundTag(pub u32);
impl BackgroundTag {
    pub const SOLID: Self = Self(0);
    pub const LINEAR_GRADIENT: Self = Self(1);
    pub const PATTERN_SLASH: Self = Self(2);
    pub const CHECKERBOARD: Self = Self(3);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColorSpace(pub u32);
impl ColorSpace {
    pub const SRGB: Self = Self(0);
    pub const OKLAB: Self = Self(1);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BorderStyle(pub u32);
impl BorderStyle {
    pub const SOLID: Self = Self(0);
    pub const DASHED: Self = Self(1);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinearColorStop {
    pub color: Hsla,
    pub percentage: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Background {
    pub tag: BackgroundTag,
    pub color_space: ColorSpace,
    pub solid: Hsla,
    pub gradient_angle_or_pattern_height: f32,
    pub colors: [LinearColorStop; 2],
    pad: u32,
}

impl Background {
    pub fn solid(color: Hsla) -> Self {
        Self {
            tag: BackgroundTag::SOLID,
            solid: color,
            ..Self::default()
        }
    }

    pub fn linear_gradient(
        angle_degrees: f32,
        from: LinearColorStop,
        to: LinearColorStop,
        color_space: ColorSpace,
    ) -> Self {
        Self {
            tag: BackgroundTag::LINEAR_GRADIENT,
            color_space,
            gradient_angle_or_pattern_height: angle_degrees,
            colors: [from, to],
            ..Self::default()
        }
    }

    pub fn solid_color(&self) -> Option<Hsla> {
        (self.tag == BackgroundTag::SOLID).then_some(self.solid)
    }

    fn looks_valid(&self) -> bool {
        self.tag.0 <= BackgroundTag::CHECKERBOARD.0
            && self.color_space.0 <= ColorSpace::OKLAB.0
            && self.solid.looks_valid()
            && self.pad == 0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaddedBool32(pub u32);

impl PaddedBool32 {
    pub const FALSE: Self = Self(0);
    pub const TRUE: Self = Self(1);

    pub const fn get(self) -> bool {
        self.0 != 0
    }

    fn looks_valid(self) -> bool {
        self.0 <= 1
    }
}

impl From<bool> for PaddedBool32 {
    fn from(value: bool) -> Self {
        Self(value as u32)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DevicePoint {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceSize {
    pub width: i32,
    pub height: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceBounds {
    pub origin: DevicePoint,
    pub size: DeviceSize,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasTextureKind(pub u32);
impl AtlasTextureKind {
    pub const MONOCHROME: Self = Self(0);
    pub const POLYCHROME: Self = Self(1);
    pub const SUBPIXEL: Self = Self(2);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasTextureId {
    pub index: u32,
    pub kind: AtlasTextureKind,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TileId(pub u32);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AtlasTile {
    pub texture_id: AtlasTextureId,
    pub tile_id: TileId,
    pub padding: u32,
    pub bounds: DeviceBounds,
}

impl AtlasTile {
    /// anything larger than this isn't realistic
    const MAX_DIMENSION: i32 = 1 << 16;

    fn looks_valid(&self, kind: AtlasTextureKind) -> bool {
        let (o, s) = (self.bounds.origin, self.bounds.size);
        self.texture_id.kind == kind
            && [o.x, o.y, s.width, s.height]
                .iter()
                .all(|v| (0..=Self::MAX_DIMENSION).contains(v))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformationMatrix {
    pub rotation_scale: [[f32; 2]; 2],
    pub translation: [f32; 2],
}

impl TransformationMatrix {
    pub const fn unit() -> Self {
        Self {
            rotation_scale: [[1.0, 0.0], [0.0, 1.0]],
            translation: [0.0, 0.0],
        }
    }

    fn looks_valid(&self) -> bool {
        let [[a, b], [c, d]] = self.rotation_scale;
        let [x, y] = self.translation;
        all_in_range(&[a, b, c, d, x, y], -1e6, 1e6)
    }
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
    }
}

//
// MARK: Paint Primitives
//
// every type below is made only of 4-byte u32, i32, and f32 fields, so it
// has no padding, 4-byte alignment, and every bit pattern is a valid value
// so transmutation is (generally) safe.
//

/// Scene primitives that can be reinterpreted from raw bytes in a mapped VkBuffer. This is
/// (for the most part) any child type of GPUI's [`Primitive`] enum.
///
/// [`Primitive`]: https://github.com/zed-industries/zed/blob/main/crates/gpui/src/scene.rs#L222C10-L222C19
///
/// # Safety
///
/// Implementors must be `#[repr(C)]`, contain no padding bytes, and be valid for every bit
/// pattern.
pub unsafe trait PaintPrimitive: Copy + PartialEq {
    /// Returns `true` if this instance plausibly came from GPUI rather than being garbage.
    fn looks_valid(&self) -> bool;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    pub order: u32,
    pub border_style: BorderStyle,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub background: Background,
    pub border_color: Hsla,
    pub corner_radii: Corners,
    pub border_widths: Edges,
}

unsafe impl PaintPrimitive for Quad {
    // this might be really overkill
    fn looks_valid(&self) -> bool {
        self.border_style.0 <= BorderStyle::DASHED.0
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.background.looks_valid()
            && self.border_color.looks_valid()
            && self.corner_radii.looks_valid()
            && self.border_widths.looks_valid()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Underline {
    pub order: u32,
    pub pad: u32, // align to 8 bytes
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub thickness: f32,
    pub wavy: PaddedBool32,
}

unsafe impl PaintPrimitive for Underline {
    fn looks_valid(&self) -> bool {
        self.pad == 0
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && all_in_range(&[self.thickness], 0.0, 1e6)
            && self.wavy.looks_valid()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shadow {
    pub order: u32,
    pub blur_radius: f32,
    pub bounds: Bounds,
    pub corner_radii: Corners,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub element_bounds: Bounds,
    pub element_corner_radii: Corners,
    /// 0 = drop shadow (rendered outside the element), 1 = inset shadow (rendered inside).
    pub inset: u32,
    pub pad: u32, // align to 8 bytes
}

unsafe impl PaintPrimitive for Shadow {
    fn looks_valid(&self) -> bool {
        all_in_range(&[self.blur_radius], 0.0, 1e6)
            && self.bounds.looks_valid()
            && self.corner_radii.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && self.element_bounds.looks_valid()
            && self.element_corner_radii.looks_valid()
            && self.inset <= 1
            && self.pad == 0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MonochromeSprite {
    pub order: u32,
    pub pad: u32,
    pub tile: AtlasTile,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub transformation: TransformationMatrix,
}

unsafe impl PaintPrimitive for MonochromeSprite {
    fn looks_valid(&self) -> bool {
        self.pad == 0
            && self.tile.looks_valid(AtlasTextureKind::MONOCHROME)
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && self.transformation.looks_valid()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SubpixelSprite {
    pub order: u32,
    /// align to 8 bytes
    pub pad: u32,
    pub tile: AtlasTile,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub color: Hsla,
    pub transformation: TransformationMatrix,
}

unsafe impl PaintPrimitive for SubpixelSprite {
    fn looks_valid(&self) -> bool {
        self.pad == 0
            && self.tile.looks_valid(AtlasTextureKind::SUBPIXEL)
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.color.looks_valid()
            && self.transformation.looks_valid()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PolychromeSprite {
    pub order: u32,
    pub pad: u32,
    pub tile: AtlasTile,
    pub grayscale: PaddedBool32,
    pub opacity: f32,
    pub bounds: Bounds,
    pub content_mask: ContentMask,
    pub corner_radii: Corners,
}

unsafe impl PaintPrimitive for PolychromeSprite {
    fn looks_valid(&self) -> bool {
        self.pad == 0
            && self.tile.looks_valid(AtlasTextureKind::POLYCHROME)
            && self.grayscale.looks_valid()
            && all_in_range(&[self.opacity], 0.0, 1.0)
            && self.bounds.looks_valid()
            && self.content_mask.looks_valid()
            && self.corner_radii.looks_valid()
    }
}

//
// MARK: Detection and dispatch
//

/// The primitive types we currently scan for and theme in a GPU upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveKind {
    Shadow,
    Quad,
    Underline,
    MonochromeSprite,
    SubpixelSprite,
    PolychromeSprite,
}

impl PrimitiveKind {
    /// Tries to deduce which primitive type a given upload contains.
    pub fn classify_bytes(bytes: &[u8]) -> Option<Self> {
        let candidates = [
            (PrimitiveKind::Quad, is_array_of::<Quad>(bytes)),
            (PrimitiveKind::Shadow, is_array_of::<Shadow>(bytes)),
            (PrimitiveKind::Underline, is_array_of::<Underline>(bytes)),
            (
                PrimitiveKind::MonochromeSprite,
                is_array_of::<MonochromeSprite>(bytes),
            ),
            (
                PrimitiveKind::SubpixelSprite,
                is_array_of::<SubpixelSprite>(bytes),
            ),
            (
                PrimitiveKind::PolychromeSprite,
                is_array_of::<PolychromeSprite>(bytes),
            ),
        ];

        // only consider canditates that are valid arrays of the primitive
        let mut matches = candidates.iter().filter(|(_, is_valid)| *is_valid);

        match (matches.next(), matches.next()) {
            (Some(&(kind, _)), None) => Some(kind),

            // either the bytes weren't valid for any primitive, or the bytes are
            // possibly valid for more than one different primitive array types. In
            // the second case, that's pretty bad and shouldn't be guessed on, so
            // return None
            _ => None,
        }
    }
}

/// Is `bytes` a non-empty, properly aligned array of `T` where every element looks valid?
fn is_array_of<T: PaintPrimitive>(bytes: &[u8]) -> bool {
    // SAFETY: `Instance` guarantees every bit pattern is a valid `T`.
    let (head, items, tail) = unsafe { bytes.align_to::<T>() };
    head.is_empty() && tail.is_empty() && !items.is_empty() && items.iter().all(T::looks_valid)
}

/// Reinterprets `bytes` as `[T]`, returning `None` unless every element is valid.
pub fn primitives_from_bytes_mut<T: PaintPrimitive>(bytes: &mut [u8]) -> Option<&mut [T]> {
    // SAFETY: `Instance` guarantees every bit pattern is a valid `T`.
    let (head, items, tail) = unsafe { bytes.align_to_mut::<T>() };

    let fits = head.is_empty() && tail.is_empty() && !items.is_empty();

    (fits && items.iter().all(T::looks_valid)).then_some(items)
}

/// Runs `f` over every `T` in `bytes`, returning whether any bytes changed.
fn visit_primitives<T: PaintPrimitive>(bytes: &mut [u8], mut f: impl FnMut(&mut T)) -> bool {
    let Some(items) = primitives_from_bytes_mut::<T>(bytes) else {
        return false;
    };

    let mut changed = false;
    for item in items {
        // SAFETY: `PaintPrimitive` must be valid for any byte representation, so converting it to a slice is fine.
        let before_bytes =
            unsafe { std::slice::from_raw_parts(item as *const T as *const u8, size_of::<T>()) };
        f(item);
        let after_bytes =
            unsafe { std::slice::from_raw_parts(item as *const T as *const u8, size_of::<T>()) };

        changed |= before_bytes != after_bytes;
    }

    changed
}

/// Identifies the type of paint primitives in `bytes`, and applies a theme to them.
///
/// Returns `None` if the bytes don't contain a primitive list, otherwise `Some(changed)`
/// where `changed` says whether any byte was modified and the upload therefore needs to
/// be flushed.
pub fn apply_theme(bytes: &mut [u8], theme: &mut impl Theme) -> Option<bool> {
    Some(match PrimitiveKind::classify_bytes(bytes)? {
        PrimitiveKind::Quad => visit_primitives::<Quad>(bytes, |mut p| theme.quad(&mut p)),
        PrimitiveKind::Shadow => visit_primitives::<Shadow>(bytes, |mut p| theme.shadow(&mut p)),
        PrimitiveKind::Underline => {
            visit_primitives::<Underline>(bytes, |mut p| theme.underline(&mut p))
        }
        PrimitiveKind::MonochromeSprite => {
            visit_primitives::<MonochromeSprite>(bytes, |mut p| theme.monochrome_sprite(&mut p))
        }
        PrimitiveKind::SubpixelSprite => {
            visit_primitives::<SubpixelSprite>(bytes, |mut p| theme.subpixel_sprite(&mut p))
        }
        PrimitiveKind::PolychromeSprite => {
            visit_primitives::<PolychromeSprite>(bytes, |mut p| theme.polychrome_sprite(&mut p))
        }
    })
}

/// returns true if every value is a finite non-subnormal float within lo..=hi
fn all_in_range(values: &[f32], lo: f32, hi: f32) -> bool {
    values
        .iter()
        .all(|&v| (v == 0.0 || v.is_normal()) && (lo..=hi).contains(&v))
}
