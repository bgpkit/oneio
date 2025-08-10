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
                return Err(OneIoError::Io(e))
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

/// Progress tracking callback type
pub type ProgressCallback<F> = F;

/// Progress reader wrapper that tracks bytes read
pub struct ProgressReader<R, F> {
    inner: R,
    bytes_read: u64,
    total_size: u64,
    callback: F,
}

impl<R: Read, F> ProgressReader<R, F>
where
    F: Fn(u64, u64) + Send,
{
    fn new(inner: R, total_size: u64, callback: F) -> Self {
        Self {
            inner,
            bytes_read: 0,
            total_size,
            callback,
        }
    }
}

impl<R: Read, F> Read for ProgressReader<R, F>
where
    F: Fn(u64, u64) + Send,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        if bytes_read > 0 {
            self.bytes_read += bytes_read as u64;
            (self.callback)(self.bytes_read, self.total_size);
        }
        Ok(bytes_read)
    }
}

/// Determines the content length of a file or URL
/// 
/// This function attempts to get the total size of the content at the given path.
/// It fails early if the size cannot be determined reliably.
///
/// # Arguments
/// * `path` - File path or URL to check
///
/// # Returns
/// * `Ok(u64)` - Total content size in bytes
/// * `Err(OneIoError::NotSupported)` - If size cannot be determined
/// * `Err(OneIoError::Network)` - If network error occurs
/// * `Err(OneIoError::Io)` - If file system error occurs
pub fn get_content_length(path: &str) -> Result<u64, OneIoError> {
    match get_protocol(path) {
        #[cfg(feature = "http")]
        Some(protocol) if protocol == "http" || protocol == "https" => {
            // HEAD request to get Content-Length
            let client = reqwest::blocking::Client::new();
            let response = client.head(path).send()?;
            
            response
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| {
                    OneIoError::NotSupported(
                        "Cannot determine file size - server doesn't provide Content-Length"
                            .to_string(),
                    )
                })
        }
        #[cfg(feature = "ftp")]
        Some(protocol) if protocol == "ftp" => {
            // For FTP, we'll need to implement SIZE command
            // For now, return not supported
            Err(OneIoError::NotSupported(
                "FTP size determination not yet implemented".to_string(),
            ))
        }
        #[cfg(feature = "s3")]
        Some(protocol) if protocol == "s3" || protocol == "r2" => {
            // S3 HEAD object
            let (bucket, key) = s3::s3_url_parse(path)?;
            let stats = s3::s3_stats(&bucket, &key)?;
            Ok(stats.size)
        }
        Some(_) => Err(OneIoError::NotSupported(format!(
            "Protocol not supported for progress tracking: {}",
            path
        ))),
        None => {
            // Local file
            let metadata = std::fs::metadata(path)?;
            Ok(metadata.len())
        }
    }
}

/// Gets a reader with progress tracking that reports bytes read and total file size
///
/// This function returns both a reader and the total file size. It fails early if the
/// total size cannot be determined (e.g., streaming endpoints without Content-Length).
///
/// The progress callback receives (bytes_read, total_bytes) and tracks raw bytes read
/// from the source before any decompression.
///
/// # Arguments
/// * `path` - File path or URL to read
/// * `progress` - Callback function called with (bytes_read, total_bytes)
///
/// # Returns
/// * `Ok((reader, total_size))` - Reader and total file size in bytes
/// * `Err(OneIoError::NotSupported)` - If file size cannot be determined
/// * `Err(OneIoError::Network)` - If network error occurs
/// * `Err(OneIoError::Io)` - If file system error occurs
///
/// # Examples
///
/// ```rust,no_run
/// use oneio;
/// use std::io::Read;
///
/// let (mut reader, total_size) = oneio::get_reader_with_progress(
///     "https://example.com/file.gz",
///     |bytes_read, total_bytes| {
///         let percent = (bytes_read as f64 / total_bytes as f64) * 100.0;
///         println!("Progress: {:.1}% ({}/{})", percent, bytes_read, total_bytes);
///     }
/// )?;
///
/// println!("File size: {} bytes", total_size);
/// let mut content = String::new();
/// reader.read_to_string(&mut content)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn get_reader_with_progress<F>(
    path: &str,
    progress: F,
) -> Result<(Box<dyn Read + Send>, u64), OneIoError>
where
    F: Fn(u64, u64) + Send + 'static,
{
    // Determine total size first - fail early if not possible
    let total_size = get_content_length(path)?;
    
    // Get raw reader (before compression)
    let raw_reader = get_reader_raw(path)?;
    
    // Wrap raw reader with progress tracking
    let progress_reader = ProgressReader::new(raw_reader, total_size, progress);
    
    // Apply compression to the progress-wrapped reader
    let file_type = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    
    let final_reader = get_compression_reader(Box::new(progress_reader), file_type)?;
    
    Ok((final_reader, total_size))
}
