mod library;
mod logs;
mod runtime;
mod ui;
mod watcher;

use adw::prelude::*;

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("network.uwu.Erebus")
        .build();

    app.connect_activate(ui::build);
    app.run()
}
