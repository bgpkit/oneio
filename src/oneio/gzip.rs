use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use crate::oneio::OneIOCompression;
use crate::OneIoError;

pub(crate) struct OneIOGzip;

impl OneIOCompression for OneIOGzip {
    fn get_reader(raw_reader: Box<dyn Read>) -> Result<Box<dyn BufRead>, OneIoError> {
        let reader = Box::new(GzDecoder::new(raw_reader));
        Ok(Box::new(BufReader::new(reader)))
    }

    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(GzEncoder::new(raw_writer, Compression::default())))
    }
}

