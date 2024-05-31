use std::{cell::RefCell, fmt::Write, io};

pub(crate) struct Cursor<'buf>(RefCell<&'buf mut String>);

impl io::Write for &Cursor<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.0.borrow_mut();
        let s =
            std::str::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        inner.write_str(s);

        Ok(s.as_bytes().len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'buf> Cursor<'buf> {
    pub fn new(inner: &'buf mut String) -> Self {
        Self(RefCell::new(inner))
    }

    pub fn position(&self) -> usize {
        self.0.borrow().len()
    }
}
