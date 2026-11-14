use std::rc::Rc;

use adw::prelude::*;
use erebus_ipc::config::ThemeEntry;

use super::{ThemeInfo, settings::setting_row};
use crate::{
    logs,
    runtime::BackendInstance,
    ui::{
        app::App,
        format,
        widgets::{MenuItem, menu, page},
    },
};

impl App {
    pub fn app_page(self: &Rc<Self>, process: &str) -> adw::ToolbarView {
        let instances = self.instances(process);
        let header = self.app_header(process, instances);

        let content = adw::PreferencesPage::new();
        if instances > 0 {
            content.add(&processes_group(self.runtime.borrow().instances(process)));
        }

        let library = self.library.borrow();
        let description = if library.store.themes.is_empty() {
            "Themes you add to your Theme Library will show up here."
        } else {
            "Themes are applied in the order of your library."
        };

        let themes = adw::PreferencesGroup::builder()
            .title("Themes")
            .description(description)
            .build();

        for entry in &library.store.themes {
            let info = ThemeInfo::new(entry, library.manifests.get(&entry.id));
            themes.add(&self.app_theme_row(process, entry, info));
        }

        content.add(&themes);
        page(&header, &content)
    }

    fn app_header(self: &Rc<Self>, process: &str, instances: usize) -> adw::HeaderBar {
        let header = adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new(process, &format::status(instances)))
            .build();

        let (app, name) = (self.clone(), process.to_owned());
        let mut items: Vec<MenuItem> = vec![(
            "View Logs",
            Box::new(move || {
                let buffer = app.runtime.borrow_mut().logs(&name);
                logs::present(&app.window, &name, &buffer);
            }),
        )];

        if instances == 0 {
            let (app, name) = (self.clone(), process.to_owned());
            items.push(("Forget App", Box::new(move || app.forget_app(&name))));
        }

        header.pack_end(&menu(items));
        header
    }

    /// initial settings for a given theme on one app
    fn app_theme_row(
        self: &Rc<Self>,
        process: &str,
        entry: &ThemeEntry,
        info: ThemeInfo,
    ) -> gtk::Widget {
        let (enabled, settings) = self
            .library
            .borrow()
            .app_theme(process, &entry.id)
            .map(|theme| (theme.enabled, theme.settings.clone()))
            .unwrap_or_default();

        let switch = gtk::Switch::builder()
            .active(enabled)
            .valign(gtk::Align::Center)
            .build();
        switch.connect_active_notify({
            let (app, process, id) = (self.clone(), process.to_owned(), entry.id.clone());
            move |switch| app.set_enabled(&process, &id, switch.is_active())
        });

        let tooltip = (!info.description.is_empty()).then_some(info.description);

        if info.settings.is_empty() {
            let row = adw::ActionRow::builder()
                .title(info.title)
                .subtitle(info.subtitle)
                .use_markup(false)
                .activatable_widget(&switch)
                .build();
            row.set_tooltip_text(tooltip);
            row.add_suffix(&switch);
            return row.upcast();
        }

        let key = format!("{process}/{}", entry.id);
        let row = adw::ExpanderRow::builder()
            .title(info.title)
            .subtitle(info.subtitle)
            .use_markup(false)
            .expanded(self.expanded.borrow().contains(&key))
            .build();
        row.set_tooltip_text(tooltip);
        row.add_suffix(&switch);

        row.connect_expanded_notify({
            let app = self.clone();
            move |row| {
                let mut expanded = app.expanded.borrow_mut();
                if row.is_expanded() {
                    expanded.insert(key.clone());
                } else {
                    expanded.remove(&key);
                }
            }
        });

        for field in info.settings {
            let set = {
                let (app, process, id, key) = (
                    self.clone(),
                    process.to_owned(),
                    entry.id.clone(),
                    field.key.clone(),
                );
                move |value| app.set_setting(&process, &id, &key, value)
            };
            row.add_row(&setting_row(field, settings.get(&field.key), set));
        }

        row.upcast()
    }
}

fn processes_group(instances: &[BackendInstance]) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Processes").build();

    for instance in instances {
        let mut subtitle = format!("{} (protocol v{})", instance.backend, instance.version);
        if let Some(time) = instance
            .connected_at
            .as_ref()
            .and_then(|time| time.format("%H:%M:%S").ok())
        {
            subtitle.push_str(&format!(" connected at {time}"));
        }

        let row = adw::ActionRow::builder()
            .title(format!("PID {}", instance.pid))
            .subtitle(subtitle)
            .use_markup(false)
            .build();
        row.add_prefix(&gtk::Image::from_icon_name(
            "application-x-executable-symbolic",
        ));
        group.add(&row);
    }

    group
}
