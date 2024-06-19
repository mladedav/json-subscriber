use std::{
    fmt,
    io,
    sync::{Arc, Mutex, MutexGuard, TryLockError},
};

use tracing_subscriber::fmt::{format::Writer, time::FormatTime, MakeWriter};

pub(crate) struct MockWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl MockWriter {
    pub(crate) fn new(buf: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { buf }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub(crate) fn map_error<Guard>(err: TryLockError<Guard>) -> io::Error {
        match err {
            TryLockError::WouldBlock => io::Error::from(io::ErrorKind::WouldBlock),
            TryLockError::Poisoned(_) => io::Error::from(io::ErrorKind::Other),
        }
    }

    pub(crate) fn buf(&self) -> io::Result<MutexGuard<'_, Vec<u8>>> {
        self.buf.try_lock().map_err(Self::map_error)
    }
}

impl io::Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf()?.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buf()?.flush()
    }
}

#[derive(Clone, Default)]
pub(crate) struct MockMakeWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl MockMakeWriter {
    pub(crate) fn buf(&self) -> MutexGuard<'_, Vec<u8>> {
        self.buf.lock().unwrap()
    }
}

impl<'a> MakeWriter<'a> for MockMakeWriter {
    type Writer = MockWriter;

    fn make_writer(&'a self) -> Self::Writer {
        MockWriter::new(self.buf.clone())
    }
}

pub(crate) struct MockTime;
impl FormatTime for MockTime {
    fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
        write!(w, "fake time")
    }
}
