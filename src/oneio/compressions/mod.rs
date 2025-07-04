//! Compression algorithms and utilities for OneIO.
//!
//! This module provides a unified interface for reading and writing files with various compression
//! formats, including gzip, bzip2, lz4, xz, and zstd. The available algorithms depend on enabled
//! Cargo features. Each compression algorithm implements the [`OneIOCompression`] trait, which
//! defines methods for creating readers and writers that transparently handle compression and
//! decompression. Utility functions are provided to select the appropriate algorithm based on file
//! suffixes.

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

/// Trait for compression algorithms used in OneIO.
///
/// This trait defines the interface for compression and decompression implementations.
/// Types implementing this trait provide methods to create readers and writers that
/// transparently handle compression or decompression for supported formats.
///
/// # Required Methods
///
/// - `get_reader`: Returns a boxed reader that decompresses data from the given reader.
/// - `get_writer`: Returns a boxed writer that compresses data to the given writer.
///
/// # Errors
///
/// Both methods return a `Result` that contains an error if the compression or decompression
/// stream could not be created.
pub trait OneIOCompression {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError>;
    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError>;
}

/// Returns a compression reader for the given file suffix.
///
/// This function selects the appropriate compression algorithm based on the provided
/// `file_suffix` (such as `"gz"`, `"bz2"`, `"lz4"`, `"xz"`, or `"zst"`), and returns a
/// reader that transparently decompresses data as it is read. If the suffix is not recognized,
/// the original `raw_reader` is returned unchanged.
///
/// # Arguments
///
/// * `raw_reader` - A boxed reader implementing `Read + Send`, typically the source file or stream.
/// * `file_suffix` - The file extension or suffix indicating the compression type.
///
/// # Returns
///
/// * `Ok(Box<dyn Read + Send>)` - A boxed reader that decompresses data on the fly, or the original reader if no compression is detected.
/// * `Err(OneIoError)` - If the compression reader could not be created.
///
/// # Feature Flags
///
/// The available compression algorithms depend on enabled Cargo features:
/// - `"gz"` for gzip
/// - `"bz"` for bzip2
/// - `"lz"` for lz4
/// - `"xz"` for xz/lzma
/// - `"zstd"` for zstandard
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

/// Returns a compression writer for the given file suffix.
///
/// This function selects the appropriate compression algorithm based on the provided
/// `file_suffix` (such as `"gz"`, `"bz2"`, `"lz4"`, `"xz"`, or `"zst"`), and returns a
/// writer that transparently compresses data as it is written. If the suffix is not recognized,
/// the original `raw_writer` is returned unchanged.
///
/// # Arguments
///
/// * `raw_writer` - A buffered writer for the target file.
/// * `file_suffix` - The file extension or suffix indicating the compression type.
///
/// # Returns
///
/// * `Ok(Box<dyn Write>)` - A boxed writer that compresses data on the fly, or the original writer if no compression is detected.
/// * `Err(OneIoError)` - If the compression writer could not be created.
///
/// # Feature Flags
///
/// The available compression algorithms depend on enabled Cargo features:
/// - `"gz"` for gzip
/// - `"bz"` for bzip2
/// - `"lz"` for lz4
/// - `"xz"` for xz/lzma
/// - `"zstd"` for zstandard
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
