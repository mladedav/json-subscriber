use std::{collections::hash_map::Entry, fmt, sync::atomic::Ordering};

use tracing_core::field;

use crate::fields::JsonFieldsInner;

/// The [visitor] produced by [`JsonFields`]'s [`MakeVisitor`] implementation.
///
/// [visitor]: tracing_subscriber::field::Visit
/// [`MakeVisitor`]: tracing_subscriber::field::MakeVisitor
pub(crate) struct JsonVisitor<'a> {
    fields: &'a mut JsonFieldsInner,
    increment_version: bool,
}

impl<'a> JsonVisitor<'a> {
    pub fn new(fields: &'a mut JsonFieldsInner) -> Self {
        Self {
            fields,
            increment_version: false,
        }
    }
}

impl<'a> Drop for JsonVisitor<'a> {
    fn drop(&mut self) {
        if self.increment_version {
            self.fields.version.fetch_add(1, Ordering::Release);
        }
    }
}

impl<'a> field::Visit for JsonVisitor<'a> {
    /// Visit a double precision floating point value.
    fn record_f64(&mut self, field: &field::Field, value: f64) {
        let value = serde_json::Value::from(value);
        let entry = self.fields.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.increment_version = true;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.increment_version = true;
                }
                entry.insert(value);
            },
        }
    }

    /// Visit a signed 64-bit integer value.
    fn record_i64(&mut self, field: &field::Field, value: i64) {
        let value = serde_json::Value::from(value);
        let entry = self.fields.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.increment_version = true;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.increment_version = true;
                }
                entry.insert(value);
            },
        }
    }

    /// Visit an unsigned 64-bit integer value.
    fn record_u64(&mut self, field: &field::Field, value: u64) {
        let value = serde_json::Value::from(value);
        let entry = self.fields.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.increment_version = true;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.increment_version = true;
                }
                entry.insert(value);
            },
        }
    }

    /// Visit a boolean value.
    fn record_bool(&mut self, field: &field::Field, value: bool) {
        let value = serde_json::Value::from(value);
        let entry = self.fields.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.increment_version = true;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.increment_version = true;
                }
                entry.insert(value);
            },
        }
    }

    /// Visit a string value.
    fn record_str(&mut self, field: &field::Field, value: &str) {
        // We don't want to clone the `value` until we know we want to update it
        // so this closure is here to defer the actual value creation.
        let serde_value = || serde_json::Value::from(value);
        let entry = self.fields.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.increment_version = true;
                vacant.insert(serde_value());
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != value {
                    self.increment_version = true;
                }
                entry.insert(serde_value());
            },
        }
    }

    fn record_debug(&mut self, field: &field::Field, value: &dyn fmt::Debug) {
        match field.name() {
            // Skip fields that are actually log metadata that have already been handled
            #[cfg(feature = "tracing-log")]
            name if name.starts_with("log.") => (),
            name if name.starts_with("r#") => {
                self.fields
                    .fields
                    .insert(&name[2..], serde_json::Value::from(format!("{value:?}")));
            },
            name => {
                self.fields
                    .fields
                    .insert(name, serde_json::Value::from(format!("{value:?}")));
            },
        };
    }
}
