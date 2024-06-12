use std::{borrow::Cow, fmt, ops::Deref};

use serde::{ser::SerializeMap, Serializer};
use tracing::{Event, Metadata, Subscriber};
#[cfg(feature = "tracing-log")]
use tracing_log::NormalizeEvent;
use tracing_subscriber::{
    layer::Context,
    registry::{LookupSpan, SpanRef},
};

use crate::{
    cached::Cached,
    cursor::Cursor,
    layer::{JsonLayer, DynamicJsonValue, JsonValue, SchemaKey},
    serde::JsonSubscriberFormatter,
};

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
                            MaybeCached::Serde(value) => {
                                if value.flatten {
                                    let map = value.value.as_object().unwrap();
                                    if !map.is_empty() {
                                        if serialized_anything && !serialized_anything_serde {
                                            writer.inner_mut().push(',');
                                        }
                                        serialized_anything = true;
                                        serialized_anything_serde = true;
                                        for (key, value) in map {
                                            serializer.serialize_entry(key, value)?;
                                        }
                                    }
                                } else {
                                    if serialized_anything && !serialized_anything_serde {
                                        writer.inner_mut().push(',');
                                    }
                                    serialized_anything = true;
                                    serialized_anything_serde = true;
                                    serializer.serialize_entry(key, &value.value)?
                                }
                            },
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
                            },
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
                                        "[json-subscriber] provided cached value in array is not \
                                         valid json: {}",
                                        raw,
                                    );
                                    if !first {
                                        writer.push(',');
                                    }
                                    first = false;
                                    writer.push_str(&raw);
                                }
                                writer.push(']');
                            },
                            MaybeCached::Raw(raw_fun) => {
                                let mut writer = writer.inner_mut();
                                let rollback_position = writer.len();
                                if serialized_anything {
                                    writer.push(',');
                                }
                                writer.push('"');
                                writer.push_str(key);
                                writer.push_str("\":");
                                let start_position = writer.len();
                                match raw_fun(&event_ref, &mut *writer) {
                                    Ok(()) => {
                                        debug_assert!(
                                            serde_json::to_value(&writer[start_position..]).is_ok(),
                                            "[json-subscriber] raw value factory created invalid \
                                             json: {}",
                                            &writer[start_position..],
                                        );
                                        serialized_anything = true;
                                    },
                                    Err(error) => {
                                        eprintln!(
                                            "[json-subscriber] unable to format raw value to \
                                             string: {error}"
                                        );
                                        writer.truncate(rollback_position);
                                    },
                                }
                            },
                        }
                    },
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
) -> Option<MaybeCached<'a, S>> {
    match value {
        JsonValue::Serde(value) => Some(MaybeCached::Serde(Cow::Borrowed(value))),
        JsonValue::DynamicFromEvent(fun) => fun(event).map(Cow::Owned).map(MaybeCached::Serde),
        JsonValue::DynamicFromSpan(fun) => {
            span.and_then(fun).map(Cow::Owned).map(MaybeCached::Serde)
        },
        JsonValue::DynamicCachedFromSpan(fun) => span.and_then(fun).map(MaybeCached::Cached),
        JsonValue::DynamicRawFromEvent(fun) => Some(MaybeCached::Raw(fun)),
    }
}

enum MaybeCached<'a, S> {
    Serde(Cow<'a, DynamicJsonValue>),
    Cached(Cached),
    Raw(&'a Box<dyn Fn(&EventRef<'_, S>, &mut dyn fmt::Write) -> fmt::Result + Send + Sync>),
}
