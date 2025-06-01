use crate::OneIoError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

#[cfg(feature = "bz")]
pub(crate) mod bzip2;
#[cfg(feature = "gz")]
pub(crate) mod gzip;
#[cfg(feature = "lz")]
pub(crate) mod lz4;
#[cfg(feature = "xz")]
pub(crate) mod xz;
#[cfg(feature = "zstd")]
pub(crate) mod zstd;

pub trait OneIOCompression {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError>;
    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError>;
}

/// Returns a reader for the given file path, handling compression based on the file type.
pub(crate) fn get_compression_reader(
    raw_reader: Box<dyn Read + Send>,
    file_suffix: &str,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    match file_suffix {
        #[cfg(feature = "gz")]
        "gz" | "gzip" | "tgz" => gzip::OneIOGzip::get_reader(raw_reader),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => bzip2::OneIOBzip2::get_reader(raw_reader),
        #[cfg(feature = "lz")]
        "lz4" | "lz" => lz4::OneIOLz4::get_reader(raw_reader),
        #[cfg(feature = "xz")]
        "xz" | "xz2" | "lzma" => xz::OneIOXz::get_reader(raw_reader),
        #[cfg(feature = "zstd")]
        "zst" | "zstd" => zstd::OneIOZstd::get_reader(raw_reader),
        _ => {
            // unknown file type of file {}. return the raw bytes reader as is
            Ok(raw_reader)
        }
    }
}

pub(crate) fn get_compression_writer(
    raw_writer: BufWriter<File>,
    file_suffix: &str,
) -> Result<Box<dyn Write>, OneIoError> {
    match file_suffix {
        #[cfg(feature = "gz")]
        "gz" | "gzip" | "tgz" => gzip::OneIOGzip::get_writer(raw_writer),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => bzip2::OneIOBzip2::get_writer(raw_writer),
        #[cfg(feature = "lz")]
        "lz4" | "lz" => lz4::OneIOLz4::get_writer(raw_writer),
        #[cfg(feature = "xz")]
        "xz" | "xz2" | "lzma" => xz::OneIOXz::get_writer(raw_writer),
        #[cfg(feature = "zstd")]
        "zst" | "zstd" => zstd::OneIOZstd::get_writer(raw_writer),
        _ => Ok(Box::new(raw_writer)),
    }
}
