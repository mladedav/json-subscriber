use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, fmt, io, sync::Arc};

// `fmt::Write` is needed for `write!` on `FmtWrite(&mut Vec<u8>)` inside the raw closures.
use std::fmt::Write;

use serde::Serialize;
use tracing_core::{
    span::{Attributes, Id, Record},
    Event, Subscriber,
};
use tracing_serde::fields::AsMap;
use tracing_subscriber::{
    fmt::{format::Writer, time::FormatTime, MakeWriter, TestWriter},
    layer::Context,
    registry::{LookupSpan, SpanRef},
    Layer, Registry,
};

mod event;

use event::EventRef;
use uuid::Uuid;

use crate::{
    cached::Cached,
    cursor::FmtWrite,
    field_writer::FieldWriter,
    fields::{JsonFields, JsonFieldsInner},
    serde::RenamedFields,
    visitor::JsonVisitor,
};

/// Layer that implements logging JSON to a configured output. This is a lower-level API that may
/// change a bit in next versions.
///
/// See [`fmt::Layer`](crate::fmt::Layer) for an alternative especially if you're migrating from
/// `tracing_subscriber`.
pub struct JsonLayer<S: for<'lookup> LookupSpan<'lookup> = Registry, W = fn() -> io::Stdout> {
    make_writer: W,
    log_internal_errors: bool,
    keyed_values: BTreeMap<SchemaKey, JsonValue<S>>,
    flattened_values: BTreeMap<FlatSchemaKey, JsonValue<S>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SchemaKey {
    Static(Cow<'static, str>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FlatSchemaKey {
    Uuid(Uuid),
    FlattenedEvent,
    FlattenedCurrentSpan,
    FlattenedSpanList,
}

impl FlatSchemaKey {
    fn new_uuid() -> Self {
        Self::Uuid(uuid::Uuid::new_v4())
    }
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

#[allow(clippy::type_complexity)]
pub(crate) enum JsonValue<S: for<'lookup> LookupSpan<'lookup>> {
    Serde(serde_json::Value),
    DynamicFromEvent(
        Box<dyn Fn(&EventRef<'_, '_, '_, S>) -> Option<serde_json::Value> + Send + Sync>,
    ),
    DynamicFromSpan(Box<dyn Fn(&SpanRef<'_, S>) -> Option<serde_json::Value> + Send + Sync>),
    DynamicCachedFromSpan(Box<dyn Fn(&SpanRef<'_, S>) -> Option<Cached> + Send + Sync>),
    DynamicRawFromEvent(
        Box<dyn Fn(&EventRef<'_, '_, '_, S>, &mut Vec<u8>) -> fmt::Result + Send + Sync>,
    ),
    DynamicFromEventWithWriter(
        Box<dyn Fn(&EventRef<'_, '_, '_, S>, &mut FieldWriter<'_>) + Send + Sync>,
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
            let mut fields = JsonFieldsInner::default();
            let mut visitor = JsonVisitor::new(&mut fields);
            attrs.record(&mut visitor);
            fields
                .fields
                .insert("name", serde_json::Value::from(attrs.metadata().name()));
            let fields = fields.finish();
            extensions.insert(fields);
        } else if self.log_internal_errors {
            eprintln!(
                "[json-subscriber] Unable to format the following event, ignoring: {attrs:?}",
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

        values.record(&mut JsonVisitor::new(&mut fields.inner));
        let serialized = serde_json::to_string(&fields).unwrap();
        fields.serialized = Arc::from(serialized.as_str());
    }

    fn on_enter(&self, _id: &Id, _ctx: Context<'_, S>) {}

    fn on_exit(&self, _id: &Id, _ctx: Context<'_, S>) {}

    fn on_close(&self, _id: Id, _ctx: Context<'_, S>) {}

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        thread_local! {
            static BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
        }

        BUF.with(|buf| {
            let borrow = buf.try_borrow_mut();
            let mut a;
            let mut b;
            let buf: &mut Vec<u8> = if let Ok(buf) = borrow {
                a = buf;
                &mut a
            } else {
                b = Vec::new();
                &mut b
            };

            if self.format_event(&ctx, buf, event).is_ok() {
                let mut writer = self.make_writer.make_writer_for(event.metadata());
                let res = io::Write::write_all(&mut writer, buf);
                if self.log_internal_errors {
                    if let Err(e) = res {
                        eprintln!(
                            "[tracing-json] Unable to write an event to the Writer for this \
                             Subscriber! Error: {e}\n",
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

impl<S> JsonLayer<S>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    /// Creates an empty [`JsonLayer`] which will output logs to stdout.
    pub fn stdout() -> JsonLayer<S, fn() -> io::Stdout> {
        JsonLayer::new(io::stdout)
    }

    /// Creates an empty [`JsonLayer`] which will output logs to stderr.
    pub fn stderr() -> JsonLayer<S, fn() -> io::Stderr> {
        JsonLayer::new(io::stderr)
    }

    /// Creates an empty [`JsonLayer`] which will output logs to the configured
    /// [`Writer`](io::Write).
    pub fn new<W>(make_writer: W) -> JsonLayer<S, W>
    where
        W: for<'writer> MakeWriter<'writer> + 'static,
    {
        JsonLayer::<S, W> {
            make_writer,
            log_internal_errors: false,
            keyed_values: BTreeMap::new(),
            flattened_values: BTreeMap::new(),
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
    /// # use tracing_subscriber::prelude::*;
    /// let layer = json_subscriber::JsonLayer::stdout()
    ///     .with_writer(std::io::stderr);
    /// # tracing_subscriber::registry().with(layer);
    /// ```
    ///
    /// [`MakeWriter`]: MakeWriter
    /// [`JsonLayer`]: super::JsonLayer
    pub fn with_writer<W2>(self, make_writer: W2) -> JsonLayer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        JsonLayer {
            make_writer,
            log_internal_errors: self.log_internal_errors,
            keyed_values: self.keyed_values,
            flattened_values: self.flattened_values,
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
    /// [`reload::Handle::modify`](tracing_subscriber::reload::Handle::modify) method.
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
    /// let layer = json_subscriber::JsonLayer::stdout().with_writer(non_blocking(std::io::stderr()));
    /// let (layer, reload_handle) = reload::Layer::new(layer);
    ///
    /// tracing_subscriber::registry().with(layer).init();
    ///
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
    /// # use tracing_subscriber::prelude::*;
    /// let layer = json_subscriber::JsonLayer::stdout()
    ///     .with_test_writer();
    /// # tracing_subscriber::registry().with(layer);
    /// ```
    /// [capturing]:
    /// https://doc.rust-lang.org/book/ch11-02-running-tests.html#showing-function-output
    pub fn with_test_writer(self) -> JsonLayer<S, TestWriter> {
        JsonLayer {
            make_writer: TestWriter::default(),
            log_internal_errors: self.log_internal_errors,
            keyed_values: self.keyed_values,
            flattened_values: self.flattened_values,
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
    /// [`FormatEvent`]: tracing_subscriber::fmt::FormatEvent
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
    /// # use tracing_subscriber::prelude::*;
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    ///
    /// let stderr = std::io::stderr.with_max_level(tracing::Level::WARN);
    /// let layer = json_subscriber::JsonLayer::stdout()
    ///     .map_writer(move |w| stderr.or_else(w));
    /// # tracing_subscriber::registry().with(layer);
    /// ```
    pub fn map_writer<W2>(self, f: impl FnOnce(W) -> W2) -> JsonLayer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        JsonLayer {
            make_writer: f(self.make_writer),
            log_internal_errors: self.log_internal_errors,
            keyed_values: self.keyed_values,
            flattened_values: self.flattened_values,
        }
    }

    /// Adds a new static field with a given key to the output.
    ///
    /// # Examples
    ///
    /// Print hostname in each log:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_static_field(
    ///     "hostname",
    ///     serde_json::Value::String(get_hostname().to_owned()),
    /// );
    /// # tracing_subscriber::registry().with(layer);
    /// # fn get_hostname() -> &'static str { "localhost" }
    /// ```
    pub fn add_static_field(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.keyed_values
            .insert(SchemaKey::from(key.into()), JsonValue::Serde(value));
    }

    /// Removes a field that was inserted to the output. This can only remove fields that have a
    /// static key, not keys added with
    /// [`add_multiple_dynamic_fields`](Self::add_multiple_dynamic_fields).
    ///
    /// # Examples
    ///
    /// Add a field and then remove it:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_static_field(
    ///     "deleteMe",
    ///     serde_json::json!("accident"),
    /// );
    /// layer.remove_field("deleteMe");
    ///
    /// # tracing_subscriber::registry().with(layer);
    /// ```
    pub fn remove_field(&mut self, key: impl Into<String>) {
        self.keyed_values.remove(&SchemaKey::from(key.into()));
    }

    pub(crate) fn remove_flattened_field(&mut self, key: &FlatSchemaKey) {
        self.flattened_values.remove(key);
    }

    /// Adds a new dynamic field with a given key to the output. This method is more general than
    /// [`add_static_field`](Self::add_static_field) but also more expensive.
    ///
    /// This method takes a closure argument that will be called with the event and tracing context.
    /// Through these, the parent span can be accessed among other things. This closure returns an
    /// `Option` where nothing will be added to the output if `None` is returned.
    ///
    /// # Examples
    ///
    /// Print an atomic counter.
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    /// # use std::sync::atomic::{AtomicU32, Ordering};
    /// static COUNTER: AtomicU32 = AtomicU32::new(42);
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_dynamic_field(
    ///     "counter",
    ///     |_event, _context| {
    ///         Some(serde_json::Value::Number(COUNTER.load(Ordering::Relaxed).into()))
    /// });
    /// # tracing_subscriber::registry().with(layer);
    /// ```
    pub fn add_dynamic_field<Fun, Res>(&mut self, key: impl Into<String>, mapper: Fun)
    where
        for<'a> Fun: Fn(&'a Event<'_>, &Context<'_, S>) -> Option<Res> + Send + Sync + 'a,
        Res: serde::Serialize,
    {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicFromEvent(Box::new(move |event| {
                serde_json::to_value(mapper(event.event(), event.context())?).ok()
            })),
        );
    }

    /// Adds multiple new dynamic fields where the keys may not be known when calling this method.
    ///
    /// This method takes a closure argument that will be called with the event and tracing context
    /// along with a [`FieldWriter`]. Call [`FieldWriter::write_field`] to write key-value pairs
    /// directly into the JSON output. Values accept any type implementing [`serde::Serialize`].
    ///
    /// It is the user's responsibility to make sure that no two keys clash as that would create an
    /// invalid JSON. It's generally better to use [`add_dynamic_field`](Self::add_dynamic_field)
    /// instead if the field names are known.
    ///
    /// # Examples
    ///
    /// Print a question or an answer:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_multiple_dynamic_fields(
    ///     |_event, _context, writer| {
    /// #       let condition = true;
    ///         if condition {
    ///             _ = writer.write_field("question", "What?");
    ///         } else {
    ///             _ = writer.write_field("answer", 42u64);
    ///         }
    ///     }
    /// );
    /// # tracing_subscriber::registry().with(layer);
    /// ```
    pub fn add_multiple_dynamic_fields<Fun>(&mut self, mapper: Fun)
    where
        Fun: Fn(&Event<'_>, &Context<'_, S>, &mut FieldWriter<'_>) + Send + Sync + 'static,
    {
        self.flattened_values.insert(
            FlatSchemaKey::new_uuid(),
            JsonValue::DynamicFromEventWithWriter(Box::new(move |event, writer| {
                mapper(event.event(), event.context(), writer);
            })),
        );
    }

    /// Adds a new dynamic field with a given key to the output. This method is a specialized
    /// version of [`add_dynamic_field`](Self::add_dynamic_field) where just a reference to the
    /// parent span is needed.
    ///
    /// This method takes a closure argument that will be called with the parent span context. This
    /// closure returns an `Option` where nothing will be added to the output if `None` is returned.
    ///
    /// # Examples
    ///
    /// Print uppercase target:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_from_span(
    ///     "TARGET",
    ///     |span| Some(span.metadata().target().to_uppercase())
    /// );
    /// # tracing_subscriber::registry().with(layer);
    /// ```
    pub fn add_from_span<Fun, Res>(&mut self, key: impl Into<String>, mapper: Fun)
    where
        for<'a> Fun: Fn(&'a SpanRef<'_, S>) -> Option<Res> + Send + Sync + 'a,
        Res: serde::Serialize,
    {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicFromSpan(Box::new(move |span| {
                serde_json::to_value(mapper(span)?).ok()
            })),
        );
    }

    /// Adds a field with a given key to the output. The value will be serialized JSON of the
    /// provided extension. Other [`Layer`]s may add these extensions to the span.
    ///
    /// The serialization happens every time a log line is emitted so if the extension changes, the
    /// latest version will be emitted.
    ///
    /// If the extension is not found, nothing is added to the output.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tracing::span::Attributes;
    /// # use tracing::Id;
    /// # use tracing::Subscriber;
    /// # use tracing_subscriber::registry;
    /// # use tracing_subscriber::registry::LookupSpan;
    /// # use tracing_subscriber::Layer;
    /// # use tracing_subscriber::layer::Context;
    /// # use tracing_subscriber::prelude::*;
    /// # use serde::Serialize;
    /// struct FooLayer;
    ///
    /// #[derive(Serialize)]
    /// struct Foo(String);
    ///
    /// impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for FooLayer {
    ///     fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
    ///         let span = ctx.span(id).unwrap();
    ///         let mut extensions = span.extensions_mut();
    ///         let foo = Foo("hello".to_owned());
    ///         extensions.insert(foo);
    ///     }
    /// }
    ///
    /// # fn main() {
    /// let foo_layer = FooLayer;
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.serialize_extension::<Foo>("foo");
    ///
    /// registry().with(foo_layer).with(layer);
    /// # }
    /// ```
    pub fn serialize_extension<Ext: Serialize + 'static>(&mut self, key: impl Into<String>) {
        self.add_from_extension_ref(key, |extension: &Ext| Some(extension));
    }

    /// Adds a field with a given key to the output. The user-provided closure can transform the
    /// extension and return reference to any serializable structure.
    ///
    /// The mapping and serialization happens every time a log line is emitted so if the extension
    /// changes, the latest version will be emitted.
    ///
    /// If the extension is not found, or the mapping returns `None`, nothing is added to the
    /// output.
    ///
    /// Use [`Self::add_from_extension`] if you cannot return a reference.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tracing::span::Attributes;
    /// # use tracing::Id;
    /// # use tracing::Subscriber;
    /// # use tracing_subscriber::registry;
    /// # use tracing_subscriber::registry::LookupSpan;
    /// # use tracing_subscriber::Layer;
    /// # use tracing_subscriber::layer::Context;
    /// # use tracing_subscriber::prelude::*;
    /// # use serde::Serialize;
    /// struct FooLayer;
    ///
    /// #[derive(Serialize)]
    /// struct Foo(String);
    ///
    /// impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for FooLayer {
    ///     fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
    ///         let span = ctx.span(id).unwrap();
    ///         let mut extensions = span.extensions_mut();
    ///         let foo = Foo("hello".to_owned());
    ///         extensions.insert(foo);
    ///     }
    /// }
    ///
    /// # fn main() {
    /// let foo_layer = FooLayer;
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_from_extension_ref::<Foo, _, _>("foo", |foo| Some(&foo.0));
    ///
    /// registry().with(foo_layer).with(layer);
    /// # }
    /// ```
    pub fn add_from_extension_ref<Ext, Fun, Res>(&mut self, key: impl Into<String>, mapper: Fun)
    where
        Ext: 'static,
        for<'a> Fun: Fn(&'a Ext) -> Option<&'a Res> + Send + Sync + 'a,
        Res: serde::Serialize,
    {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicFromSpan(Box::new(move |span| {
                let extensions = span.extensions();
                let extension = extensions.get::<Ext>()?;
                serde_json::to_value(mapper(extension)).ok()
            })),
        );
    }

    /// Adds a field with a given key to the output. The user-provided closure can transform the
    /// extension and return any serializable structure.
    ///
    /// The mapping and serialization happens every time a log line is emitted so if the extension
    /// changes, the latest version will be emitted.
    ///
    /// If the extension is not found, or the mapping returns `None`, nothing is added to the
    /// output.
    ///
    /// Use [`Self::add_from_extension_ref`] if you want to return a reference to data in the
    /// extension.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use tracing::span::Attributes;
    /// # use tracing::Id;
    /// # use tracing::Subscriber;
    /// # use tracing_subscriber::registry;
    /// # use tracing_subscriber::registry::LookupSpan;
    /// # use tracing_subscriber::Layer;
    /// # use tracing_subscriber::layer::Context;
    /// # use tracing_subscriber::prelude::*;
    /// # use serde::Serialize;
    /// struct FooLayer;
    ///
    /// #[derive(Serialize)]
    /// struct Foo(String);
    ///
    /// impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for FooLayer {
    ///     fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
    ///         let span = ctx.span(id).unwrap();
    ///         let mut extensions = span.extensions_mut();
    ///         let foo = Foo("hello".to_owned());
    ///         extensions.insert(foo);
    ///     }
    /// }
    ///
    /// # fn main() {
    /// let foo_layer = FooLayer;
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_from_extension::<Foo, _, _>("foo", |foo| foo.0.parse::<u64>().ok());
    ///
    /// registry().with(foo_layer).with(layer);
    /// # }
    /// ```
    pub fn add_from_extension<Ext, Fun, Res>(&mut self, key: impl Into<String>, mapper: Fun)
    where
        Ext: 'static,
        for<'a> Fun: Fn(&'a Ext) -> Option<Res> + Send + Sync + 'a,
        Res: serde::Serialize,
    {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicFromSpan(Box::new(move |span| {
                let extensions = span.extensions();
                let extension = extensions.get::<Ext>()?;
                serde_json::to_value(mapper(extension)).ok()
            })),
        );
    }

    /// Print all event fields in an object with the key as specified.
    pub fn with_event(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer: &mut Vec<u8>| {
                // With the `Vec<u8>` buffer, `serde_json` writes straight into the buffer
                // (no `Value` tree, no `Cursor`, no per-token UTF-8 check), so this is both
                // the cleanest and the fastest shape. Key order within the `fields` object
                // is declaration order (visit order), not alphabetical.
                serde_json::to_writer(writer, &event.event().field_map()).map_err(|_| fmt::Error)
            })),
        );
        self
    }

    /// Print all current span fields, each as its own top level member of the JSON.
    ///
    /// It is the user's responsibility to make sure that the field names will not clash with other
    /// defined members of the output JSON. If they clash, invalid JSON with multiple fields with
    /// the same key may be generated.
    ///
    /// It's therefore preferable to use [`with_current_span`](Self::with_current_span) instead.
    pub fn with_top_level_flattened_current_span(&mut self) -> &mut Self {
        self.flattened_values.insert(
            FlatSchemaKey::FlattenedCurrentSpan,
            JsonValue::DynamicCachedFromSpan(Box::new(move |span| {
                span.extensions()
                    .get::<JsonFields>()
                    .map(|fields| Cached::Raw(fields.serialized.clone()))
            })),
        );
        self
    }

    /// Print all parent spans' fields, each as its own top level member of the JSON.
    ///
    /// If multiple spans define the same field, the one furthest from the root span will be kept.
    ///
    /// It is the user's responsibility to make sure that the field names will not clash with other
    /// defined members of the output JSON. If they clash, invalid JSON with multiple fields with
    /// the same key may be generated.
    ///
    /// It's therefore preferable to use [`with_span_list`](Self::with_span_list) instead.
    pub fn with_top_level_flattened_span_list(&mut self) -> &mut Self {
        self.flattened_values.insert(
            FlatSchemaKey::FlattenedSpanList,
            JsonValue::DynamicFromSpan(Box::new(|span| {
                let fields =
                    span.scope()
                        .from_root()
                        .fold(BTreeMap::new(), |mut accumulator, span| {
                            let extensions = span.extensions();
                            let Some(fields) = extensions.get::<JsonFields>() else {
                                return accumulator;
                            };
                            accumulator.extend(
                                fields
                                    .inner
                                    .fields
                                    .iter()
                                    .map(|(key, value)| (*key, value.clone())),
                            );
                            accumulator
                        });

                serde_json::to_value(fields).ok()
            })),
        );
        self
    }

    /// Print all event fields, each as its own top level member of the JSON.
    ///
    /// It is the user's responsibility to make sure that the field names will not clash with other
    /// defined members of the output JSON. If they clash, invalid JSON with multiple fields with
    /// the same key may be generated.
    ///
    /// It's therefore preferable to use [`with_event`](Self::with_event) instead.
    pub fn with_flattened_event(&mut self) -> &mut Self {
        self.flattened_values.insert(
            FlatSchemaKey::FlattenedEvent,
            JsonValue::DynamicFromEvent(Box::new(move |event| {
                serde_json::to_value(event.field_map()).ok()
            })),
        );
        self
    }

    /// Print all event fields, each as its own top level member of the JSON. This also allows the
    /// fields to have different keys than the field names used in tracing.
    ///
    /// Only field names can be renamed this way, not other fields such as `timestamp` or `target`
    /// which usually take the key as their parameter (such as `Self::with_target`).
    ///
    /// The renames are realized by a provided function that will be passed the original field name
    /// and a user-defined context and it must produce the new name.
    ///
    /// For example when simple static renames are needed the following can work:
    ///
    /// ```rust
    /// # use std::collections::HashMap;
    /// # use tracing_subscriber::prelude::*;
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    ///
    /// let renames = HashMap::from([
    ///     ("message".to_owned(), "msg".to_owned()),
    ///     ("foo".to_owned(), "bar".to_owned()),
    /// ]);
    /// layer.with_flattened_event_with_renames(
    ///     move |name, map| map.get(name).map_or(name, String::as_str),
    ///     renames,
    /// );
    /// # tracing_subscriber::registry().with(layer);
    ///
    /// // This will produce something like `{"bar":3,"msg":"x",...}`
    /// tracing::info!(foo = 3, "x");
    /// ```
    ///
    /// It is the user's responsibility to make sure that the field names will not clash with other
    /// defined members of the output JSON. If they clash, invalid JSON with multiple fields with
    /// the same key may be generated.
    pub fn with_flattened_event_with_renames<F, T>(&mut self, renames: F, context: T) -> &mut Self
    where
        F: for<'a> Fn(&'a str, &'a T) -> &'a str + Send + Sync + 'static + Clone,
        T: Clone + Send + Sync + 'static,
    {
        self.flattened_values.insert(
            FlatSchemaKey::FlattenedEvent,
            JsonValue::DynamicFromEvent(Box::new(move |event| {
                serde_json::to_value(RenamedFields::new(event.event(), renames.clone(), &context))
                    .ok()
            })),
        );
        self
    }

    /// Sets whether or not the log line will include the current span in formatted events.
    pub fn with_current_span(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicCachedFromSpan(Box::new(move |span| {
                span.extensions()
                    .get::<JsonFields>()
                    .map(|fields| Cached::Raw(fields.serialized.clone()))
            })),
        );
        self
    }

    /// Sets whether or not the formatter will include a list (from root to leaf) of all currently
    /// entered spans in formatted events.
    pub fn with_span_list(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer: &mut Vec<u8>| {
                // Iterate the parent span's scope root→leaf and write each cached
                // serialized span directly into the buffer as a JSON array. Byte-identical
                // to the previous `Cached::Array(Vec<Arc<str>>)` path with no per-event
                // `Vec` allocation.
                let Some(parent) = event.parent_span() else {
                    return Ok(());
                };
                writer.push(b'[');
                let mut first = true;
                for ancestor in parent.scope().from_root() {
                    let extensions = ancestor.extensions();
                    let Some(fields) = extensions.get::<JsonFields>() else {
                        continue;
                    };
                    if !first {
                        writer.push(b',');
                    }
                    first = false;
                    writer.extend_from_slice(fields.serialized.as_bytes());
                }
                writer.push(b']');
                Ok(())
            })),
        );
        self
    }

    /// Sets the formatter to include an object containing all parent spans' fields. If multiple
    /// ancestor spans recorded the same field, the span closer to the leaf span overrides the
    /// values of spans that are closer to the root spans.
    pub fn with_flattened_span_fields(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicFromSpan(Box::new(|span| {
                let fields =
                    span.scope()
                        .from_root()
                        .fold(BTreeMap::new(), |mut accumulator, span| {
                            let extensions = span.extensions();
                            let Some(fields) = extensions.get::<JsonFields>() else {
                                return accumulator;
                            };
                            accumulator.extend(
                                fields
                                    .inner
                                    .fields
                                    .iter()
                                    .map(|(key, value)| (*key, value.clone())),
                            );
                            accumulator
                        });

                serde_json::to_value(fields).ok()
            })),
        );
        self
    }

    /// Use the given [`timer`] for log message timestamps with the `timestamp` key.
    ///
    /// See the [`time` module] for the provided timer implementations.
    ///
    /// [`timer`]: tracing_subscriber::fmt::time::FormatTime
    /// [`time` module]: mod@tracing_subscriber::fmt::time
    pub fn with_timer<T: FormatTime + Send + Sync + 'static>(
        &mut self,
        key: impl Into<String>,
        timer: T,
    ) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(move |_event, writer: &mut Vec<u8>| {
                // Write the quoted timestamp straight into the buffer while escaping any
                // string characters emitted by custom timers.
                writer.push(b'"');
                {
                    let mut writer = EscapedFmtWrite(&mut *writer);
                    timer.format_time(&mut Writer::new(&mut writer))?;
                }
                writer.push(b'"');
                Ok(())
            })),
        );
        self
    }

    /// Sets whether or not an event's target is displayed. It will use the `target` key if so.
    pub fn with_target(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                write_escaped(writer, event.metadata().target())
            })),
        );

        self
    }

    /// Sets whether or not an event's [source code file path][file] is displayed. It will use the
    /// `file` key if so.
    ///
    /// [file]: tracing_core::Metadata::file
    pub fn with_file(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer: &mut Vec<u8>| {
                if let Some(file) = event.metadata().file() {
                    write_escaped(writer, file)
                } else {
                    write_null(writer);
                    Ok(())
                }
            })),
        );
        self
    }

    /// Sets whether or not an event's [source code line number][line] is displayed. It will use the
    /// `line_number` key if so.
    ///
    /// [line]: tracing_core::Metadata::line
    pub fn with_line_number(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer: &mut Vec<u8>| {
                if let Some(line) = event.metadata().line() {
                    write!(FmtWrite(&mut *writer), "{line}")
                } else {
                    write_null(writer);
                    Ok(())
                }
            })),
        );
        self
    }

    /// Sets whether or not an event's level is displayed. It will use the `level` key if so.
    pub fn with_level(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                write_escaped(writer, event.metadata().level().as_str())
            })),
        );
        self
    }

    /// Sets whether or not the [name] of the current thread is displayed when formatting events. It
    /// will use the `threadName` key if so.
    ///
    /// [name]: std::thread#naming-threads
    pub fn with_thread_names(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|_event, writer: &mut Vec<u8>| {
                if let Some(name) = std::thread::current().name() {
                    write_escaped(writer, name)
                } else {
                    write_null(writer);
                    Ok(())
                }
            })),
        );
        self
    }

    /// Sets whether or not the [thread ID] of the current thread is displayed when formatting
    /// events. It will use the `threadId` key if so.
    ///
    /// [thread ID]: std::thread::ThreadId
    pub fn with_thread_ids(&mut self, key: impl Into<String>) -> &mut Self {
        self.keyed_values.insert(
            SchemaKey::from(key.into()),
            JsonValue::DynamicRawFromEvent(Box::new(|_event, writer: &mut Vec<u8>| {
                writer.push(b'"');
                write!(EscapedFmtWrite(writer), "{:?}", std::thread::current().id())?;
                writer.push(b'"');
                Ok(())
            })),
        );

        self
    }

    /// Sets whether or not [OpenTelemetry] trace ID and span ID is displayed when formatting
    /// events. It will use the `openTelemetry` key if so and the value will be an object with
    /// `traceId` and `spanId` fields, each being a string.
    ///
    /// This works only if your `tracing-opentelemetry` version and this crate's features match. If
    /// you update that dependency, you need to change the feature here or this call will do
    /// nothing.
    ///
    /// [OpenTelemetry]: https://opentelemetry.io
    #[cfg(any(
        feature = "opentelemetry",
        feature = "tracing-opentelemetry-0-28",
        feature = "tracing-opentelemetry-0-29",
        feature = "tracing-opentelemetry-0-30",
        feature = "tracing-opentelemetry-0-31",
        feature = "tracing-opentelemetry-0-32",
    ))]
    #[cfg_attr(
        docsrs,
        doc(any(
            feature = "opentelemetry",
            feature = "tracing-opentelemetry-0-28",
            feature = "tracing-opentelemetry-0-29",
            feature = "tracing-opentelemetry-0-30",
            feature = "tracing-opentelemetry-0-31",
            feature = "tracing-opentelemetry-0-32",
        ))
    )]
    pub fn with_opentelemetry_ids(&mut self, display_opentelemetry_ids: bool) -> &mut Self {
        if display_opentelemetry_ids {
            self.keyed_values.insert(
                SchemaKey::from("openTelemetry"),
                JsonValue::DynamicRawFromEvent(Box::new(|event, writer: &mut Vec<u8>| {
                    let Some(span) = event.parent_span() else {
                        return Ok(());
                    };

                    // On the first feature that yields a valid span context, write
                    // `{"spanId":"<hex>","traceId":"<hex>"}` straight into the buffer. Both
                    // `TraceId` and `SpanId` implement `Display` as lowercase zero-padded hex,
                    // so this skips the `Value::Object` + `BTreeMap` + per-id `String`
                    // allocations of the previous path. Key order matches the previous
                    // `BTreeMap`-sorted output (`spanId` before `traceId`).
                    macro_rules! write_ids {
                        ($trace_id:expr, $span_id:expr) => {{
                            return write!(
                                FmtWrite(&mut *writer),
                                "{{\"spanId\":\"{}\",\"traceId\":\"{}\"}}",
                                $span_id,
                                $trace_id,
                            );
                        }};
                    }

                    macro_rules! otel_extraction {
                        ($feature:literal, $tracing_otel_crate:ident, $otel_crate:ident) => {
                            #[cfg(feature = $feature)]
                            {
                                use $otel_crate::trace::{TraceContextExt, TraceId};
                                if let Some(otel_data) =
                                    span.extensions().get::<$tracing_otel_crate::OtelData>()
                                {
                                    // Prefer the parent's trace id: it is possible to create a
                                    // new trace and then change the parent, in which case the
                                    // value in the builder is stale.
                                    let parent_trace_id =
                                        otel_data.parent_cx.span().span_context().trace_id();
                                    let trace_id = if parent_trace_id == TraceId::INVALID {
                                        otel_data.builder.trace_id
                                    } else {
                                        Some(parent_trace_id)
                                    };
                                    if let (Some(trace_id), Some(span_id)) =
                                        (trace_id, otel_data.builder.span_id)
                                    {
                                        write_ids!(trace_id, span_id);
                                    }
                                }
                            }
                        };
                    }

                    #[cfg(feature = "tracing-opentelemetry-0-32")]
                    {
                        if let Some(otel_data) = span
                            .extensions()
                            .get::<tracing_opentelemetry_0_32::OtelData>()
                        {
                            if let (Some(trace_id), Some(span_id)) =
                                (otel_data.trace_id(), otel_data.span_id())
                            {
                                write_ids!(trace_id, span_id);
                            }
                        }
                    }
                    otel_extraction!(
                        "tracing-opentelemetry-0-31",
                        tracing_opentelemetry_0_31,
                        opentelemetry_0_30
                    );
                    otel_extraction!(
                        "tracing-opentelemetry-0-30",
                        tracing_opentelemetry_0_30,
                        opentelemetry_0_29
                    );
                    otel_extraction!(
                        "tracing-opentelemetry-0-29",
                        tracing_opentelemetry_0_29,
                        opentelemetry_0_28
                    );
                    otel_extraction!(
                        "tracing-opentelemetry-0-28",
                        tracing_opentelemetry_0_28,
                        opentelemetry_0_27
                    );
                    otel_extraction!(
                        "opentelemetry",
                        tracing_opentelemetry_0_25,
                        opentelemetry_0_24
                    );

                    Ok(())
                })),
            );
        } else {
            self.keyed_values.remove(&SchemaKey::from("openTelemetry"));
        }

        self
    }
}

fn write_null(writer: &mut Vec<u8>) {
    writer.extend_from_slice(b"null");
}

struct EscapedFmtWrite<'a>(&'a mut Vec<u8>);

impl fmt::Write for EscapedFmtWrite<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        write_escaped_contents(self.0, value)
    }
}

fn write_escaped(writer: &mut Vec<u8>, value: &str) -> fmt::Result {
    writer.push(b'"');
    write_escaped_contents(writer, value)?;
    writer.push(b'"');
    Ok(())
}

fn write_escaped_contents(writer: &mut Vec<u8>, value: &str) -> fmt::Result {
    let mut start = 0;
    for (index, character) in value.char_indices() {
        let escaped = match character {
            '"' => Some(b"\\\"".as_slice()),
            '\\' => Some(b"\\\\".as_slice()),
            '\n' => Some(b"\\n".as_slice()),
            '\r' => Some(b"\\r".as_slice()),
            '\t' => Some(b"\\t".as_slice()),
            '\u{08}' => Some(b"\\b".as_slice()),
            '\u{0c}' => Some(b"\\f".as_slice()),
            '\u{00}'..='\u{1f}' => {
                writer.extend_from_slice(&value.as_bytes()[start..index]);
                write!(FmtWrite(writer), "\\u{:04x}", character as u32)?;
                start = index + character.len_utf8();
                None
            },
            _ => None,
        };

        if let Some(escaped) = escaped {
            writer.extend_from_slice(&value.as_bytes()[start..index]);
            writer.extend_from_slice(escaped);
            start = index + character.len_utf8();
        }
    }
    writer.extend_from_slice(&value.as_bytes()[start..]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fmt};

    use serde_json::json;
    use tracing::subscriber::with_default;
    use tracing_subscriber::{
        fmt::{format::Writer, time::FormatTime},
        registry, Layer, Registry,
    };

    use super::JsonLayer;
    use crate::tests::MockMakeWriter;

    fn test_json<W, T>(
        expected: &serde_json::Value,
        layer: JsonLayer<Registry, W>,
        producer: impl FnOnce() -> T,
    ) {
        let actual = produce_log_line(layer, producer);
        assert_eq!(
            expected,
            &serde_json::from_str::<serde_json::Value>(&actual).unwrap(),
        );
    }

    fn produce_log_line<W, T>(
        layer: JsonLayer<Registry, W>,
        producer: impl FnOnce() -> T,
    ) -> String {
        let make_writer = MockMakeWriter::default();
        let collector = layer
            .with_writer(make_writer.clone())
            .with_subscriber(registry());

        with_default(collector, producer);

        let buf = make_writer.buf();
        dbg!(std::str::from_utf8(&buf[..]).unwrap()).to_owned()
    }

    #[test]
    fn add_and_remove_static() {
        let mut layer = JsonLayer::stdout();
        layer.add_static_field("static", json!({"lorem": "ipsum", "answer": 42}));
        layer.add_static_field(String::from("zero"), json!(0));
        layer.add_static_field(String::from("one").as_str(), json!(1));
        layer.add_static_field("nonExistent", json!(1));
        layer.remove_field("nonExistent");

        let expected = json!({
            "static": {
                "lorem": "ipsum",
                "answer": 42,
            },
            "zero": 0,
            "one": 1,
        });

        test_json(&expected, layer, || {
            tracing::info!(does = "not matter", "whatever");
        });
    }

    #[test]
    fn flattened_event_with_renames() {
        let renames = HashMap::from([
            ("message".to_owned(), "msg".to_owned()),
            ("msg".to_owned(), "message".to_owned()),
            ("same".to_owned(), "same".to_owned()),
            ("different".to_owned(), "gone".to_owned()),
        ]);
        let mut layer = JsonLayer::stdout();
        layer.with_flattened_event_with_renames(
            move |name, map| map.get(name).map_or(name, String::as_str),
            renames,
        );

        let expected = json!({
            "message": "msg",
            "msg": "message",
            "same": "same",
            "gone": "different",
            "another": "another",
        });

        test_json(&expected, layer, || {
            tracing::info!(
                msg = "msg",
                same = "same",
                different = "different",
                another = "another",
                "message"
            );
        });
    }

    #[test]
    fn raw_event_key_is_json_escaped() {
        let mut layer = JsonLayer::stdout();
        layer.with_event("fields\"\\key");

        let expected = json!({
            "fields\"\\key": {
                "answer": 42,
            },
        });

        test_json(&expected, layer, || {
            tracing::info!(answer = 42);
        });
    }

    struct EscapingTimer;

    impl FormatTime for EscapingTimer {
        fn format_time(&self, writer: &mut Writer<'_>) -> fmt::Result {
            writeln!(writer, "bad\\\"time")
        }
    }

    #[test]
    fn timer_output_is_json_escaped() {
        let mut layer = JsonLayer::stdout();
        layer.with_timer("timestamp", EscapingTimer);

        let expected = json!({
            "timestamp": "bad\\\"time\n",
        });

        test_json(&expected, layer, || {
            tracing::info!("whatever");
        });
    }

    #[test]
    fn thread_names_escape_json_control_characters() {
        let actual = std::thread::Builder::new()
            .name("worker\n1".to_owned())
            .spawn(|| {
                let mut layer = JsonLayer::stdout();
                layer.with_thread_names("threadName");

                produce_log_line(layer, || {
                    tracing::info!("whatever");
                })
            })
            .unwrap()
            .join()
            .unwrap();

        let actual = serde_json::from_str::<serde_json::Value>(&actual).unwrap();
        assert_eq!(json!({ "threadName": "worker\n1" }), actual);
    }

    #[cfg(all(
        feature = "tracing-opentelemetry-0-31",
        feature = "tracing-opentelemetry-0-30"
    ))]
    mod opentelemetry_tests {
        use tracing::{span::Attributes, Id, Subscriber};
        use tracing_subscriber::{layer::Context, prelude::*, registry::LookupSpan};

        use super::*;

        struct MixedOtelDataLayer;

        impl<S> Layer<S> for MixedOtelDataLayer
        where
            S: Subscriber + for<'lookup> LookupSpan<'lookup>,
        {
            fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
                let span = ctx.span(id).unwrap();
                let mut extensions = span.extensions_mut();

                extensions.insert(tracing_opentelemetry_0_31::OtelData {
                    parent_cx: opentelemetry_0_30::Context::new(),
                    builder: opentelemetry_0_30::trace::SpanBuilder::from_name("incomplete"),
                });
                extensions.insert(tracing_opentelemetry_0_30::OtelData {
                    parent_cx: opentelemetry_0_29::Context::new(),
                    builder: opentelemetry_0_29::trace::SpanBuilder::from_name("complete")
                        .with_trace_id(opentelemetry_0_29::trace::TraceId::from_bytes([
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
                        ]))
                        .with_span_id(opentelemetry_0_29::trace::SpanId::from_bytes([
                            0, 0, 0, 0, 0, 0, 0, 2,
                        ])),
                });
            }
        }

        #[test]
        fn opentelemetry_ids_fall_back_after_incomplete_newer_data() {
            let mut layer = JsonLayer::stdout();
            layer.with_opentelemetry_ids(true);
            let make_writer = MockMakeWriter::default();
            let collector = registry()
                .with(MixedOtelDataLayer)
                .with(layer.with_writer(make_writer.clone()));

            with_default(collector, || {
                let span = tracing::info_span!("span");
                let _entered = span.enter();
                tracing::info!("whatever");
            });

            let buf = make_writer.buf();
            let actual = serde_json::from_slice::<serde_json::Value>(&buf).unwrap();
            assert_eq!(
                json!({
                    "openTelemetry": {
                        "spanId": "0000000000000002",
                        "traceId": "00000000000000000000000000000001",
                    },
                }),
                actual,
            );
        }
    }
}
