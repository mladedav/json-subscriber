use arc_swap::access::Access;
use serde_json::Value;
use tracing::{Level, Subscriber};
use tracing_subscriber::{
    fmt::{
        format::Writer,
        time::{FormatTime, SystemTime},
        MakeWriter,
    },
    registry::LookupSpan,
};

use crate::{visitor::JsonVisitor, JsonLayer};

const BUNYAN_VERSION: &str = "v";
const LEVEL: &str = "level";
const NAME: &str = "name";
const HOSTNAME: &str = "hostname";
const PID: &str = "pid";
const TIME: &str = "time";
const MESSAGE: &str = "msg";

pub fn layer<S, W>(make_writer: W) -> JsonLayer<S, W>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    W: for<'writer> MakeWriter<'writer> + 'static,
{
    let mut layer = JsonLayer::new(make_writer);
    layer.add_static_field(BUNYAN_VERSION, Value::String("0".to_owned()));
    layer.add_dynamic_field(LEVEL, |event, _| {
        Some(to_bunyan_level(*event.metadata().level()))
    });
    layer.add_static_field(NAME, Value::String("json-subscriber".to_owned()));
    layer.add_static_field(
        HOSTNAME,
        Value::String(gethostname::gethostname().to_string_lossy().into_owned()),
    );
    layer.add_static_field(PID, Value::from(std::process::id()));
    layer.add_dynamic_field(TIME, |_, _| {
        let mut timestamp = String::with_capacity(32);
        SystemTime
            .format_time(&mut Writer::new(&mut timestamp))
            .ok()?;
        Some(timestamp)
    });
    layer.add_raw_dynamic_field(MESSAGE, |event, write| {
        write.write_char('\"')?;
        match event
            .fields()
            .fields()
            .get("message")
            .and_then(|field| field.load_full())
        {
            Some(message) => write.write_str(message.as_str())?,
            None => write.write_str(event.metadata().target())?,
        }
        write.write_char('\"')
    });
    layer.with_file("file");
    layer.with_target("target");
    layer.with_line_number("line");
    layer.with_event("fields", false);
    layer.flatten_span_list("spans");

    layer
}

fn to_bunyan_level(level: Level) -> u8 {
    if level == Level::ERROR {
        50
    } else if level == Level::WARN {
        40
    } else if level == Level::INFO {
        30
    } else if level == Level::DEBUG {
        20
    } else if level == Level::TRACE {
        10
    } else {
        0
    }
}
