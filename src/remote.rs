//! This module provides functionality to handle remote file operations such as downloading files
//! from HTTP, FTP, and S3 protocols.
use crate::client::OneIo;
use crate::OneIoError;
#[cfg(feature = "http")]
use reqwest::blocking::Client;
#[cfg(feature = "ftp")]
use std::io::Read;

#[cfg(feature = "ftp")]
pub(crate) fn get_ftp_reader_raw(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    if !path.starts_with("ftp://") {
        return Err(OneIoError::NotSupported(path.to_string()));
    }

    #[cfg(feature = "rustls")]
    crate::crypto::ensure_default_provider()?;

    let path_without_scheme = path
        .strip_prefix("ftp://")
        .ok_or_else(|| OneIoError::NotSupported(path.to_string()))?;
    let (host, remote_path) = path_without_scheme
        .split_once('/')
        .ok_or_else(|| OneIoError::NotSupported(path.to_string()))?;
    let socket = match host.contains(':') {
        true => host.to_string(),
        false => format!("{host}:21"),
    };

    let mut ftp_stream = suppaftp::FtpStream::connect(socket)?;
    // use anonymous login
    ftp_stream.login("anonymous", "oneio")?;
    ftp_stream.transfer_type(suppaftp::types::FileType::Binary)?;
    let reader = Box::new(ftp_stream.retr_as_stream(remote_path)?);
    Ok(reader)
}

#[cfg(feature = "http")]
pub(crate) fn get_http_reader_raw(
    path: &str,
    client: &Client,
) -> Result<reqwest::blocking::Response, OneIoError> {
    let res = client
        .execute(client.get(path).build()?)?
        .error_for_status()
        .map_err(|e| OneIoError::NetworkWithContext {
            source: Box::new(e),
            url: path.to_string(),
        })?;
    Ok(res)
}

/// Creates a reqwest blocking client with custom headers.
///
/// Prefer [`OneIo::builder()`] for reusable configuration. This helper is
/// **deprecated** and will be removed in a future release. Use the builder instead.
#[deprecated(
    since = "0.21.0",
    note = "Use OneIo::builder().header_str(k, v).build()?.http_client().clone() instead"
)]
#[allow(dead_code)]
#[cfg(feature = "http")]
pub fn create_client_with_headers<I, K, V>(headers: I) -> Result<Client, OneIoError>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    let mut builder = OneIo::builder();
    for (k, v) in headers {
        builder = builder.header_str(k.into().as_str(), v.into().as_str());
    }
    Ok(builder.build()?.http_client().clone())
}

#[cfg(feature = "http")]
pub(crate) fn get_http_content_length(path: &str, client: &Client) -> Result<u64, OneIoError> {
    let response = client.head(path).send()?.error_for_status()?;

    response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            OneIoError::NotSupported(
                "Cannot determine file size - server doesn't provide Content-Length".to_string(),
            )
        })
}

/// Check if a remote or local file exists.
///
/// # Arguments
///
/// * `path` - The path of the file to check.
///
/// # Returns
///
/// Returns a `Result` containing a `bool` indicating whether the file exists or not. If the path is not supported,
/// an `Err` variant with a `OneIoError::NotSupported` error is returned. If there is an error during the file check,
/// an `Err` variant with a `OneIoError` is returned.
pub(crate) fn http_file_exists(path: &str, client: &Client) -> Result<bool, OneIoError> {
    let res = client
        .head(path)
        .timeout(std::time::Duration::from_secs(2))
        .send()?;
    Ok(res.status().is_success())
}
