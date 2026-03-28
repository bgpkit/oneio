//! This module contains functions to calculate the digest of a file.
//!
//! The digest is calculated using the SHA256 algorithm.

use crate::OneIoError;
use ring::digest::{Context, SHA256};

/// Calculate the SHA256 digest of a file.
///
/// This function takes a path to a file as input and returns the SHA256 digest of the file
/// as a hexadecimal string.
pub fn get_sha256_digest(path: &str) -> Result<String, OneIoError> {
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1024];

    // Open file for reading
    let file = std::fs::File::open(path)?;
    let mut reader: Box<dyn std::io::Read + Send> = Box::new(std::io::BufReader::new(file));

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    let digest = context.finish();
    Ok(hex::encode(digest.as_ref()))
}
