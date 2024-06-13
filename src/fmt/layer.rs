use std::io;

use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{
        time::{FormatTime, SystemTime},
        MakeWriter,
        TestWriter,
    },
    registry::LookupSpan,
    Layer as Subscribe,
    Registry,
};

use crate::layer::JsonLayer;

/// A [`Layer`] that logs JSON formatted representations of `tracing` events.
///
/// This is just a wrapper around [`JsonLayer`] which exists for compatibility with
/// `tracing_subscriber`.
///
/// ## Examples
///
/// Constructing a layer with the default configuration:
///
/// ```rust
/// use tracing_subscriber::Registry;
/// use tracing_subscriber::layer::SubscriberExt as _;
/// use json_subscriber::fmt;
///
/// let subscriber = Registry::default()
///     .with(fmt::Layer::default());
///
/// tracing::subscriber::set_global_default(subscriber).unwrap();
/// ```
///
/// Overriding the layer's behavior:
///
/// ```rust
/// use tracing_subscriber::Registry;
/// use tracing_subscriber::layer::SubscriberExt as _;
/// use json_subscriber::fmt;
///
/// let fmt_layer = fmt::layer()
///    .with_target(false) // don't include event targets when logging
///    .with_level(false); // don't include event levels when logging
///
/// let subscriber = Registry::default().with(fmt_layer);
/// # tracing::subscriber::set_global_default(subscriber).unwrap();
/// ```
///
/// [`Layer`]: tracing_subscriber::Layer
pub struct Layer<S = Registry, W = fn() -> io::Stdout> {
    inner: JsonLayer<S, W>,
}

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Default for Layer<S> {
    fn default() -> Self {
        let mut inner = JsonLayer::stdout();

        inner
            // If we do not call this, fields are not printed at all.
            .flatten_event(false)
            .with_timer(SystemTime)
            .with_target(true)
            .with_level(true)
            .with_current_span(true)
            .with_span_list(true);

        Self { inner }
    }
}

impl<S, W> Subscribe<S> for Layer<S, W>
where
    JsonLayer<S, W>: Subscribe<S>,
    S: Subscriber,
{
    fn on_register_dispatch(&self, subscriber: &tracing::Dispatch) {
        self.inner.on_register_dispatch(subscriber)
    }

    fn on_layer(&mut self, subscriber: &mut S) {
        self.inner.on_layer(subscriber)
    }

    fn register_callsite(
        &self,
        metadata: &'static tracing::Metadata<'static>,
    ) -> tracing_core::Interest {
        self.inner.register_callsite(metadata)
    }

    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.inner.enabled(metadata, ctx)
    }

    fn on_new_span(
        &self,
        attrs: &tracing_core::span::Attributes<'_>,
        id: &tracing_core::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_new_span(attrs, id, ctx)
    }

    fn on_record(
        &self,
        span: &tracing_core::span::Id,
        values: &tracing_core::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_record(span, values, ctx)
    }

    fn on_follows_from(
        &self,
        span: &tracing_core::span::Id,
        follows: &tracing_core::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_follows_from(span, follows, ctx)
    }

    fn event_enabled(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.inner.event_enabled(event, ctx)
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_event(event, ctx)
    }

    fn on_enter(
        &self,
        id: &tracing_core::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_enter(id, ctx)
    }

    fn on_exit(&self, id: &tracing_core::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_exit(id, ctx)
    }

    fn on_close(&self, id: tracing_core::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.inner.on_close(id, ctx)
    }

    fn on_id_change(
        &self,
        old: &tracing_core::span::Id,
        new: &tracing_core::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.inner.on_id_change(old, new, ctx)
    }
}

impl<S, W> Layer<S, W>
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
    /// [`MakeWriter`]: MakeWriter
    /// [`JsonLayer`]: JsonLayer
    pub fn with_writer<W2>(self, make_writer: W2) -> Layer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        Layer::<S, W2> {
            inner: self.inner.with_writer(make_writer),
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
    pub fn map_writer<W2>(self, f: impl FnOnce(W) -> W2) -> Layer<S, W2>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        Layer::<S, W2> {
            inner: self.inner.map_writer(f),
        }
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
    /// [`TestWriter`]: TestWriter
    pub fn with_test_writer(self) -> Layer<S, TestWriter> {
        Layer::<S, TestWriter> {
            inner: self.inner.with_test_writer(),
        }
    }

    /// Borrows the [writer] for this subscriber.
    ///
    /// [writer]: MakeWriter
    pub fn writer(&self) -> &W {
        self.inner.writer()
    }

    /// Mutably borrows the [writer] for this subscriber.
    ///
    /// This method is primarily expected to be used with the [`reload::Handle::modify`] method.
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
    /// [`reload::Handle::modify`]: tracing_subscriber::reload::Handle::modify
    pub fn writer_mut(&mut self) -> &mut W {
        self.inner.writer_mut()
    }

    /// Sets whether to write errors from [`FormatEvent`] to the writer.
    /// Defaults to true.
    ///
    /// By default, `fmt::JsonLayer` will write any `FormatEvent`-internal errors to the writer.
    /// These errors are unlikely and will only occur if there is a bug in the `FormatEvent`
    /// implementation or its dependencies.
    ///
    /// If writing to the writer fails, the error message is printed to stderr as a fallback.
    ///
    /// [`FormatEvent`]: tracing_subscriber::fmt::FormatEvent
    pub fn log_internal_errors(mut self, log_internal_errors: bool) -> Self {
        self.inner.log_internal_errors(log_internal_errors);
        self
    }

    /// Sets the JSON subscriber being built to flatten event metadata.
    pub fn flatten_event(mut self, flatten_event: bool) -> Self {
        self.inner.flatten_event(flatten_event);
        self
    }

    /// Sets whether or not the formatter will include the current span in formatted events.
    pub fn with_current_span(mut self, display_current_span: bool) -> Self {
        self.inner.with_current_span(display_current_span);
        self
    }

    /// Sets whether or not the formatter will include a list (from root to leaf) of all currently
    /// entered spans in formatted events.
    pub fn with_span_list(mut self, display_span_list: bool) -> Self {
        self.inner.with_span_list(display_span_list);
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
    /// [`timer`]: tracing_subscriber::fmt::time::FormatTime
    /// [`time` module]: mod@tracing_subscriber::fmt::time
    /// [`UtcTime`]: tracing_subscriber::fmt::time::UtcTime
    /// [`LocalTime`]: tracing_subscriber::fmt::time::LocalTime
    /// [`time` crate]: https://docs.rs/time/0.3
    pub fn with_timer<T: FormatTime + Send + Sync + 'static>(mut self, timer: T) -> Self {
        self.inner.with_timer(timer);
        self
    }

    /// Do not emit timestamps with log messages.
    pub fn without_time(mut self) -> Self {
        self.inner.without_time();
        self
    }

    /// Sets whether or not an event's target is displayed.
    pub fn with_target(mut self, display_target: bool) -> Self {
        self.inner.with_target(display_target);

        self
    }

    /// Sets whether or not an event's [source code file path][file] is
    /// displayed.
    ///
    /// [file]: tracing_core::Metadata::file
    pub fn with_file(mut self, display_filename: bool) -> Self {
        self.inner.with_file(display_filename);
        self
    }

    /// Sets whether or not an event's [source code line number][line] is
    /// displayed.
    ///
    /// [line]: tracing_core::Metadata::line
    pub fn with_line_number(mut self, display_line_number: bool) -> Self {
        self.inner.with_line_number(display_line_number);
        self
    }

    /// Sets whether or not an event's level is displayed.
    pub fn with_level(mut self, display_level: bool) -> Self {
        self.inner.with_level(display_level);
        self
    }

    /// Sets whether or not the [name] of the current thread is displayed
    /// when formatting events.
    ///
    /// [name]: std::thread#naming-threads
    pub fn with_thread_names(mut self, display_thread_name: bool) -> Self {
        self.inner.with_thread_names(display_thread_name);
        self
    }

    /// Sets whether or not the [thread ID] of the current thread is displayed
    /// when formatting events.
    ///
    /// [thread ID]: std::thread::ThreadId
    pub fn with_thread_ids(mut self, display_thread_id: bool) -> Self {
        self.inner.with_thread_ids(display_thread_id);
        self
    }

    /// Sets whether or not [OpenTelemetry] trace ID and span ID is displayed when formatting
    /// events.
    ///
    /// [OpenTelemetry]: https://opentelemetry.io
    #[cfg(feature = "opentelemetry")]
    #[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
    pub fn with_opentelemetry_ids(mut self, display_opentelemetry_ids: bool) -> Self {
        self.inner.with_opentelemetry_ids(display_opentelemetry_ids);
        self
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tracing::subscriber::with_default;
    use tracing_subscriber::{registry, Layer as _, Registry};

    use super::Layer;
    use crate::tests::{MockMakeWriter, MockTime};

    fn test_json<W, T>(
        expected: serde_json::Value,
        layer: Layer<Registry, W>,
        producer: impl FnOnce() -> T,
    ) {
        let actual = produce_log_line(layer, producer);
        assert_eq!(
            expected,
            serde_json::from_str::<serde_json::Value>(&actual).unwrap(),
        );
    }

    fn produce_log_line<W, T>(layer: Layer<Registry, W>, producer: impl FnOnce() -> T) -> String {
        let make_writer = MockMakeWriter::default();
        let collector = layer
            .with_writer(make_writer.clone())
            .with_timer(MockTime)
            .with_subscriber(registry());

        with_default(collector, producer);

        let buf = make_writer.buf();
        dbg!(std::str::from_utf8(&buf[..]).unwrap()).to_owned()
    }

    #[test]
    fn default() {
        let expected = json!(
            {
                "timestamp": "fake time",
                "level": "INFO",
                "span": {
                    "answer": 42,
                    "name": "json_span",
                    "number": 3,
                },
                "spans": [
                    {
                        "answer": 42,
                        "name": "json_span",
                        "number": 3,
                    },
                ],
                "target": "json_subscriber::fmt::layer::tests",
                "fields": {
                    "message": "some json test",
                },
            }
        );

        let layer = Layer::default();

        test_json(expected, layer, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn flatten() {
        let expected = json!(
            {
                "timestamp": "fake time",
                "level": "INFO",
                "span": {
                    "answer": 42,
                    "name": "json_span",
                    "number": 3,
                },
                "spans": [
                    {
                        "answer": 42,
                        "name": "json_span",
                        "number": 3,
                    },
                ],
                "target": "json_subscriber::fmt::layer::tests",
                "message": "some json test",
            }
        );

        let layer = Layer::default()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, layer, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn flatten_conflict() {
        // This probably should not work like this. But it's an open question how it *should* work.

        // Notice that there is `level` twice so this is not a valid JSON.
        #[rustfmt::skip]
        let expected = "{\"level\":\"this is a bug\",\"message\":\"some json test\",\"level\":\"INFO\",\"timestamp\":\"fake time\"}\n";

        let layer = Layer::default()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .with_target(false);

        let actual = produce_log_line(layer, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!(level = "this is a bug", "some json test");
        });

        assert_eq!(expected, actual);
    }
}
