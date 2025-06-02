mod yak_shave;

#[cfg(feature = "tracing-opentelemetry-0-29")]
fn main() {
    use opentelemetry_0_28::trace::TracerProvider;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let exporter = opentelemetry_stdout::SpanExporter::builder()
        .with_writer(std::io::sink())
        .build();
    let builder =
        opentelemetry_sdk::trace::SdkTracerProvider::builder().with_simple_exporter(exporter);
    let provider = builder.build();
    let tracer = provider
        .tracer_builder("opentelemetry-stdout-exporter")
        .build();
    opentelemetry_0_28::global::set_tracer_provider(provider);

    let opentelemetry = tracing_opentelemetry_0_29::layer().with_tracer(tracer);
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

#[cfg(not(feature = "tracing-opentelemetry-0-29"))]
fn main() {
    panic!("This example needs the `tracing-opentelemetry-0-29` feature.");
}
