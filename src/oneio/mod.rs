pub mod compressions;
#[cfg(feature = "digest")]
pub mod digest;
#[cfg(any(feature = "http", feature = "ftp"))]
pub mod remote;
#[cfg(feature = "s3")]
pub mod s3;

pub mod utils;

use crate::OneIoError;

use crate::oneio::compressions::{get_compression_reader, get_compression_writer};
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

/// Extracts the protocol from a given path.
pub(crate) fn get_protocol(path: &str) -> Option<String> {
    let parts = path.split("://").collect::<Vec<&str>>();
    if parts.len() < 2 {
        return None;
    }
    Some(parts[0].to_string())
}

pub fn get_writer_raw(path: &str) -> Result<BufWriter<File>, OneIoError> {
    let path = Path::new(path);
    if let Some(prefix) = path.parent() {
        std::fs::create_dir_all(prefix)?;
    }
    let output_file = BufWriter::new(File::create(path)?);
    Ok(output_file)
}

pub fn get_reader_raw(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let raw_reader: Box<dyn Read + Send> = match get_protocol(path) {
        Some(protocol) => match protocol.as_str() {
            #[cfg(feature = "http")]
            "http" | "https" => {
                let response = remote::get_http_reader_raw(path, None)?;
                Box::new(response)
            }
            #[cfg(feature = "ftp")]
            "ftp" => {
                let response = remote::get_ftp_reader_raw(path)?;
                Box::new(response)
            }
            #[cfg(feature = "s3")]
            "s3" | "r2" => {
                let (bucket, path) = s3::s3_url_parse(path)?;
                Box::new(s3::s3_reader(bucket.as_str(), path.as_str())?)
            }
            _ => {
                return Err(OneIoError::NotSupported(path.to_string()));
            }
        },
        None => Box::new(File::open(path)?),
    };
    Ok(raw_reader)
}

/// Gets a reader for the given file path.
///
/// # Arguments
///
/// * `path` - The path of the file to read.
///
/// # Returns
///
/// A `Result` containing a boxed `Read+Sync` trait object with the file reader, or `OneIoError` if an error occurs.
///
/// # Examples
///
/// ```no_run
/// use std::io::Read;
/// use oneio::get_reader;
///
/// let mut reader = get_reader("file.txt").unwrap();
/// let mut buffer = Vec::new();
/// reader.read_to_end(&mut buffer).unwrap();
/// println!("{}", String::from_utf8_lossy(&buffer));
/// ```
pub fn get_reader(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    // get raw bytes reader
    let raw_reader = get_reader_raw(path)?;

    let file_type = *path.split('.').collect::<Vec<&str>>().last().unwrap();
    get_compression_reader(raw_reader, file_type)
}

/// get file reader with local cache.
///
/// parameters:
/// * `path`: file path to open, remote or local
/// * `cache_dir`: path str to cache directory
/// * `cache_file_name`: optional file name for cache file, default to use the same filename as the to-read file
/// * `force_cache`: whether to force refresh cache file if a local cache file already exists
pub fn get_cache_reader(
    path: &str,
    cache_dir: &str,
    cache_file_name: Option<String>,
    force_cache: bool,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    let dir_path = Path::new(cache_dir);
    if !dir_path.is_dir() {
        match std::fs::create_dir_all(dir_path) {
            Ok(_) => {}
            Err(e) => {
                return Err(OneIoError::CacheIoError(format!(
                    "cache directory creation failed: {}",
                    e
                )))
            }
        }
    }

    let cache_file_name = cache_file_name.unwrap_or_else(|| {
        path.split('/')
            .collect::<Vec<&str>>()
            .into_iter()
            .next_back()
            .unwrap()
            .to_string()
    });

    let cache_file_path = format!("{}/{}", cache_dir, cache_file_name);

    // if cache file already exists
    if !force_cache && Path::new(cache_file_path.as_str()).exists() {
        return get_reader(cache_file_path.as_str());
    }

    // read all to cache file, no encode/decode happens
    let mut reader = get_reader_raw(path)?;
    let mut data: Vec<u8> = vec![];
    reader.read_to_end(&mut data)?;
    let mut writer = get_writer_raw(cache_file_path.as_str())?;
    writer.write_all(&data).unwrap();
    drop(writer);

    // return reader from cache file
    get_reader(cache_file_path.as_str())
}

/// Returns a writer for the given file path with the corresponding compression.
///
/// # Arguments
///
/// * `path` - A string slice representing the file path.
///
/// # Returns
///
/// * `Result<Box<dyn Write>, OneIoError>` - A result containing a boxed writer trait object or an error.
///
/// # Examples
///
/// ```rust,no_run
/// use std::io::{self, Write};
/// use oneio::get_writer;
///
/// let writer = match get_writer("output.txt") {
///     Ok(writer) => writer,
///     Err(error) => panic!("Failed to create writer: {:?}", error),
/// };
/// ```
pub fn get_writer(path: &str) -> Result<Box<dyn Write>, OneIoError> {
    let output_file = BufWriter::new(File::create(path)?);

    let file_type = *path.split('.').collect::<Vec<&str>>().last().unwrap();
    get_compression_writer(output_file, file_type)
}

/// Check if a file or directory exists.
///
/// This function takes a path as an argument and returns a `Result` indicating whether the file or directory at the given path exists or not.
///
/// # Examples
///
/// ```rust
/// use crate::oneio::exists;
///
/// match exists("path/to/file.txt") {
///     Ok(true) => println!("File exists."),
///     Ok(false) => println!("File does not exist."),
///     Err(error) => eprintln!("An error occurred: {}", error),
/// }
/// ```
///
/// # Errors
///
/// This function may return a `OneIoError` if there is an error accessing the file system or if the `remote` feature is enabled and there is an error
pub fn exists(path: &str) -> Result<bool, OneIoError> {
    #[cfg(any(feature = "http", feature = "s3"))]
    {
        remote::remote_file_exists(path)
    }
    #[cfg(not(any(feature = "http", feature = "s3")))]
    {
        Ok(Path::new(path).exists())
    }
}
