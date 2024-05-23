use crate::layer::JsonValue;
use crate::serde::SerializableContext;
use crate::serde::SerializableSpan;
use crate::{layer::JsonLayer, write_adaptor::WriteAdaptor};
use serde::ser::SerializeMap;
use serde::Serializer as _;
use std::fmt;
use tracing_serde::AsSerde;
use tracing_subscriber::registry::Extensions;
use tracing_subscriber::registry::SpanRef;

use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime},
    layer::Context,
    registry::LookupSpan,
};

impl<W, T> JsonLayer<W, T> {
    pub(crate) fn format_event<S>(
        &self,
        ctx: Context<'_, S>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
        T: FormatTime,
    {
        let mut timestamp = String::new();
        self.timer.format_time(&mut Writer::new(&mut timestamp))?;

        #[cfg(feature = "tracing-log")]
        let normalized_meta = event.normalized_metadata();
        #[cfg(feature = "tracing-log")]
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());
        #[cfg(not(feature = "tracing-log"))]
        let meta = event.metadata();

        let mut visit = || {
            let mut serializer = serde_json::Serializer::new(WriteAdaptor::new(&mut writer));

            let mut serializer = serializer.serialize_map(None)?;

            if self.display_timestamp {
                serializer.serialize_entry("timestamp", &timestamp)?;
            }

            if self.display_level {
                serializer.serialize_entry("level", &meta.level().as_serde())?;
            }

            let current_span = if !event.is_root() {
                event
                    .parent()
                    .and_then(|id| ctx.span(id))
                    .or_else(|| ctx.lookup_current())
            } else {
                None
            };

            if self.flatten_event {
                let mut visitor = tracing_serde::SerdeMapVisitor::new(serializer);
                event.record(&mut visitor);

                serializer = visitor.take_serializer()?;
            } else {
                use tracing_serde::fields::AsMap;
                serializer.serialize_entry("fields", &event.field_map())?;
            };

            if self.display_target {
                serializer.serialize_entry("target", meta.target())?;
            }

            if self.display_filename {
                if let Some(filename) = meta.file() {
                    serializer.serialize_entry("filename", filename)?;
                }
            }

            if self.display_line_number {
                if let Some(line_number) = meta.line() {
                    serializer.serialize_entry("line_number", &line_number)?;
                }
            }

            if self.display_current_span {
                if let Some(ref span) = current_span {
                    serializer
                        .serialize_entry("span", &SerializableSpan(span))
                        .unwrap_or(());
                }
            }

            if self.display_span_list {
                if let Some(ref span) = current_span {
                    serializer.serialize_entry("spans", &SerializableContext(span))?;
                }
            }

            if self.display_thread_name {
                let current_thread = std::thread::current();
                match current_thread.name() {
                    Some(name) => {
                        serializer.serialize_entry("threadName", name)?;
                    }
                    // fall-back to thread id when name is absent and ids are not enabled
                    None if !self.display_thread_id => {
                        serializer
                            .serialize_entry("threadName", &format!("{:?}", current_thread.id()))?;
                    }
                    _ => {}
                }
            }

            let extensions = current_span.as_ref().map(SpanRef::extensions);

            for (key, value) in &self.schema {
                let value = resolve_json_value(value, extensions.as_ref());
                serializer.serialize_entry(key, &value)?;
            }

            if self.display_thread_id {
                serializer
                    .serialize_entry("threadId", &format!("{:?}", std::thread::current().id()))?;
            }

            serializer.end()
        };

        visit().map_err(|_| fmt::Error)?;
        writeln!(writer)
    }
}

fn resolve_json_value(value: &JsonValue, extensions: Option<&Extensions<'_>>) -> serde_json::Value {
    match value {
        JsonValue::Serde(value) => value.to_owned(),
        JsonValue::Struct(map) => serde_json::Value::Object(serde_json::Map::from_iter(
            map.iter()
                .map(|(key, value)| (key.to_string(), resolve_json_value(value, extensions))),
        )),
        JsonValue::Dynamic(fun) => fun(extensions),
    }
}
