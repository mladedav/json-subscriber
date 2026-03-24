use crate::cursor::Cursor;

/// A writer passed to closures registered with
/// [`add_multiple_dynamic_fields`](crate::JsonLayer::add_multiple_dynamic_fields).
///
/// Call [`write_field`](FieldWriter::write_field) to add key-value pairs directly to the JSON
/// output. Keys are `&str` (no allocation required for static strings), and values accept any
/// type that implements [`serde::Serialize`].
pub struct FieldWriter<'a> {
    writer: &'a mut String,
    prefix_comma: bool,
    wrote_anything: bool,
}

impl<'a> FieldWriter<'a> {
    pub(crate) fn new(writer: &'a mut String, prefix_comma: bool) -> Self {
        Self {
            writer,
            prefix_comma,
            wrote_anything: false,
        }
    }

    pub(crate) fn wrote_anything(&self) -> bool {
        self.wrote_anything
    }

    /// Writes a single key-value pair into the JSON output.
    ///
    /// The key is serialized as a quoted, escaped JSON string. The value can be any type
    /// implementing [`serde::Serialize`]: strings, numbers, booleans, `Option<T>`, structs
    /// with `#[derive(Serialize)]`, etc.
    ///
    /// Returns `Err` if serialization fails. On error the writer state is rolled back so the
    /// output remains valid JSON. Errors from this method are rare and indicate a bug in the
    /// `Serialize` implementation of the value type.
    ///
    /// # Errors
    ///
    /// This function errors if serialization of the provided value fails. This means that either
    /// the implementation of `Serialize` decides to fail, or if the value contains a map with
    /// non-string keys.
    pub fn write_field(
        &mut self,
        key: &str,
        value: impl serde::Serialize,
    ) -> serde_json::Result<()> {
        let rollback = self.writer.len();

        if self.wrote_anything || self.prefix_comma {
            self.writer.push(',');
        }

        if let Err(error) = serde_json::to_writer(&Cursor::new(self.writer), key) {
            self.writer.truncate(rollback);
            return Err(error);
        }

        self.writer.push(':');

        if let Err(error) = serde_json::to_writer(&Cursor::new(self.writer), &value) {
            self.writer.truncate(rollback);
            return Err(error);
        }

        self.wrote_anything = true;
        Ok(())
    }
}
