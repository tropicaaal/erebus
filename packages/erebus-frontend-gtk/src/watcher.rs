//! File watchers for themes.
//! 
//! Each theme has a source directory associated with it containing the original path
//! it was installed from. While the frontend is running, we watch these paths for changes
//! for development purposes.

use std::{collections::HashSet, path::PathBuf, time::Duration};

use erebus_ipc::config::ThemeEntry;
use notify_debouncer_mini::{
    DebounceEventResult, Debouncer, new_debouncer,
    notify::{self, RecommendedWatcher, RecursiveMode},
};

/// how long a theme binary has to stop changing before it's reloaded
const DEBOUNCE: Duration = Duration::from_millis(250);

pub type Watcher = Debouncer<RecommendedWatcher>;

pub fn watch_sources(
    themes: &[ThemeEntry],
    on_change: async_channel::Sender<Vec<PathBuf>>,
) -> notify::Result<Watcher> {
    let mut debouncer = new_debouncer(DEBOUNCE, move |events: DebounceEventResult| {
        if let Ok(events) = events {
            let paths = events.into_iter().map(|event| event.path).collect();
            let _ = on_change.send_blocking(paths);
        }
    })?;

    let dirs: HashSet<_> = themes
        .iter()
        .filter_map(|entry| entry.source.parent())
        .collect();

    for dir in dirs {
        if let Err(err) = debouncer.watcher().watch(dir, RecursiveMode::NonRecursive) {
            eprintln!("couldn't watch {}: {err}", dir.display());
        }
    }

    Ok(debouncer)
}
