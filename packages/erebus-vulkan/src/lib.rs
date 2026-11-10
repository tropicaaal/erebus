#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_snake_case)]

pub mod layer;
pub mod scene;
pub mod theme;

use scene::{Background, Hsla, Quad};
use theme::Theme;

use crate::scene::SubpixelSprite;

// example theme that turns every quad black and all text magenta
struct ExampleTheme;

impl Theme for ExampleTheme {
    fn quad(&mut self, quad: &mut Quad) {
        quad.background = Background::solid(Hsla::from_rgba(0, 0, 0, 255));
        quad.border_color = Hsla::from_rgb(255, 0, 255);
    }

    fn monochrome_sprite(&mut self, sprite: &mut scene::MonochromeSprite) {
        sprite.color = Hsla::from_rgb(255, 0, 255);
    }

    fn subpixel_sprite(&mut self, sprite: &mut SubpixelSprite) {
        sprite.color = Hsla::from_rgb(255, 0, 255);
    }
}
