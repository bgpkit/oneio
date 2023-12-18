use crate::oneio::compressions::OneIOCompression;
use crate::OneIoError;
use lz4::Decoder;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

pub(crate) struct OneIOLz4;

impl OneIOCompression for OneIOLz4 {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(Decoder::new(raw_reader).unwrap()))
    }

    fn get_writer(_raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Err(OneIoError::NotSupported(
            "lz4 writer is not currently supported.".to_string(),
        ))
    }
}
