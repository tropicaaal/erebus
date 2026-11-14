use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use erebus_engine::ThemeModule;
use erebus_ipc::config::{self, AppTheme, ConfigStore, ThemeEntry};
use erebus_theme::OwnedManifest;

#[derive(Default)]
pub struct Library {
    pub store: ConfigStore,
    /// keyed by theme id. `Err` holds why the binary couldn't be read
    pub manifests: HashMap<String, Result<OwnedManifest, String>>,
}

impl Library {
    pub fn load() -> io::Result<Self> {
        let store = ConfigStore::load()?;
        let manifests = store
            .themes
            .iter()
            .map(|entry| (entry.id.clone(), load_manifest(entry)))
            .collect();

        Ok(Self { store, manifests })
    }

    fn position(&self, id: &str) -> Option<usize> {
        self.store.themes.iter().position(|entry| entry.id == id)
    }

    //
    // MARK: apps
    //

    /// Adds `process` to the known apps. Returns whether it wasn't known before.
    pub fn register_app(&mut self, process: &str) -> bool {
        if self.store.apps.contains_key(process) {
            return false;
        }

        self.store
            .apps
            .insert(process.to_owned(), Default::default());
        true
    }

    pub fn app_theme(&self, process: &str, id: &str) -> Option<&AppTheme> {
        self.store.apps.get(process)?.themes.get(id)
    }

    pub fn app_theme_mut(&mut self, process: &str, id: &str) -> &mut AppTheme {
        let app = self.store.apps.entry(process.to_owned()).or_default();
        app.themes.entry(id.to_owned()).or_default()
    }

    //
    // MARK: themes
    //

    /// Copies the theme at `source` into the themes directory. A theme with the same file
    /// name will replace the existing one but keep its settings.
    pub fn install(&mut self, source: &Path) -> erebus_engine::Result<()> {
        let source = source.canonicalize()?;
        let wasm = fs::read(&source)?;
        let manifest = read_manifest(&wasm)?;

        let id = source
            .file_stem()
            .ok_or_else(|| io::Error::other("theme path has no file name"))?
            .to_string_lossy()
            .into_owned();

        let index = self.position(&id).unwrap_or_else(|| {
            self.store.themes.push(ThemeEntry {
                id: id.clone(),
                source: source.clone(),
            });
            self.store.themes.len() - 1
        });

        let entry = &mut self.store.themes[index];
        entry.source = source;
        config::write_atomic(&entry.binary_path()?, &wasm)?;
        self.manifests.insert(id, Ok(manifest));

        Ok(())
    }

    pub fn move_theme(&mut self, id: &str, target: &str) -> bool {
        let (Some(from), Some(to)) = (self.position(id), self.position(target)) else {
            return false;
        };
        if from == to {
            return false;
        }

        let entry = self.store.themes.remove(from);
        self.store.themes.insert(to, entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> io::Result<()> {
        let Some(index) = self.position(id) else {
            return Ok(());
        };

        let entry = self.store.themes.remove(index);
        self.manifests.remove(id);
        for app in self.store.apps.values_mut() {
            app.themes.remove(id);
        }

        match fs::remove_file(entry.binary_path()?) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    pub fn outdated_sources(&self, changed: &[PathBuf]) -> Vec<PathBuf> {
        self.store
            .themes
            .iter()
            .filter(|entry| changed.contains(&entry.source) && is_outdated(entry))
            .map(|entry| entry.source.clone())
            .collect()
    }
}

fn read_manifest(wasm: &[u8]) -> erebus_engine::Result<OwnedManifest> {
    ThemeModule::from_bytes(wasm)?.manifest()
}

fn load_manifest(entry: &ThemeEntry) -> Result<OwnedManifest, String> {
    entry
        .binary_path()
        .and_then(fs::read)
        .map_err(Into::into)
        .and_then(|wasm| read_manifest(&wasm))
        .map_err(|err| err.to_string())
}

/// Whether the theme's source was modified after it was installed.
fn is_outdated(entry: &ThemeEntry) -> bool {
    let modified = |path: &Path| fs::metadata(path).and_then(|meta| meta.modified()).ok();
    let installed = entry.binary_path().ok().and_then(|path| modified(&path));

    match (modified(&entry.source), installed) {
        (Some(source), Some(installed)) => source > installed,
        (source, None) => source.is_some(),
        (None, _) => false,
    }
}
