mod yak_shave;

#[cfg(feature = "opentelemetry")]
fn main() {
    use opentelemetry::trace::TracerProvider;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let exporter = opentelemetry_stdout::SpanExporter::default();
    let builder =
        opentelemetry_sdk::trace::TracerProvider::builder().with_simple_exporter(exporter);
    let provider = builder.build();
    let tracer = provider.tracer("opentelemetry-stdout-exporter");
    opentelemetry::global::set_tracer_provider(provider);

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let json = json_subscriber::layer()
        .with_current_span(false)
        .with_span_list(false)
        .with_opentelemetry_ids(true);

    tracing_subscriber::registry()
        .with(opentelemetry)
        .with(json)
        .init();

    let number_of_yaks = 3;
    // this creates a new event, outside of any spans.
    tracing::info!(number_of_yaks, "preparing to shave yaks");

    let number_shaved = yak_shave::shave_all(number_of_yaks);
    tracing::info!(
        all_yaks_shaved = number_shaved == number_of_yaks,
        "yak shaving completed."
    );
}

#[cfg(not(feature = "opentelemetry"))]
fn main() {
    panic!("This example needs the `opentelemetry` feature.");
}
