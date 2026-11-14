//! small set of common widgets shared across pages

use adw::prelude::*;
use gtk::gio;

pub type MenuItem<'a> = (&'a str, Box<dyn Fn()>);

pub fn page(header: &adw::HeaderBar, content: &impl IsA<gtk::Widget>) -> adw::ToolbarView {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(header);
    toolbar.set_content(Some(content));
    toolbar
}

pub fn preferences(group: &adw::PreferencesGroup) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.add(group);
    page
}

pub fn menu(items: Vec<MenuItem>) -> gtk::MenuButton {
    let model = gio::Menu::new();
    let actions = gio::SimpleActionGroup::new();

    for (index, (label, activate)) in items.into_iter().enumerate() {
        let name = format!("item-{index}");
        let action = gio::SimpleAction::new(&name, None);
        action.connect_activate(move |_, _| activate());
        actions.add_action(&action);
        model.append(Some(label), Some(&format!("page.{name}")));
    }

    let button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Main Menu")
        .menu_model(&model)
        .primary(true)
        .build();
    button.insert_action_group("page", Some(&actions));
    button
}
