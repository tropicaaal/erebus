#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

pub mod layer;
pub mod scene;

use scene::{Background, Hsla, Quad};

fn theme(quad: &mut Quad) {
    if quad
        .background
        .solid_color()
        .is_some_and(|color| color.approx_eq(Hsla::from_rgba(40, 44, 51, 255)))
    {
        quad.background = Background::solid(Hsla::from_rgba(255, 0, 0, 255));
    }
}
