use crate::OneIoError;
use std::fs::File;
use std::io::{BufWriter, Read, Write};

#[cfg(feature = "bz")]
pub(crate) mod bzip2;
#[cfg(feature = "gz")]
pub(crate) mod gzip;
#[cfg(feature = "lz")]
pub(crate) mod lz4;
#[cfg(feature = "xz")]
pub(crate) mod xz;

pub trait OneIOCompression {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError>;
    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError>;
}
