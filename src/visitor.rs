use std::{collections::hash_map::Entry, fmt, sync::atomic::Ordering};

use tracing_core::field;

use crate::fields::JsonFields;

/// The [visitor] produced by [`JsonFields`]'s [`MakeVisitor`] implementation.
///
/// [visitor]: tracing_subscriber::field::Visit
/// [`MakeVisitor`]: tracing_subscriber::field::MakeVisitor
pub(crate) struct JsonVisitor<'a> {
    fields: &'a JsonFields,
    increment_version: bool,
}

impl<'a> JsonVisitor<'a> {
    pub fn new(fields: &'a JsonFields) -> Self {
        Self {
            fields,
            increment_version: false,
        }
    }
}

impl<'a> field::Visit for JsonVisitor<'a> {
    /// Visit a double precision floating point value.
    fn record_f64(&mut self, field: &field::Field, value: f64) {
        self.fields.set(field, value.to_string())
    }

    /// Visit a signed 64-bit integer value.
    fn record_i64(&mut self, field: &field::Field, value: i64) {
        self.fields.set(field, value.to_string())
    }

    /// Visit an unsigned 64-bit integer value.
    fn record_u64(&mut self, field: &field::Field, value: u64) {
        self.fields.set(field, value.to_string())
    }

    /// Visit a boolean value.
    fn record_bool(&mut self, field: &field::Field, value: bool) {
        self.fields.set(field, value.to_string())
    }

    /// Visit a string value.
    fn record_str(&mut self, field: &field::Field, value: &str) {
        let mut split = value.split('"');
        let mut string = String::with_capacity(value.len() + 2);
        string.push('"');
        string.push_str(split.next().unwrap());
        while let Some(next) = split.next() {
            string.push_str(r#"\""#);
            string.push_str(next);
        }
        string.push('"');
        self.fields.set(field, string);
    }

    fn record_debug(&mut self, field: &field::Field, value: &dyn fmt::Debug) {
        match field.name() {
            // Skip fields that are actually log metadata that have already been handled
            #[cfg(feature = "tracing-log")]
            name if name.starts_with("log.") => (),
            _ => {
                self.fields.set(field, format!("{value:?}"));
            },
        };
    }
}
