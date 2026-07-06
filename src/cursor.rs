use std::{
    cell::{RefCell, RefMut},
    fmt, io,
};

pub(crate) struct Cursor<'buf>(RefCell<&'buf mut Vec<u8>>);

impl io::Write for &Cursor<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'buf> Cursor<'buf> {
    pub fn new(inner: &'buf mut Vec<u8>) -> Self {
        Self(RefCell::new(inner))
    }

    pub fn inner_mut(&self) -> RefMut<'_, &'buf mut Vec<u8>> {
        self.0.borrow_mut()
    }
}

pub(crate) struct FmtWrite<'a>(pub(crate) &'a mut Vec<u8>);

impl fmt::Write for FmtWrite<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
