mod tracing_serde;

use serde_json::ser::Formatter;
pub(crate) use tracing_serde::RenamedFields;

pub(crate) struct JsonSubscriberFormatter;

impl Formatter for JsonSubscriberFormatter {}
