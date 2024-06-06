use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, fmt, io, sync::Arc};

use serde::Serialize;
use tracing_core::{
    span::{Attributes, Id, Record},
    Event,
    Subscriber,
};
use tracing_serde::fields::AsMap;
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime, MakeWriter, TestWriter},
    layer::Context,
    registry::{LookupSpan, SpanRef},
    Layer,
    Registry,
};

use super::event::EventRef;
use crate::{cached::Cached, fields::JsonFields, visitor::JsonVisitor};

pub struct CustomJsonLayer<S = Registry, W = fn() -> io::Stdout> {
    make_writer: W,
    log_internal_errors: bool,
    pub(crate) schema: BTreeMap<SchemaKey, JsonValue<S>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchemaKey {
    Static(Cow<'static, str>),
}

impl From<Cow<'static, str>> for SchemaKey {
    fn from(value: Cow<'static, str>) -> Self {
        Self::Static(value)
    }
}

impl From<&'static str> for SchemaKey {
    fn from(value: &'static str) -> Self {
        Self::Static(value.into())
    }
}

impl From<String> for SchemaKey {
    fn from(value: String) -> Self {
        Self::Static(value.into())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DynamicJsonValue {
    pub(crate) flatten: bool,
    pub(crate) value: serde_json::Value,
}

pub(crate) enum JsonValue<S> {
    Serde(DynamicJsonValue),
    #[allow(clippy::type_complexity)]
    DynamicFromEvent(
        Box<dyn for<'a> Fn(&'a EventRef<'_, S>) -> Option<DynamicJsonValue> + Send + Sync>,
    ),
    DynamicFromSpan(
        Box<dyn for<'a> Fn(&'a SpanRef<'_, S>) -> Option<DynamicJsonValue> + Send + Sync>,
    ),
    DynamicCachedFromSpan(Box<dyn for<'a> Fn(&'a SpanRef<'_, S>) -> Option<Cached> + Send + Sync>),
    DynamicRawFromEvent(
        Box<dyn for<'a> Fn(&'a EventRef<'_, S>, &mut dyn fmt::Write) -> fmt::Result + Send + Sync>,
    ),
}

impl<S, W> Layer<S> for CustomJsonLayer<S, W>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    W: for<'writer> MakeWriter<'writer> + 'static,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            if self.log_internal_errors {
                eprintln!("[json-subscriber] Span not found, this is a bug.");
            }
            return;
        };

        let mut extensions = span.extensions_mut();

        if extensions.get_mut::<JsonFields>().is_none() {
            let mut fields = JsonFields::default();
            let mut visitor = JsonVisitor::new(&mut fields);
            attrs.record(&mut visitor);
            fields
                .fields
                .insert("name", serde_json::Value::from(attrs.metadata().name()));
            let serialized = serde_json::to_string(&fields).unwrap();
            fields.serialized = Some(Arc::from(serialized.as_str()));
            extensions.insert(fields);
        } else if self.log_internal_errors {
            eprintln!(
                "[json-subscriber] Unable to format the following event, ignoring: {:?}",
                attrs
            );
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            if self.log_internal_errors {
                eprintln!("[json-subscriber] Span not found, this is a bug.");
            }
            return;
        };

        let mut extensions = span.extensions_mut();
        let Some(fields) = extensions.get_mut::<JsonFields>() else {
            if self.log_internal_errors {
                eprintln!(
                    "[json-subscriber] Span was created but does not contain formatted fields, \
                     this is a bug and some fields may have been lost."
                );
            }
            return;
        };

        values.record(&mut JsonVisitor::new(fields));
        let serialized = serde_json::to_string(&fields).unwrap();
        fields.serialized = Some(Arc::from(serialized.as_str()));
    }

    fn on_enter(&self, _id: &Id, _ctx: Context<'_, S>) {}

    fn on_exit(&self, _id: &Id, _ctx: Context<'_, S>) {}

    fn on_close(&self, _id: Id, _ctx: Context<'_, S>) {}

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        thread_local! {
            static BUF: RefCell<String> = const { RefCell::new(String::new()) };
        }

        BUF.with(|buf| {
            let borrow = buf.try_borrow_mut();
            let mut a;
            let mut b;
            let buf = match borrow {
                Ok(buf) => {
                    a = buf;
                    &mut *a
                },
                _ => {
                    b = String::new();
                    &mut b
                },
            };

            if self.format_event(ctx, buf, event).is_ok() {
                let mut writer = self.make_writer.make_writer_for(event.metadata());
                let res = io::Write::write_all(&mut writer, buf.as_bytes());
                if self.log_internal_errors {
                    if let Err(e) = res {
                        eprintln!(
                            "[tracing-json] Unable to write an event to the Writer for this \
                             Subscriber! Error: {}\n",
                            e
                        );
                    }
                }
            } else if self.log_internal_errors {
                eprintln!(
                    "[tracing-json] Unable to format the following event. Name: {}; Fields: {:?}",
                    event.metadata().name(),
                    event.fields()
                );
            }

            buf.clear();
        });
    }
}

impl<S> CustomJsonLayer<S>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    pub fn empty() -> Self {
        Self {
            make_writer: io::stdout,
            log_internal_errors: false,
            schema: BTreeMap::new(),
        }
    }
}

impl<S, W> CustomJsonLayer<S, W>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    /// Sets the [`MakeWriter`] that the [`JsonLayer`] being built will use to write events.
    ///
    /// # Examples
    ///
    /// Using `stderr` rather than `stdout`:
    ///
    /// ```rust
    /// use std::io;
    /// use tracing_subscriber::fmt;
    ///
    /// let fmt_subscriber = fmt::subscriber()
    ///     .with_writer(io::stderr);
    /// # // this is necessary for type inference.
    /// # use tracing_subscriber::Subscribe as _;
    /// # let _ = fmt_subscriber.with_collector(tracing_subscriber::registry::Registry::default());
    /// ```
    ///
    /// [`MakeWriter`]: super::writer::MakeWriter
    /// [`JsonLayer`]: super::JsonLayer
    pub fn with_writer<W2>(self, make_writer: W2) -> CustomJsonLayer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        CustomJsonLayer {
            make_writer,
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
        }
    }

    /// Borrows the [writer] for this subscriber.
    ///
    /// [writer]: MakeWriter
    pub fn writer(&self) -> &W {
        &self.make_writer
    }

    /// Mutably borrows the [writer] for this subscriber.
    ///
    /// This method is primarily expected to be used with the
    /// [`reload::Handle::modify`](crate::reload::Handle::modify) method.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tracing::info;
    /// # use tracing_subscriber::{fmt,reload,Registry,prelude::*};
    /// # fn non_blocking<T: std::io::Write>(writer: T) -> (fn() -> std::io::Stdout) {
    /// #   std::io::stdout
    /// # }
    /// # fn main() {
    /// let subscriber = fmt::subscriber().with_writer(non_blocking(std::io::stderr()));
    /// let (subscriber, reload_handle) = reload::JsonLayer::new(subscriber);
    /// #
    /// # // specifying the Registry type is required
    /// # let _: &reload::Handle<fmt::JsonLayer<S, W, T> = &reload_handle;
    /// #
    /// info!("This will be logged to stderr");
    /// reload_handle.modify(|subscriber| *subscriber.writer_mut() = non_blocking(std::io::stdout()));
    /// info!("This will be logged to stdout");
    /// # }
    /// ```
    ///
    /// [writer]: MakeWriter
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.make_writer
    }

    /// Configures the subscriber to support [`libtest`'s output capturing][capturing] when used in
    /// unit tests.
    ///
    /// See [`TestWriter`] for additional details.
    ///
    /// # Examples
    ///
    /// Using [`TestWriter`] to let `cargo test` capture test output:
    ///
    /// ```rust
    /// use std::io;
    /// use tracing_subscriber::fmt;
    ///
    /// let fmt_subscriber = fmt::subscriber()
    ///     .with_test_writer();
    /// # // this is necessary for type inference.
    /// # use tracing_subscriber::Subscribe as _;
    /// # let _ = fmt_subscriber.with_collector(tracing_subscriber::registry::Registry::default());
    /// ```
    /// [capturing]:
    /// https://doc.rust-lang.org/book/ch11-02-running-tests.html#showing-function-output
    /// [`TestWriter`]: super::writer::TestWriter
    pub fn with_test_writer(self) -> CustomJsonLayer<S, TestWriter> {
        CustomJsonLayer {
            make_writer: TestWriter::default(),
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
        }
    }

    /// Sets whether to write errors from [`FormatEvent`] to the writer.
    /// Defaults to true.
    ///
    /// By default, `fmt::JsonLayer` will write any `FormatEvent`-internal errors to
    /// the writer. These errors are unlikely and will only occur if there is a
    /// bug in the `FormatEvent` implementation or its dependencies.
    ///
    /// If writing to the writer fails, the error message is printed to stderr
    /// as a fallback.
    ///
    /// [`FormatEvent`]: crate::fmt::FormatEvent
    pub fn log_internal_errors(&mut self, log_internal_errors: bool) -> &mut Self {
        self.log_internal_errors = log_internal_errors;
        self
    }

    /// Updates the [`MakeWriter`] by applying a function to the existing [`MakeWriter`].
    ///
    /// This sets the [`MakeWriter`] that the subscriber being built will use to write events.
    ///
    /// # Examples
    ///
    /// Redirect output to stderr if level is <= WARN:
    ///
    /// ```rust
    /// use tracing::Level;
    /// use tracing_subscriber::fmt::{self, writer::MakeWriterExt};
    ///
    /// let stderr = std::io::stderr.with_max_level(Level::WARN);
    /// let subscriber = fmt::subscriber()
    ///     .map_writer(move |w| stderr.or_else(w));
    /// # // this is necessary for type inference.
    /// # use tracing_subscriber::Subscribe as _;
    /// # let _ = subscriber.with_collector(tracing_subscriber::registry::Registry::default());
    /// ```
    pub fn map_writer<W2>(self, f: impl FnOnce(W) -> W2) -> CustomJsonLayer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        CustomJsonLayer {
            make_writer: f(self.make_writer),
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
        }
    }

    pub fn add_object(&mut self, key: impl Into<Cow<'static, str>>, value: JsonValue<S>) {
        self.schema.insert(SchemaKey::from(key.into()), value);
    }

    pub fn remove_object(&mut self, key: impl Into<Cow<'static, str>>) {
        self.schema.remove(&SchemaKey::from(key.into()));
    }

    pub fn serialize_extension<Ext: Serialize + 'static>(
        &mut self,
        key: impl Into<Cow<'static, str>>,
    ) {
        self.add_from_extension_ref(key, |extension: &Ext| Some(extension))
    }

    pub fn add_from_extension_ref<Ext, Fun, Res>(
        &mut self,
        key: impl Into<Cow<'static, str>>,
        mapper: Fun,
    ) where
        Ext: 'static,
        for<'a> Fun: Fn(&'a Ext) -> Option<&'a Res> + Send + Sync + 'a,
        Res: serde::Serialize,
    {
        self.schema.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicFromSpan(Box::new(move |span| {
                Some(DynamicJsonValue {
                    flatten: false,
                    value: serde_json::to_value(span.extensions().get::<Ext>().and_then(&mapper))
                        .ok()?,
                })
            })),
        );
    }

    pub fn add_from_extension<Ext, Fun, Res>(
        &mut self,
        key: impl Into<Cow<'static, str>>,
        mapper: Fun,
    ) where
        Ext: 'static,
        for<'a> Fun: Fn(&'a Ext) -> Option<Res> + Send + Sync + 'a,
        Res: serde::Serialize,
    {
        self.schema.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicFromSpan(Box::new(move |span| {
                Some(DynamicJsonValue {
                    flatten: false,
                    value: serde_json::to_value(span.extensions().get::<Ext>().and_then(&mapper))
                        .ok()?,
                })
            })),
        );
    }

    /// Sets the JSON subscriber being built to flatten event metadata.
    ///
    /// See [`format::Json`]
    pub fn flatten_event(&mut self, flatten_event: bool) -> &mut Self {
        self.schema.insert(
            SchemaKey::from("fields"),
            JsonValue::DynamicFromEvent(Box::new(move |event| {
                Some(DynamicJsonValue {
                    flatten: flatten_event,
                    value: serde_json::to_value(event.field_map()).ok()?,
                })
            })),
        );
        self
    }

    /// Sets whether or not the formatter will include the current span in
    /// formatted events.
    ///
    /// See [`format::Json`]
    pub fn with_current_span(&mut self, display_current_span: bool) -> &mut Self {
        if display_current_span {
            self.schema.insert(
                SchemaKey::from("span"),
                JsonValue::DynamicCachedFromSpan(Box::new(move |span| {
                    span.extensions()
                        .get::<JsonFields>()
                        .map(|fields| Cached::Raw(fields.serialized.as_ref().unwrap().clone()))
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("span"));
        }
        self
    }

    /// Sets whether or not the formatter will include a list (from root to leaf)
    /// of all currently entered spans in formatted events.
    ///
    /// See [`format::Json`]
    pub fn with_span_list(&mut self, display_span_list: bool) -> &mut Self {
        if display_span_list {
            self.schema.insert(
                SchemaKey::from("spans"),
                JsonValue::DynamicCachedFromSpan(Box::new(|span| {
                    Some(Cached::Array(
                        span.scope()
                            .from_root()
                            .flat_map(|span| {
                                span.extensions()
                                    .get::<JsonFields>()
                                    .map(|fields| fields.serialized.as_ref().unwrap().clone())
                            })
                            .collect::<Vec<_>>(),
                    ))
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("spans"));
        }
        self
    }

    /// Use the given [`timer`] for log message timestamps.
    ///
    /// See the [`time` module] for the provided timer implementations.
    ///
    /// Note that using the `"time`"" feature flag enables the
    /// additional time formatters [`UtcTime`] and [`LocalTime`], which use the
    /// [`time` crate] to provide more sophisticated timestamp formatting
    /// options.
    ///
    /// [`timer`]: time::FormatTime
    /// [`time` module]: mod@time
    /// [`UtcTime`]: time::UtcTime
    /// [`LocalTime`]: time::LocalTime
    /// [`time` crate]: https://docs.rs/time/0.3
    pub fn with_timer<T: FormatTime + Send + Sync + 'static>(&mut self, timer: T) -> &mut Self {
        self.schema.insert(
            SchemaKey::from("timestamp"),
            JsonValue::DynamicFromEvent(Box::new(move |_| {
                let mut timestamp = String::with_capacity(32);
                timer.format_time(&mut Writer::new(&mut timestamp)).ok()?;

                Some(DynamicJsonValue {
                    flatten: false,
                    value: timestamp.into(),
                })
            })),
        );
        self
    }

    /// Do not emit timestamps with log messages.
    pub fn without_time(&mut self) -> &mut Self {
        self.schema.remove(&SchemaKey::from("timestamp"));
        self
    }

    /// Sets whether or not an event's target is displayed.
    pub fn with_target(&mut self, display_target: bool) -> &mut Self {
        if display_target {
            self.schema.insert(
                SchemaKey::from("target"),
                JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                    writer.write_str("\"")?;
                    writer.write_str(event.metadata().target())?;
                    writer.write_str("\"")
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("target"));
        }

        self
    }

    /// Sets whether or not an event's [source code file path][file] is
    /// displayed.
    ///
    /// [file]: tracing_core::Metadata::file
    pub fn with_file(&mut self, display_filename: bool) -> &mut Self {
        if display_filename {
            self.schema.insert(
                SchemaKey::from("filename"),
                JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                    event
                        .metadata()
                        .file()
                        .map(|file| {
                            writer.write_str("\"")?;
                            writer.write_str(file)?;
                            writer.write_str("\"")
                        })
                        .unwrap_or(Ok(()))
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("filename"));
        }
        self
    }

    /// Sets whether or not an event's [source code line number][line] is
    /// displayed.
    ///
    /// [line]: tracing_core::Metadata::line
    pub fn with_line_number(&mut self, display_line_number: bool) -> &mut Self {
        if display_line_number {
            self.schema.insert(
                SchemaKey::from("line_number"),
                JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                    event
                        .metadata()
                        .line()
                        .map(|file| write!(writer, "{}", file))
                        .unwrap_or(Ok(()))
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("line_number"));
        }
        self
    }

    /// Sets whether or not an event's level is displayed.
    pub fn with_level(&mut self, display_level: bool) -> &mut Self {
        if display_level {
            self.schema.insert(
                SchemaKey::from("level"),
                JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                    writer.write_str("\"")?;
                    writer.write_str(event.metadata().level().as_str())?;
                    writer.write_str("\"")
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("level"));
        }
        self
    }

    /// Sets whether or not the [name] of the current thread is displayed
    /// when formatting events.
    ///
    /// [name]: std::thread#naming-threads
    pub fn with_thread_names(&mut self, display_thread_name: bool) -> &mut Self {
        if display_thread_name {
            self.schema.insert(
                SchemaKey::from("threadName"),
                JsonValue::DynamicRawFromEvent(Box::new(|_event, writer| {
                    std::thread::current()
                        .name()
                        .map(|name| {
                            writer.write_str("\"")?;
                            writer.write_str(name)?;
                            writer.write_str("\"")
                        })
                        .unwrap_or(Ok(()))
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("threadName"));
        }
        self
    }

    /// Sets whether or not the [thread ID] of the current thread is displayed
    /// when formatting events.
    ///
    /// [thread ID]: std::thread::ThreadId
    pub fn with_thread_ids(&mut self, display_thread_id: bool) -> &mut Self {
        if display_thread_id {
            self.schema.insert(
                SchemaKey::from("threadId"),
                JsonValue::DynamicRawFromEvent(Box::new(|_event, writer| {
                    writer.write_str("\"")?;
                    write!(writer, "{:?}", std::thread::current().id())?;
                    writer.write_str("\"")
                })),
            );
        } else {
            self.schema.remove(&SchemaKey::from("threadId"));
        }
        self
    }
}
