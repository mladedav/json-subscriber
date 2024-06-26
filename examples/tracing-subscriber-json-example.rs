mod yak_shave;

fn main() {
    json_subscriber::fmt()
        // .json()
        .with_max_level(tracing::Level::TRACE)
        .with_current_span(false)
        .init();

    let number_of_yaks = 3;
    // this creates a new event, outside of any spans.
    tracing::info!(number_of_yaks, "preparing to shave yaks");

    let number_shaved = yak_shave::shave_all(number_of_yaks);
    tracing::info!(
        all_yaks_shaved = number_shaved == number_of_yaks,
        "yak shaving completed"
    );
}
