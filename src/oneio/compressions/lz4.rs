//! LZ4 compression support for OneIO.
//!
//! This module provides the [`OneIOLz4`] struct, which implements the [`OneIOCompression`] trait
//! for reading lz4-compressed files. Writing lz4-compressed files is not currently supported.

use crate::oneio::compressions::OneIOCompression;
use crate::OneIoError;
use lz4::Decoder;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

pub(crate) struct OneIOLz4;

impl OneIOCompression for OneIOLz4 {
    /// Returns a reader that decompresses lz4-compressed data from the given reader.
    ///
    /// # Arguments
    /// * `raw_reader` - A boxed reader containing lz4-compressed data.
    ///
    /// # Returns
    /// * `Ok(Box<dyn Read + Send>)` - A reader that decompresses lz4 data on the fly.
    /// * `Err(OneIoError)` - If the lz4 decoder could not be created.
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(Decoder::new(raw_reader).unwrap()))
    }

    /// Returns an error because lz4 writer is not currently supported.
    ///
    /// # Arguments
    /// * `_raw_writer` - A buffered writer for the target file (unused).
    ///
    /// # Returns
    /// * `Err(OneIoError)` - Always returns an error indicating lz4 writer is not supported.
    fn get_writer(_raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Err(OneIoError::NotSupported(
            "lz4 writer is not currently supported.".to_string(),
        ))
    }
}
