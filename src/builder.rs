use std::io;

use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{
        time::{FormatTime, SystemTime},
        MakeWriter, TestWriter,
    },
    layer::SubscriberExt,
    registry::LookupSpan,
    Registry,
};

use crate::layer::JsonLayer;

pub struct SubscriberBuilder<W = fn() -> io::Stdout, T = SystemTime> {
    make_writer: W,
    timer: T,

    log_internal_errors: bool,

    display_timestamp: bool,
    display_target: bool,
    display_level: bool,
    display_thread_id: bool,
    display_thread_name: bool,
    display_filename: bool,
    display_line_number: bool,
    flatten_event: bool,
    display_current_span: bool,
    display_span_list: bool,
}

impl Default for SubscriberBuilder {
    fn default() -> Self {
        let this = Self {
            make_writer: io::stdout,
            timer: SystemTime,
            log_internal_errors: false,

            display_timestamp: true,
            display_target: true,
            display_level: true,
            display_thread_id: false,
            display_thread_name: false,
            display_filename: false,
            display_line_number: false,
            flatten_event: false,
            display_current_span: true,
            display_span_list: true,
        };

        this.with_target(true)
            .with_level(true)
            .with_timer(SystemTime)
            .with_current_span(true)
            .flatten_event(false)
            .with_span_list(true)
    }
}

impl<W, T> SubscriberBuilder<W, T>
where
    W: for<'writer> MakeWriter<'writer> + 'static,
    T: FormatTime + Send + Sync + 'static,
{
    pub fn finish(self) -> impl Subscriber + for<'a> LookupSpan<'a> {
        let mut layer = JsonLayer::<Registry>::empty().with_writer(self.make_writer);

        if self.display_timestamp {
            layer.with_timer(self.timer);
        }

        layer
            .with_level(self.display_level)
            .flatten_event(self.flatten_event)
            .with_target(self.display_target)
            .with_file(self.display_filename)
            .with_line_number(self.display_line_number)
            .with_current_span(self.display_current_span)
            .with_span_list(self.display_span_list)
            .with_thread_names(self.display_thread_name)
            .with_thread_ids(self.display_thread_id);

        tracing_subscriber::registry().with(layer)
    }
}

impl<W, T> SubscriberBuilder<W, T> {
    /// Sets the [`MakeWriter`] that the [`SubscriberBuilder`] being built will use to write events.
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
    /// [`SubscriberBuilder`]: super::SubscriberBuilder
    pub fn with_writer<W2>(self, make_writer: W2) -> SubscriberBuilder<W2, T>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        SubscriberBuilder {
            make_writer,
            timer: self.timer,
            log_internal_errors: self.log_internal_errors,
            display_timestamp: self.display_timestamp,
            display_target: self.display_target,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
            display_filename: self.display_filename,
            display_line_number: self.display_line_number,
            flatten_event: self.flatten_event,
            display_current_span: self.display_current_span,
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
    /// let (subscriber, reload_handle) = reload::SubscriberBuilder::new(subscriber);
    /// #
    /// # // specifying the Registry type is required
    /// # let _: &reload::Handle<fmt::SubscriberBuilder<W, T> = &reload_handle;
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
    pub fn with_test_writer(self) -> SubscriberBuilder<TestWriter, T> {
        SubscriberBuilder {
            make_writer: TestWriter::default(),
            timer: self.timer,
            log_internal_errors: self.log_internal_errors,
            display_timestamp: self.display_timestamp,
            display_target: self.display_target,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
            display_filename: self.display_filename,
            display_line_number: self.display_line_number,
            flatten_event: self.flatten_event,
            display_current_span: self.display_current_span,
            display_span_list: self.display_span_list,
        }
    }

    /// Sets whether to write errors from [`FormatEvent`] to the writer.
    /// Defaults to true.
    ///
    /// By default, `fmt::SubscriberBuilder` will write any `FormatEvent`-internal errors to
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
    pub fn map_writer<W2>(self, f: impl FnOnce(W) -> W2) -> SubscriberBuilder<W2, T>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        SubscriberBuilder {
            make_writer: f(self.make_writer),
            timer: self.timer,
            log_internal_errors: self.log_internal_errors,
            display_timestamp: self.display_timestamp,
            display_target: self.display_target,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
            display_filename: self.display_filename,
            display_line_number: self.display_line_number,
            flatten_event: self.flatten_event,
            display_current_span: self.display_current_span,
            display_span_list: self.display_span_list,
        }
    }

    /// Sets the JSON subscriber being built to flatten event metadata.
    ///
    /// See [`format::Json`]
    pub fn flatten_event(self, flatten_event: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            flatten_event,
            ..self
        }
    }

    /// Sets whether or not the formatter will include the current span in
    /// formatted events.
    ///
    /// See [`format::Json`]
    pub fn with_current_span(self, display_current_span: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            display_current_span,
            ..self
        }
    }

    /// Sets whether or not the formatter will include a list (from root to leaf)
    /// of all currently entered spans in formatted events.
    ///
    /// See [`format::Json`]
    pub fn with_span_list(self, display_span_list: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
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
    pub fn with_timer<T2>(self, timer: T2) -> SubscriberBuilder<W, T2> {
        SubscriberBuilder {
            make_writer: self.make_writer,
            timer,
            log_internal_errors: self.log_internal_errors,
            display_timestamp: self.display_timestamp,
            display_target: self.display_target,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
            display_filename: self.display_filename,
            display_line_number: self.display_line_number,
            flatten_event: self.flatten_event,
            display_current_span: self.display_current_span,
            display_span_list: self.display_span_list,
        }
    }

    /// Do not emit timestamps with log messages.
    pub fn without_time(self) -> SubscriberBuilder<W, ()> {
        SubscriberBuilder {
            make_writer: self.make_writer,
            timer: (),
            log_internal_errors: self.log_internal_errors,
            display_timestamp: self.display_timestamp,
            display_target: self.display_target,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
            display_filename: self.display_filename,
            display_line_number: self.display_line_number,
            flatten_event: self.flatten_event,
            display_current_span: self.display_current_span,
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
    // /// [time]: SubscriberBuilder::without_time()
    // pub fn with_span_events(self, kind: format::FmtSpan) -> Self {
    //     SubscriberBuilder {
    //         inner: self.inner.with_span_events(kind),
    //         ..self
    //     }
    // }

    /// Sets whether or not an event's target is displayed.
    pub fn with_target(self, display_target: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            display_target,
            ..self
        }
    }

    /// Sets whether or not an event's [source code file path][file] is
    /// displayed.
    ///
    /// [file]: tracing_core::Metadata::file
    pub fn with_file(self, display_filename: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            display_filename,
            ..self
        }
    }

    /// Sets whether or not an event's [source code line number][line] is
    /// displayed.
    ///
    /// [line]: tracing_core::Metadata::line
    pub fn with_line_number(self, display_line_number: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            display_line_number,
            ..self
        }
    }

    /// Sets whether or not an event's level is displayed.
    pub fn with_level(self, display_level: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            display_level,
            ..self
        }
    }

    /// Sets whether or not the [name] of the current thread is displayed
    /// when formatting events.
    ///
    /// [name]: std::thread#naming-threads
    pub fn with_thread_names(self, display_thread_name: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            display_thread_name,
            ..self
        }
    }

    /// Sets whether or not the [thread ID] of the current thread is displayed
    /// when formatting events.
    ///
    /// [thread ID]: std::thread::ThreadId
    pub fn with_thread_ids(self, display_thread_id: bool) -> SubscriberBuilder<W, T> {
        SubscriberBuilder {
            display_thread_id,
            ..self
        }
    }
}

#[cfg(test)]
mod test {
    use super::SubscriberBuilder;
    use crate::tests::MockMakeWriter;

    use tracing_subscriber::fmt::format::Writer;
    use tracing_subscriber::fmt::time::FormatTime;

    use tracing::subscriber::with_default;

    use std::fmt;
    use std::path::Path;

    struct MockTime;
    impl FormatTime for MockTime {
        fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
            write!(w, "fake time")
        }
    }

    fn subscriber() -> SubscriberBuilder {
        SubscriberBuilder::default()
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
            .join("builder.rs")
            .to_str()
            .expect("path must be valid unicode")
            // escape windows backslashes
            .replace('\\', "\\\\");
        let expected =
            &format!("{}{}{}",
                    "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"json_subscriber::builder::test\",\"filename\":\"",
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
            "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"json_subscriber::builder::test\",\"line_number\":42,\"fields\":{\"message\":\"some json test\"}}\n";
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
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"json_subscriber::builder::test\",\"message\":\"some json test\"}\n";

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
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"json_subscriber::builder::test\",\"fields\":{\"message\":\"some json test\"}}\n";
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
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"target\":\"json_subscriber::builder::test\",\"fields\":{\"message\":\"some json test\"}}\n";
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
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3},{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4}],\"target\":\"json_subscriber::builder::test\",\"fields\":{\"message\":\"some json test\"}}\n";
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
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3},{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4}],\"target\":\"json_subscriber::builder::test\",\"fields\":{\"message\":\"some json test\"}}\n";
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
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"target\":\"json_subscriber::builder::test\",\"fields\":{\"message\":\"some json test\"}}\n";
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
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"target\":\"json_subscriber::builder::test\",\"fields\":{\"message\":\"some json test\"}}\n";
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
        let subscriber = SubscriberBuilder::default()
            .with_writer(buffer.clone())
            .finish();

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

    fn test_json<T>(expected: &str, builder: SubscriberBuilder, producer: impl FnOnce() -> T) {
        let make_writer = MockMakeWriter::default();
        let collector = builder
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
        builder: SubscriberBuilder,
        producer: impl FnOnce() -> T,
    ) {
        let make_writer = MockMakeWriter::default();
        let collector = builder
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
