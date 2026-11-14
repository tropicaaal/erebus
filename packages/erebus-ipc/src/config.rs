//! Persistent data stored in the OS config directory.
//!
//! The frontend is the only one writing to this directory. When that happens, it notifies
//! backends to re-evaluate with [`FrontendMessage::Sync`](crate::FrontendMessage::Sync).
//!
//! ```text
//! <config_dir>/erebus/
//! - config.toml
//! - themes/
//!     - <id>.wasm
//! ```

use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
};

use erebus_theme::settings::SettingValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigStore {
    /// the theme library. Themes an app enables are applied in this order
    #[serde(default)]
    pub themes: Vec<ThemeEntry>,
    /// keyed by the app's executable name
    #[serde(default)]
    pub apps: BTreeMap<String, AppConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeEntry {
    /// file stem of the theme's binary in [`themes_dir`]
    pub id: String,
    /// the file the frontend installed the binary from, and watches for changes
    pub source: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// keyed by theme id. Themes missing here are disabled
    #[serde(default)]
    pub themes: BTreeMap<String, AppTheme>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppTheme {
    pub enabled: bool,
    #[serde(default)]
    pub settings: HashMap<String, SettingValue>,
}

impl ThemeEntry {
    pub fn binary_path(&self) -> io::Result<PathBuf> {
        Ok(themes_dir()?.join(&self.id).with_extension("wasm"))
    }
}

pub fn config_dir() -> io::Result<PathBuf> {
    dirs::config_dir()
        .map(|dir| dir.join("erebus"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config directory"))
}

pub fn themes_dir() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("themes"))
}

fn state_path() -> io::Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

impl ConfigStore {
    /// Reads the state from disk. A missing file is an empty state.
    pub fn load() -> io::Result<Self> {
        match fs::read(state_path()?) {
            Ok(bytes) => toml::from_slice(&bytes)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    pub fn save(&self) -> io::Result<()> {
        write_atomic(
            &state_path()?,
            toml::to_string_pretty(self)
                .map_err(io::Error::other)?
                .as_bytes(),
        )
    }
}

/// Writes `bytes` to `path` such that readers see either the old or new contents and never
/// a partial write.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}
