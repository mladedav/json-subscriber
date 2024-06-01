use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, io, sync::Arc};

use serde::Serialize;
use tracing_core::{
    span::{Attributes, Id, Record},
    Event, Subscriber,
};
use tracing_serde::fields::AsMap;
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime, MakeWriter, TestWriter},
    registry::SpanRef,
    Registry,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

use crate::{cached::Cached, event::EventRef, fields::JsonFields, value::Value, visitor::JsonVisitor};

pub struct JsonLayer<S = Registry, W = fn() -> io::Stdout> {
    make_writer: W,
    log_internal_errors: bool,
    pub(crate) schema: BTreeMap<SchemaKey, JsonValue<S>>,
}

struct SerializedCache {
    inner: BTreeMap<SchemaKey, (usize, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchemaKey {
    Static(Cow<'static, str>),
    // TODO this doesn't work because we'd have just a single flatten field
    Flatten,
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

pub enum JsonValue<S> {
    Serde(serde_json::Value),
    Struct(BTreeMap<&'static str, JsonValue<S>>),
    Array(Vec<JsonValue<S>>),
    #[allow(clippy::type_complexity)]
    DynamicFromEvent(Box<dyn for<'a> Fn(&'a EventRef<'_, S>) -> Option<Value<'a>> + Send + Sync>),
    DynamicFromSpan(Box<dyn for<'a> Fn(&'a SpanRef<'_, S>) -> Option<Value<'a>> + Send + Sync>),
    DynamicCachedFromSpan(
        Box<dyn for<'a> Fn(&'a SpanRef<'_, S>) -> Option<Cached> + Send + Sync>,
    ),
}

impl<S, W> Layer<S> for JsonLayer<S, W>
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
                eprintln!("[json-subscriber] Span was created but does not contain formatted fields, this is a bug and some fields may have been lost.");
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
                }
                _ => {
                    b = String::new();
                    &mut b
                }
            };

            if self.format_event(ctx, buf, event)
                .is_ok()
            {
                let mut writer = self.make_writer.make_writer_for(event.metadata());
                let res = io::Write::write_all(&mut writer, buf.as_bytes());
                if self.log_internal_errors {
                    if let Err(e) = res {
                        eprintln!("[tracing-json] Unable to write an event to the Writer for this Subscriber! Error: {}\n", e);
                    }
                }
            } else if self.log_internal_errors {
                eprintln!("[tracing-json] Unable to format the following event. Name: {}; Fields: {:?}",
                    event.metadata().name(), event.fields());
            }

            buf.clear();
        });
    }
}

impl<S> JsonLayer<S>
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

impl<S, W> JsonLayer<S, W>
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
    pub fn with_writer<W2>(self, make_writer: W2) -> JsonLayer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        JsonLayer {
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
    pub fn with_test_writer(self) -> JsonLayer<S, TestWriter> {
        JsonLayer {
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
    pub fn map_writer<W2>(self, f: impl FnOnce(W) -> W2) -> JsonLayer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        JsonLayer {
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

    // pub fn serialize_cachable_extension<Ext: CachableExtension + 'static>(
    //     &mut self,
    //     key: impl Into<Cow<'static, str>>,
    // ) {
    //     let key = SchemaKey::from(key.into());
    //     self.schema.insert(
    //         key.clone(),
    //         JsonValue::DynamicFromSpan(Box::new(move |span| {
    //             let mut extensions = span.extensions_mut();
    //             let Some(extension) = extensions.get_mut::<Ext>() else {
    //                 return None;
    //             };
    //             let current_version = extension.version();
    //             let new_value = || serde_json::to_string(extension.value()).ok();

    //             if let Some(cache) = extensions.get_mut::<SerializedCache>() {
    //                 if let Some((cached_version, cached_value)) = cache.inner.get(&key) {
    //                     if cached_version == &current_version {
    //                         Some(Value::Serialized(cached_value))
    //                     } else {
    //                         let new_value = new_value()?;
    //                         cache.inner.insert(key, (current_version, new_value));
    //                         let value = cache.inner.get(&key).unwrap();
    //                         Some(Value::Serialized(&value.1))
    //                     }
    //                 } else {
    //                     todo!()
    //                 }
    //             } else {
    //                 todo!()
    //             }
    //         })),
    //     );
    // }

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
                span.extensions()
                    .get::<Ext>()
                    .and_then(&mapper)
                    .map(serde_json::to_value)
                    .and_then(Result::ok)
                    .map(Value::SerdeJson)
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
                span.extensions()
                    .get::<Ext>()
                    .and_then(&mapper)
                    .map(serde_json::to_value)
                    .and_then(Result::ok)
                    .map(Value::SerdeJson)
            })),
        );
    }

    /// Sets the JSON subscriber being built to flatten event metadata.
    ///
    /// See [`format::Json`]
    pub fn flatten_event(&mut self, flatten_event: bool) -> &mut Self {
        let fields = JsonValue::DynamicFromEvent(Box::new(|event| {
            serde_json::to_value(event.field_map())
                .ok()
                .map(Value::SerdeJson)
        }));
        if flatten_event {
            self.schema.insert(SchemaKey::Flatten, fields);
            self.schema.remove(&SchemaKey::from("fields"));
        } else {
            self.schema.insert(SchemaKey::from("fields"), fields);
            self.schema.remove(&SchemaKey::Flatten);
        }
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
                            .map(|span| {
                                span.extensions()
                                    .get::<JsonFields>()
                                    .map(|fields| fields.serialized.as_ref().unwrap().clone())
                            })
                            .filter_map(std::convert::identity)
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

                Some(Value::SerdeJson(timestamp.into()))
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
                JsonValue::DynamicFromEvent(Box::new(|event| {
                    Some(Value::Str(event.metadata().target()))
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
                JsonValue::DynamicFromEvent(Box::new(|event| {
                    event.metadata().file().map(Value::Str)
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
                JsonValue::DynamicFromEvent(Box::new(|event| {
                    event
                        .metadata()
                        .line()
                        .map(Into::into)
                        .map(Value::SerdeJson)
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
                JsonValue::DynamicFromEvent(Box::new(|event| {
                    Some(Value::Str(event.metadata().level().as_str()))
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
                JsonValue::Serde(
                    std::thread::current()
                        .name()
                        .map(ToOwned::to_owned)
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null),
                ),
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
                JsonValue::Serde(serde_json::Value::String(format!(
                    "{:?}",
                    std::thread::current().id()
                ))),
            );
        } else {
            self.schema.remove(&SchemaKey::from("threadId"));
        }
        self
    }
}
