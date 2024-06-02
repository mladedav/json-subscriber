use crate::cached::Cached;
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
                    SchemaKey::Static(key) => match value {
                        MaybeCached::Serde(value) => {
                            if serialized_anything && !serialized_anything_serde {
                                writer.inner_mut().push(',');
                            }
                            serialized_anything = true;
                            serialized_anything_serde = true;
                            serializer.serialize_entry(key, &value)?
                        }
                        MaybeCached::Cached(Cached::Raw(raw)) => {
                            debug_assert!(
                                serde_json::to_value(&*raw).is_ok(),
                                "[json-subscriber] provided cached value is not valid json: {}",
                                raw,
                            );
                            let mut writer = writer.inner_mut();
                            if serialized_anything {
                                writer.push(',');
                            }
                            serialized_anything = true;
                            writer.push('"');
                            writer.push_str(key);
                            writer.push_str("\":");
                            writer.push_str(&raw);
                        }
                        MaybeCached::Cached(Cached::Array(arr)) => {
                            let mut writer = writer.inner_mut();
                            if serialized_anything {
                                writer.push(',');
                            }
                            serialized_anything = true;
                            writer.push('"');
                            writer.push_str(key);
                            writer.push_str("\":[");
                            let mut first = true;
                            for raw in arr {
                                debug_assert!(
                                    serde_json::to_value(&*raw).is_ok(),
                                    "[json-subscriber] provided cached value in array is not valid json: {}",
                                    raw,
                                );
                                if !first {
                                    writer.push(',');
                                }
                                first = false;
                                writer.push_str(&raw);
                            }
                            writer.push(']');
                        }
                    },
                    SchemaKey::Flatten => {
                        let value = value.into_value();
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

        debug_assert!(
            serde_json::to_value(&*writer).is_ok(),
            "[json-subscriber] serialized line is not valid json: {}",
            writer,
        );

        Ok(())
    }
}

fn resolve_json_value<'a, S: for<'lookup> LookupSpan<'lookup>>(
    value: &'a JsonValue<S>,
    event: &EventRef<'_, S>,
    span: Option<&SpanRef<'_, S>>,
) -> Option<MaybeCached<'a>> {
    match value {
        JsonValue::Serde(value) => Some(MaybeCached::Serde(Cow::Borrowed(value))),
        JsonValue::Struct(map) => Some(MaybeCached::Serde(Cow::Owned(serde_json::Value::Object(
            serde_json::Map::from_iter(map.iter().filter_map(|(key, value)| {
                Some((
                    key.to_string(),
                    resolve_json_value(value, event, span)?
                        .into_value()
                        .into_owned(),
                ))
            })),
        )))),
        JsonValue::Array(array) => Some(MaybeCached::Serde(Cow::Owned(serde_json::Value::Array(
            array
                .iter()
                .filter_map(|value| {
                    resolve_json_value(value, event, span)
                        .map(MaybeCached::into_value)
                        .map(Cow::into_owned)
                })
                .collect(),
        )))),
        JsonValue::DynamicFromEvent(fun) => fun(event)
            .map(Value::into_json)
            .map(Cow::Owned)
            .map(MaybeCached::Serde),
        JsonValue::DynamicFromSpan(fun) => span
            .and_then(fun)
            .map(Value::into_json)
            .map(Cow::Owned)
            .map(MaybeCached::Serde),
        JsonValue::DynamicCachedFromSpan(fun) => span.and_then(fun).map(MaybeCached::Cached),
    }
}

enum MaybeCached<'a> {
    Serde(Cow<'a, serde_json::Value>),
    Cached(Cached),
}

impl<'a> MaybeCached<'a> {
    fn into_value(self) -> Cow<'a, serde_json::Value> {
        match self {
            MaybeCached::Serde(serde) => serde,
            MaybeCached::Cached(Cached::Raw(raw)) => {
                Cow::Owned(serde_json::from_str(&raw).unwrap())
            }
            MaybeCached::Cached(Cached::Array(array)) => Cow::Owned(serde_json::Value::Array(
                array
                    .into_iter()
                    .map(|raw| serde_json::from_str(&raw).unwrap())
                    .collect(),
            )),
        }
    }
}
