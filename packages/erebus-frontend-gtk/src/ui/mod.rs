//! The main window: a sidebar with the theme library and every known app, and a page for
//! whichever one is selected.

mod actions;
mod app;
mod events;
mod format;
mod pages;
mod sidebar;
mod widgets;
mod window;

use std::rc::Rc;

use adw::prelude::*;
use erebus_ipc::frontend::IpcFrontend;
use gtk::glib;

use self::{app::App, window::Shell};
use crate::library::Library;

pub fn build(application: &adw::Application) {
    sidebar::install_css();

    let (events_tx, events_rx) = async_channel::unbounded();
    let (sources_tx, sources_rx) = async_channel::unbounded();

    let (library, library_err) = match Library::load() {
        Ok(library) => (library, None),
        Err(err) => (Library::default(), Some(err)),
    };

    let on_event = move |event| {
        let _ = events_tx.send_blocking(event);
    };
    let (ipc, ipc_err) = match IpcFrontend::start(on_event) {
        Ok(ipc) => {
            if let Ok(name) = erebus_ipc::transport::socket_name() {
                eprintln!("listening on {name:?}");
            }
            (Some(ipc), None)
        }
        Err(err) => (None, Some(err)),
    };

    let app = Rc::new(App::new(Shell::new(application), library, ipc, sources_tx));

    if let Some(err) = library_err {
        app.toast(&format!("Couldn't load the theme library: {err}"));
    }
    if let Some(err) = ipc_err {
        app.toast(&format!("Can't talk to apps: {err}"));
    }

    app.sidebar.connect_row_selected({
        let app = app.clone();
        move |_, row| {
            if let Some(row) = row {
                app.select(row.index());
            }
        }
    });
    app.sidebar.connect_row_activated({
        let app = app.clone();
        move |_, _| app.split.set_show_content(true)
    });

    glib::spawn_future_local({
        let app = app.clone();
        async move {
            while let Ok(event) = events_rx.recv().await {
                app.handle_event(event);
            }
        }
    });

    glib::spawn_future_local({
        let app = app.clone();
        async move {
            while let Ok(paths) = sources_rx.recv().await {
                app.reload_sources(&paths);
            }
        }
    });

    app.rebuild_sidebar();
    app.show_page();
    app.watch();
    app.window.present();
}
