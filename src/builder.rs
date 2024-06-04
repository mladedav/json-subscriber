use std::{error::Error, io};

use tracing::{Dispatch, Subscriber};
use tracing_core::LevelFilter;
use tracing_subscriber::{
    fmt::{
        time::{FormatTime, SystemTime},
        MakeWriter, TestWriter,
    },
    layer::{Layered, SubscriberExt},
    registry::LookupSpan,
    reload, Layer, Registry,
};

use crate::layer::CustomJsonLayer;

/// Configures and constructs `Subscriber`s.
///
/// This should be this library's replacement for [`tracing_subscriber::fmt::SubscriberBuilder`].
///
/// Returns a new [`SubscriberBuilder`] for configuring a [formatting subscriber]. The default value should be mostly equivalent to calling `tracing_subscriber::fmt().json()`.
///
/// # Examples
///
/// Using [`init`] to set the default subscriber:
///
/// ```rust
/// json_subscriber::builder::SubscriberBuilder::default().init();
/// ```
///
/// Configuring the output format:
///
/// ```rust
/// json_subscriber::fmt()
///     // Configure formatting settings.
///     .with_target(false)
///     .with_timer(tracing_subscriber::fmt::time::uptime())
///     .with_level(true)
///     // Set the subscriber as the default.
///     .init();
/// ```
///
/// [`try_init`] returns an error if the default subscriber could not be set:
///
/// ```rust
/// use std::error::Error;
///
/// fn init_subscriber() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
///     json_subscriber::fmt()
///         // This is no-op. This subscriber uses only JSON.
///         .json()
///         // Configure the subscriber to flatten event fields in the output JSON objects.
///         .flatten_event(true)
///         // Set the subscriber as the default, returning an error if this fails.
///         .try_init()?;
///
///     Ok(())
/// }
/// ```
///
/// Rather than setting the subscriber as the default, [`finish`] _returns_ the
/// constructed subscriber, which may then be passed to other functions:
///
/// ```rust
/// let subscriber = json_subscriber::fmt()
///     .with_max_level(tracing::Level::DEBUG)
///     .compact()
///     .finish();
///
/// tracing::subscriber::with_default(subscriber, || {
///     // the subscriber will only be set as the default
///     // inside this closure...
/// })
/// ```
///
/// [formatting subscriber]: Subscriber
/// [`SubscriberBuilder::default()`]: SubscriberBuilder::default
/// [`init`]: SubscriberBuilder::init()
/// [`try_init`]: SubscriberBuilder::try_init()
/// [`finish`]: SubscriberBuilder::finish()
pub struct SubscriberBuilder<W = fn() -> io::Stdout, T = SystemTime, F = LevelFilter> {
    make_writer: W,
    timer: T,
    filter: F,

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
        Self {
            make_writer: io::stdout,
            filter: LevelFilter::INFO,
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
        }
    }
}

impl<W, T, F> SubscriberBuilder<W, T, F>
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
    T: FormatTime + Send + Sync + 'static,
    F: Layer<Layered<CustomJsonLayer<Registry, W>, Registry>> + 'static,
    Layered<F, Layered<CustomJsonLayer<Registry, W>, Registry>>:
        tracing_core::Subscriber + Into<Dispatch>,
{
    pub(crate) fn layers<S>(self) -> (CustomJsonLayer<S, W>, F)
    where
        S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    {
        let mut layer = CustomJsonLayer::<S>::empty().with_writer(self.make_writer);

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

        (layer, self.filter)
    }

    /// Finish the builder, returning a new [`Subscriber`] which can be used to [lookup spans].
    ///
    /// [lookup spans]: LookupSpan
    pub fn finish(self) -> Layered<F, Layered<CustomJsonLayer<Registry, W>, Registry>> {
        let (json_layer, filter_layer) = self.layers();
        tracing_subscriber::registry()
            .with(json_layer)
            .with(filter_layer)
    }

    /// Install this Subscriber as the global default if one is
    /// not already set.
    ///
    /// If the `tracing-log` feature is enabled, this will also install
    /// the LogTracer to convert `Log` records into `tracing` `Event`s.
    ///
    /// # Errors
    /// Returns an Error if the initialization was unsuccessful, likely
    /// because a global subscriber was already installed by another
    /// call to `try_init`.
    pub fn try_init(self) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
        use tracing_subscriber::util::SubscriberInitExt;
        self.finish().try_init()?;

        Ok(())
    }

    /// Install this Subscriber as the global default.
    ///
    /// If the `tracing-log` feature is enabled, this will also install
    /// the LogTracer to convert `Log` records into `tracing` `Event`s.
    ///
    /// # Panics
    /// Panics if the initialization was unsuccessful, likely because a
    /// global subscriber was already installed by another call to `try_init`.
    pub fn init(self) {
        self.try_init()
            .expect("Unable to install global subscriber")
    }
}

impl<W, T, F> SubscriberBuilder<W, T, F> {
    /// This does nothing. It exists only to mimic `tracing-subscriber`'s API.
    #[deprecated(note = "Calling `json()` does nothing.")]
    pub fn json(self) -> Self {
        self
    }

    /// This does nothing. It exists only to mimic `tracing-subscriber`'s API.
    #[deprecated(note = "Calling `with_ansi()` does nothing.")]
    pub fn with_ansi(self, _ansi: bool) -> Self {
        self
    }

    /// Sets the [`MakeWriter`] that the [`SubscriberBuilder`] being built will use to write events.
    ///
    /// # Examples
    ///
    /// Using `stderr` rather than `stdout`:
    ///
    /// ```rust
    /// use std::io;
    ///
    /// let fmt_subscriber = json_subscriber::fmt()
    ///     .with_writer(io::stderr);
    /// ```
    pub fn with_writer<W2>(self, make_writer: W2) -> SubscriberBuilder<W2, T, F>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        SubscriberBuilder {
            make_writer,
            timer: self.timer,
            filter: self.filter,
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
    /// [`reload::Handle::modify`](reload::Handle::modify) method.
    ///
    /// # Examples
    ///
    /// ```
    /// use tracing_subscriber::{fmt::writer::EitherWriter, reload};
    /// # fn main() {
    /// let subscriber = json_subscriber::fmt::subscriber()
    ///     .with_writer::<Box<dyn Fn() -> EitherWriter<_, _>>>(Box::new(|| EitherWriter::A(std::io::stderr())));
    /// let (subscriber, reload_handle) = reload::Layer::new(subscriber);
    /// # let subscriber: reload::Layer<_, tracing_subscriber::Registry> = subscriber;
    ///
    /// tracing::info!("This will be logged to stderr");
    /// reload_handle.modify(|subscriber| *subscriber.writer_mut() = Box::new(|| EitherWriter::B(std::io::stdout())));
    /// tracing::info!("This will be logged to stdout");
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
    /// use json_subscriber::fmt;
    ///
    /// let fmt_subscriber = fmt::subscriber()
    ///     .with_test_writer();
    /// ```
    /// [capturing]:
    /// https://doc.rust-lang.org/book/ch11-02-running-tests.html#showing-function-output
    /// [`TestWriter`]: tracing_subscriber::fmt::writer::TestWriter
    pub fn with_test_writer(self) -> SubscriberBuilder<TestWriter, T, F> {
        SubscriberBuilder {
            make_writer: TestWriter::default(),
            timer: self.timer,
            filter: self.filter,
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

    /// Sets whether to write errors during internal operations to stderr.
    /// This can help identify problems with serialization and with debugging issues.
    ///
    /// Defaults to false.
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
    /// use json_subscriber::fmt;
    /// use tracing_subscriber::fmt::writer::MakeWriterExt;
    ///
    /// let stderr = std::io::stderr.with_max_level(Level::WARN);
    /// let subscriber = fmt::subscriber()
    ///     .map_writer(move |w| stderr.or_else(w));
    /// ```
    pub fn map_writer<W2>(self, f: impl FnOnce(W) -> W2) -> SubscriberBuilder<W2, T, F>
    where
        W2: for<'writer> MakeWriter<'writer> + 'static,
    {
        SubscriberBuilder {
            make_writer: f(self.make_writer),
            timer: self.timer,
            filter: self.filter,
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
    pub fn flatten_event(self, flatten_event: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            flatten_event,
            ..self
        }
    }

    /// Sets whether or not the formatter will include the current span in
    /// formatted events.
    pub fn with_current_span(self, display_current_span: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_current_span,
            ..self
        }
    }

    /// Sets whether or not the formatter will include a list (from root to leaf)
    /// of all currently entered spans in formatted events.
    pub fn with_span_list(self, display_span_list: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_span_list,
            ..self
        }
    }

    /// Use the given [`timer`] for log message timestamps.
    ///
    /// See the [`tracing_subscriber::fmt::time` module][`time` module] for the
    /// provided timer implementations.
    ///
    /// Note that using the `time` feature flag on `tracing_subscriber` enables the
    /// additional time formatters [`UtcTime`] and [`LocalTime`], which use the
    /// [`time` crate] to provide more sophisticated timestamp formatting
    /// options.
    ///
    /// [`timer`]: tracing_subscriber::fmt::time::FormatTime
    /// [`time` module]: mod@tracing_subscriber::fmt::time
    /// [`UtcTime`]: tracing_subscriber::fmt::time::UtcTime
    /// [`LocalTime`]: tracing_subscriber::fmt::time::LocalTime
    /// [`time` crate]: https://docs.rs/time/0.3
    pub fn with_timer<T2>(self, timer: T2) -> SubscriberBuilder<W, T2, F> {
        SubscriberBuilder {
            make_writer: self.make_writer,
            timer,
            filter: self.filter,
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
    pub fn without_time(self) -> SubscriberBuilder<W, (), F> {
        SubscriberBuilder {
            make_writer: self.make_writer,
            timer: (),
            filter: self.filter,
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
    pub fn with_target(self, display_target: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_target,
            ..self
        }
    }

    /// Sets whether or not an event's [source code file path][file] is
    /// displayed.
    ///
    /// [file]: tracing_core::Metadata::file
    pub fn with_file(self, display_filename: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_filename,
            ..self
        }
    }

    /// Sets whether or not an event's [source code line number][line] is
    /// displayed.
    ///
    /// [line]: tracing_core::Metadata::line
    pub fn with_line_number(self, display_line_number: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_line_number,
            ..self
        }
    }

    /// Sets whether or not an event's level is displayed.
    pub fn with_level(self, display_level: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_level,
            ..self
        }
    }

    /// Sets whether or not the [name] of the current thread is displayed
    /// when formatting events.
    ///
    /// [name]: std::thread#naming-threads
    pub fn with_thread_names(self, display_thread_name: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_thread_name,
            ..self
        }
    }

    /// Sets whether or not the [thread ID] of the current thread is displayed
    /// when formatting events.
    ///
    /// [thread ID]: std::thread::ThreadId
    pub fn with_thread_ids(self, display_thread_id: bool) -> SubscriberBuilder<W, T, F> {
        SubscriberBuilder {
            display_thread_id,
            ..self
        }
    }

    /// Sets the [`EnvFilter`] that the collector will use to determine if
    /// a span or event is enabled.
    ///
    /// Note that this method requires the "env-filter" feature flag to be enabled.
    ///
    /// If a filter was previously set, or a maximum level was set by the
    /// [`with_max_level`] method, that value is replaced by the new filter.
    ///
    /// # Examples
    ///
    /// Setting a filter based on the value of the `RUST_LOG` environment
    /// variable:
    /// ```rust
    /// use tracing_subscriber::EnvFilter;
    ///
    /// json_subscriber::fmt()
    ///     .with_env_filter(EnvFilter::from_default_env())
    ///     .init();
    /// ```
    ///
    /// Setting a filter based on a pre-set filter directive string:
    /// ```rust
    /// json_subscriber::fmt()
    ///     .with_env_filter("my_crate=info,my_crate::my_mod=debug,[my_span]=trace")
    ///     .init();
    /// ```
    ///
    /// Adding additional directives to a filter constructed from an env var:
    /// ```rust
    /// use tracing_subscriber::filter::{EnvFilter, LevelFilter};
    ///
    /// # fn filter() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    /// let filter = EnvFilter::try_from_env("MY_CUSTOM_FILTER_ENV_VAR")?
    ///     // Set the base level when not matched by other directives to WARN.
    ///     .add_directive(LevelFilter::WARN.into())
    ///     // Set the max level for `my_crate::my_mod` to DEBUG, overriding
    ///     // any directives parsed from the env variable.
    ///     .add_directive("my_crate::my_mod=debug".parse()?);
    ///
    /// json_subscriber::fmt()
    ///     .with_env_filter(filter)
    ///     .try_init()?;
    /// # Ok(())}
    /// ```
    /// [`EnvFilter`]: tracing_subscriber::filter::EnvFilter
    /// [`with_max_level`]: Self::with_max_level()
    #[cfg(feature = "env-filter")]
    #[cfg_attr(docsrs, doc(cfg(feature = "env-filter")))]
    pub fn with_env_filter(
        self,
        filter: impl Into<tracing_subscriber::EnvFilter>,
    ) -> SubscriberBuilder<W, T, tracing_subscriber::EnvFilter> {
        SubscriberBuilder {
            make_writer: self.make_writer,
            timer: self.timer,
            filter: filter.into(),
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

    /// Sets the maximum [verbosity level] that will be enabled by the
    /// collector.
    ///
    /// If the max level has already been set, or a [`EnvFilter`] was added by
    /// [`with_env_filter`], this replaces that configuration with the new
    /// maximum level.
    ///
    /// # Examples
    ///
    /// Enable up to the `DEBUG` verbosity level:
    /// ```rust
    /// use tracing_subscriber::fmt;
    /// use tracing::Level;
    ///
    /// fmt()
    ///     .with_max_level(Level::DEBUG)
    ///     .init();
    /// ```
    /// This collector won't record any spans or events!
    /// ```rust
    /// use tracing_subscriber::{fmt, filter::LevelFilter};
    ///
    /// let subscriber = fmt()
    ///     .with_max_level(LevelFilter::OFF)
    ///     .finish();
    /// ```
    /// [verbosity level]: tracing_core::Level
    /// [`EnvFilter`]: struct@crate::filter::EnvFilter
    /// [`with_env_filter`]: fn@Self::with_env_filter
    pub fn with_max_level(
        self,
        filter: impl Into<LevelFilter>,
    ) -> SubscriberBuilder<W, T, LevelFilter> {
        SubscriberBuilder {
            make_writer: self.make_writer,
            timer: self.timer,
            filter: filter.into(),
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

    /// Configures the collector being built to allow filter reloading at
    /// runtime.
    ///
    /// The returned builder will have a [`reload_handle`] method, which returns
    /// a [`reload::Handle`] that may be used to set a new filter value.
    ///
    /// For example:
    ///
    /// ```
    /// use tracing::Level;
    /// use tracing_subscriber::util::SubscriberInitExt;
    ///
    /// let builder = tracing_subscriber::fmt()
    ///      // Set a max level filter on the collector
    ///     .with_max_level(Level::INFO)
    ///     .with_filter_reloading();
    ///
    /// // Get a handle for modifying the collector's max level filter.
    /// let handle = builder.reload_handle();
    ///
    /// // Finish building the collector, and set it as the default.
    /// builder.finish().init();
    ///
    /// // Currently, the max level is INFO, so this event will be disabled.
    /// tracing::debug!("this is not recorded!");
    ///
    /// // Use the handle to set a new max level filter.
    /// // (this returns an error if the collector has been dropped, which shouldn't
    /// // happen in this example.)
    /// handle.reload(Level::DEBUG).expect("the collector should still exist");
    ///
    /// // Now, the max level is INFO, so this event will be recorded.
    /// tracing::debug!("this is recorded!");
    /// ```
    ///
    /// [`reload_handle`]: Self::reload_handle
    /// [`reload::Handle`]: crate::reload::Handle
    pub fn with_filter_reloading(self) -> SubscriberBuilder<W, T, reload::Layer<F, Registry>> {
        let (filter, _) = reload::Layer::new(self.filter);
        SubscriberBuilder {
            make_writer: self.make_writer,
            timer: self.timer,
            filter,
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
}

impl<W, T, F> SubscriberBuilder<W, T, reload::Layer<F, Registry>> {
    /// Returns a `Handle` that may be used to reload the constructed collector's
    /// filter.
    pub fn reload_handle(&self) -> reload::Handle<F, Registry> {
        self.filter.handle()
    }
}

#[cfg(test)]
mod test {
    //! These tests are copied from `tracing-subscriber` for compatibility.

    use std::path::Path;

    use tracing_core::Dispatch;
    use tracing_subscriber::{filter::LevelFilter, registry::LookupSpan, Registry};

    use tracing::subscriber::with_default;

    use super::SubscriberBuilder;
    use crate::{
        layer::CustomJsonLayer,
        tests::{MockMakeWriter, MockTime},
    };

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
        let actual = dbg!(std::str::from_utf8(&buf[..]).unwrap());
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

    #[test]
    fn subscriber_downcasts() {
        let subscriber = SubscriberBuilder::default().finish();
        let dispatch = Dispatch::new(subscriber);
        assert!(dispatch.downcast_ref::<Registry>().is_some());
    }

    #[test]
    fn subscriber_downcasts_to_parts() {
        let subscriber = SubscriberBuilder::default().finish();
        let dispatch = Dispatch::new(subscriber);
        assert!(dispatch.downcast_ref::<CustomJsonLayer>().is_some());
        assert!(dispatch.downcast_ref::<LevelFilter>().is_some());
    }

    #[test]
    fn is_lookup_span() {
        fn assert_lookup_span<T: for<'a> LookupSpan<'a>>(_: T) {}
        let subscriber = SubscriberBuilder::default().finish();
        assert_lookup_span(subscriber)
    }
}
