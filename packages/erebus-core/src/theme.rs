
use crate::scene::{MonochromeSprite, PolychromeSprite, Quad, Shadow, SubpixelSprite, Underline};

pub trait Theme {
    fn quad(&mut self, _quad: &mut Quad) {}
    fn shadow(&mut self, _shadow: &mut Shadow) {}
    fn underline(&mut self, _underline: &mut Underline) {}
    fn monochrome_sprite(&mut self, _sprite: &mut MonochromeSprite) {}
    fn subpixel_sprite(&mut self, _sprite: &mut SubpixelSprite) {}
    fn polychrome_sprite(&mut self, _sprite: &mut PolychromeSprite) {}
}
