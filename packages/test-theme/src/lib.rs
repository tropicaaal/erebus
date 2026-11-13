use erebus_theme::{Background, Hsla, Quad, Theme, Version};

struct TestTheme;

impl Theme for TestTheme {
    const NAME: &str = "example";
    const AUTHOR: &str = "tropical";
    const DESCRIPTION: &str = "meow";
    const VERSION: Version = Version::new(1, 0, 0);

    fn init(_settings: ()) -> Self {
        TestTheme {}   
    }

    fn quad(&mut self, quad: &mut Quad) {
        quad.background = Background::solid(Hsla::from_rgb(0, 0, 0));
    }
}

erebus_theme::export_theme!(TestTheme);
