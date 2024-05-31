use tracing::Subscriber as Collect;
use tracing_subscriber::registry::LookupSpan;

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
    SubscriberBuilder::default().layers().0
}

pub struct Subscriber;

impl Subscriber {
    pub fn builder() -> SubscriberBuilder {
        SubscriberBuilder::default()
    }
}
