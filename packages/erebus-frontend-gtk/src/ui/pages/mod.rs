mod app;
mod library;
mod settings;

use erebus_ipc::config::ThemeEntry;
use erebus_theme::{OwnedManifest, settings::OwnedSettingsField};

/// data shown in a theme ActionRow
struct ThemeInfo<'a> {
    title: String,
    subtitle: String,
    /// empty if there's none
    description: &'a str,
    settings: &'a [OwnedSettingsField],
}

impl<'a> ThemeInfo<'a> {
    fn new(entry: &ThemeEntry, manifest: Option<&'a Result<OwnedManifest, String>>) -> Self {
        let (title, subtitle) = match manifest {
            Some(Ok(manifest)) => (
                manifest.name.clone(),
                format!("v{} by {}", manifest.version, manifest.author),
            ),
            Some(Err(err)) => (entry.id.clone(), err.clone()),
            None => (entry.id.clone(), String::new()),
        };

        let (description, settings) = match manifest {
            Some(Ok(manifest)) => (manifest.description.as_str(), &manifest.settings[..]),
            _ => ("", &[][..]),
        };

        Self {
            title,
            subtitle,
            description,
            settings,
        }
    }
}
