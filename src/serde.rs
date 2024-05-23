use serde::ser::SerializeMap;
use tracing_core::Subscriber;

use crate::layer::FormattedFields;

pub(crate) struct SerializableContext<'a, 'b, Span>(
    pub(crate) &'b tracing_subscriber::registry::SpanRef<'a, Span>,
)
where
    Span: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>;

impl<'a, 'b, Span> serde::ser::Serialize for SerializableContext<'a, 'b, Span>
where
    Span: Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn serialize<Ser>(&self, serializer_o: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::ser::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut serializer = serializer_o.serialize_seq(None)?;

        for span in self.0.scope().from_root() {
            serializer.serialize_element(&SerializableSpan(&span))?;
        }

        serializer.end()
    }
}

pub(crate) struct SerializableSpan<'a, 'b, Span>(
    pub(crate) &'b tracing_subscriber::registry::SpanRef<'a, Span>,
)
where
    Span: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>;

impl<'a, 'b, Span> serde::ser::Serialize for SerializableSpan<'a, 'b, Span>
where
    Span: for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::ser::Serializer,
    {
        let mut serializer = serializer.serialize_map(None)?;

        let ext = self.0.extensions();
        let fields = ext
            .get::<FormattedFields>()
            .expect("Unable to find FormattedFields in extensions; this is a bug");

        for field in &fields.fields {
            serializer.serialize_entry(&field.0, &field.1)?;
        }
        serializer.serialize_entry("name", self.0.metadata().name())?;
        serializer.end()
    }
}
