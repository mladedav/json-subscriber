use std::io;

use serde_json::ser::Formatter;

pub(crate) struct JsonSubscriberFormatterInsideObject {
    depth: usize,
}

impl JsonSubscriberFormatterInsideObject {
    pub(crate) fn new() -> Self {
        Self { depth: 0 }
    }
}

impl Formatter for JsonSubscriberFormatterInsideObject {
    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.depth += 1;
        if self.depth > 1 {
            writer.write_all(b"{")
        } else {
            Ok(())
        }
    }

    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.depth -= 1;
        if self.depth > 0 {
            writer.write_all(b"}")
        } else {
            Ok(())
        }
    }
}
