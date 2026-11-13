#[doc(hidden)]
pub mod abi;

pub mod primitives {
    pub use erebus_primitives::*;
}

use erebus_primitives::{
    MonochromeSprite, PolychromeSprite, Quad, Shadow, SubpixelSprite, Underline,
};
pub use semver::Version;

pub trait Theme {
    const NAME: &str;
    const AUTHOR: &str;
    const DESCRIPTION: &str = "";
    const VERSION: Version;

    fn init(settings_placeholder: ()) -> Self;

    fn quad(&mut self, _quad: &mut Quad) {}
    fn shadow(&mut self, _shadow: &mut Shadow) {}
    fn underline(&mut self, _underline: &mut Underline) {}
    fn monochrome_sprite(&mut self, _sprite: &mut MonochromeSprite) {}
    fn subpixel_sprite(&mut self, _sprite: &mut SubpixelSprite) {}
    fn polychrome_sprite(&mut self, _sprite: &mut PolychromeSprite) {}
}

#[macro_export]
macro_rules! export_theme {
    (@primitives $theme:ty { $($export:ident => $method:ident($primitive:ident)),* $(,)? }) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "C" fn $export(ptr: u32, len: u32) -> u32 {
                let mut theme = <$theme as ::erebus_theme::Theme>::init(());

                // SAFETY: the host is calling us with the block it just filled
                unsafe {
                    $crate::abi::apply::<$crate::primitives::$primitive>(ptr, len, |item| {
                        $crate::Theme::$method(&mut theme, item)
                    }) as _
                }
            }
        )*
    };

    ($theme:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn alloc(len: u32) -> u32 {
            $crate::abi::alloc(len)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn dealloc(ptr: u32, len: u32) {
            unsafe { $crate::abi::dealloc(ptr, len) }
        }

        $crate::export_theme!(@primitives $theme {
            theme_quad => quad(Quad),
            theme_shadow => shadow(Shadow),
            theme_underline => underline(Underline),
            theme_monochrome_sprite => monochrome_sprite(MonochromeSprite),
            theme_polychrome_sprite => polychrome_sprite(PolychromeSprite),
            theme_subpixel_sprite => subpixel_sprite(SubpixelSprite),
        });
    };
}
