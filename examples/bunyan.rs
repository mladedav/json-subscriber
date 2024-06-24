use tracing::{subscriber::with_default, Subscriber};
use tracing_subscriber::{registry::LookupSpan, Layer};

fn main() {
    use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
    use tracing_subscriber::{layer::SubscriberExt, Registry};

    let formatting_layer = BunyanFormattingLayer::new("tracing_demo".into(), std::io::stdout);
    let bunyan = Registry::default()
        .with(JsonStorageLayer)
        .with(formatting_layer);

    with_default(bunyan, do_stuff);

    println!();
    println!("========================================");
    println!();

    let json_subscriber = Registry::default()
        .with(json_subscriber::bunyan::layer(std::io::stdout))
        .with(EnterExitLayer);

    with_default(json_subscriber, do_stuff);
}

fn do_stuff() {
    tracing::info!(lorem = "ipsum", "hello");

    tracing::info_span!("parent", depth = 0, question = "what?").in_scope(|| {
        tracing::info!("in parent");
        tracing::info_span!("child", depth = 1, answer = 42).in_scope(|| {
            tracing::info!("in child");
        });
        tracing::info!("in parent again");
    });
}

struct EnterExitLayer;

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for EnterExitLayer {
    fn on_enter(
        &self,
        id: &tracing_core::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        tracing::info!(parent: id, "enter");
    }

    fn on_exit(
        &self,
        id: &tracing_core::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        tracing::info!(parent: id, "exit");
    }
}
