#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

pub mod layer;
pub mod scene;
pub mod theme;

use scene::{Background, Hsla, Quad};
use theme::Theme;

// example theme that turns every quad black
struct ExampleTheme;

impl Theme for ExampleTheme {
    fn quad(&mut self, quad: &mut Quad) {
        quad.background = Background::solid(Hsla::from_rgba(0, 0, 0, 255));
    }
}
