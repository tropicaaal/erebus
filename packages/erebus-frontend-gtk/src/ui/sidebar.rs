//! Rows for the sidebar.

use adw::prelude::*;
use gtk::gdk;

use super::format;

pub fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".status-dot { min-width: 8px; min-height: 8px; border-radius: 4px; \
             background-color: alpha(currentColor, 0.25); } \
         .status-dot.running { background-color: @success_color; }",
    );

    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn library_row(themes: usize) -> gtk::ListBoxRow {
    let content = two_line_content(
        "view-grid-symbolic",
        "Theme Library",
        &format::theme_count(themes),
    );
    gtk::ListBoxRow::builder().child(&content).build()
}

pub fn apps_header() -> gtk::Label {
    gtk::Label::builder()
        .label("Apps")
        .xalign(0.0)
        .css_classes(["heading", "dim-label"])
        .margin_top(12)
        .margin_bottom(6)
        .margin_start(20)
        .build()
}

pub fn app_row(process: &str, instances: usize) -> gtk::ListBoxRow {
    let dot = gtk::Box::builder()
        .css_classes(["status-dot"])
        .valign(gtk::Align::Center)
        .build();
    if instances > 0 {
        dot.add_css_class("running");
    }

    let content = two_line_content(
        "application-x-executable-symbolic",
        process,
        &format::status(instances),
    );
    content.append(&dot);

    gtk::ListBoxRow::builder().child(&content).build()
}

pub fn no_apps_row() -> gtk::ListBoxRow {
    gtk::ListBoxRow::builder()
        .selectable(false)
        .activatable(false)
        .child(&{
            let content = row_content();
            content.append(
                &gtk::Label::builder()
                    .label("Apps show up here once they run with erebus")
                    .wrap(true)
                    .xalign(0.0)
                    .css_classes(["caption", "dim-label"])
                    .build(),
            );
            content
        })
        .build()
}

fn row_content() -> gtk::Box {
    gtk::Box::builder()
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(6)
        .margin_end(6)
        .build()
}

fn two_line_content(icon: &str, title: &str, subtitle: &str) -> gtk::Box {
    let labels = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();
    labels.append(
        &gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .build(),
    );
    labels.append(
        &gtk::Label::builder()
            .label(subtitle)
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build(),
    );

    let content = row_content();
    content.set_margin_top(6);
    content.set_margin_bottom(6);
    content.append(&gtk::Image::from_icon_name(icon));
    content.append(&labels);
    content
}
