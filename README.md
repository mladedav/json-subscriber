# `json-subscriber`

`json-subscriber` is (mostly) a drop-in replacement for `tracing_subscriber::fmt().json()`.

It provides helpers to be as compatible as possible with `tracing_subscriber` while also allowing
for simple extensions to the format to include custom data in the log lines.

The end goal is for each user to be able to define the structure of their JSON log lines as they
wish.  Currently, the library only allows what `tracing-subscriber` plus OpenTelemetry trace and
span IDs.

## Compatibility

However you created your `FmtSubscriber` or `fmt::Layer`, the same thing should work in this crate.

For example in `README.md` in Tracing, you can see an yak-shaving example where if you just change
`tracing_subscriber` to `json_subscriber`, everything will work the same, except the logs will be in
JSON.

```rust
use tracing::info;
use json_subscriber;

fn main() {
    // install global collector configured based on RUST_LOG env var.
    json_subscriber::fmt::init();

    let number_of_yaks = 3;
    // this creates a new event, outside of any spans.
    info!(number_of_yaks, "preparing to shave yaks");

    let number_shaved = yak_shave::shave_all(number_of_yaks);
    info!(
        all_yaks_shaved = number_shaved == number_of_yaks,
        "yak shaving completed."
    );
}
```

Most configuration under `tracing_subscriber::fmt` should work equivalently. For example one can
create a layer like this:

```rust
json_subscriber::fmt()
    // .json()
    .with_max_level(tracing::Level::TRACE)
    .with_current_span(false)
    .init();
```

Calling `.json()` is not needed and the method does nothing and is marked as deprecated. It is kept
around for simpler migration from `tracing-subscriber` though.

Trying to call `.pretty()` or `.compact()` will however result in an error. `json-tracing` does not
support any output other than JSON.

## Extensions

### OpenTelemetry

To include trace ID and span ID from opentelemetry in log lines, simply call
`with_opentelemetry_ids`. This will have no effect if you don't also configure a
`tracing-opentelemetry` layer.

```rust
let tracer = todo!();
let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
let json = json_subscriber::layer()
    .with_current_span(false)
    .with_span_list(false)
    .with_opentelemetry_ids(true);

tracing_subscriber::registry()
    .with(opentelemetry)
    .with(json)
    .init();
```

This will produce log lines like for example this (without the formatting):

```json
{
  "fields": {
    "message": "shaving yaks"
  },
  "level": "INFO",
  "openTelemetry": {
    "spanId": "35249d86bfbcf774",
    "traceId": "fb4b6ae1fa52d4aaf56fa9bda541095f"
  },
  "target": "readme_opentelemetry::yak_shave",
  "timestamp": "2024-06-06T23:09:07.620167Z"
}
```

See the `readme-opentelemetry` example for full code.

### Custom

You can also specify custom static fields to be added to each log line, or serialize extensions provided by other `Layer`s:

```rust
#[derive(Serialize)]
struct Foo(String);

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for FooLayer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();
        let mut extensions = span.extensions_mut();
        let foo = Foo("hello".to_owned());
        extensions.insert(foo);
    }
}

fn main() {
  let foo_layer = FooLayer;

  let mut layer = json_subscriber::JsonLayer::stdout();
  layer.serialize_extension::<Foo>("foo");

  registry().with(foo_layer).with(layer);
}
```

## Supported Rust Versions

`json-subscriber` is built against the latest stable release. The minimum supported version is 1.65.
The current version is not guaranteed to build on Rust versions earlier than the minimum supported
version.

`json-subscriber` follows the same compiler support policies as the Tokio project. The current
stable Rust compiler and the three most recent minor versions before it will always be supported.
For example, if the current stable compiler version is 1.69, the minimum supported version will not
be increased past 1.66, three minor versions prior. Increasing the minimum supported compiler
version is not considered a semver breaking change as long as doing so complies with this policy.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
`json-subscriber` by you, shall be licensed as MIT, without any additional terms or conditions.
