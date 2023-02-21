use std::fs::File;
use std::io::{BufWriter, Read, Write};
use lz4::Decoder;
use crate::oneio::OneIOCompression;
use crate::{OneIoError, OneIoErrorKind};

pub(crate) struct OneIOLz4;

impl OneIOCompression for OneIOLz4 {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError> {
        Ok(Box::new(Decoder::new(raw_reader).unwrap()))
    }

    fn get_writer(_raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError> {
        Err(OneIoError{ kind: OneIoErrorKind::NotSupported("lz4 writer is not currently supported.".to_string()) })
    }
}

