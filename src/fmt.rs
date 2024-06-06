use std::error::Error;

use tracing::Subscriber as Collect;
use tracing_subscriber::{registry::LookupSpan, util::SubscriberInitExt};

use crate::{builder::SubscriberBuilder, layer::JsonLayer};

/// Returns a new [`SubscriberBuilder`] for configuring a json [formatting subscriber].
///
/// This is essentially shorthand for [`SubscriberBuilder::default()`].
///
/// # Examples
///
/// Using [`init`] to set the default subscriber:
///
/// ```rust
/// json_subscriber::fmt().init();
/// ```
///
/// Configuring the output format:
///
/// ```rust
/// 
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
///     tracing_subscriber::fmt()
///         // Configure the subscriber to emit logs in JSON format.
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
/// let subscriber = tracing_subscriber::fmt()
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
/// [formatting subscriber]: tracing::Subscriber
/// [`SubscriberBuilder::default()`]: SubscriberBuilder::default
/// [`init`]: SubscriberBuilder::init()
/// [`try_init`]: SubscriberBuilder::try_init()
/// [`finish`]: SubscriberBuilder::finish()
pub fn fmt() -> SubscriberBuilder {
    SubscriberBuilder::default()
}

/// Returns a new [json formatting layer] that can be [composed] with other layers to
/// construct a [`Subscriber`].
///
/// [json formatting layer]: JsonLayer
/// [composed]: tracing_subscriber::layer
/// [`Layer::default()`]: Layer::default
pub fn layer<S>() -> JsonLayer<S>
where
    S: Collect + for<'lookup> LookupSpan<'lookup>,
{
    JsonLayer::default()
}

pub struct Subscriber;

impl Subscriber {
    pub fn builder() -> SubscriberBuilder {
        SubscriberBuilder::default()
    }
}

/// Install a global tracing subscriber that listens for events and
/// filters based on the value of the [`RUST_LOG` environment variable],
/// if one is not already set.
///
/// If the `tracing-log` feature is enabled, this will also install
/// the [`LogTracer`] to convert `log` records into `tracing` `Event`s.
///
/// This is shorthand for
///
/// ```rust
/// # fn doc() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
/// json_subscriber::fmt().try_init()
/// # }
/// ```
///
///
/// # Errors
///
/// Returns an Error if the initialization was unsuccessful,
/// likely because a global subscriber was already installed by another
/// call to `try_init`.
///
/// [`LogTracer`]:
///     https://docs.rs/tracing-log/0.1.0/tracing_log/struct.LogTracer.html
/// [`RUST_LOG` environment variable]: crate::filter::EnvFilter::DEFAULT_ENV
pub fn try_init() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let builder = Subscriber::builder();

    #[cfg(feature = "env-filter")]
    let builder = builder.with_env_filter(tracing_subscriber::EnvFilter::from_default_env());

    // If `env-filter` is disabled, remove the default max level filter from the
    // subscriber; it will be added to the `Targets` filter instead if no filter
    // is set in `RUST_LOG`.
    // Replacing the default `LevelFilter` with an `EnvFilter` would imply this,
    // but we can't replace the builder's filter with a `Targets` filter yet.
    #[cfg(not(feature = "env-filter"))]
    let builder = builder.with_max_level(tracing_core::LevelFilter::TRACE);

    let subscriber = builder.finish();
    #[cfg(not(feature = "env-filter"))]
    let subscriber = {
        use std::{env, str::FromStr};

        use tracing_subscriber::{filter::Targets, layer::SubscriberExt};
        let targets = match env::var("RUST_LOG") {
            Ok(var) => {
                Targets::from_str(&var)
                    .map_err(|e| {
                        eprintln!("Ignoring `RUST_LOG={:?}`: {}", var, e);
                    })
                    .unwrap_or_default()
            },
            Err(env::VarError::NotPresent) => {
                Targets::new().with_default(tracing_core::LevelFilter::INFO)
            },
            Err(e) => {
                eprintln!("Ignoring `RUST_LOG`: {}", e);
                Targets::new().with_default(tracing_core::LevelFilter::INFO)
            },
        };
        subscriber.with(targets)
    };

    subscriber.try_init().map_err(Into::into)
}

/// Install a global tracing subscriber that listens for events and
/// filters based on the value of the [`RUST_LOG` environment variable].
///
/// The configuration of the subscriber initialized by this function
/// depends on what [feature flags](crate#feature-flags) are enabled.
///
/// If the `tracing-log` feature is enabled, this will also install
/// the LogTracer to convert `Log` records into `tracing` `Event`s.
///
/// If the `env-filter` feature is enabled, this is shorthand for
///
/// ```rust
/// # use tracing_subscriber::EnvFilter;
/// json_subscriber::fmt()
///     .with_env_filter(EnvFilter::from_default_env())
///     .init();
/// ```
///
/// # Panics
/// Panics if the initialization was unsuccessful, likely because a
/// global subscriber was already installed by another call to `try_init`.
///
/// [`RUST_LOG` environment variable]: crate::filter::EnvFilter::DEFAULT_ENV
pub fn init() {
    try_init().expect("Unable to install global subscriber")
}
