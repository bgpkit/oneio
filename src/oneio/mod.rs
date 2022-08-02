#[cfg(feature = "lz")]
mod lz4;
#[cfg(feature = "gz")]
mod gzip;
#[cfg(feature = "bz")]
mod bzip2;

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use crate::OneIoError;

pub trait OneIOCompression {
    fn get_reader(raw_reader: Box<dyn Read>) -> Result<Box<dyn BufRead>, OneIoError>;
    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError>;
}

pub fn get_reader(path: &str) -> Result<Box<dyn BufRead>, OneIoError> {
    #[cfg(feature="remote")]
    let raw_reader: Box<dyn Read> = match path.starts_with("http") {
        true => {
            let response = reqwest::blocking::get(path)?;
            Box::new(response)
        }
        false => {
            Box::new(std::fs::File::open(path)?)
        }
    };
    #[cfg(not(feature="remote"))]
    let raw_reader: Box<dyn Read> = Box::new(std::fs::File::open(path)?);

    let file_type = path.split(".").collect::<Vec<&str>>().last().unwrap().clone();
    match file_type {
        #[cfg(feature="gz")]
        "gz" | "gzip" => {
            gzip::OneIOGzip::get_reader(raw_reader)
        }
        #[cfg(feature="bz")]
        "bz2" | "bz" => {
            bzip2::OneIOBzip2::get_reader(raw_reader)
        }
        #[cfg(feature="lz4")]
        "lz4"| "lz" => {
            lz4::OneIOLz4::get_reader(raw_reader)
        }
        _ => {
            // unknown file type of file {}. try to read as uncompressed file
            let reader = Box::new(raw_reader);
            Ok(Box::new(BufReader::new(reader)))
        }
    }
}

pub fn get_writer(path: &str) -> Result<Box<dyn Write>, OneIoError> {
    let output_file = BufWriter::new(File::create(path)?);

    let file_type = path.split(".").collect::<Vec<&str>>().last().unwrap().clone();
    match file_type {
        #[cfg(feature = "gz")]
        "gz" | "gzip" => {
            gzip::OneIOGzip::get_writer(output_file)
        }
        #[cfg(feature = "bz")]
        "bz2" | "bz" => {
            bzip2::OneIOBzip2::get_writer(output_file)
        }
        #[cfg(feature = "lz4")]
        "lz4" | "lz" => {
            lz4::OneIOLz4::get_writer(output_file)
        }
        _ => {
            Ok(Box::new(BufWriter::new(output_file)))
        }
    }
}
