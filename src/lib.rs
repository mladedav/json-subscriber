//! # `json-subscriber`
//!
//! `json-subscriber` is (mostly) a drop-in replacement for `tracing_subscriber::fmt().json()`.
//!
//! It provides helpers to be as compatible as possible with `tracing_subscriber` while also
//! allowing for simple extensions to the format to include custom data in the log lines.
//!
//! The end goal is for each user to be able to define the structure of their JSON log lines as they
//! wish.  Currently, the library only allows what `tracing-subscriber` plus OpenTelemetry trace and
//! span IDs.
//!
//! ## Compatibility
//!
//! However you created your `FmtSubscriber` or `fmt::Layer`, the same thing should work in this
//! crate.
//!
//! For example in `README.md` in Tracing, you can see an yak-shaving example where if you just
//! change `tracing_subscriber` to `json_subscriber`, everything will work the same, except the logs
//! will be in JSON.
//!
//! ```rust
//! use tracing::info;
//! use json_subscriber;
//! #
//! # mod yak_shave { pub fn shave_all(n: u32) -> u32 { n } }
//!
//! // install global collector configured based on RUST_LOG env var.
//! json_subscriber::fmt::init();
//!
//! let number_of_yaks = 3;
//! // this creates a new event, outside of any spans.
//! info!(number_of_yaks, "preparing to shave yaks");
//!
//! let number_shaved = yak_shave::shave_all(number_of_yaks);
//! info!(
//!     all_yaks_shaved = number_shaved == number_of_yaks,
//!     "yak shaving completed."
//! );
//! ```
//!
//! Most configuration under `tracing_subscriber::fmt` should work equivalently. For example one can
//! create a layer like this:
//!
//! ```rust
//! json_subscriber::fmt()
//!     // .json()
//!     .with_max_level(tracing::Level::TRACE)
//!     .with_current_span(false)
//!     .init();
//! ```
//!
//! Calling `.json()` is not needed and the method does nothing and is marked as deprecated. It is
//! kept around for simpler migration from `tracing-subscriber` though.
//!
//! Trying to call `.pretty()` or `.compact()` will however result in an error. `json-tracing` does
//! not support any output other than JSON.
//!
//! ## Extensions
//!
//! ### OpenTelemetry
//!
//! To include trace ID and span ID from opentelemetry in log lines, simply call
//! `with_opentelemetry_ids`. This will have no effect if you don't also configure a
//! `tracing-opentelemetry` layer.
//!
//! ```rust
//! # #[cfg(opentelemetry)]
//! # {
//! let tracer = todo!();
//! let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
//! let json = json_subscriber::layer()
//!     .with_current_span(false)
//!     .with_span_list(false)
//!     .with_opentelemetry_ids(true);
//!
//! tracing_subscriber::registry()
//!     .with(opentelemetry)
//!     .with(json)
//!     .init();
//! # }
//! ```
//!
//! This will produce log lines like for example this (without the formatting):
//!
//! ```json
//! {
//!   "fields": {
//!     "message": "shaving yaks"
//!   },
//!   "level": "INFO",
//!   "openTelemetry": {
//!     "spanId": "35249d86bfbcf774",
//!     "traceId": "fb4b6ae1fa52d4aaf56fa9bda541095f"
//!   },
//!   "target": "readme_opentelemetry::yak_shave",
//!   "timestamp": "2024-06-06T23:09:07.620167Z"
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]
#![warn(clippy::pedantic)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_lines)]

pub mod bunyan;
mod cached;
mod cursor;
mod fields;
pub mod fmt;
mod layer;
mod serde;
mod visitor;
mod write_adaptor;

#[cfg(test)]
mod tests;

pub use fmt::{fmt, layer};
pub use layer::JsonLayer;
