//! Compression algorithms and utilities for OneIO.
//!
//! This module provides a unified interface for reading and writing files with various compression
//! formats, including gzip, bzip2, lz4, xz, and zstd. The available algorithms depend on enabled
//! Cargo features.

use crate::OneIoError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

/// Returns a compression reader for the given file suffix.
///
/// This function selects the appropriate compression algorithm based on the provided
/// `file_suffix` (such as `"gz"`, `"bz2"`, `"lz4"`, `"xz"`, or `"zst"`), and returns a
/// reader that transparently decompresses data as it is read. If the suffix is not recognized,
/// the original `raw_reader` is returned unchanged.
pub(crate) fn get_compression_reader(
    raw_reader: Box<dyn Read + Send>,
    file_suffix: &str,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    match file_suffix {
        #[cfg(feature = "any_gz")]
        "gz" | "gzip" | "tgz" => gzip::get_reader(raw_reader),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => bzip2::get_reader(raw_reader),
        #[cfg(feature = "lz")]
        "lz4" | "lz" => lz4::get_reader(raw_reader),
        #[cfg(feature = "xz")]
        "xz" | "xz2" | "lzma" => xz::get_reader(raw_reader),
        #[cfg(feature = "zstd")]
        "zst" | "zstd" => zstd::get_reader(raw_reader),
        _ => {
            // unknown file type - return the raw bytes reader as is
            Ok(raw_reader)
        }
    }
}

/// Returns a compression writer for the given file suffix.
///
/// This function selects the appropriate compression algorithm based on the provided
/// `file_suffix` (such as `"gz"`, `"bz2"`, `"lz4"`, `"xz"`, or `"zst"`), and returns a
/// writer that transparently compresses data as it is written. If the suffix is not recognized,
/// the original `raw_writer` is returned unchanged.
pub(crate) fn get_compression_writer(
    raw_writer: BufWriter<File>,
    file_suffix: &str,
) -> Result<Box<dyn Write>, OneIoError> {
    match file_suffix {
        #[cfg(feature = "any_gz")]
        "gz" | "gzip" | "tgz" => gzip::get_writer(raw_writer),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => bzip2::get_writer(raw_writer),
        #[cfg(feature = "lz")]
        "lz4" | "lz" => lz4::get_writer(raw_writer),
        #[cfg(feature = "xz")]
        "xz" | "xz2" | "lzma" => xz::get_writer(raw_writer),
        #[cfg(feature = "zstd")]
        "zst" | "zstd" => zstd::get_writer(raw_writer),
        _ => Ok(Box::new(raw_writer)),
    }
}

#[cfg(feature = "any_gz")]
pub(crate) mod gzip {
    use crate::OneIoError;
    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs::File;
    use std::io::{BufWriter, Read, Write};

    pub(crate) fn get_reader(
        raw_reader: Box<dyn Read + Send>,
    ) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(GzDecoder::new(raw_reader)))
    }

    pub(crate) fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(GzEncoder::new(raw_writer, Compression::default())))
    }
}

#[cfg(feature = "bz")]
pub(crate) mod bzip2 {
    use crate::OneIoError;
    use std::fs::File;
    use std::io::{BufWriter, Read, Write};

    pub(crate) fn get_reader(
        raw_reader: Box<dyn Read + Send>,
    ) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(bzip2::read::BzDecoder::new(raw_reader)))
    }

    pub(crate) fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(bzip2::write::BzEncoder::new(
            raw_writer,
            bzip2::Compression::default(),
        )))
    }
}

#[cfg(feature = "lz")]
pub(crate) mod lz4 {
    use crate::OneIoError;
    use std::fs::File;
    use std::io::{BufWriter, Read, Write};

    pub(crate) fn get_reader(
        raw_reader: Box<dyn Read + Send>,
    ) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(lz4::Decoder::new(raw_reader)?))
    }

    pub(crate) fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        let encoder = lz4::EncoderBuilder::new().build(raw_writer)?;
        Ok(Box::new(Lz4Writer(Some(encoder))))
    }

    /// Wrapper around `lz4::Encoder` that writes the frame end marker on drop.
    ///
    /// `lz4::Encoder` has no `Drop` impl — `finish()` must be called explicitly
    /// to flush the end-of-stream marker. Without it the compressed stream is
    /// incomplete and the decoder returns 0 bytes.
    struct Lz4Writer<W: Write>(Option<lz4::Encoder<W>>);

    impl<W: Write> Write for Lz4Writer<W> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.as_mut().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.as_mut().unwrap().flush()
        }
    }

    impl<W: Write> Drop for Lz4Writer<W> {
        fn drop(&mut self) {
            if let Some(encoder) = self.0.take() {
                let (mut w, result) = encoder.finish();
                if result.is_ok() {
                    let _ = w.flush();
                }
            }
        }
    }
}

#[cfg(feature = "xz")]
pub(crate) mod xz {
    use crate::OneIoError;
    use std::fs::File;
    use std::io::{BufWriter, Read, Write};

    pub(crate) fn get_reader(
        raw_reader: Box<dyn Read + Send>,
    ) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(xz2::read::XzDecoder::new(raw_reader)))
    }

    pub(crate) fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(xz2::write::XzEncoder::new(raw_writer, 6)))
    }
}

#[cfg(feature = "zstd")]
pub(crate) mod zstd {
    use crate::OneIoError;
    use std::fs::File;
    use std::io::{BufWriter, Read, Write};

    pub(crate) fn get_reader(
        raw_reader: Box<dyn Read + Send>,
    ) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(zstd::Decoder::new(raw_reader)?))
    }

    pub(crate) fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        let encoder = zstd::Encoder::new(raw_writer, 3)?;
        Ok(Box::new(encoder.auto_finish()))
    }
}
