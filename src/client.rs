use crate::compression::{get_compression_reader, get_compression_writer};
#[cfg(any(feature = "http", feature = "ftp"))]
use crate::remote;
#[cfg(feature = "s3")]
use crate::s3;
use crate::OneIoError;
#[cfg(feature = "http")]
use reqwest::blocking::Client;
#[cfg(feature = "json")]
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Lines, Read, Write};
use std::path::Path;

/// Reusable OneIO client for applying request configuration across multiple operations.
///
/// Use [`OneIo::builder()`] to customize default headers, TLS certificates, and
/// other HTTP options once, then reuse the resulting client across reads and
/// downloads.
#[derive(Clone)]
pub struct OneIo {
    #[cfg(feature = "http")]
    pub(crate) http_client: Client,
}

impl OneIo {
    /// Creates a new reusable OneIO client with default configuration.
    pub fn new() -> Result<Self, OneIoError> {
        Self::builder().build()
    }

    /// Creates a new builder for customizing a reusable [`OneIo`] client.
    pub fn builder() -> crate::builder::OneIoBuilder {
        crate::builder::OneIoBuilder::new()
    }

    /// Wraps an already-constructed reqwest blocking client.
    #[cfg(feature = "http")]
    pub fn from_client(http_client: Client) -> Self {
        Self { http_client }
    }

    /// Returns the underlying reqwest blocking client.
    #[cfg(feature = "http")]
    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    /// Creates a raw writer without compression.
    pub fn get_writer_raw(&self, path: &str) -> Result<BufWriter<File>, OneIoError> {
        crate::get_writer_raw_impl(path)
    }

    /// Creates a writer with compression inferred from the path extension.
    pub fn get_writer(&self, path: &str) -> Result<Box<dyn Write>, OneIoError> {
        let output_file = self.get_writer_raw(path)?;
        let file_type = crate::file_extension(path);
        get_compression_writer(output_file, file_type)
    }

    /// Creates a raw reader without decompression.
    pub fn get_reader_raw(&self, path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
        let raw_reader: Box<dyn Read + Send> = match crate::get_protocol(path) {
            Some(protocol) => match protocol {
                #[cfg(feature = "http")]
                "http" | "https" => Box::new(self.get_http_reader_raw(path)?),
                #[cfg(feature = "ftp")]
                "ftp" => remote::get_ftp_reader_raw(path)?,
                #[cfg(feature = "s3")]
                "s3" | "r2" => {
                    let (bucket, path) = s3::s3_url_parse(path)?;
                    s3::s3_reader(bucket.as_str(), path.as_str())?
                }
                _ => return Err(OneIoError::NotSupported(path.to_string())),
            },
            None => Box::new(File::open(path)?),
        };
        Ok(raw_reader)
    }

    /// Creates a reader with decompression inferred from the path extension.
    pub fn get_reader(&self, path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
        let raw_reader = self.get_reader_raw(path)?;
        let file_type = crate::file_extension(path);
        get_compression_reader(raw_reader, file_type)
    }

    /// Creates a reader with explicit compression type override.
    ///
    /// Useful for URLs with query params or non-standard extensions.
    /// Pass empty string for no decompression.
    pub fn get_reader_with_type(
        &self,
        path: &str,
        compression: &str,
    ) -> Result<Box<dyn Read + Send>, OneIoError> {
        let raw_reader = self.get_reader_raw(path)?;
        get_compression_reader(raw_reader, compression)
    }

    /// Creates a reader backed by a local cache file.
    pub fn get_cache_reader(
        &self,
        path: &str,
        cache_dir: &str,
        cache_file_name: Option<String>,
        force_cache: bool,
    ) -> Result<Box<dyn Read + Send>, OneIoError> {
        let dir_path = Path::new(cache_dir);
        if !dir_path.is_dir() {
            std::fs::create_dir_all(dir_path)?;
        }

        let cache_file_name = cache_file_name.unwrap_or_else(|| {
            path.split('/')
                .next_back()
                .unwrap_or("cached_file")
                .to_string()
        });

        let cache_file_path = format!("{cache_dir}/{cache_file_name}");

        if !force_cache && Path::new(cache_file_path.as_str()).exists() {
            return self.get_reader(cache_file_path.as_str());
        }

        let mut reader = self.get_reader_raw(path)?;
        let mut writer = self.get_writer_raw(cache_file_path.as_str())?;
        std::io::copy(&mut reader, &mut writer)?;
        writer.flush()?;

        self.get_reader(cache_file_path.as_str())
    }

    /// Checks whether a local or remote path exists.
    pub fn exists(&self, path: &str) -> Result<bool, OneIoError> {
        match crate::get_protocol(path) {
            #[cfg(feature = "http")]
            Some("http" | "https") => remote::http_file_exists(path, self.http_client()),
            #[cfg(feature = "s3")]
            Some("s3" | "r2") => {
                let (bucket, path) = s3::s3_url_parse(path)?;
                s3::s3_exists(bucket.as_str(), path.as_str())
            }
            Some(_) => Err(OneIoError::NotSupported(path.to_string())),
            None => Ok(Path::new(path).exists()),
        }
    }

    /// Reads the full contents of a file or URL into a string.
    pub fn read_to_string(&self, path: &str) -> Result<String, OneIoError> {
        let mut reader = self.get_reader(path)?;
        let mut content = String::new();
        reader.read_to_string(&mut content)?;
        Ok(content)
    }

    /// Reads and deserializes JSON into the requested type.
    #[cfg(feature = "json")]
    pub fn read_json_struct<T: DeserializeOwned>(&self, path: &str) -> Result<T, OneIoError> {
        let reader = self.get_reader(path)?;
        let res: T = serde_json::from_reader(reader)?;
        Ok(res)
    }

    /// Returns an iterator over lines from the provided path.
    pub fn read_lines(
        &self,
        path: &str,
    ) -> Result<Lines<BufReader<Box<dyn Read + Send>>>, OneIoError> {
        let reader = self.get_reader(path)?;
        Ok(BufReader::new(reader).lines())
    }

    /// Determines the raw content length for a local or remote path.
    pub fn get_content_length(&self, path: &str) -> Result<u64, OneIoError> {
        match crate::get_protocol(path) {
            #[cfg(feature = "http")]
            Some(protocol) if protocol == "http" || protocol == "https" => {
                remote::get_http_content_length(path, self.http_client())
            }
            #[cfg(feature = "ftp")]
            Some(protocol) if protocol == "ftp" => Err(OneIoError::NotSupported(
                "FTP size determination not yet implemented".to_string(),
            )),
            #[cfg(feature = "s3")]
            Some(protocol) if protocol == "s3" || protocol == "r2" => {
                let (bucket, key) = s3::s3_url_parse(path)?;
                let stats = s3::s3_stats(&bucket, &key)?;
                stats
                    .content_length
                    .ok_or_else(|| {
                        OneIoError::NotSupported(
                            "S3 object doesn't have content length information".to_string(),
                        )
                    })
                    .map(|len| len as u64)
            }
            Some(_) => Err(OneIoError::NotSupported(format!(
                "Protocol not supported for progress tracking: {path}"
            ))),
            None => Ok(std::fs::metadata(path)?.len()),
        }
    }

    /// Creates a reader that reports progress while reading raw bytes.
    pub fn get_reader_with_progress<F>(
        &self,
        path: &str,
        progress: F,
    ) -> Result<(Box<dyn Read + Send>, Option<u64>), OneIoError>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        let (total_size, size_option) = match self.get_content_length(path) {
            Ok(size) => (size, Some(size)),
            Err(_) => (0, None),
        };

        let raw_reader = self.get_reader_raw(path)?;
        let progress_reader =
            crate::progress::ProgressReader::new(raw_reader, total_size, progress);
        let file_type = crate::file_extension(path);
        let final_reader = get_compression_reader(Box::new(progress_reader), file_type)?;

        Ok((final_reader, size_option))
    }

    /// Returns the blocking HTTP response for a URL.
    #[cfg(feature = "http")]
    pub fn get_http_reader_raw(
        &self,
        path: &str,
    ) -> Result<reqwest::blocking::Response, OneIoError> {
        remote::get_http_reader_raw(path, self.http_client())
    }

    /// Returns an HTTP reader with decompression inferred from the URL suffix.
    #[cfg(feature = "http")]
    pub fn get_http_reader(&self, path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
        let raw_reader: Box<dyn Read + Send> = Box::new(self.get_http_reader_raw(path)?);
        let file_type = crate::file_extension(path);
        get_compression_reader(raw_reader, file_type)
    }

    /// Downloads a remote resource to a local path without decompression.
    pub fn download(&self, remote_path: &str, local_path: &str) -> Result<(), OneIoError> {
        let _ = local_path;

        match crate::get_protocol(remote_path) {
            #[cfg(feature = "http")]
            Some("http" | "https") => {
                let mut writer = self.get_writer_raw(local_path)?;
                let mut response = self.get_http_reader_raw(remote_path)?;
                response.copy_to(&mut writer)?;
                Ok(())
            }
            #[cfg(feature = "ftp")]
            Some("ftp") => {
                let mut writer = self.get_writer_raw(local_path)?;
                let mut reader = remote::get_ftp_reader_raw(remote_path)?;
                std::io::copy(&mut reader, &mut writer)?;
                Ok(())
            }
            #[cfg(feature = "s3")]
            Some("s3" | "r2") => {
                let (bucket, path) = s3::s3_url_parse(remote_path)?;
                s3::s3_download(bucket.as_str(), path.as_str(), local_path)?;
                Ok(())
            }
            Some(_) | None => Err(OneIoError::NotSupported(remote_path.to_string())),
        }
    }

    /// Downloads with retry support and exponential backoff.
    pub fn download_with_retry(
        &self,
        remote_path: &str,
        local_path: &str,
        retry: usize,
    ) -> Result<(), OneIoError> {
        let mut attempts = 0;
        loop {
            match self.download(remote_path, local_path) {
                Ok(()) => return Ok(()),
                Err(_) if attempts < retry => {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(
                        100 * (1 << attempts.min(6)),
                    ));
                }
                Err(err) => return Err(err),
            }
        }
    }
}
