use erebus_theme::{
    Theme, Version,
    primitives::{Background, Hsla, Quad},
};

struct Example;

impl Theme for Example {
    const NAME: &str = "example";
    const AUTHOR: &str = "tropical";
    const DESCRIPTION: &str = "meow";
    const VERSION: Version = Version::new(1, 0, 0);

    fn init(_settings: ()) -> Self {
        Example {}
    }

    fn quad(&mut self, quad: &mut Quad) {
        quad.background = Background::solid(Hsla::from_rgb(0, 0, 0));
    }
}

erebus_theme::export_theme!(Example);
