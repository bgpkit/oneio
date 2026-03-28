//! Async reader support for OneIO.

use crate::OneIoError;
#[cfg(feature = "async")]
use futures::StreamExt;
#[cfg(feature = "async")]
use tokio::io::{AsyncRead, AsyncReadExt};

/// Gets an async reader for the given file path
///
/// This is the async version of `get_reader()`. It supports all the same protocols
/// and compression formats as the sync version.
#[cfg(feature = "async")]
pub async fn get_reader_async(path: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>, OneIoError> {
    let raw_reader = get_async_reader_raw(path).await?;
    let file_type = crate::file_extension(path);
    get_async_compression_reader(raw_reader, file_type)
}

/// Reads the entire content of a file asynchronously into a string
#[cfg(feature = "async")]
pub async fn read_to_string_async(path: &str) -> Result<String, OneIoError> {
    let mut reader = get_reader_async(path).await?;
    let mut content = String::new();
    reader.read_to_string(&mut content).await?;
    Ok(content)
}

/// Downloads a file asynchronously from a URL to a local path
#[cfg(feature = "async")]
pub async fn download_async(url: &str, path: &str) -> Result<(), OneIoError> {
    use std::path::Path;
    use tokio::fs::File;
    use tokio::io::{copy, AsyncWriteExt};

    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }

    let mut reader = get_async_reader_raw(url).await?;
    let mut file = File::create(path).await?;
    copy(&mut reader, &mut file).await?;
    file.flush().await?;
    Ok(())
}

/// Gets a raw async reader for the given path (before compression)
#[cfg(feature = "async")]
async fn get_async_reader_raw(path: &str) -> Result<Box<dyn AsyncRead + Send + Unpin>, OneIoError> {
    let raw_reader: Box<dyn AsyncRead + Send + Unpin> = match crate::get_protocol(path) {
        #[cfg(feature = "http")]
        Some(protocol) if protocol == "http" || protocol == "https" => {
            #[cfg(feature = "rustls")]
            crate::crypto::ensure_default_provider()?;

            let response = reqwest::get(path).await?;
            let stream = response
                .bytes_stream()
                .map(|result| result.map_err(std::io::Error::other));
            Box::new(tokio_util::io::StreamReader::new(stream))
        }
        #[cfg(feature = "ftp")]
        Some(protocol) if protocol == "ftp" => {
            return Err(OneIoError::NotSupported(
                "FTP async not supported - use sync get_reader() instead".to_string(),
            ));
        }
        #[cfg(feature = "s3")]
        Some(protocol) if protocol == "s3" || protocol == "r2" => {
            return Err(OneIoError::NotSupported(
                "S3 async not supported - use sync get_reader() instead".to_string(),
            ));
        }
        Some(_) => {
            return Err(OneIoError::NotSupported(format!(
                "Async support not available for protocol in path: {path}"
            )));
        }
        None => {
            // Local file
            use tokio::fs::File;
            let file = File::open(path).await?;
            Box::new(file)
        }
    };
    Ok(raw_reader)
}

/// Applies async decompression based on file extension
#[cfg(feature = "async")]
fn get_async_compression_reader(
    reader: Box<dyn AsyncRead + Send + Unpin>,
    file_type: &str,
) -> Result<Box<dyn AsyncRead + Send + Unpin>, OneIoError> {
    match file_type {
        #[cfg(all(feature = "async", feature = "any_gz"))]
        "gz" | "gzip" => {
            use async_compression::tokio::bufread::GzipDecoder;
            use tokio::io::BufReader;
            let buf_reader = BufReader::new(reader);
            let decoder = GzipDecoder::new(buf_reader);
            Ok(Box::new(decoder))
        }
        #[cfg(all(feature = "async", feature = "bz"))]
        "bz" | "bz2" => {
            use async_compression::tokio::bufread::BzDecoder;
            use tokio::io::BufReader;
            let buf_reader = BufReader::new(reader);
            let decoder = BzDecoder::new(buf_reader);
            Ok(Box::new(decoder))
        }
        #[cfg(all(feature = "async", feature = "zstd"))]
        "zst" | "zstd" => {
            use async_compression::tokio::bufread::ZstdDecoder;
            use tokio::io::BufReader;
            let buf_reader = BufReader::new(reader);
            let decoder = ZstdDecoder::new(buf_reader);
            Ok(Box::new(decoder))
        }
        #[cfg(all(feature = "async", feature = "lz"))]
        "lz4" | "lz" => Err(OneIoError::NotSupported(
            "LZ4 async decompression not yet supported - use spawn_blocking with sync version"
                .to_string(),
        )),
        #[cfg(all(feature = "async", feature = "xz"))]
        "xz" | "xz2" => Err(OneIoError::NotSupported(
            "XZ async decompression not yet supported - use spawn_blocking with sync version"
                .to_string(),
        )),
        _ => Ok(reader),
    }
}
