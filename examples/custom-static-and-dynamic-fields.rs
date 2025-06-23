use json_subscriber::fmt;
use serde_json::Number;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() {
    let mut json_layer = fmt::layer()
        .with_level(true)
        // .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .flatten_event(true);

    let inner_layer = json_layer.inner_layer_mut();
    // Setting it inside the inner layer allows us to rename the field.
    inner_layer.with_thread_ids("thread_id");
    // This field will not change during the lifetime of the application.
    inner_layer.add_static_field("app", serde_json::Value::String("monitor".to_owned()));
    // This field might change (we might fork a subprocess or something)
    inner_layer.add_dynamic_field("pid", |_, _| {
        Some(serde_json::Value::Number(Number::from(std::process::id())))
    });

    tracing_subscriber::registry().with(json_layer).init();

    tracing::info!("Log with logger, app, and pid.");
    // Prints the following json (without newlines and indentation):
    // {
    //     "app": "monitor",
    //     "filename": "examples/custom-static-and-dynamic-fields.rs",
    //     "level": "INFO",
    //     "line_number": 24,
    //     "pid": 3341821,
    //     "thread_id": "ThreadId(1)",
    //     "timestamp": "2025-06-23T20:04:51.512451Z",
    //     "message": "Log with logger, app, and pid."
    // }
}
