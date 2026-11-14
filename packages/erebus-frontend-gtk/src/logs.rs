//! everything the app does with logs over the connection, including the logs window

use std::cell::Cell;

use adw::prelude::*;
use erebus_ipc::protocol::LogRecord;
use gtk::glib;

const ALL_LEVELS: [log::Level; 5] = [
    log::Level::Error,
    log::Level::Warn,
    log::Level::Info,
    log::Level::Debug,
    log::Level::Trace,
];

fn label(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "Errors",
        log::Level::Warn => "Warnings",
        log::Level::Info => "Info",
        log::Level::Debug => "Debug",
        log::Level::Trace => "Trace",
    }
}

/// Each line is tagged with level name so that hiding a tag filters levels out.
pub fn new_buffer() -> gtk::TextBuffer {
    let buffer = gtk::TextBuffer::new(None);
    for level in ALL_LEVELS {
        buffer.create_tag(Some(level.as_str()), &[]);
    }

    let tags = buffer.tag_table();
    let colors = [
        (log::Level::Error, "#e01b24"),
        (log::Level::Warn, "#c64600"),
    ];
    for (level, color) in colors {
        if let Some(tag) = tags.lookup(level.as_str()) {
            tag.set_foreground(Some(color));
        }
    }

    // created last so it takes priority over the level colors
    buffer.create_tag(Some("dim"), &[("foreground", &"#77767b")]);
    buffer
}

pub fn append(buffer: &gtk::TextBuffer, pid: u32, record: &LogRecord) {
    let time = glib::DateTime::from_unix_local((record.timestamp / 1000) as i64)
        .and_then(|time| time.format("%H:%M:%S"))
        .map_or_else(|_| String::new(), |time| time.to_string());
    let millis = record.timestamp % 1000;

    let level = record.level.as_str();
    let start = buffer.end_iter().offset();

    let mut end = buffer.end_iter();
    buffer.insert_with_tags_by_name(&mut end, &format!("{time}.{millis:03} [{pid}] "), &["dim"]);
    buffer.insert(&mut end, &format!("{level:<5} "));
    buffer.insert_with_tags_by_name(&mut end, &format!("{}: ", record.target), &["dim"]);
    buffer.insert(&mut end, &format!("{}\n", record.message));

    buffer.apply_tag_by_name(level, &buffer.iter_at_offset(start), &buffer.end_iter());
}

fn max_level(buffer: &gtk::TextBuffer) -> log::Level {
    let tags = buffer.tag_table();
    ALL_LEVELS
        .into_iter()
        .rev()
        .find(|level| {
            tags.lookup(level.as_str())
                .is_some_and(|tag| !tag.is_invisible())
        })
        .unwrap_or(log::Level::Error)
}

fn set_max_level(buffer: &gtk::TextBuffer, max: log::Level) {
    let tags = buffer.tag_table();
    for level in ALL_LEVELS {
        if let Some(tag) = tags.lookup(level.as_str()) {
            tag.set_invisible(level > max);
        }
    }
}

/// shows `buffer` in a new window that follows new records as they come in.
pub fn present(parent: &impl IsA<gtk::Window>, process: &str, buffer: &gtk::TextBuffer) {
    let view = gtk::TextView::builder()
        .buffer(buffer)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(12)
        .bottom_margin(12)
        .left_margin(12)
        .right_margin(12)
        .build();

    let filter = gtk::DropDown::builder()
        .model(&gtk::StringList::new(&ALL_LEVELS.map(label)))
        .tooltip_text("Most Verbose Level Shown")
        .build();
    let selected = ALL_LEVELS.iter().position(|level| *level == max_level(buffer));
    filter.set_selected(selected.unwrap_or(ALL_LEVELS.len() - 1) as u32);
    filter.connect_selected_notify({
        let buffer = buffer.clone();
        move |filter| {
            if let Some(level) = ALL_LEVELS.get(filter.selected() as usize) {
                set_max_level(&buffer, *level);
            }
        }
    });

    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new(process, "Logs"))
        .build();
    header.pack_start(&filter);

    let clear = gtk::Button::builder()
        .icon_name("edit-clear-all-symbolic")
        .css_classes(["destructive-action"])
        .tooltip_text("Clear Logs")
        .build();
    clear.connect_clicked({
        let buffer = buffer.clone();
        move |_| buffer.set_text("")
    });
    header.pack_end(&clear);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(
        &gtk::ScrolledWindow::builder()
            .child(&view)
            .vexpand(true)
            .build(),
    ));

    let window = adw::Window::builder()
        .transient_for(parent)
        .title(format!("{process} logs"))
        .default_width(760)
        .default_height(480)
        .content(&toolbar)
        .build();

    let end = buffer.create_mark(None, &buffer.end_iter(), false);
    view.scroll_to_mark(&end, 0.0, false, 0.0, 1.0);

    let follow = Cell::new(Some(buffer.connect_changed({
        let (view, end) = (view.clone(), end.clone());
        move |_| view.scroll_to_mark(&end, 0.0, false, 0.0, 1.0)
    })));

    window.connect_close_request({
        let buffer = buffer.clone();
        move |_| {
            if let Some(follow) = follow.take() {
                buffer.disconnect(follow);
            }
            buffer.delete_mark(&end);
            glib::Propagation::Proceed
        }
    });

    window.present();
}
