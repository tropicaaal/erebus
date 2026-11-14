//! Theme settings.

use crate::primitives::Hsla;
use erebus_primitives::Rgba;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

//
// MARK: schema
//

/// Schema for a single configurable option in a theme.
#[derive(Clone, Copy, PartialEq, Serialize)]
pub struct SettingsField {
    /// stable identifier used as the key in [`SettingValues`]
    pub key: &'static str,
    /// human-readable name shown in frontends
    pub label: &'static str,
    pub description: &'static str,
    pub kind: SettingKind,
}

#[derive(Clone, Copy, PartialEq, Serialize)]
pub enum SettingKind {
    Bool {
        default: bool,
    },
    Int {
        default: i64,
        min: i64,
        max: i64,
    },
    Float {
        default: f64,
        min: f64,
        max: f64,
        step: f64,
    },
    Color {
        default: Rgba,
    },
    Choice {
        default: &'static str,
        options: &'static [ChoiceOption],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ChoiceOption {
    pub value: &'static str,
    pub label: &'static str,
}

impl ChoiceOption {
    pub const fn new(value: &'static str, label: &'static str) -> Self {
        Self { value, label }
    }
}

impl SettingsField {
    pub const fn new(key: &'static str, label: &'static str, kind: SettingKind) -> Self {
        Self {
            key,
            label,
            description: "",
            kind,
        }
    }

    // getters

    pub const fn bool(key: &'static str, label: &'static str, default: bool) -> Self {
        Self::new(key, label, SettingKind::Bool { default })
    }

    pub const fn int(
        key: &'static str,
        label: &'static str,
        default: i64,
        range: std::ops::RangeInclusive<i64>,
    ) -> Self {
        let (min, max) = (*range.start(), *range.end());
        Self::new(key, label, SettingKind::Int { default, min, max })
    }

    pub const fn float(
        key: &'static str,
        label: &'static str,
        default: f64,
        range: std::ops::RangeInclusive<f64>,
        step: f64,
    ) -> Self {
        let (min, max) = (*range.start(), *range.end());
        Self::new(
            key,
            label,
            SettingKind::Float {
                default,
                min,
                max,
                step,
            },
        )
    }

    pub const fn color(key: &'static str, label: &'static str, default: Rgba) -> Self {
        Self::new(key, label, SettingKind::Color { default })
    }

    pub const fn choice(
        key: &'static str,
        label: &'static str,
        default: &'static str,
        options: &'static [ChoiceOption],
    ) -> Self {
        Self::new(key, label, SettingKind::Choice { default, options })
    }

    pub const fn description(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }
}

//
// MARK: values
//

/// A value the user picked for a setting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SettingValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Color(Rgba),
    Choice(String),
}

//
// MARK: validation
//

// these two are non-panicking versions of clamp
fn clamp_int(v: i64, min: i64, max: i64) -> i64 {
    v.clamp(min, max.max(min))
}
fn clamp_float(v: f64, min: f64, max: f64) -> f64 {
    v.max(min).min(max.max(min))
}

impl SettingKind {
    /// Returns `value` coerced into something valid for this kind, or `None` if it's the
    /// wrong type entirely (or names a choice that doesn't exist).
    pub fn validate(&self, value: &SettingValue) -> Option<SettingValue> {
        Some(match (*self, value) {
            (Self::Bool { .. }, SettingValue::Bool(v)) => SettingValue::Bool(*v),
            (Self::Int { min, max, .. }, SettingValue::Int(v)) => {
                SettingValue::Int(clamp_int(*v, min, max))
            }
            (Self::Float { min, max, .. }, SettingValue::Float(v)) if v.is_finite() => {
                SettingValue::Float(clamp_float(*v, min, max))
            }
            (Self::Color { .. }, SettingValue::Color(v)) => SettingValue::Color(*v),
            (Self::Choice { options, .. }, SettingValue::Choice(v))
                if options.iter().any(|o| o.value == v) =>
            {
                SettingValue::Choice(v.clone())
            }
            _ => return None,
        })
    }

    /// The kind's default, coerced to be valid for it where possible.
    pub fn default_value(&self) -> SettingValue {
        match *self {
            Self::Bool { default } => SettingValue::Bool(default),
            Self::Int { default, min, max } => SettingValue::Int(clamp_int(default, min, max)),
            Self::Float {
                default, min, max, ..
            } => {
                let default = if default.is_finite() { default } else { 0.0 };
                SettingValue::Float(clamp_float(default, min, max))
            }
            Self::Color { default } => SettingValue::Color(default),
            Self::Choice { default, options } => {
                // fall back to the first option if the default isn't one of them
                let pick = if options.iter().any(|o| o.value == default) {
                    default
                } else {
                    options.first().map_or(default, |o| o.value)
                };
                SettingValue::Choice(pick.to_owned())
            }
        }
    }
}

#[doc(hidden)]
pub const fn validate_settings(fields: &[SettingsField]) {
    const fn const_str_eq(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    const fn validate_rgba(rgba: &Rgba) -> bool {
        const fn ok(v: f32) -> bool {
            v >= 0.0 && v <= 1.0
        }

        ok(rgba.r) && ok(rgba.g) && ok(rgba.b) && ok(rgba.a)
    }

    let mut i = 0;
    while i < fields.len() {
        let field = &fields[i];
        assert!(!field.key.is_empty(), "setting key must not be empty");

        let mut j = i + 1;
        while j < fields.len() {
            assert!(
                !const_str_eq(field.key, fields[j].key),
                "duplicate setting key"
            );
            j += 1;
        }

        match field.kind {
            SettingKind::Bool { .. } => {}
            SettingKind::Int { default, min, max } => {
                assert!(min <= max, "int setting: min > max");
                assert!(
                    min <= default && default <= max,
                    "int setting: default out of range"
                );
            }
            SettingKind::Float {
                default,
                min,
                max,
                step,
            } => {
                assert!(
                    min.is_finite() && max.is_finite(),
                    "float setting: bounds must be finite"
                );
                assert!(min <= max, "float setting: min > max");
                assert!(
                    default >= min && default <= max,
                    "float setting: default out of range"
                );
                assert!(
                    step.is_finite() && step >= 0.0,
                    "float setting: step must be finite and >= 0"
                );
            }
            SettingKind::Color { default } => {
                assert!(
                    validate_rgba(&default),
                    "color setting: components must be in 0.0..=1.0"
                );
            }
            SettingKind::Choice { default, options } => {
                assert!(!options.is_empty(), "choice setting: no options");
                let mut found = false;
                let mut a = 0;
                while a < options.len() {
                    found |= const_str_eq(options[a].value, default);
                    let mut b = a + 1;
                    while b < options.len() {
                        assert!(
                            !const_str_eq(options[a].value, options[b].value),
                            "choice setting: duplicate option value"
                        );
                        b += 1;
                    }
                    a += 1;
                }
                assert!(found, "choice setting: default is not one of the options");
            }
        }
        i += 1;
    }
}

//
// MARK: resolved settings
//

/// Settings as a theme sees them: every defined key is present and valid.
#[derive(Clone, Default, PartialEq)]
pub struct Settings {
    values: HashMap<String, SettingValue>,
}

impl Settings {
    /// Combines `values` with the defaults in `defs`. Drops keys not specified in the schema and
    /// replaces values that don't fit the schema.
    ///
    /// Works from either the theme's `const` schema or a host-decoded one.
    pub fn resolve(defs: &[SettingsField], values: &HashMap<String, SettingValue>) -> Self {
        let values = defs
            .iter()
            .map(|def| {
                let value = values
                    .get(def.key)
                    .and_then(|v| def.kind.validate(v))
                    .unwrap_or_else(|| def.kind.default_value());
                (def.key.to_owned(), value)
            })
            .collect();

        Self { values }
    }

    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.values.get(key)
    }

    pub fn bool(&self, key: &str) -> bool {
        match self.get(key) {
            Some(SettingValue::Bool(v)) => *v,
            v => panic!("unexpected settings value for {key}: {v:?}"),
        }
    }

    pub fn int(&self, key: &str) -> i64 {
        match self.get(key) {
            Some(SettingValue::Int(v)) => *v,
            v => panic!("unexpected settings value for {key}: {v:?}"),
        }
    }

    pub fn float(&self, key: &str) -> f64 {
        match self.get(key) {
            Some(SettingValue::Float(v)) => *v,
            v => panic!("unexpected settings value for {key}: {v:?}"),
        }
    }

    pub fn color(&self, key: &str) -> Hsla {
        match self.get(key) {
            Some(SettingValue::Color(v)) => (*v).into(),
            v => panic!("unexpected settings value for {key}: {v:?}"),
        }
    }

    pub fn choice(&self, key: &str) -> &str {
        match self.get(key) {
            Some(SettingValue::Choice(v)) => v,
            v => panic!("unexpected settings value for {key}: {v:?}"),
        }
    }
}
