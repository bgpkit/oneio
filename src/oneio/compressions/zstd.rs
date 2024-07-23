use crate::oneio::compressions::OneIOCompression;
use crate::OneIoError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

pub(crate) struct OneIOZstd;

impl OneIOCompression for OneIOZstd {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        match zstd::Decoder::new(raw_reader) {
            Ok(dec) => Ok(Box::new(dec)),
            Err(e) => Err(OneIoError::IoError(e)),
        }
    }

    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        match zstd::Encoder::new(raw_writer, 9) {
            Ok(dec) => Ok(Box::new(dec.auto_finish())),
            Err(e) => Err(OneIoError::IoError(e)),
        }
    }
}
