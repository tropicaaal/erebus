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

#![no_std]

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
        Self::from_rgba_f32(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }

    /// Converts straight sRGB components in `0.0..=1.0` to HSLA.
    pub const fn from_rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContentMask {
    pub bounds: Bounds,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
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
    pub pad: u32,
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
}

impl Default for TransformationMatrix {
    fn default() -> Self {
        Self::unit()
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

pub trait Primitive: Copy + PartialEq {}

impl Primitive for Quad {}
impl Primitive for Shadow {}
impl Primitive for Underline {}
impl Primitive for MonochromeSprite {}
impl Primitive for SubpixelSprite {}
impl Primitive for PolychromeSprite {}
