//! S3 related functions.
//!
//! The following environment variables are needed (e.g. in .env):
//! - AWS_ACCESS_KEY_ID
//! - AWS_SECRET_ACCESS_KEY
//! - AWS_REGION (e.g. "us-east-1") (use "auto" for Cloudflare R2)
//! - AWS_ENDPOINT
use crate::oneio::{get_reader_raw, get_writer_raw};
use crate::OneIoError;
use s3::creds::Credentials;
use s3::serde_types::{HeadObjectResult, ListBucketResult};
use s3::{Bucket, Region};
use std::io::{Cursor, Read};

/// Check if the environment variables for S3 are set.
pub fn s3_env_check() -> Result<(), OneIoError> {
    dotenvy::dotenv().ok();
    let _ = Region::from_default_env()?;
    let _ = Credentials::from_env()?;
    Ok(())
}

/// parse s3 url into bucket and file path
pub fn s3_url_parse(path: &str) -> Result<(String, String), OneIoError> {
    if !path.starts_with("s3://") {
        return Err(OneIoError::S3UrlError(path.to_string()));
    }
    let parts = path.split('/').collect::<Vec<&str>>();
    if parts.len() < 3 {
        return Err(OneIoError::S3UrlError(path.to_string()));
    }
    let bucket = parts[2];
    let key = parts[3..].join("/");
    Ok((bucket.to_string(), key))
}

/// Get a S3 bucket object from the given bucket name.
pub fn s3_bucket(bucket: &str) -> Result<Bucket, OneIoError> {
    dotenvy::dotenv().ok();
    let mut bucket = Bucket::new(
        bucket,
        Region::from_default_env()?,
        Credentials::from_env()?,
    )?;
    bucket.set_request_timeout(Some(std::time::Duration::from_secs(10 * 60)));
    Ok(bucket)
}

/// Get a reader for a S3 object.
///
/// **NOTE**: The content is read into memory first before returning the reader. Use with caution
/// for large files.
pub fn s3_reader(bucket: &str, path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let object = bucket.get_object(path)?;
    let buf: Vec<u8> = object.to_vec();
    Ok(Box::new(Cursor::new(buf)))
}

/// Upload a file to S3.
pub fn s3_upload(bucket: &str, s3_path: &str, file_path: &str) -> Result<(), OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let mut reader = get_reader_raw(file_path)?;
    bucket.put_object_stream(&mut reader, s3_path)?;
    Ok(())
}

/// Download file from S3 bucket.
pub fn s3_download(bucket: &str, s3_path: &str, file_path: &str) -> Result<(), OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let mut output_file = get_writer_raw(file_path)?;
    let res: u16 = bucket.get_object_to_writer(s3_path, &mut output_file)?;
    match res {
        200..=299 => Ok(()),
        _ => Err(OneIoError::S3DownloadError(res)),
    }
}

/// Get S3 object head.
pub fn s3_stats(bucket: &str, path: &str) -> Result<HeadObjectResult, OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let (head_object, code): (HeadObjectResult, u16) = bucket.head_object(path)?;
    match code {
        200..=299 => Ok(head_object),
        _ => Err(OneIoError::S3DownloadError(code)),
    }
}

/// Check if an S3 object exists.
pub fn s3_exists(bucket: &str, path: &str) -> Result<bool, OneIoError> {
    if let Err(OneIoError::S3DownloadError(code)) = s3_stats(bucket, path) {
        if code == 404 {
            return Ok(false);
        } else {
            return Err(OneIoError::S3DownloadError(code));
        }
    } else {
        return Ok(true);
    }
}

/// List S3 objects.
pub fn s3_list(
    bucket: &str,
    prefix: &str,
    delimiter: Option<&str>,
) -> Result<Vec<String>, OneIoError> {
    let bucket = s3_bucket(bucket)?;
    let mut list: Vec<ListBucketResult> =
        bucket.list(prefix.to_string(), delimiter.map(|x| x.to_string()))?;
    let mut result = vec![];
    for item in list.iter_mut() {
        result.extend(item.contents.iter().map(|x| x.key.clone()));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_s3_url_parse() {
        const S3_URL: &str = "s3://test-bucket/test-path/test-file.txt";
        let (bucket, path) = s3_url_parse(S3_URL).unwrap();
        assert_eq!(bucket, "test-bucket");
        assert_eq!(path, "test-path/test-file.txt");

        const NON_S3_URL: &str = "http://test-bucket/test-path/test-file.txt";
        assert!(s3_url_parse(NON_S3_URL).is_err());
    }
}
