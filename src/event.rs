use crate::layer::JsonValue;
use crate::layer::SchemaKey;
use crate::{layer::JsonLayer, write_adaptor::WriteAdaptor};
use serde::ser::SerializeMap;
use serde::Serializer as _;
use std::fmt;
use std::ops::Deref;
use tracing::Metadata;
use tracing_subscriber::registry::SpanRef;

use tracing::{Event, Subscriber};
use tracing_subscriber::{fmt::format::Writer, layer::Context, registry::LookupSpan};

#[cfg(feature = "tracing-log")]
use tracing_log::NormalizeEvent;

/// The same thing as [`SpanRef`] but for events.
pub struct EventRef<'a, R> {
    context: &'a Context<'a, R>,
    event: &'a Event<'a>,
}

impl<'a, R> Deref for EventRef<'a, R> {
    type Target = Event<'a>;

    fn deref(&self) -> &Self::Target {
        self.event
    }
}

impl<'a, R: Subscriber + for<'lookup> LookupSpan<'lookup>> EventRef<'a, R> {
    /// Returns the span's name,
    pub fn name(&self) -> &'static str {
        self.event.metadata().name()
    }

    pub fn metadata(&self) -> &'static Metadata<'static> {
        #[cfg(feature = "tracing-log")]
        {
            let normalized_meta = self.event.normalized_metadata();
            normalized_meta.as_ref().unwrap_or_else(|| event.metadata())
        }
        #[cfg(not(feature = "tracing-log"))]
        self.event.metadata()
    }

    /// Returns a `SpanRef` describing this span's parent, or `None` if this
    /// span is the root of its trace tree.
    pub fn parent_span(&self) -> Option<SpanRef<'a, R>> {
        self.context.event_span(self.event)
    }
}

impl<S, W> JsonLayer<S, W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    pub(crate) fn format_event(
        &self,
        ctx: Context<'_, S>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut visit = || {
            let mut serializer = serde_json::Serializer::new(WriteAdaptor::new(&mut writer));

            let mut serializer = serializer.serialize_map(None)?;

            let event_ref = EventRef {
                context: &ctx,
                event,
            };

            for (key, value) in &self.schema {
                let Some(value) = resolve_json_value(value, &event_ref) else {
                    continue;
                };
                match key {
                    SchemaKey::Static(key) => {
                        serializer.serialize_entry(key, &value)?;
                    }
                    SchemaKey::Flatten => {
                        let map = value.as_object().unwrap();
                        for (key, value) in map {
                            serializer.serialize_entry(key, value)?;
                        }
                    }
                }
            }

            serializer.end()
        };

        visit().map_err(|_| fmt::Error)?;
        writeln!(writer)
    }
}

fn resolve_json_value<S: for<'lookup> LookupSpan<'lookup>>(
    value: &JsonValue<S>,
    event: &EventRef<'_, S>,
) -> Option<serde_json::Value> {
    match value {
        JsonValue::Serde(value) => Some(value.to_owned()),
        JsonValue::Struct(map) => Some(serde_json::Value::Object(serde_json::Map::from_iter(
            map.iter().filter_map(|(key, value)| {
                Some((key.to_string(), resolve_json_value(value, event)?))
            }),
        ))),
        JsonValue::Array(array) => Some(serde_json::Value::Array(
            array
                .iter()
                .filter_map(|value| resolve_json_value(value, event))
                .collect(),
        )),
        JsonValue::Dynamic(fun) => fun(event),
    }
}
