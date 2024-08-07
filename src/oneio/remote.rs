use crate::oneio::compressions::OneIOCompression;
use crate::oneio::{compressions, get_writer_raw};
use crate::OneIoError;
use reqwest::blocking::Client;
use std::io::Read;

fn get_protocol(path: &str) -> Option<String> {
    let parts = path.split("://").collect::<Vec<&str>>();
    if parts.len() < 2 {
        return None;
    }
    Some(parts[0].to_string())
}

fn get_ftp_reader_raw(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    if !path.starts_with("ftp://") {
        return Err(OneIoError::NotSupported(path.to_string()));
    }

    let parts = path.split('/').collect::<Vec<&str>>();
    let socket = match parts[2].contains(':') {
        true => parts[2].to_string(),
        false => format!("{}:21", parts[2]),
    };
    let path = parts[3..].join("/");

    let mut ftp_stream = suppaftp::FtpStream::connect(socket)?;
    ftp_stream.login("anonymous", "oneio").unwrap();
    ftp_stream.transfer_type(suppaftp::types::FileType::Binary)?;
    let reader = Box::new(ftp_stream.retr_as_stream(path.as_str())?);
    Ok(reader)
}

fn get_http_reader_raw(
    path: &str,
    opt_client: Option<Client>,
) -> Result<reqwest::blocking::Response, OneIoError> {
    dotenvy::dotenv().ok();
    let accept_invalid_certs = matches!(
        std::env::var("ONEIO_ACCEPT_INVALID_CERTS")
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "true" | "yes" | "y" | "1"
    );

    let client = match opt_client {
        Some(c) => c,
        None => {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::USER_AGENT,
                reqwest::header::HeaderValue::from_static("oneio"),
            );
            headers.insert(
                reqwest::header::CONTENT_LENGTH,
                reqwest::header::HeaderValue::from_static("0"),
            );
            #[cfg(feature = "cli")]
            headers.insert(
                reqwest::header::CACHE_CONTROL,
                reqwest::header::HeaderValue::from_static("no-cache"),
            );
            Client::builder()
                .default_headers(headers)
                .danger_accept_invalid_certs(accept_invalid_certs)
                .build()?
        }
    };
    let res = client
        .execute(client.get(path).build()?)?
        .error_for_status()?;
    Ok(res)
}

/// Get a reader for remote content with the capability to specify headers, and customer reqwest options.
///
/// Example usage with custom header fields:
/// ```no_run
/// use std::collections::HashMap;
/// use reqwest::header::HeaderMap;
/// let headers: HeaderMap = (&HashMap::from([("X-Custom-Auth-Key".to_string(), "TOKEN".to_string())])).try_into().expect("invalid headers");
/// let client = reqwest::blocking::Client::builder()
///        .default_headers(headers)
///        .build().unwrap();
/// let mut reader = oneio::get_http_reader(
///   "https://SOME_REMOTE_RESOURCE_PROTECTED_BY_ACCESS_TOKEN",
///   Some(client),
/// ).unwrap();
/// let mut text = "".to_string();
/// reader.read_to_string(&mut text).unwrap();
/// println!("{}", text);
/// ```
///
/// Example with customer builder that allows invalid certificates (bad practice):
/// ```no_run
/// use std::collections::HashMap;
/// let client = reqwest::blocking::ClientBuilder::new().danger_accept_invalid_certs(true).build().unwrap();
/// let mut reader = oneio::get_http_reader(
///     "https://example.com",
///     Some(client)
/// ).unwrap();
/// let mut text = "".to_string();
/// reader.read_to_string(&mut text).unwrap();
/// println!("{}", text);
/// ```
pub fn get_http_reader(
    path: &str,
    opt_client: Option<Client>,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    let raw_reader: Box<dyn Read + Send> = Box::new(get_http_reader_raw(path, opt_client)?);
    let file_type = *path.split('.').collect::<Vec<&str>>().last().unwrap();
    match file_type {
        #[cfg(feature = "gz")]
        "gz" | "gzip" => compressions::gzip::OneIOGzip::get_reader(raw_reader),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => compressions::bzip2::OneIOBzip2::get_reader(raw_reader),
        #[cfg(feature = "lz4")]
        "lz4" | "lz" => compressions::lz4::OneIOLz4::get_reader(raw_reader),
        #[cfg(feature = "xz")]
        "xz" | "xz2" | "lzma" => compressions::xz::OneIOXz::get_reader(raw_reader),
        _ => {
            // unknown file type of file {}. try to read as uncompressed file
            Ok(Box::new(raw_reader))
        }
    }
}

/// Downloads a file from a remote location to a local path.
///
/// # Arguments
///
/// * `remote_path` - The remote path of the file to download.
/// * `local_path` - The local path where the downloaded file will be saved.
/// * `header` - Optional header information to include in the request. If not specified, an empty HashMap should be provided.
///
/// # Errors
///
/// Returns an `Err` variant of `OneIoError` if any of the following occur:
///
/// * The protocol of the remote path is not supported.
/// * An error occurs while downloading the file.
///
/// # Example
///
/// ```rust,no_run
/// use std::collections::HashMap;
/// use crate::oneio::{download, OneIoError};
///
/// fn main() -> Result<(), OneIoError> {
///     let remote_path = "https://example.com/file.txt";
///     let local_path = "path/to/save/file.txt";
///     download(remote_path, local_path, None)?;
///
///     Ok(())
/// }
/// ```
pub fn download(
    remote_path: &str,
    local_path: &str,
    opt_client: Option<Client>,
) -> Result<(), OneIoError> {
    match get_protocol(remote_path) {
        None => {
            return Err(OneIoError::NotSupported(remote_path.to_string()));
        }
        Some(protocol) => match protocol.as_str() {
            "http" | "https" => {
                let mut writer = get_writer_raw(local_path)?;
                let mut response = get_http_reader_raw(remote_path, opt_client)?;
                response.copy_to(&mut writer)?;
            }
            "ftp" => {
                let mut writer = get_writer_raw(local_path)?;
                let mut reader = get_ftp_reader_raw(remote_path)?;
                std::io::copy(&mut reader, &mut writer)?;
            }
            #[cfg(feature = "s3")]
            "s3" => {
                let (bucket, path) = crate::oneio::s3::s3_url_parse(remote_path)?;
                crate::oneio::s3::s3_download(bucket.as_str(), path.as_str(), local_path)?;
            }
            _ => {
                return Err(OneIoError::NotSupported(remote_path.to_string()));
            }
        },
    };
    Ok(())
}

/// Downloads a file from a remote path and saves it locally with retry mechanism.
///
/// # Arguments
///
/// * `remote_path` - The URL or file path of the file to download.
/// * `local_path` - The file path to save the downloaded file.
/// * `header` - Optional headers to include in the download request.
/// * `retry` - The number of times to retry downloading in case of failure.
///
/// # Errors
///
/// Returns an `Err` variant if downloading fails after all retries, otherwise `Ok(())` indicating success.
///
/// # Examples
///
/// ```rust,no_run
/// use oneio::download_with_retry;
///
/// let remote_path = "https://example.com/file.txt";
/// let local_path = "/path/to/save/file.txt";
/// let retry = 3;
///
/// match download_with_retry(remote_path, local_path, retry, None) {
///     Ok(_) => println!("File downloaded successfully"),
///     Err(e) => eprintln!("Error downloading file: {:?}", e),
/// }
/// ```
pub fn download_with_retry(
    remote_path: &str,
    local_path: &str,
    retry: usize,
    opt_client: Option<Client>,
) -> Result<(), OneIoError> {
    let mut retry = retry;
    loop {
        match download(remote_path, local_path, opt_client.clone()) {
            Ok(_) => {
                return Ok(());
            }
            Err(e) => {
                if retry > 0 {
                    retry -= 1;
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }
}

pub(crate) fn get_reader_raw_remote(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let raw_reader: Box<dyn Read + Send> = match get_protocol(path) {
        Some(protocol) => match protocol.as_str() {
            "http" | "https" => {
                let response = get_http_reader_raw(path, None)?;
                Box::new(response)
            }
            "ftp" => {
                let response = get_ftp_reader_raw(path)?;
                Box::new(response)
            }
            #[cfg(feature = "s3")]
            "s3" => {
                let (bucket, path) = crate::oneio::s3::s3_url_parse(path)?;
                Box::new(crate::oneio::s3::s3_reader(bucket.as_str(), path.as_str())?)
            }
            _ => {
                return Err(OneIoError::NotSupported(path.to_string()));
            }
        },
        None => Box::new(std::fs::File::open(path)?),
    };

    Ok(raw_reader)
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
pub(crate) fn remote_file_exists(path: &str) -> Result<bool, OneIoError> {
    match get_protocol(path) {
        Some(protocol) => match protocol.as_str() {
            "http" | "https" => {
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(2))
                    .build()?;
                let res = client.head(path).send()?;
                Ok(res.status().is_success())
            }
            #[cfg(feature = "s3")]
            "s3" => {
                let (bucket, path) = crate::oneio::s3::s3_url_parse(path)?;
                let res = crate::oneio::s3::s3_exists(bucket.as_str(), path.as_str())?;
                Ok(res)
            }
            _ => Err(OneIoError::NotSupported(path.to_string())),
        },
        None => {
            // check if local file exists
            Ok(std::path::Path::new(path).exists())
        }
    }
}
