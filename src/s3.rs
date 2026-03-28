//! S3 related functions.
//!
//! The following environment variables are needed (e.g., in .env):
//! - AWS_ACCESS_KEY_ID
//! - AWS_SECRET_ACCESS_KEY
//! - AWS_REGION (e.g. "us-east-1") (use "auto" for Cloudflare R2)
//! - AWS_ENDPOINT
use crate::get_writer_raw_impl;
use crate::OneIoError;
use s3::creds::Credentials;
use s3::serde_types::{HeadObjectResult, ListBucketResult};
use s3::{Bucket, Region};
use std::io::{Cursor, Read, Write};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

/// Checks if the necessary environment variables for AWS S3 are set.
pub fn s3_env_check() -> Result<(), OneIoError> {
    dotenvy::dotenv().ok();
    let _ = Region::from_default_env()?;
    let _ = Credentials::from_env()?;
    Ok(())
}

/// Parse an S3 URL into a bucket and key.
pub fn s3_url_parse(path: &str) -> Result<(String, String), OneIoError> {
    let (_, remaining) = path
        .split_once("://")
        .ok_or_else(|| OneIoError::NotSupported(format!("Invalid S3 URL: {path}")))?;
    let (bucket, key) = remaining
        .split_once('/')
        .ok_or_else(|| OneIoError::NotSupported(format!("Invalid S3 URL: {path}")))?;
    if bucket.is_empty() || key.is_empty() {
        return Err(OneIoError::NotSupported(format!("Invalid S3 URL: {path}")));
    }
    Ok((bucket.to_string(), key.to_string()))
}

enum StreamMessage {
    Chunk(Vec<u8>),
    Error(String),
    Eof,
}

struct StreamWriter {
    sender: SyncSender<StreamMessage>,
    closed: bool,
}

impl StreamWriter {
    fn new(sender: SyncSender<StreamMessage>) -> Self {
        Self {
            sender,
            closed: false,
        }
    }

    fn send_error(&mut self, err: std::io::Error) -> std::io::Result<()> {
        self.closed = true;
        self.sender
            .send(StreamMessage::Error(err.to_string()))
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stream closed"))
    }

    fn close(&mut self) -> std::io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.sender
            .send(StreamMessage::Eof)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stream closed"))
    }
}

impl Write for StreamWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .send(StreamMessage::Chunk(buf.to_vec()))
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stream closed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for StreamWriter {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

struct StreamReader {
    receiver: Receiver<StreamMessage>,
    current_chunk: Cursor<Vec<u8>>,
    done: bool,
}

impl StreamReader {
    fn new(receiver: Receiver<StreamMessage>) -> Self {
        Self {
            receiver,
            current_chunk: Cursor::new(Vec::new()),
            done: false,
        }
    }
}

impl Read for StreamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            let bytes_read = self.current_chunk.read(buf)?;
            if bytes_read > 0 {
                return Ok(bytes_read);
            }

            if self.done {
                return Ok(0);
            }

            match self.receiver.recv() {
                Ok(StreamMessage::Chunk(chunk)) => {
                    self.current_chunk = Cursor::new(chunk);
                }
                Ok(StreamMessage::Error(message)) => {
                    self.done = true;
                    return Err(std::io::Error::other(message));
                }
                Ok(StreamMessage::Eof) => {
                    self.done = true;
                    return Ok(0);
                }
                Err(_) => {
                    self.done = true;
                    return Err(std::io::Error::other("S3 stream closed unexpectedly"));
                }
            }
        }
    }
}

/// Creates an S3 bucket object with the specified bucket name.
pub fn s3_bucket(bucket: &str) -> Result<Bucket, OneIoError> {
    dotenvy::dotenv().ok();

    #[cfg(feature = "rustls")]
    crate::crypto::ensure_default_provider()?;

    let mut bucket = *Bucket::new(
        bucket,
        Region::from_default_env()?,
        Credentials::new(None, None, None, None, None)?,
    )?;
    bucket.set_request_timeout(Some(std::time::Duration::from_secs(10 * 60)));
    Ok(bucket)
}

/// Reads a file from an S3 bucket and returns a boxed reader implementing `Read` trait.
pub fn s3_reader(bucket: &str, path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let path = path.to_string();
    let (sender, receiver) = sync_channel(8);

    std::thread::spawn(move || {
        let mut writer = StreamWriter::new(sender);
        match bucket.get_object_to_writer(path, &mut writer) {
            Ok(200..=299) => {
                let _ = writer.close();
            }
            Ok(code) => {
                let _ =
                    writer.send_error(std::io::Error::other(format!("S3 status error: {code}")));
            }
            Err(err) => {
                let _ = writer.send_error(std::io::Error::other(err.to_string()));
            }
        }
    });

    Ok(Box::new(StreamReader::new(receiver)))
}

/// Uploads a file to an S3 bucket at the specified path.
pub fn s3_upload(bucket: &str, s3_path: &str, file_path: &str) -> Result<(), OneIoError> {
    // Early validation: check if file exists before attempting S3 operations
    if !std::path::Path::new(file_path).exists() {
        return Err(OneIoError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {file_path}"),
        )));
    }

    let bucket = s3_bucket(bucket)?;
    let file = std::fs::File::open(file_path)?;
    let mut reader: Box<dyn Read + Send> = Box::new(std::io::BufReader::new(file));
    bucket.put_object_stream(&mut reader, s3_path)?;
    Ok(())
}

/// Copies an object within the same Amazon S3 bucket.
pub fn s3_copy(bucket: &str, s3_path: &str, s3_path_new: &str) -> Result<(), OneIoError> {
    let bucket = s3_bucket(bucket)?;
    bucket.copy_object_internal(s3_path, s3_path_new)?;
    Ok(())
}

/// Deletes an object from an S3 bucket.
pub fn s3_delete(bucket: &str, s3_path: &str) -> Result<(), OneIoError> {
    let bucket = s3_bucket(bucket)?;
    bucket.delete_object(s3_path)?;
    Ok(())
}

/// Downloads a file from an S3 bucket and saves it locally.
pub fn s3_download(bucket: &str, s3_path: &str, file_path: &str) -> Result<(), OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let mut output_file = get_writer_raw_impl(file_path)?;
    let res: u16 = bucket.get_object_to_writer(s3_path, &mut output_file)?;
    match res {
        200..=299 => Ok(()),
        _ => Err(OneIoError::Status {
            service: "S3",
            code: res,
        }),
    }
}

/// Retrieves the head object result for a given bucket and path in Amazon S3.
pub fn s3_stats(bucket: &str, path: &str) -> Result<HeadObjectResult, OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let (head_object, code): (HeadObjectResult, u16) = bucket.head_object(path)?;
    match code {
        200..=299 => Ok(head_object),
        _ => Err(OneIoError::Status {
            service: "S3",
            code,
        }),
    }
}

/// Check if a file exists in an S3 bucket.
pub fn s3_exists(bucket: &str, path: &str) -> Result<bool, OneIoError> {
    match s3_stats(bucket, path) {
        Ok(_) => Ok(true),
        Err(OneIoError::Status {
            service: "S3",
            code: 404,
        }) => Ok(false),
        Err(err) => Err(err),
    }
}

/// Lists objects in the specified Amazon S3 bucket with given prefix and delimiter.
pub fn s3_list(
    bucket: &str,
    prefix: &str,
    delimiter: Option<String>,
    dirs: bool,
) -> Result<Vec<String>, OneIoError> {
    let fixed_delimiter = match dirs && delimiter.is_none() {
        true => Some("/".to_string()),
        false => delimiter,
    };
    let bucket = s3_bucket(bucket)?;
    let mut list: Vec<ListBucketResult> = bucket.list(prefix.to_string(), fixed_delimiter)?;
    let mut result = vec![];
    for item in list.iter_mut() {
        match dirs {
            true => result.extend(
                item.common_prefixes
                    .iter()
                    .flat_map(|x| x.iter().map(|p| p.prefix.clone())),
            ),
            false => result.extend(item.contents.iter().map(|x| x.key.clone())),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn test_s3_url_parse() {
        const S3_URL: &str = "s3://test-bucket/test-path/test-file.txt";
        let (bucket, path) = s3_url_parse(S3_URL).unwrap();
        assert_eq!(bucket, "test-bucket");
        assert_eq!(path, "test-path/test-file.txt");

        const NON_S3_URL: &str = "s3:/test-bucket";
        assert!(s3_url_parse(NON_S3_URL).is_err());
    }

    #[test]
    fn test_s3_upload_nonexistent_file_early_validation() {
        let non_existent_file = "/tmp/oneio_test_nonexistent_file_12345.txt";
        let _ = std::fs::remove_file(non_existent_file);
        assert!(!std::path::Path::new(non_existent_file).exists());

        let start = std::time::Instant::now();
        match s3_upload("test-bucket", "test-path", non_existent_file) {
            Ok(_) => panic!("Upload should have failed for non-existent file"),
            Err(OneIoError::Io(e)) => {
                let duration = start.elapsed();
                assert!(
                    duration < std::time::Duration::from_millis(100),
                    "Early validation should be instant"
                );
                assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
            }
            Err(_) => {
                let duration = start.elapsed();
                assert!(duration < std::time::Duration::from_secs(1));
            }
        }
    }

    #[test]
    fn test_stream_reader_reads_in_order() {
        let (sender, receiver) = sync_channel(2);
        let writer_thread = std::thread::spawn(move || {
            let mut writer = StreamWriter::new(sender);
            writer.write_all(b"hello ").unwrap();
            writer.write_all(b"world").unwrap();
            writer.close().unwrap();
        });

        let mut reader = StreamReader::new(receiver);
        let mut output = String::new();
        reader.read_to_string(&mut output).unwrap();
        writer_thread.join().unwrap();

        assert_eq!(output, "hello world");
    }

    #[test]
    fn test_stream_reader_propagates_error() {
        let (sender, receiver) = sync_channel(2);
        let mut writer = StreamWriter::new(sender);
        writer.write_all(b"hello").unwrap();
        writer
            .send_error(std::io::Error::other("stream failed"))
            .unwrap();

        let mut reader = StreamReader::new(receiver);
        let mut buf = [0; 5];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");
        assert!(reader.read(&mut [0; 1]).is_err());
    }
}
