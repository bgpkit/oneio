use crate::oneio::compressions::OneIOCompression;
use crate::OneIoError;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

pub(crate) struct OneIOGzip;

impl OneIOCompression for OneIOGzip {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(GzDecoder::new(raw_reader)))
    }

    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        // see libflate docs on the reasons of using [AutoFinishUnchecked].
        let encoder = GzEncoder::new(raw_writer, Compression::default());
        Ok(Box::new(encoder))
    }
}
