use crate::oneio::compressions::OneIOCompression;
use crate::OneIoError;
use libflate::finish::AutoFinishUnchecked;
use libflate::gzip::Decoder;
use libflate::gzip::Encoder;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

pub(crate) struct OneIOGzip;

impl OneIOCompression for OneIOGzip {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(BufReader::new(Decoder::new(raw_reader)?)))
    }

    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        // see libflate docs on the reasons of using [AutoFinishUnchecked].
        let encoder = AutoFinishUnchecked::new(Encoder::new(raw_writer)?);
        Ok(Box::new(encoder))
    }
}
