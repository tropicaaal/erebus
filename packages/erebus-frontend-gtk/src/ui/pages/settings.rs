//! ActionRows and such for settings

use adw::prelude::*;
use erebus_theme::{
    primitives::Rgba,
    settings::{OwnedSettingKind, OwnedSettingsField, SettingValue},
};
use gtk::gdk;

/// each new value.
pub fn setting_row(
    field: &OwnedSettingsField,
    current: Option<&SettingValue>,
    set: impl Fn(SettingValue) + 'static,
) -> adw::ActionRow {
    let row: adw::ActionRow = match &field.kind {
        OwnedSettingKind::Bool { default } => {
            let row = adw::SwitchRow::new();
            row.set_active(match current {
                Some(SettingValue::Bool(value)) => *value,
                _ => *default,
            });
            row.connect_active_notify(move |row| set(SettingValue::Bool(row.is_active())));
            row.upcast()
        }

        OwnedSettingKind::Int { default, min, max } => {
            let row = adw::SpinRow::with_range(*min as f64, *max as f64, 1.0);
            row.set_value(match current {
                Some(SettingValue::Int(value)) => *value,
                _ => *default,
            } as f64);
            row.connect_value_notify(move |row| set(SettingValue::Int(row.value().round() as i64)));
            row.upcast()
        }

        OwnedSettingKind::Float {
            default,
            min,
            max,
            step,
        } => {
            let step = if *step > 0.0 { *step } else { 0.01 };
            let row = adw::SpinRow::with_range(*min, *max, step);
            row.set_digits((-step.log10()).ceil().clamp(0.0, 6.0) as u32);
            row.set_value(match current {
                Some(SettingValue::Float(value)) => *value,
                _ => *default,
            });
            row.connect_value_notify(move |row| set(SettingValue::Float(row.value())));
            row.upcast()
        }

        OwnedSettingKind::Color { default } => {
            let color = match current {
                Some(SettingValue::Color(value)) => *value,
                _ => *default,
            };

            let button = gtk::ColorDialogButton::new(Some(gtk::ColorDialog::new()));
            button.set_valign(gtk::Align::Center);
            button.set_rgba(&gdk::RGBA::new(color.r, color.g, color.b, color.a));
            button.connect_rgba_notify(move |button| {
                let rgba = button.rgba();
                set(SettingValue::Color(Rgba {
                    r: rgba.red(),
                    g: rgba.green(),
                    b: rgba.blue(),
                    a: rgba.alpha(),
                }))
            });

            let row = adw::ActionRow::new();
            row.add_suffix(&button);
            row.set_activatable_widget(Some(&button));
            row
        }

        OwnedSettingKind::Choice { default, options } => {
            let selected = match current {
                Some(SettingValue::Choice(value)) => value,
                _ => default,
            };

            let labels: Vec<&str> = options.iter().map(|option| option.label.as_str()).collect();
            let row = adw::ComboRow::new();
            row.set_model(Some(&gtk::StringList::new(&labels)));
            row.set_selected(
                options
                    .iter()
                    .position(|option| &option.value == selected)
                    .unwrap_or(0) as u32,
            );

            let values: Vec<String> = options.iter().map(|option| option.value.clone()).collect();
            row.connect_selected_notify(move |row| {
                if let Some(value) = values.get(row.selected() as usize) {
                    set(SettingValue::Choice(value.clone()));
                }
            });
            row.upcast()
        }
    };

    row.set_use_markup(false);
    row.set_title(&field.label);
    row.set_subtitle(&field.description);
    row
}
