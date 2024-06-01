use crate::cursor::Cursor;
use crate::layer::JsonLayer;
use crate::layer::JsonValue;
use crate::layer::SchemaKey;
use crate::serde::JsonSubscriberFormatter;
use crate::value::Value;
use serde::ser::SerializeMap;
use serde::Serializer;
use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use tracing::Metadata;
use tracing_subscriber::registry::SpanRef;

use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, registry::LookupSpan};

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

    #[cfg(feature = "tracing-log")]
    pub fn normalized_metadata(&self) -> Option<Metadata<'_>> {
        self.event.normalized_metadata()
    }

    pub fn metadata(&self) -> &'static Metadata<'static> {
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
        writer: &mut String,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut visit = || {
            let writer = Cursor::new(writer);
            let mut serializer =
                serde_json::Serializer::with_formatter(&writer, JsonSubscriberFormatter);

            let mut serializer = serializer.serialize_map(None)?;

            let event_ref = EventRef {
                context: &ctx,
                event,
            };

            let mut serialized_anything = false;
            let mut serialized_anything_serde = false;

            let current_span = event_ref.parent_span();

            for (key, value) in &self.schema {
                let Some(value) = resolve_json_value(value, &event_ref, current_span.as_ref())
                else {
                    continue;
                };
                match key {
                    SchemaKey::Static(key) => {
                        match value {
                            MaybeHack::Serde(value) => {
                                if serialized_anything && !serialized_anything_serde {
                                    writer.inner_mut().push(',');
                                }
                                serialized_anything = true;
                                serialized_anything_serde = true;
                                serializer.serialize_entry(key, &value)?
                            }
                            MaybeHack::Str(str) => {
                                let mut writer = writer.inner_mut();
                                if serialized_anything {
                                    writer.push(',');
                                }
                                serialized_anything = true;
                                writer.push('"');
                                writer.push_str(key);
                                writer.push_str("\":");
                                writer.push_str(&str);
                            }
                        }
                    }
                    SchemaKey::Flatten => {
                        let value = value.unwrap();
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
        writer.push('\n');
        Ok(())
    }
}

fn resolve_json_value<'a, S: for<'lookup> LookupSpan<'lookup>>(
    value: &'a JsonValue<S>,
    event: &EventRef<'_, S>,
    span: Option<&SpanRef<'_, S>>,
) -> Option<MaybeHack<'a>> {
    match value {
        JsonValue::Serde(value) => Some(Cow::Borrowed(value)).map(MaybeHack::Serde),
        JsonValue::Struct(map) => Some(Cow::Owned(serde_json::Value::Object(
            serde_json::Map::from_iter(map.iter().filter_map(|(key, value)| {
                Some((
                    key.to_string(),
                    resolve_json_value(value, event, span)?.unwrap().into_owned(),
                ))
            }))
        ))).map(MaybeHack::Serde),
        JsonValue::Array(array) => Some(Cow::Owned(serde_json::Value::Array(
            array
                .iter()
                .filter_map(|value| resolve_json_value(value, event, span).map(MaybeHack::unwrap).map(Cow::into_owned))
                .collect(),
        ))).map(MaybeHack::Serde),
        JsonValue::DynamicFromEvent(fun) => fun(event).map(Value::to_json).map(Cow::Owned).map(MaybeHack::Serde),
        JsonValue::DynamicFromSpan(fun) => span.and_then(fun).map(Value::to_json).map(Cow::Owned).map(MaybeHack::Serde),
        JsonValue::DynamicCachedFromSpan(fun) => span.and_then(fun).map(MaybeHack::Str),
    }
}

enum MaybeHack<'a> {
    Serde(Cow<'a, serde_json::Value>),
    Str(Arc<str>),
}

impl<'a> MaybeHack<'a> {
    #[deprecated]
    fn unwrap(self) -> Cow<'a, serde_json::Value> {
        match self {
            MaybeHack::Serde(serde) => serde,
            MaybeHack::Str(_) => todo!(),
        }
    }
}
