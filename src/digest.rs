//! This module contains functions to calculate the digest of a file.
//!
//! The digest is calculated using the SHA256 algorithm.

use crate::OneIoError;
use ring::digest::{Context, SHA256};

/// Calculate the SHA256 digest of a file.
///
/// This function takes a path to a file as input and returns the SHA256 digest of the file
/// as a hexadecimal string. Supports both local files and remote URLs (HTTP, HTTPS, FTP, S3).
///
/// Note: This function computes the hash of the raw file bytes without any decompression,
/// even if the file has a compression extension (e.g., `.gz`, `.bz2`).
///
/// # Arguments
///
/// * `path` - The path to the file. Can be a local file path or a remote URL.
///
/// # Returns
///
/// Returns the SHA256 digest as a hexadecimal string, or an error if the file cannot be read.
pub fn get_sha256_digest(path: &str) -> Result<String, OneIoError> {
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1024];

    // Open file for reading (supports both local and remote files, no decompression)
    let mut reader = crate::OneIo::new()?.get_reader_raw(path)?;

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
