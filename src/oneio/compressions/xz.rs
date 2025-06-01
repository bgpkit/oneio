//! XZ compression support for OneIO.
//!
//! This module provides the [`OneIOXz`] struct, which implements the [`OneIOCompression`] trait
//! for reading and writing xz-compressed files.

use crate::oneio::compressions::OneIOCompression;
use crate::OneIoError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use xz2::read::XzDecoder;
use xz2::write::XzEncoder;

pub(crate) struct OneIOXz;

impl OneIOCompression for OneIOXz {
    /// Returns a reader that decompresses xz-compressed data from the given reader.
    ///
    /// # Arguments
    /// * `raw_reader` - A boxed reader containing xz-compressed data.
    ///
    /// # Returns
    /// * `Ok(Box<dyn Read + Send>)` - A reader that decompresses xz data on the fly.
    /// * `Err(OneIoError)` - If the xz decoder could not be created.
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(XzDecoder::new(raw_reader)))
    }

    /// Returns a writer that compresses data to xz format.
    ///
    /// # Arguments
    /// * `raw_writer` - A buffered writer for the target file.
    ///
    /// # Returns
    /// * `Ok(Box<dyn Write>)` - A writer that compresses data to xz format.
    /// * `Err(OneIoError)` - If the xz encoder could not be created.
    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(XzEncoder::new(raw_writer, 9)))
    }
}
