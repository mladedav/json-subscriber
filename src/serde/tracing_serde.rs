use std::{fmt, mem::transmute};

use serde::{ser::SerializeMap, Serialize, Serializer};
use tracing::{field::Visit, Event};
use tracing_core::Field;

pub(crate) struct RenamedFields<'a, F, C> {
    event: &'a Event<'a>,
    renames: F,
    context: &'a C,
}

impl<'a, F, C> RenamedFields<'a, F, C> {
    pub(crate) fn new(event: &'a Event<'a>, renames: F, context: &'a C) -> Self {
        Self {
            event,
            renames,
            context,
        }
    }
}

impl<F, C> Serialize for RenamedFields<'_, F, C>
where
    F: for<'a> Fn(&'a str, &'a C) -> &'a str + Send + Sync + 'static,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let len = self.event.fields().count();
        let serializer = serializer.serialize_map(Some(len))?;
        let renames: &'static F = unsafe { transmute(&self.renames) };
        let mut visitor = SerdeMapVisitor::new(serializer, renames, self.context);
        self.event.record(&mut visitor);
        visitor.finish()
    }
}

/// Implements `tracing_core::field::Visit` for some `serde::ser::SerializeMap`.
#[derive(Debug)]
pub struct SerdeMapVisitor<'a, S: SerializeMap, F, C> {
    serializer: S,
    renames: F,
    context: &'a C,
    state: Result<(), S::Error>,
}

impl<'a, S, F, C> SerdeMapVisitor<'a, S, F, C>
where
    S: SerializeMap,
{
    /// Create a new map visitor.
    pub fn new(serializer: S, renames: F, context: &'a C) -> Self {
        Self {
            serializer,
            renames,
            context,
            state: Ok(()),
        }
    }

    /// Completes serializing the visited object, returning `Ok(())` if all
    /// fields were serialized correctly, or `Error(S::Error)` if a field could
    /// not be serialized.
    pub fn finish(self) -> Result<S::Ok, S::Error> {
        self.state?;
        self.serializer.end()
    }
}

impl<S, F, C> Visit for SerdeMapVisitor<'_, S, F, C>
where
    S: SerializeMap,
    F: for<'a> Fn(&'a str, &'a C) -> &'a str + Send + Sync + 'static,
{
    fn record_bool(&mut self, field: &Field, value: bool) {
        // If previous fields serialized successfully, continue serializing,
        // otherwise, short-circuit and do nothing.
        if self.state.is_ok() {
            self.state = self
                .serializer
                .serialize_entry((self.renames)(field.name(), self.context), &value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.state.is_ok() {
            self.state = self.serializer.serialize_entry(
                (self.renames)(field.name(), self.context),
                &format_args!("{value:?}"),
            );
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if self.state.is_ok() {
            self.state = self
                .serializer
                .serialize_entry((self.renames)(field.name(), self.context), &value);
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if self.state.is_ok() {
            self.state = self
                .serializer
                .serialize_entry((self.renames)(field.name(), self.context), &value);
        }
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        if self.state.is_ok() {
            self.state = self
                .serializer
                .serialize_entry((self.renames)(field.name(), self.context), &value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if self.state.is_ok() {
            self.state = self
                .serializer
                .serialize_entry((self.renames)(field.name(), self.context), &value);
        }
    }
}
