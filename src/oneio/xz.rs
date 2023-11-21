use crate::oneio::OneIOCompression;
use crate::OneIoError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use xz2::read::XzDecoder;
use xz2::write::XzEncoder;

pub(crate) struct OneIOXz;

impl OneIOCompression for OneIOXz {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(XzDecoder::new(raw_reader)))
    }

    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(XzEncoder::new(raw_writer, 9)))
    }
}
