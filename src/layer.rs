use std::{borrow::Cow, cell::RefCell, collections::BTreeMap, io};

use tracing::Metadata;
use tracing_core::{
    span::{Attributes, Id, Record},
    Event, Subscriber,
};
use tracing_serde::fields::AsMap;
use tracing_subscriber::{
    fmt::{
        format,
        time::{FormatTime, SystemTime},
        MakeWriter, TestWriter,
    },
    layer::SubscriberExt,
    registry::Extensions,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

use crate::visitor::JsonVisitor;

#[derive(Default, Debug)]
pub struct JsonFields {
    pub(crate) fields: BTreeMap<&'static str, serde_json::Value>,
    pub(crate) unformatted_fields: bool,
}

impl serde::Serialize for JsonFields {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut serializer = serializer.serialize_map(Some(self.fields.len()))?;

        for (key, value) in &self.fields {
            serializer.serialize_entry(key, value)?;
        }

        serializer.end()
    }
}

impl JsonFields {
    pub fn fields(&self) -> &BTreeMap<&'static str, serde_json::Value> {
        &self.fields
    }
}

pub struct JsonLayer<W = fn() -> io::Stdout, T = SystemTime> {
    pub(crate) make_writer: W,
    pub(crate) timer: T,

    pub(crate) log_internal_errors: bool,

    pub(crate) display_timestamp: bool,
    pub(crate) display_level: bool,
    pub(crate) display_line_number: bool,
    pub(crate) display_span_list: bool,

    pub(crate) schema: BTreeMap<SchemaKey, JsonValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SchemaKey {
    Static(Cow<'static, str>),
    // TODO this doesn't work because we'd have just a single flatten field
    Flatten,
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

pub enum JsonValue {
    Serde(serde_json::Value),
    Struct(BTreeMap<&'static str, JsonValue>),
    Array(Vec<JsonValue>),
    #[allow(clippy::type_complexity)]
    Dynamic(
        Box<dyn Fn(&Event<'_>, Option<&Extensions<'_>>) -> Option<serde_json::Value> + Send + Sync>,
    ),
}

impl Default for JsonLayer {
    fn default() -> Self {
        let this = Self {
            make_writer: io::stdout,
            timer: SystemTime,
            log_internal_errors: false,
            schema: BTreeMap::new(),

            display_timestamp: true,
            display_level: true,
            display_line_number: false,
            display_span_list: true,
        };

        this.with_target(true)
            .with_current_span(true)
            .flatten_event(false)
    }
}

impl<S, W, T> Layer<S> for JsonLayer<W, T>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    W: for<'writer> MakeWriter<'writer> + 'static,
    T: FormatTime + 'static,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            if self.log_internal_errors {
                eprintln!("[tracing-json] Span not found, this is a bug.");
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
            extensions.insert(fields);
        } else if self.log_internal_errors {
            eprintln!(
                "[tracing-json] Unable to format the following event, ignoring: {:?}",
                attrs
            );
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            if self.log_internal_errors {
                eprintln!("[tracing-json] Span not found, this is a bug.");
            }
            return;
        };

        let mut extensions = span.extensions_mut();
        let Some(fields) = extensions.get_mut::<JsonFields>() else {
            if self.log_internal_errors {
                eprintln!("[tracing-json] Span was created but does not contain formatted fields, this is a bug and some fields may have been lost.");
            }
            return;
        };

        values.record(&mut JsonVisitor::new(fields));
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
            let mut buf = match borrow {
                Ok(buf) => {
                    a = buf;
                    &mut *a
                }
                _ => {
                    b = String::new();
                    &mut b
                }
            };

            if self.format_event(ctx, format::Writer::new(&mut buf), event)
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

impl<W, T> JsonLayer<W, T>
where
    W: for<'a> MakeWriter<'a> + 'static,
    T: FormatTime + 'static,
{
    pub fn finish(self) -> impl Subscriber + for<'a> LookupSpan<'a> {
        tracing_subscriber::registry().with(self)
    }
}

impl<W, T> JsonLayer<W, T> {
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
    pub fn with_writer<W2>(self, make_writer: W2) -> JsonLayer<W2, T>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        JsonLayer {
            make_writer,
            timer: self.timer,
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_line_number: self.display_line_number,
            display_span_list: self.display_span_list,
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
    /// # let _: &reload::Handle<fmt::JsonLayer<W, T> = &reload_handle;
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
    pub fn with_test_writer(self) -> JsonLayer<TestWriter, T> {
        JsonLayer {
            make_writer: TestWriter::default(),
            timer: self.timer,
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_line_number: self.display_line_number,
            display_span_list: self.display_span_list,
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
    pub fn log_internal_errors(self, log_internal_errors: bool) -> Self {
        Self {
            log_internal_errors,
            ..self
        }
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
    pub fn map_writer<W2>(self, f: impl FnOnce(W) -> W2) -> JsonLayer<W2, T>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        JsonLayer {
            make_writer: f(self.make_writer),
            timer: self.timer,
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_line_number: self.display_line_number,
            display_span_list: self.display_span_list,
        }
    }

    /// Sets the JSON subscriber being built to flatten event metadata.
    ///
    /// See [`format::Json`]
    pub fn flatten_event(mut self, flatten_event: bool) -> JsonLayer<W, T> {
        let fields = JsonValue::Dynamic(Box::new(|event, _| {
            serde_json::to_value(event.field_map()).ok()
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
    pub fn with_current_span(mut self, display_current_span: bool) -> JsonLayer<W, T> {
        if display_current_span {
            self.schema.insert(
                SchemaKey::from("span"),
                JsonValue::Dynamic(Box::new(|_, extensions| {
                    extensions
                        .and_then(|extensions| extensions.get::<JsonFields>())
                        .and_then(|fields| serde_json::to_value(fields).ok())
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
    pub fn with_span_list(self, display_span_list: bool) -> JsonLayer<W, T> {
        JsonLayer {
            display_span_list,
            ..self
        }
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
    pub fn with_timer<T2>(self, timer: T2) -> JsonLayer<W, T2> {
        JsonLayer {
            make_writer: self.make_writer,
            timer,
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_line_number: self.display_line_number,
            display_span_list: self.display_span_list,
        }
    }

    /// Do not emit timestamps with log messages.
    pub fn without_time(self) -> JsonLayer<W, ()> {
        JsonLayer {
            make_writer: self.make_writer,
            timer: (),
            log_internal_errors: self.log_internal_errors,
            schema: self.schema,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_line_number: self.display_line_number,
            display_span_list: self.display_span_list,
        }
    }

    // /// Configures how synthesized events are emitted at points in the [span
    // /// lifecycle][lifecycle].
    // ///
    // /// The following options are available:
    // ///
    // /// - `FmtSpan::NONE`: No events will be synthesized when spans are
    // ///    created, entered, exited, or closed. Data from spans will still be
    // ///    included as the context for formatted events. This is the default.
    // /// - `FmtSpan::NEW`: An event will be synthesized when spans are created.
    // /// - `FmtSpan::ENTER`: An event will be synthesized when spans are entered.
    // /// - `FmtSpan::EXIT`: An event will be synthesized when spans are exited.
    // /// - `FmtSpan::CLOSE`: An event will be synthesized when a span closes. If
    // ///    [timestamps are enabled][time] for this formatter, the generated
    // ///    event will contain fields with the span's _busy time_ (the total
    // ///    time for which it was entered) and _idle time_ (the total time that
    // ///    the span existed but was not entered).
    // /// - `FmtSpan::ACTIVE`: An event will be synthesized when spans are entered
    // ///    or exited.
    // /// - `FmtSpan::FULL`: Events will be synthesized whenever a span is
    // ///    created, entered, exited, or closed. If timestamps are enabled, the
    // ///    close event will contain the span's busy and idle time, as
    // ///    described above.
    // ///
    // /// The options can be enabled in any combination. For instance, the following
    // /// will synthesize events whenever spans are created and closed:
    // ///
    // /// ```rust
    // /// use tracing_subscriber::fmt::format::FmtSpan;
    // /// use tracing_subscriber::fmt;
    // ///
    // /// let subscriber = fmt()
    // ///     .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    // ///     .finish();
    // /// ```
    // ///
    // /// Note that the generated events will only be part of the log output by
    // /// this formatter; they will not be recorded by other `Collector`s or by
    // /// `Subscriber`s added to this subscriber.
    // ///
    // /// [lifecycle]: mod@tracing::span#the-span-lifecycle
    // /// [time]: JsonLayer::without_time()
    // pub fn with_span_events(self, kind: format::FmtSpan) -> Self {
    //     JsonLayer {
    //         inner: self.inner.with_span_events(kind),
    //         ..self
    //     }
    // }

    /// Sets whether or not an event's target is displayed.
    pub fn with_target(mut self, display_target: bool) -> JsonLayer<W, T> {
        if display_target {
            self.schema.insert(
                SchemaKey::from("target"),
                JsonValue::Dynamic(Box::new(|event, _| {
                    Some(serde_json::Value::String(
                        event.metadata().target().to_owned(),
                    ))
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
    pub fn with_file(mut self, display_filename: bool) -> JsonLayer<W, T> {
        if display_filename {
            self.schema.insert(
                SchemaKey::from("filename"),
                JsonValue::Dynamic(Box::new(|event, _| event.metadata().file().map(Into::into))),
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
    pub fn with_line_number(self, display_line_number: bool) -> JsonLayer<W, T> {
        JsonLayer {
            display_line_number,
            ..self
        }
    }

    /// Sets whether or not an event's level is displayed.
    pub fn with_level(self, display_level: bool) -> JsonLayer<W, T> {
        JsonLayer {
            display_level,
            ..self
        }
    }

    /// Sets whether or not the [name] of the current thread is displayed
    /// when formatting events.
    ///
    /// [name]: std::thread#naming-threads
    pub fn with_thread_names(mut self, display_thread_name: bool) -> JsonLayer<W, T> {
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
    pub fn with_thread_ids(mut self, display_thread_id: bool) -> JsonLayer<W, T> {
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

#[cfg(test)]
mod test {
    use crate::tests::MockMakeWriter;

    use super::*;
    use tracing_subscriber::fmt::format::Writer;
    use tracing_subscriber::fmt::{format::FmtSpan, time::FormatTime};

    use tracing::subscriber::with_default;

    use std::fmt;
    use std::path::Path;

    struct MockTime;
    impl FormatTime for MockTime {
        fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
            write!(w, "fake time")
        }
    }

    fn subscriber() -> JsonLayer {
        JsonLayer::default()
    }

    // TODO uncomment when `tracing` releases version where `&[u8]: Value`
    // #[test]
    // fn json() {
    //     let expected =
    //     "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3,\"slice\":[97,98,99]},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3,\"slice\":[97,98,99]}],\"target\":\"tracing_json::layer::test\",\"fields\":{\"message\":\"some json test\"}}\n";
    //     let collector = subscriber()
    //         .flatten_event(false)
    //         .with_current_span(true)
    //         .with_span_list(true);
    //     test_json(expected, collector, || {
    //         let span = tracing::span!(
    //             tracing::Level::INFO,
    //             "json_span",
    //             answer = 42,
    //             number = 3,
    //             slice = &b"abc"[..]
    //         );
    //         let _guard = span.enter();
    //         tracing::info!("some json test");
    //     });
    // }

    #[test]
    fn json_filename() {
        let current_path = Path::new("src")
            .join("layer.rs")
            .to_str()
            .expect("path must be valid unicode")
            // escape windows backslashes
            .replace('\\', "\\\\");
        let expected =
            &format!("{}{}{}",
                    "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"tracing_json::layer::test\",\"filename\":\"",
                    current_path,
                    "\",\"fields\":{\"message\":\"some json test\"}}\n");
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_file(true)
            .with_span_list(true);
        test_json(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_line_number() {
        let expected =
            "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"tracing_json::layer::test\",\"line_number\":42,\"fields\":{\"message\":\"some json test\"}}\n";
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_line_number(true)
            .with_span_list(true);
        test_json_with_line_number(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_flattened_event() {
        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"tracing_json::layer::test\",\"message\":\"some json test\"}\n";

        let collector = subscriber()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_disabled_current_span_event() {
        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"tracing_json::layer::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(false)
            .with_span_list(true);
        test_json(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_disabled_span_list_event() {
        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"target\":\"tracing_json::layer::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(false);
        test_json(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_nested_span() {
        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3},{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4}],\"target\":\"tracing_json::layer::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            let span = tracing::span!(
                tracing::Level::INFO,
                "nested_json_span",
                answer = 43,
                number = 4
            );
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_explicit_span() {
        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3},{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4}],\"target\":\"tracing_json::layer::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let span = tracing::span!(
                parent: &span,
                tracing::Level::INFO,
                "nested_json_span",
                answer = 43,
                number = 4
            );
            // No enter
            tracing::info!(parent: &span, "some json test");
        });
    }

    #[test]
    fn json_explicit_no_span() {
        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"target\":\"tracing_json::layer::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, collector, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            let span = tracing::span!(
                tracing::Level::INFO,
                "nested_json_span",
                answer = 43,
                number = 4
            );
            let _guard = span.enter();
            tracing::info!(parent: None, "some json test");
        });
    }

    #[test]
    fn json_no_span() {
        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"target\":\"tracing_json::layer::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let collector = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, collector, || {
            tracing::info!("some json test");
        });
    }

    #[test]
    fn record_works() {
        // This test reproduces tracing issue #707, where using `Span::record` causes
        // any events inside the span to be ignored.

        let buffer = MockMakeWriter::default();
        let subscriber = JsonLayer::default().with_writer(buffer.clone()).finish();

        with_default(subscriber, || {
            tracing::info!("an event outside the root span");
            assert_eq!(
                parse_as_json(&buffer)["fields"]["message"],
                "an event outside the root span"
            );

            let span = tracing::info_span!("the span", na = tracing::field::Empty);
            span.record("na", "value");
            let _enter = span.enter();

            tracing::info!("an event inside the root span");
            assert_eq!(
                parse_as_json(&buffer)["fields"]["message"],
                "an event inside the root span"
            );
        });
    }

    // #[test]
    // fn json_span_event_show_correct_context() {
    //     let buffer = MockMakeWriter::default();
    //     let subscriber = subscriber()
    //         .with_writer(buffer.clone())
    //         .flatten_event(false)
    //         .with_current_span(true)
    //         .with_span_list(false)
    //         .with_span_events(FmtSpan::FULL)
    //         .finish();

    //     with_default(subscriber, || {
    //         let context = "parent";
    //         let parent_span = tracing::info_span!("parent_span", context);

    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "new");
    //         assert_eq!(event["span"]["context"], "parent");

    //         let _parent_enter = parent_span.enter();
    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "enter");
    //         assert_eq!(event["span"]["context"], "parent");

    //         let context = "child";
    //         let child_span = tracing::info_span!("child_span", context);
    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "new");
    //         assert_eq!(event["span"]["context"], "child");

    //         let _child_enter = child_span.enter();
    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "enter");
    //         assert_eq!(event["span"]["context"], "child");

    //         drop(_child_enter);
    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "exit");
    //         assert_eq!(event["span"]["context"], "child");

    //         drop(child_span);
    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "close");
    //         assert_eq!(event["span"]["context"], "child");

    //         drop(_parent_enter);
    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "exit");
    //         assert_eq!(event["span"]["context"], "parent");

    //         drop(parent_span);
    //         let event = parse_as_json(&buffer);
    //         assert_eq!(event["fields"]["message"], "close");
    //         assert_eq!(event["span"]["context"], "parent");
    //     });
    // }

    // #[test]
    // fn json_span_event_with_no_fields() {
    //     // Check span events serialize correctly.
    //     // Discussion: https://github.com/tokio-rs/tracing/issues/829#issuecomment-661984255
    //     //
    //     let buffer = MockMakeWriter::default();
    //     let subscriber = subscriber()
    //         .with_writer(buffer.clone())
    //         .flatten_event(false)
    //         .with_current_span(false)
    //         .with_span_list(false)
    //         .with_span_events(FmtSpan::FULL)
    //         .finish();

    //     with_default(subscriber, || {
    //         let span = tracing::info_span!("valid_json");
    //         assert_eq!(parse_as_json(&buffer)["fields"]["message"], "new");

    //         let _enter = span.enter();
    //         assert_eq!(parse_as_json(&buffer)["fields"]["message"], "enter");

    //         drop(_enter);
    //         assert_eq!(parse_as_json(&buffer)["fields"]["message"], "exit");

    //         drop(span);
    //         assert_eq!(parse_as_json(&buffer)["fields"]["message"], "close");
    //     });
    // }

    fn parse_as_json(buffer: &MockMakeWriter) -> serde_json::Value {
        let buf = String::from_utf8(buffer.buf().to_vec()).unwrap();
        let json = buf
            .lines()
            .last()
            .expect("expected at least one line to be written!");
        match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => panic!(
                "assertion failed: JSON shouldn't be malformed\n  error: {}\n  json: {}",
                e, json
            ),
        }
    }

    fn test_json<T>(expected: &str, layer: JsonLayer, producer: impl FnOnce() -> T) {
        let make_writer = MockMakeWriter::default();
        let collector = layer
            .with_writer(make_writer.clone())
            .with_timer(MockTime)
            .finish();

        with_default(collector, producer);

        let buf = make_writer.buf();
        let actual = std::str::from_utf8(&buf[..]).unwrap();
        assert_eq!(
            serde_json::from_str::<std::collections::HashMap<&str, serde_json::Value>>(expected)
                .unwrap(),
            serde_json::from_str(actual).unwrap()
        );
    }

    fn test_json_with_line_number<T>(
        expected: &str,
        layer: JsonLayer,
        producer: impl FnOnce() -> T,
    ) {
        let make_writer = MockMakeWriter::default();
        let collector = layer
            .with_writer(make_writer.clone())
            .with_timer(MockTime)
            .finish();

        with_default(collector, producer);

        let buf = make_writer.buf();
        let actual = std::str::from_utf8(&buf[..]).unwrap();
        let mut expected =
            serde_json::from_str::<std::collections::HashMap<&str, serde_json::Value>>(expected)
                .unwrap();
        let expect_line_number = expected.remove("line_number").is_some();
        let mut actual: std::collections::HashMap<&str, serde_json::Value> =
            serde_json::from_str(actual).unwrap();
        let line_number = actual.remove("line_number");
        if expect_line_number {
            assert_eq!(line_number.map(|x| x.is_number()), Some(true));
        } else {
            assert!(line_number.is_none());
        }
        assert_eq!(actual, expected);
    }
}
