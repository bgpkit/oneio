use std::io::{BufReader, Read};
use tracing::info;

mod error;

pub use error::{OneIoError, OneIoErrorKind};

#[cfg(feature = "bz")]
use bzip2::read::BzDecoder;
#[cfg(feature = "gz")]
use flate2::read::GzDecoder;
#[cfg(feature = "lz")]
use lz4::Decoder as LzDecoder;

/// create a [BufReader] on heap from a given path to a file, located locally or remotely.
pub fn get_reader(path: &str) -> Result<Box<dyn Read>, OneIoError> {
    // create reader for reading raw content from local or remote source, bytes can be compressed
    let raw_reader: Box<dyn Read> = match path.starts_with("http") {
        true => {
            let response = reqwest::blocking::get(path)?;
            Box::new(response)
        }
        false => {
            Box::new(std::fs::File::open(path)?)
        }
    };

    let file_type = path.split(".").collect::<Vec<&str>>().last().unwrap().clone();
    match file_type {
        #[cfg(feature="gz")]
        "gz" | "gzip" => {
            let reader = Box::new(GzDecoder::new(raw_reader));
            Ok(Box::new(BufReader::new(reader)))
        }
        #[cfg(feature="bz")]
        "bz2" | "bz" => {
            let reader = Box::new(BzDecoder::new(raw_reader));
            Ok(Box::new(BufReader::new(reader)))
        }
        #[cfg(feature="lz4")]
        "lz4"| "lz" => {
            let reader = Box::new(LzDecoder::new(raw_reader).unwrap());
            Ok(Box::new(BufReader::new(reader)))
        }
        _ => {
            info!("unknown file type of file {}. try to read as uncompressed file", path);
            let reader = Box::new(raw_reader);
            Ok(Box::new(BufReader::new(reader)))
        }
    }

}

pub fn get_buf_reader(path: &str) -> Result<BufReader<Box<dyn Read>>, OneIoError> {
    let reader = get_reader(path)?;

    Ok(BufReader::new(reader))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reader() {

    }
}