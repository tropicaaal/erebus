//! Theme library page

use std::{cell::Cell, rc::Rc};

use adw::prelude::*;
use erebus_ipc::config::ThemeEntry;
use gtk::{gdk, glib};

use super::ThemeInfo;
use crate::ui::{
    app::App,
    format,
    widgets::{menu, page, preferences},
};

struct LibraryRow {
    row: gtk::Widget,
    /// hidden while searching since reordering is off then
    handle: gtk::Widget,
    /// lowercase text searches match against
    haystack: String,
}

impl App {
    pub fn library_page(self: &Rc<Self>) -> adw::ToolbarView {
        let header = self.library_header();

        let rows: Vec<LibraryRow> = {
            let library = self.library.borrow();
            library
                .store
                .themes
                .iter()
                .map(|entry| {
                    self.library_row(
                        entry,
                        &ThemeInfo::new(entry, library.manifests.get(&entry.id)),
                    )
                })
                .collect()
        };

        let toolbar = if rows.is_empty() {
            page(&header, &self.empty_library())
        } else {
            self.searchable_library(&header, rows)
        };

        self.accept_theme_files(&toolbar);
        toolbar
    }

    fn library_header(self: &Rc<Self>) -> adw::HeaderBar {
        let header = adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new(
                "Theme Library",
                &format::theme_count(self.library.borrow().store.themes.len()),
            ))
            .build();

        let (open, reload) = (self.clone(), self.clone());
        header.pack_end(&menu(vec![
            (
                "Open Config Folder",
                Box::new(move || open.open_config_folder()),
            ),
            ("Reload Library", Box::new(move || reload.reload_library())),
        ]));

        header
    }

    fn add_theme_button(self: &Rc<Self>) -> gtk::Button {
        let button = gtk::Button::builder()
            .child(
                &adw::ButtonContent::builder()
                    .icon_name("list-add-symbolic")
                    .label("Add Theme")
                    .build(),
            )
            .css_classes(["suggested-action"])
            .build();

        button.connect_clicked({
            let app = self.clone();
            move |_| app.pick_theme()
        });

        button
    }

    fn empty_library(self: &Rc<Self>) -> adw::StatusPage {
        let add = self.add_theme_button();
        add.set_halign(gtk::Align::Center);
        add.add_css_class("pill");

        adw::StatusPage::builder()
            .icon_name("view-grid-symbolic")
            .title("No Themes")
            .description(
                "Add or drop a compiled theme file (.wasm) here, then enable it for specific apps.",
            )
            .child(&add)
            .build()
    }

    /// The list of themes, with a search bar that filters `rows`.
    fn searchable_library(
        self: &Rc<Self>,
        header: &adw::HeaderBar,
        rows: Vec<LibraryRow>,
    ) -> adw::ToolbarView {
        let add = self.add_theme_button();
        add.set_valign(gtk::Align::Center);
        add.set_margin_start(12);

        let group = adw::PreferencesGroup::builder()
            .title("Themes")
            .description("Add or drop theme files (.wasm) here. Drag them to change the order they're applied in.")
            .header_suffix(&add)
            .build();
        for row in &rows {
            group.add(&row.row);
        }

        let no_results = adw::StatusPage::builder()
            .icon_name("system-search-symbolic")
            .title("No Results Found")
            .description("Try a different search")
            .build();

        let results = gtk::Stack::new();
        results.add_named(&preferences(&group), Some("themes"));
        results.add_named(&no_results, Some("none"));

        let filter = {
            let results = results.clone();
            move |query: &str| {
                let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();

                let mut any = false;
                for row in &rows {
                    let visible = terms.iter().all(|term| row.haystack.contains(term));
                    row.row.set_visible(visible);
                    // positions in a filtered list are ambiguous, so only reorder unfiltered
                    row.handle.set_visible(terms.is_empty());
                    any |= visible;
                }

                results.set_visible_child_name(if any { "themes" } else { "none" });
            }
        };

        let entry = gtk::SearchEntry::builder()
            .placeholder_text("Search themes")
            .hexpand(true)
            .build();
        let search = gtk::SearchBar::builder()
            .child(
                &adw::Clamp::builder()
                    .maximum_size(400)
                    .child(&entry)
                    .build(),
            )
            .build();
        search.connect_entry(&entry);

        let toggle = gtk::ToggleButton::builder()
            .icon_name("system-search-symbolic")
            .tooltip_text("Search")
            .build();
        toggle
            .bind_property("active", &search, "search-mode-enabled")
            .bidirectional()
            .build();
        header.pack_start(&toggle);

        let query = self.library_search.borrow().clone();
        if !query.is_empty() {
            entry.set_text(&query);
            toggle.set_active(true);
            filter(&query);
        }

        entry.connect_search_changed({
            let app = self.clone();
            move |entry| {
                let query = entry.text().to_string();
                filter(&query);
                *app.library_search.borrow_mut() = query;
            }
        });

        let toolbar = page(header, &results);
        toolbar.add_top_bar(&search);
        search.set_key_capture_widget(Some(&toolbar));
        toolbar
    }

    /// Installs theme files dropped onto `widget`.
    fn accept_theme_files(self: &Rc<Self>, widget: &impl IsA<gtk::Widget>) {
        let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        drop.connect_drop({
            let app = self.clone();
            move |_, value, _, _| {
                let Ok(files) = value.get::<gdk::FileList>() else {
                    return false;
                };

                let paths: Vec<_> = files
                    .files()
                    .iter()
                    .filter_map(|file| file.path())
                    .collect();

                // after the drop finishes, since installing rebuilds the page this is on
                let app = app.clone();
                glib::idle_add_local_once(move || app.install_themes(&paths));
                true
            }
        });

        widget.add_controller(drop);
    }

    /// we need a custom ActionRow recreation here to properly show description text
    fn library_row(self: &Rc<Self>, entry: &ThemeEntry, info: &ThemeInfo) -> LibraryRow {
        let label = |text: &str, classes: &[&str]| {
            gtk::Label::builder()
                .label(text)
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .css_classes(
                    classes
                        .iter()
                        .map(|class| class.to_string())
                        .collect::<Vec<_>>(),
                )
                .build()
        };

        let text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["title"])
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();
        text.append(&label(&info.title, &["title"]));
        text.append(&label(&info.subtitle, &["subtitle", "dim-label"]));

        if !info.description.is_empty() {
            let description = label(info.description, &["dim-label"]);
            description.set_margin_top(3);
            text.append(&description);
        }

        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove from Library")
            .valign(gtk::Align::Center)
            .css_classes(["flat", "destructive-action"])
            .build();
        remove.connect_clicked({
            let (app, id, title) = (self.clone(), entry.id.clone(), info.title.clone());
            move |_| app.confirm_remove_theme(&id, &title)
        });

        let handle = gtk::Image::builder()
            .icon_name("list-drag-handle-symbolic")
            .tooltip_text("Drag to Reorder")
            .css_classes(["dim-label"])
            .build();

        let header = gtk::Box::builder().css_classes(["header"]).build();
        header.append(&handle);
        header.append(&text);
        header.append(&remove);

        let row = adw::PreferencesRow::builder()
            .title(&info.title)
            .child(&header)
            .activatable(false)
            .tooltip_text(format!("Installed from {}", entry.source.display()))
            .build();

        self.make_reorderable(&row, &entry.id);

        LibraryRow {
            haystack: format!(
                "{} {} {} {}",
                info.title, info.subtitle, info.description, entry.id
            )
            .to_lowercase(),
            row: row.upcast(),
            handle: handle.upcast(),
        }
    }

    /// allows `row` to be dragged onto another row to take its order
    fn make_reorderable(self: &Rc<Self>, row: &adw::PreferencesRow, id: &str) {
        // where the row was grabbed, so the drag icon stays under the pointer at that spot
        let grabbed = Rc::new(Cell::new((0, 0)));

        let drag = gtk::DragSource::new();
        drag.set_actions(gdk::DragAction::MOVE);
        drag.connect_prepare({
            let (app, id, grabbed) = (self.clone(), id.to_owned(), grabbed.clone());
            move |_, x, y| {
                if !app.library_search.borrow().trim().is_empty() {
                    return None;
                }

                grabbed.set((x as i32, y as i32));
                Some(gdk::ContentProvider::for_value(&id.to_value()))
            }
        });
        drag.connect_drag_begin({
            let row = row.clone();
            move |_, drag| {
                // the card background belongs to the list rather than the row, so a bare
                // snapshot of the row would be text floating on nothing
                let card = gtk::Picture::builder()
                    .paintable(&gtk::WidgetPaintable::new(Some(&row)))
                    .css_classes(["card"])
                    .width_request(row.width())
                    .height_request(row.height())
                    .build();

                gtk::DragIcon::for_drag(drag).set_child(Some(&card));
                let (x, y) = grabbed.get();
                drag.set_hotspot(x, y);
            }
        });
        row.add_controller(drag);

        let drop = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        drop.connect_drop({
            let (app, target) = (self.clone(), id.to_owned());
            move |_, value, _, _| {
                let Ok(id) = value.get::<String>() else {
                    return false;
                };

                // after the drop finishes, since moving rebuilds the page this row is in
                let (app, target) = (app.clone(), target.clone());
                glib::idle_add_local_once(move || app.move_theme(&id, &target));
                true
            }
        });
        row.add_controller(drop);
    }
}
