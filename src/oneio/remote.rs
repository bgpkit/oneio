use crate::oneio::compressions::OneIOCompression;
use crate::oneio::{compressions, get_writer_raw};
use crate::OneIoError;
use std::collections::HashMap;
use std::io::Read;

fn get_protocol(path: &str) -> Option<String> {
    let parts = path.split("://").collect::<Vec<&str>>();
    if parts.len() < 2 {
        return None;
    }
    Some(parts[0].to_string())
}

fn get_remote_ftp_raw(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
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

fn get_remote_http_raw(
    path: &str,
    header: HashMap<String, String>,
) -> Result<reqwest::blocking::Response, OneIoError> {
    let mut headers: reqwest::header::HeaderMap = (&header).try_into().expect("invalid headers");
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("oneio"),
    );
    #[cfg(feature = "cli")]
    headers.insert(
        reqwest::header::CACHE_CONTROL,
        reqwest::header::HeaderValue::from_static("no-cache"),
    );
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()?;
    let res = client
        .execute(client.get(path).build()?)?
        .error_for_status()?;
    Ok(res)
}

#[cfg(feature = "remote")]
/// get a reader for remote content with the capability to specify headers.
///
/// Example usage:
/// ```no_run
/// use std::collections::HashMap;
/// let mut reader = oneio::get_remote_reader(
///   "https://SOME_REMOTE_RESOURCE_PROTECTED_BY_ACCESS_TOKEN",
///   HashMap::from([("X-Custom-Auth-Key".to_string(), "TOKEN".to_string())])
/// ).unwrap();
/// let mut text = "".to_string();
/// reader.read_to_string(&mut text).unwrap();
/// println!("{}", text);
/// ```
pub fn get_remote_reader(
    path: &str,
    header: HashMap<String, String>,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    let raw_reader: Box<dyn Read + Send> = Box::new(get_remote_http_raw(path, header)?);
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

#[cfg(feature = "remote")]
pub fn download(
    remote_path: &str,
    local_path: &str,
    header: Option<HashMap<String, String>>,
) -> Result<(), OneIoError> {
    match get_protocol(remote_path) {
        None => {
            return Err(OneIoError::NotSupported(remote_path.to_string()));
        }
        Some(protocol) => match protocol.as_str() {
            "http" | "https" => {
                let mut writer = get_writer_raw(local_path)?;
                let mut response = get_remote_http_raw(remote_path, header.unwrap_or_default())?;
                response.copy_to(&mut writer)?;
            }
            "ftp" => {
                let mut writer = get_writer_raw(local_path)?;
                let mut reader = get_remote_ftp_raw(remote_path)?;
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

pub(crate) fn get_reader_raw_remote(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let raw_reader: Box<dyn Read + Send> = match get_protocol(path) {
        Some(protocol) => match protocol.as_str() {
            "http" | "https" => {
                let response = get_remote_http_raw(path, HashMap::new())?;
                Box::new(response)
            }
            "ftp" => {
                let response = get_remote_ftp_raw(path)?;
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
