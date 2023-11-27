use crate::oneio::OneIOCompression;
use crate::OneIoError;
use libflate::gzip::Decoder;
use libflate::gzip::Encoder;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

pub(crate) struct OneIOGzip;

impl OneIOCompression for OneIOGzip {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(Decoder::new(raw_reader)?))
    }

    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Ok(Box::new(Encoder::new(raw_writer)?))
    }
}
