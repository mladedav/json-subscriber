use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt,
    io,
    sync::Arc,
};

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

mod event;

use event::EventRef;
use uuid::Uuid;

use crate::{
    cached::Cached,
    fields::{JsonFields, JsonFieldsInner},
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
        Box<dyn Fn(&EventRef<'_, '_, '_, S>, &mut dyn fmt::Write) -> fmt::Result + Send + Sync>,
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
            static BUF: RefCell<String> = const { RefCell::new(String::new()) };
        }

        BUF.with(|buf| {
            let borrow = buf.try_borrow_mut();
            let mut a;
            let mut b;
            let buf = if let Ok(buf) = borrow {
                a = buf;
                &mut *a
            } else {
                b = String::new();
                &mut b
            };

            if self.format_event(&ctx, buf, event).is_ok() {
                let mut writer = self.make_writer.make_writer_for(event.metadata());
                let res = io::Write::write_all(&mut writer, buf.as_bytes());
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

    /// Adds multiple new dynamic field where the keys may not be known when calling this method.
    ///
    /// This method takes a closure argument that will be called with the event and tracing context.
    /// Through these, the parent span can be accessed among other things. This closure returns a
    /// value which can be iterated over to return a tuple of a `String` which will be used as a
    /// JSON key and a `serde_json::Value` which will be used as a value. In most cases returning
    /// `HashMap<String, serde_json::Value>` should be sufficient.
    ///
    /// It is the user's responsibility to make sure that no two keys clash as that would create an
    /// invalid JSON. It's generally better to use [`add_dynamic_field`](Self::add_dynamic_field)
    /// instead if the field names are known.
    ///
    /// # Examples
    ///
    /// Print either a question or an answer:
    ///
    /// ```rust
    /// # use tracing_subscriber::prelude::*;
    ///
    /// let mut layer = json_subscriber::JsonLayer::stdout();
    /// layer.add_multiple_dynamic_fields(
    ///     |_event, _context| {
    /// #       let condition = true;
    ///         if condition {
    ///             [("question".to_owned(), serde_json::Value::String("What?".to_owned()))]
    ///         } else {
    ///             [("answer".to_owned(), serde_json::Value::Number(42.into()))]
    ///         }
    /// });
    /// # tracing_subscriber::registry().with(layer);
    /// # fn get_hostname() -> &'static str { "localhost" }
    /// ```
    pub fn add_multiple_dynamic_fields<Fun, Res>(&mut self, mapper: Fun)
    where
        for<'a> Fun: Fn(&'a Event<'_>, &Context<'_, S>) -> Res + Send + Sync + 'a,
        Res: IntoIterator<Item = (String, serde_json::Value)>,
    {
        self.flattened_values.insert(
            FlatSchemaKey::new_uuid(),
            JsonValue::DynamicFromEvent(Box::new(move |event| {
                serde_json::to_value(
                    mapper(event.event(), event.context())
                        .into_iter()
                        .collect::<HashMap<_, _>>(),
                )
                .ok()
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
            JsonValue::DynamicFromEvent(Box::new(move |event| {
                serde_json::to_value(event.field_map()).ok()
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
    /// It's therefore preferable to use [`with_event`](Self::with_event) instead.
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
    /// It's therefore preferable to use [`with_event`](Self::with_event) instead.
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
            JsonValue::DynamicCachedFromSpan(Box::new(|span| {
                Some(Cached::Array(
                    span.scope()
                        .from_root()
                        .filter_map(|span| {
                            span.extensions()
                                .get::<JsonFields>()
                                .map(|fields| fields.serialized.clone())
                        })
                        .collect::<Vec<_>>(),
                ))
            })),
        );
        self
    }

    /// Sets the formatter to include an object containing all parent spans' fields. If multiple
    /// ancestor spans recorded the same field, the span closer to the leaf span overrides the
    /// values of spans that are closer to the root spans.
    pub(crate) fn with_flattened_span_fields(&mut self, key: impl Into<String>) -> &mut Self {
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
            JsonValue::DynamicFromEvent(Box::new(move |_| {
                let mut timestamp = String::with_capacity(32);
                timer.format_time(&mut Writer::new(&mut timestamp)).ok()?;
                Some(timestamp.into())
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
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                event
                    .metadata()
                    .file()
                    .map_or(Ok(()), |file| write_escaped(writer, file))
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
            JsonValue::DynamicRawFromEvent(Box::new(|event, writer| {
                event
                    .metadata()
                    .line()
                    .map_or(Ok(()), |line| write!(writer, "{line}"))
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
            JsonValue::DynamicRawFromEvent(Box::new(|_event, writer| {
                std::thread::current()
                    .name()
                    .map_or(Ok(()), |name| write_escaped(writer, name))
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
            JsonValue::DynamicRawFromEvent(Box::new(|_event, writer| {
                use std::fmt::Write;
                let mut value = String::with_capacity(12);
                write!(&mut value, "{:?}", std::thread::current().id())?;
                write_escaped(writer, &value)
            })),
        );

        self
    }

    /// Sets whether or not [OpenTelemetry] trace ID and span ID is displayed when formatting
    /// events. It will use the `openTelemetry` key if so and the value will be an object with
    /// `traceId` and `spanId` fields, each being a string.
    ///
    /// [OpenTelemetry]: https://opentelemetry.io
    #[cfg(feature = "opentelemetry")]
    #[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
    pub fn with_opentelemetry_ids(&mut self, display_opentelemetry_ids: bool) -> &mut Self {
        use opentelemetry::trace::{TraceContextExt, TraceId};
        use tracing_opentelemetry::OtelData;

        if display_opentelemetry_ids {
            self.keyed_values.insert(
                SchemaKey::from("openTelemetry"),
                JsonValue::DynamicFromSpan(Box::new(|span| {
                    span.extensions().get::<OtelData>().and_then(|otel_data| {
                        // We should use the parent first if available because we can create a
                        // new trace and then change the parent. In that case the value in the
                        // builder is not updated.
                        let mut trace_id = otel_data.parent_cx.span().span_context().trace_id();
                        if trace_id == TraceId::INVALID {
                            trace_id = otel_data.builder.trace_id?;
                        }
                        let span_id = otel_data.builder.span_id?;

                        Some(serde_json::json!({
                            "traceId": trace_id.to_string(),
                            "spanId": span_id.to_string(),
                        }))
                    })
                })),
            );
        } else {
            self.keyed_values.remove(&SchemaKey::from("openTelemetry"));
        }

        self
    }
}

fn write_escaped(writer: &mut dyn fmt::Write, value: &str) -> Result<(), fmt::Error> {
    let mut rest = value;
    writer.write_str("\"")?;
    let mut shift = 0;
    while let Some(position) = rest
        .get(shift..)
        .and_then(|haystack| haystack.find(['\"', '\\']))
    {
        let (before, after) = rest.split_at(position + shift);
        writer.write_str(before)?;
        writer.write_char('\\')?;
        rest = after;
        shift = 1;
    }
    writer.write_str(rest)?;
    writer.write_str("\"")
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tracing::subscriber::with_default;
    use tracing_subscriber::{registry, Layer, Registry};

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
}
