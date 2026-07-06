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
    field_writer::{write_json_key, FieldWriter},
    layer::{JsonLayer, JsonValue, SchemaKey},
    serde::JsonSubscriberFormatter,
};

/// The same thing as [`SpanRef`] but for events.
pub struct EventRef<'a, 'b, 'c, R: for<'lookup> LookupSpan<'lookup>> {
    context: &'a Context<'b, R>,
    event: &'a Event<'b>,
    span: Option<SpanRef<'c, R>>,
}

impl<'a, R: for<'lookup> LookupSpan<'lookup>> Deref for EventRef<'a, '_, '_, R> {
    type Target = Event<'a>;

    fn deref(&self) -> &Self::Target {
        self.event
    }
}

impl<'c, R: Subscriber + for<'lookup> LookupSpan<'lookup>> EventRef<'_, '_, 'c, R> {
    /// Returns the span's name,
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        self.event.metadata().name()
    }

    #[cfg(feature = "tracing-log")]
    #[allow(dead_code)]
    pub fn normalized_metadata(&self) -> Option<Metadata<'_>> {
        self.event.normalized_metadata()
    }

    pub fn metadata(&self) -> &'static Metadata<'static> {
        self.event.metadata()
    }

    /// Returns a `SpanRef` describing this span's parent, or `None` if this
    /// span is the root of its trace tree.
    pub fn parent_span(&self) -> Option<&SpanRef<'c, R>> {
        self.span.as_ref()
    }

    pub(super) fn event(&self) -> &Event<'_> {
        self.event
    }

    pub(super) fn context(&self) -> &Context<'_, R> {
        self.context
    }
}

impl<S, W> JsonLayer<S, W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    pub(crate) fn format_event(
        &self,
        context: &Context<'_, S>,
        writer: &mut Vec<u8>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut visit = || {
            let writer = Cursor::new(writer);
            let mut serializer =
                serde_json::Serializer::with_formatter(&writer, JsonSubscriberFormatter);

            let mut serializer = serializer.serialize_map(None)?;

            let span = context.event_span(event);

            let event_ref = EventRef {
                context,
                event,
                span,
            };

            let mut serialized_anything = false;
            let mut serialized_anything_serde = false;

            for (SchemaKey::Static(key), value) in &self.keyed_values {
                let Some(value) = resolve_json_value(value, &event_ref) else {
                    continue;
                };

                match value {
                    MaybeCached::Serde(value) => {
                        if serialized_anything && !serialized_anything_serde {
                            writer.inner_mut().push(b',');
                        }
                        serialized_anything = true;
                        serialized_anything_serde = true;
                        serializer.serialize_entry(key, &value)?;
                    },
                    MaybeCached::Cached(Cached::Raw(raw)) => {
                        debug_assert!(
                            serde_json::from_slice::<serde_json::Value>(raw.as_bytes()).is_ok(),
                            "[json-subscriber] provided cached value is not valid json: {raw}",
                        );
                        let mut writer = writer.inner_mut();
                        if serialized_anything {
                            writer.push(b',');
                        }
                        serialized_anything = true;
                        write_json_key(&mut writer, key)?;
                        writer.extend_from_slice(raw.as_bytes());
                    },
                    MaybeCached::Raw(raw_fun) => {
                        let mut writer = writer.inner_mut();
                        let rollback_position = writer.len();
                        if serialized_anything {
                            writer.push(b',');
                        }
                        write_json_key(&mut writer, key)?;
                        let start_position = writer.len();
                        match raw_fun(&event_ref, &mut writer) {
                            Ok(()) => {
                                if writer.len() == start_position {
                                    // The factory wrote nothing, meaning it had no value for this
                                    // event (e.g. no OpenTelemetry context). Roll back the key so
                                    // the field is omitted entirely instead of serialized as an
                                    // empty value.
                                    writer.truncate(rollback_position);
                                } else {
                                    debug_assert!(
                                        serde_json::from_slice::<serde_json::Value>(
                                            &writer[start_position..],
                                        )
                                        .is_ok(),
                                        "[json-subscriber] raw value factory created invalid \
                                         json: {}",
                                        std::str::from_utf8(&writer[start_position..])
                                            .unwrap_or("<invalid utf8>"),
                                    );
                                    serialized_anything = true;
                                }
                            },
                            Err(error) => {
                                eprintln!(
                                    "[json-subscriber] unable to format raw value to string: \
                                     {error}"
                                );
                                writer.truncate(rollback_position);
                            },
                        }
                    },
                }
            }

            for value in self.flattened_values.values() {
                if let JsonValue::DynamicFromEventWithWriter(fun) = value {
                    let mut inner = writer.inner_mut();
                    let mut field_writer = FieldWriter::new(&mut inner, serialized_anything);
                    fun(&event_ref, &mut field_writer);
                    if field_writer.wrote_anything() {
                        serialized_anything = true;
                    }
                    continue;
                }

                let Some(value) = resolve_json_value(value, &event_ref) else {
                    continue;
                };

                match value {
                    MaybeCached::Serde(value) => {
                        let map = value.as_object().unwrap();
                        if !map.is_empty() {
                            if serialized_anything && !serialized_anything_serde {
                                writer.inner_mut().push(b',');
                            }
                            serialized_anything = true;
                            serialized_anything_serde = true;
                            for (key, value) in map {
                                serializer.serialize_entry(key, value)?;
                            }
                        }
                    },
                    MaybeCached::Cached(Cached::Raw(raw)) => {
                        debug_assert!(
                            serde_json::from_slice::<serde_json::Value>(raw.as_bytes()).is_ok(),
                            "[json-subscriber] provided cached value is not valid json: {raw}",
                        );
                        if !raw.contains('"') {
                            // If the raw string contains at least a single quote, there is at least
                            // one field in the object. Otherwise it is empty and we just skip it.
                            // Assuming it's a valid JSON of course.
                            continue;
                        }
                        let Some(object_contents) = raw
                            .as_ref()
                            .trim()
                            .strip_prefix('{')
                            .and_then(|str| str.strip_suffix('}'))
                        else {
                            eprintln!(
                                "[json-subscriber] provided cached value cannot be flattened \
                                 because it is not an object: {raw}"
                            );
                            continue;
                        };
                        let mut writer = writer.inner_mut();
                        if serialized_anything {
                            writer.push(b',');
                        }
                        serialized_anything = true;
                        writer.extend_from_slice(object_contents.as_bytes());
                    },
                    MaybeCached::Raw(raw_fun) => {
                        let mut output: Vec<u8> = Vec::new();
                        match raw_fun(&event_ref, &mut output) {
                            Ok(()) => {
                                let Ok(s) = std::str::from_utf8(&output) else {
                                    eprintln!("[json-subscriber] raw value factory wrote non-utf8");
                                    continue;
                                };
                                debug_assert!(
                                    serde_json::from_str::<serde_json::Value>(s).is_ok(),
                                    "[json-subscriber] raw value factory created invalid json: {s}",
                                );
                                let Some(object_contents) = s
                                    .trim()
                                    .strip_prefix('{')
                                    .and_then(|str| str.strip_suffix('}'))
                                else {
                                    eprintln!(
                                        "[json-subscriber] provided cached value cannot be \
                                         flattened because it is not an object: {s}"
                                    );
                                    continue;
                                };
                                let mut writer = writer.inner_mut();
                                if serialized_anything {
                                    writer.push(b',');
                                }
                                serialized_anything = true;
                                writer.extend_from_slice(object_contents.as_bytes());
                            },
                            Err(error) => {
                                eprintln!(
                                    "[json-subscriber] unable to format raw value to string: \
                                     {error}"
                                );
                            },
                        }
                    },
                }
            }

            serializer.end()
        };

        visit().map_err(|_| fmt::Error)?;
        writer.push(b'\n');

        debug_assert!(
            serde_json::from_slice::<serde_json::Value>(writer).is_ok(),
            "[json-subscriber] serialized line is not valid json: {}",
            std::str::from_utf8(writer).unwrap_or("<invalid utf8>"),
        );

        Ok(())
    }
}

fn resolve_json_value<'a, S: Subscriber + for<'lookup> LookupSpan<'lookup>>(
    value: &'a JsonValue<S>,
    event: &EventRef<'_, '_, '_, S>,
) -> Option<MaybeCached<'a, S>> {
    match value {
        JsonValue::Serde(value) => Some(MaybeCached::Serde(Cow::Borrowed(value))),
        JsonValue::DynamicFromEvent(fun) => fun(event).map(Cow::Owned).map(MaybeCached::Serde),
        JsonValue::DynamicFromSpan(fun) => event
            .parent_span()
            .and_then(fun)
            .map(Cow::Owned)
            .map(MaybeCached::Serde),
        JsonValue::DynamicCachedFromSpan(fun) => {
            event.parent_span().and_then(fun).map(MaybeCached::Cached)
        },
        JsonValue::DynamicRawFromEvent(fun) => Some(MaybeCached::Raw(fun)),
        // This cannot be used with a static key so this should never be called
        JsonValue::DynamicFromEventWithWriter(_) => None,
    }
}

#[allow(clippy::type_complexity)]
enum MaybeCached<'a, S: for<'lookup> LookupSpan<'lookup>> {
    Serde(Cow<'a, serde_json::Value>),
    Cached(Cached),
    Raw(&'a Box<dyn Fn(&EventRef<'_, '_, '_, S>, &mut Vec<u8>) -> fmt::Result + Send + Sync>),
}
