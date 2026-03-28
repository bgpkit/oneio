//! Progress tracking reader for OneIO.

use std::io::Read;

/// Progress reader wrapper that tracks bytes read
pub(crate) struct ProgressReader<R, F> {
    inner: R,
    bytes_read: u64,
    total_size: u64,
    callback: F,
}

impl<R: Read, F> ProgressReader<R, F>
where
    F: Fn(u64, u64) + Send,
{
    pub(crate) fn new(inner: R, total_size: u64, callback: F) -> Self {
        Self {
            inner,
            bytes_read: 0,
            total_size,
            callback,
        }
    }
}

impl<R: Read, F> Read for ProgressReader<R, F>
where
    F: Fn(u64, u64) + Send,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        if bytes_read > 0 {
            self.bytes_read += bytes_read as u64;
            (self.callback)(self.bytes_read, self.total_size);
        }
        Ok(bytes_read)
    }
}
