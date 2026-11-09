//! GPUI Scene Primitives

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

    fn is_valid(self) -> bool {
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
}

// must be kept in sync with <https://github.com/zed-industries/zed/blob/main/crates/gpui/src/scene.rs>
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

impl Quad {
    // this might be really overkill
    pub fn looks_valid(&self) -> bool {
        let (bounds, mask, bg) = (&self.bounds, &self.content_mask.bounds, &self.background);
        self.order < 1 << 24
            && self.border_style.0 <= BorderStyle::DASHED.0
            && all_in_range(&[bounds.origin.x, bounds.origin.y], -1e6, 1e6)
            && all_in_range(&[bounds.size.width, bounds.size.height], 0.0, 1e6)
            && all_in_range(
                &[
                    mask.origin.x,
                    mask.origin.y,
                    mask.size.width,
                    mask.size.height,
                ],
                -1e6,
                1e6,
            )
            && bg.tag.0 <= BackgroundTag::CHECKERBOARD.0
            && bg.color_space.0 <= ColorSpace::OKLAB.0
            && bg.solid.is_valid()
            && bg.pad == 0
            && self.border_color.is_valid()
            && all_in_range(
                &[
                    self.corner_radii.top_left,
                    self.corner_radii.top_right,
                    self.corner_radii.bottom_right,
                    self.corner_radii.bottom_left,
                    self.border_widths.top,
                    self.border_widths.right,
                    self.border_widths.bottom,
                    self.border_widths.left,
                ],
                0.0,
                1e6,
            )
    }
}

pub fn quads_from_bytes_mut(bytes: &mut [u8]) -> Option<&mut [Quad]> {
    // SAFETY: Quad contains only u32/f32 fields
    let (head, quads, tail) = unsafe { bytes.align_to_mut::<Quad>() };
    let fits = head.is_empty() && tail.is_empty() && !quads.is_empty();
    (fits && quads.iter().all(Quad::looks_valid)).then_some(quads)
}

/// returns true if every value is a finite non-subnormal float within lo..=hi
fn all_in_range(values: &[f32], lo: f32, hi: f32) -> bool {
    values
        .iter()
        .all(|&v| (v == 0.0 || v.is_normal()) && (lo..=hi).contains(&v))
}
