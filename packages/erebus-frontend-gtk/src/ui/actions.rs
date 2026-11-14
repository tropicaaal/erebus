//! Changes to the config store made from the UI.

use std::{path::PathBuf, rc::Rc};

use adw::prelude::*;
use erebus_theme::settings::SettingValue;
use gtk::gio;

use super::app::{App, Selection};
use crate::{library::Library, watcher};

impl App {
    //
    // MARK: saving
    //

    pub fn save(&self) -> bool {
        match self.library.borrow().store.save() {
            Ok(()) => true,
            Err(err) => {
                self.toast(&format!("Couldn't save: {err}"));
                false
            }
        }
    }

    /// Saves the store and tells the backends of `process`, or every backend if `None`,
    /// to pick it up.
    fn commit(&self, process: Option<&str>) {
        if self.save()
            && let Some(ipc) = &self.ipc
        {
            ipc.sync(process);
        }
    }

    /// Call after anything in the library itself changes.
    fn library_changed(self: &Rc<Self>) {
        self.commit(None);
        self.watch();
        self.rebuild_sidebar();
        self.show_page();
    }

    //
    // MARK: app themes
    //

    pub fn set_enabled(&self, process: &str, id: &str, enabled: bool) {
        self.library.borrow_mut().app_theme_mut(process, id).enabled = enabled;
        self.commit(Some(process));
    }

    pub fn set_setting(&self, process: &str, id: &str, key: &str, value: SettingValue) {
        self.library
            .borrow_mut()
            .app_theme_mut(process, id)
            .settings
            .insert(key.to_owned(), value);
        self.commit(Some(process));
    }

    /// Drops everything stored for an app that isn't running.
    pub fn forget_app(self: &Rc<Self>, process: &str) {
        self.library.borrow_mut().store.apps.remove(process);
        self.runtime.borrow_mut().forget(process);
        self.save();

        *self.selection.borrow_mut() = Selection::Library;
        self.rebuild_sidebar();
        self.show_page();
    }

    //
    // MARK: library
    //

    pub fn pick_theme(self: &Rc<Self>) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("WebAssembly files"));
        filter.add_suffix("wasm");

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);

        let dialog = gtk::FileDialog::builder()
            .title("Add Theme")
            .filters(&filters)
            .default_filter(&filter)
            .build();

        let app = self.clone();
        dialog.open(Some(&self.window), gio::Cancellable::NONE, move |file| {
            // an error here means the dialog was dismissed
            let Some(path) = file.ok().and_then(|file| file.path()) else {
                return;
            };

            app.install_themes(&[path]);
        });
    }

    /// Adds theme binaries to the library, skipping anything that isn't a `.wasm` file.
    pub fn install_themes(self: &Rc<Self>, paths: &[PathBuf]) {
        let mut installed = false;

        for path in paths {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if path.extension().is_none_or(|extension| extension != "wasm") {
                self.toast(&format!("Skipped {name}, which isn't a .wasm theme"));
                continue;
            }

            let result = self.library.borrow_mut().install(path);
            match result {
                Ok(()) => installed = true,
                Err(err) => self.toast(&format!("Couldn't add {name}: {err}")),
            }
        }

        if installed {
            self.library_changed();
        }
    }

    /// Moves a theme to the position of `target` in the library.
    pub fn move_theme(self: &Rc<Self>, id: &str, target: &str) {
        let moved = self.library.borrow_mut().move_theme(id, target);
        if moved {
            self.commit(None);
            self.show_page();
        }
    }

    pub fn confirm_remove_theme(self: &Rc<Self>, id: &str, name: &str) {
        let dialog = adw::AlertDialog::new(
            Some("Remove Theme?"),
            Some(&format!(
                "“{name}” will be removed from the library, along with its settings in every app."
            )),
        );
        dialog.add_responses(&[("cancel", "Cancel"), ("remove", "Remove")]);
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");

        dialog.connect_response(Some("remove"), {
            let (app, id) = (self.clone(), id.to_owned());
            move |_, _| app.remove_theme(&id)
        });

        dialog.present(Some(&self.window));
    }

    fn remove_theme(self: &Rc<Self>, id: &str) {
        let removed = self.library.borrow_mut().remove(id);
        if let Err(err) = removed {
            self.toast(&format!("Couldn't remove theme binary: {err}"));
        }

        self.library_changed();
    }

    //
    // MARK: files on disk
    //

    pub fn open_config_folder(self: &Rc<Self>) {
        let dir = match erebus_ipc::config::config_dir() {
            Ok(dir) => dir,
            Err(err) => {
                self.toast(&format!("Couldn't find the config folder: {err}"));
                return;
            }
        };

        // it only exists once something has been saved
        if let Err(err) = std::fs::create_dir_all(&dir) {
            self.toast(&format!("Couldn't create {}: {err}", dir.display()));
            return;
        }

        let app = self.clone();
        gtk::FileLauncher::new(Some(&gio::File::for_path(&dir))).launch(
            Some(&self.window),
            gio::Cancellable::NONE,
            move |launched| {
                if let Err(err) = launched {
                    app.toast(&format!("Couldn't open {}: {err}", dir.display()));
                }
            },
        );
    }

    /// Re-reads the library from disk, e.g. after the config file was edited by hand, and
    /// has every backend do the same.
    pub fn reload_library(self: &Rc<Self>) {
        match Library::load() {
            Ok(library) => *self.library.borrow_mut() = library,
            Err(err) => {
                self.toast(&format!("Couldn't reload the theme library: {err}"));
                return;
            }
        }

        if let Some(ipc) = &self.ipc {
            ipc.sync(None);
        }

        self.watch();
        self.rebuild_sidebar();
        self.show_page();
        self.toast("Reloaded the theme library");
    }

    /// Reinstalls every theme whose source is one of `paths` and has changed.
    pub fn reload_sources(self: &Rc<Self>, paths: &[PathBuf]) {
        let sources = self.library.borrow().outdated_sources(paths);
        if sources.is_empty() {
            return;
        }

        for source in &sources {
            let installed = self.library.borrow_mut().install(source);
            match installed {
                Ok(()) => self.toast(&format!("Reloaded {}", source.display())),
                Err(err) => self.toast(&format!("Couldn't reload {}: {err}", source.display())),
            }
        }

        self.library_changed();
    }

    /// Replaces the watcher with one for the library's current theme sources.
    pub fn watch(&self) {
        let watcher = watcher::watch_sources(
            &self.library.borrow().store.themes,
            self.on_sources_changed.clone(),
        );

        match watcher {
            Ok(watcher) => *self.watcher.borrow_mut() = Some(watcher),
            Err(err) => self.toast(&format!("Couldn't watch themes for changes: {err}")),
        }
    }
}
