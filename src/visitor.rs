use std::{collections::btree_map::Entry, fmt};

use tracing_core::field;

use crate::fields::JsonFieldsInner;

/// The [visitor] produced by [`JsonFields`]'s [`MakeVisitor`] implementation.
///
/// [visitor]: tracing_subscriber::field::Visit
/// [`MakeVisitor`]: tracing_subscriber::field::MakeVisitor
pub(crate) struct JsonVisitor<'a>(&'a mut JsonFieldsInner);

impl<'a> JsonVisitor<'a> {
    pub fn new(fields: &'a mut JsonFieldsInner) -> Self {
        Self(fields)
    }
}

impl field::Visit for JsonVisitor<'_> {
    /// Visit a double precision floating point value.
    fn record_f64(&mut self, field: &field::Field, value: f64) {
        let value = serde_json::Value::from(value);
        let entry = self.0.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.0.version += 1;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.0.version += 1;
                }
                entry.insert(value);
            },
        }
    }

    /// Visit a signed 64-bit integer value.
    fn record_i64(&mut self, field: &field::Field, value: i64) {
        let value = serde_json::Value::from(value);
        let entry = self.0.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.0.version += 1;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.0.version += 1;
                }
                entry.insert(value);
            },
        }
    }

    /// Visit an unsigned 64-bit integer value.
    fn record_u64(&mut self, field: &field::Field, value: u64) {
        let value = serde_json::Value::from(value);
        let entry = self.0.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.0.version += 1;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.0.version += 1;
                }
                entry.insert(value);
            },
        }
    }

    /// Visit a boolean value.
    fn record_bool(&mut self, field: &field::Field, value: bool) {
        let value = serde_json::Value::from(value);
        let entry = self.0.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.0.version += 1;
                vacant.insert(value);
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != &value {
                    self.0.version += 1;
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
        let entry = self.0.fields.entry(field.name());
        match entry {
            Entry::Vacant(vacant) => {
                self.0.version += 1;
                vacant.insert(serde_value());
            },
            Entry::Occupied(mut entry) => {
                if entry.get() != value {
                    self.0.version += 1;
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
                self.0
                    .fields
                    .insert(&name[2..], serde_json::Value::from(format!("{value:?}")));
            },
            name => {
                self.0
                    .fields
                    .insert(name, serde_json::Value::from(format!("{value:?}")));
            },
        }
    }
}
