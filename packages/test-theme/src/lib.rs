use erebus_theme::{
    Manifest, Theme, Version,
    primitives::{Background, Hsla, Quad, Rgba},
    settings::{Settings, SettingsField},
};

struct Test {
    background: Hsla,
    recolor_borders: bool,
    border: Hsla,
}

impl Theme for Test {
    const MANIFEST: Manifest = Manifest {
        name: "test",
        author: "tropical",
        description: "meow",
        version: Version::new(1, 1, 0),
        settings: &[
            SettingsField::color("background", "Background", Rgba::from_rgb(0, 0, 0))
                .description("Color painted over quad backgrounds"),
            SettingsField::bool("recolor_borders", "Recolor borders", false),
            SettingsField::color("border", "Border color", Rgba::from_rgb(0xff, 0x4f, 0xa3)),
        ],
    };

    fn init(settings: &Settings) -> Self {
        Self {
            background: settings.color("background"),
            recolor_borders: settings.bool("recolor_borders"),
            border: settings.color("border"),
        }
    }

    fn quad(&mut self, quad: &mut Quad) {
        quad.background = Background::solid(self.background);
        if self.recolor_borders {
            quad.border_color = self.border;
        }
    }
}

erebus_theme::export_theme!(Test);
