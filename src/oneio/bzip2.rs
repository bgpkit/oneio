use std::fs::File;
use std::io::{BufWriter, Read, Write};
use bzip2::Compression;
use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use crate::oneio::OneIOCompression;
use crate::OneIoError;

pub(crate) struct OneIOBzip2;

impl OneIOCompression for OneIOBzip2 {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(BzDecoder::new(raw_reader)))
    }

    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(BzEncoder::new(raw_writer, Compression::default())))
    }
}

