//! Zstandard (zstd) compression support for OneIO.
//!
//! This module provides the [`OneIOZstd`] struct, which implements the [`OneIOCompression`] trait
//! for reading and writing zstd-compressed files.

use crate::oneio::compressions::OneIOCompression;
use crate::OneIoError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

pub(crate) struct OneIOZstd;

impl OneIOCompression for OneIOZstd {
    /// Returns a reader that decompresses zstd-compressed data from the given reader.
    ///
    /// # Arguments
    /// * `raw_reader` - A boxed reader containing zstd-compressed data.
    ///
    /// # Returns
    /// * `Ok(Box<dyn Read + Send>)` - A reader that decompresses zstd data on the fly.
    /// * `Err(OneIoError)` - If the zstd decoder could not be created.
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        match zstd::Decoder::new(raw_reader) {
            Ok(dec) => Ok(Box::new(dec)),
            Err(e) => Err(OneIoError::IoError(e)),
        }
    }

    /// Returns a writer that compresses data to zstd format.
    ///
    /// # Arguments
    /// * `raw_writer` - A buffered writer for the target file.
    ///
    /// # Returns
    /// * `Ok(Box<dyn Write>)` - A writer that compresses data to zstd format.
    /// * `Err(OneIoError)` - If the zstd encoder could not be created.
    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        match zstd::Encoder::new(raw_writer, 9) {
            Ok(dec) => Ok(Box::new(dec.auto_finish())),
            Err(e) => Err(OneIoError::IoError(e)),
        }
    }
}
