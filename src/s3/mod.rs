//! S3 operations using rusty-s3 for signing and reqwest for HTTP transport.
//!
//! # Environment Variables
//!
//! Required:
//! - `AWS_ACCESS_KEY_ID`
//! - `AWS_SECRET_ACCESS_KEY`
//! - `AWS_REGION` - Use `"auto"` for Cloudflare R2
//! - `AWS_ENDPOINT` - e.g. `https://xxx.r2.cloudflarestorage.com`
//!
//! Optional:
//! - `AWS_SESSION_TOKEN` - Temporary session token
//! - `ONEIO_S3_CHUNK_SIZE` - Multipart part size in bytes (default: 8MB)
//! - `ONEIO_S3_MULTIPART_THRESHOLD` - File size threshold for multipart upload (default: 8MB)
//!
//! # Upload Behavior
//!
//! Files smaller than the multipart threshold use a single PUT request.
//! Larger files are uploaded via multipart with auto-calculated part sizing
//! to stay within S3's 10,000 part limit.

pub mod config;

pub use config::{S3Config, S3Credentials};

use crate::OneIoError;
use hmac::{Hmac, Mac};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use quick_xml::{events::Event, Reader};
use reqwest::blocking::Response;
use rusty_s3::S3Action;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

const COPY_SOURCE_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b':')
    .add(b'?')
    .add(b'#')
    .add(b'[')
    .add(b']')
    .add(b'@')
    .add(b'!')
    .add(b'$')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b';')
    .add(b'=')
    .add(b'"')
    .add(b' ')
    .add(b'<')
    .add(b'>')
    .add(b'%')
    .add(b'{')
    .add(b'}')
    .add(b'|')
    .add(b'\\')
    .add(b'^')
    .add(b'`');

// Shared HTTP client for S3 operations
static S3_HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn get_s3_client() -> &'static reqwest::blocking::Client {
    S3_HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create S3 HTTP client")
    })
}

/// Metadata returned by s3_stats().
#[derive(Debug, Clone)]
pub struct S3ObjectMetadata {
    /// Content length in bytes.
    pub content_length: u64,
    /// Content type (MIME type), if available.
    pub content_type: Option<String>,
    /// Last modified timestamp, if available.
    pub last_modified: Option<String>,
    /// ETag of the object, if available.
    pub etag: Option<String>,
}

/// Bucket handle returned by s3_bucket().
#[derive(Debug, Clone)]
pub struct S3Bucket {
    /// Bucket name.
    pub name: String,
    /// Endpoint URL.
    pub endpoint: String,
    /// Region.
    pub region: String,
}

/// Checks if the necessary environment variables for AWS S3 are set.
pub fn s3_env_check() -> Result<(), OneIoError> {
    let _ = config::S3Config::from_env("test")?;
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

/// Creates an S3 bucket handle with the specified bucket name.
pub fn s3_bucket(name: &str) -> Result<S3Bucket, OneIoError> {
    let config = config::S3Config::from_env(name)?;
    Ok(S3Bucket {
        name: config.bucket,
        endpoint: config.endpoint,
        region: config.region,
    })
}

/// Reads a file from an S3 bucket and returns a boxed reader implementing `Read` trait.
pub fn s3_reader(bucket: &str, key: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let config = config::S3Config::from_env(bucket)?;
    let bucket = config.rusty_bucket()?;
    let creds = config.rusty_credentials();
    let action = bucket.get_object(Some(&creds), key);
    let url = action.sign(config.ttl);
    let response = ensure_s3_success(get_s3_client().get(url).send()?)?;
    Ok(Box::new(response))
}

/// Downloads a file from an S3 bucket and saves it locally.
pub fn s3_download(bucket: &str, key: &str, file_path: &str) -> Result<(), OneIoError> {
    let mut reader = s3_reader(bucket, key)?;
    let mut writer = crate::get_writer_raw_impl(file_path)?;
    std::io::copy(&mut reader, &mut writer)?;
    Ok(())
}

/// Uploads a file to an S3 bucket at the specified path.
pub fn s3_upload(bucket: &str, key: &str, file_path: &str) -> Result<(), OneIoError> {
    // Early validation: check if file exists before attempting S3 operations
    if !std::path::Path::new(file_path).exists() {
        return Err(OneIoError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {file_path}"),
        )));
    }

    let metadata = std::fs::metadata(file_path)?;
    let size = metadata.len();

    let config = config::S3Config::from_env(bucket)?;

    if size < config.multipart_threshold {
        upload_single(&config, key, file_path)
    } else {
        upload_multipart(&config, key, file_path, size)
    }
}

fn upload_single(config: &config::S3Config, key: &str, file_path: &str) -> Result<(), OneIoError> {
    let bucket = config.rusty_bucket()?;
    let creds = config.rusty_credentials();

    // Small single-PUT uploads stay under the multipart threshold, so buffering
    // them avoids reqwest surfacing early S3 rejections as body disconnects.
    let body = std::fs::read(file_path)?;
    let action = bucket.put_object(Some(&creds), key);
    let url = action.sign(config.ttl);
    ensure_s3_success(get_s3_client().put(url).body(body).send()?)?;
    Ok(())
}

fn calculate_chunk_size(file_size: u64, requested_chunk_size: u64) -> (u64, usize) {
    const MAX_PARTS: u64 = 10_000;
    const MIN_PART_SIZE: u64 = 5 * 1024 * 1024;

    #[allow(clippy::manual_div_ceil)]
    let chunk_size = {
        let required = (file_size + MAX_PARTS - 1) / MAX_PARTS;
        requested_chunk_size.max(required).max(MIN_PART_SIZE)
    };
    #[allow(clippy::manual_div_ceil)]
    let total_parts = ((file_size + chunk_size - 1) / chunk_size) as usize;

    (chunk_size, total_parts)
}

fn upload_multipart(
    config: &config::S3Config,
    key: &str,
    file_path: &str,
    size: u64,
) -> Result<(), OneIoError> {
    let (chunk_size, total_parts) = calculate_chunk_size(size, config.multipart_chunk_size);

    let bucket = config.rusty_bucket()?;
    let creds = config.rusty_credentials();

    // 1. Initiate multipart upload
    let action = bucket.create_multipart_upload(Some(&creds), key);
    let url = action.sign(config.ttl);
    let response = ensure_s3_success(get_s3_client().post(url).send()?)?;
    let init_response =
        rusty_s3::actions::CreateMultipartUpload::parse_response(response.text()?.as_bytes())
            .map_err(|e| OneIoError::Network(Box::new(e)))?;
    let upload_id = init_response.upload_id().to_string();

    // 2. Upload parts
    let mut parts: Vec<String> = Vec::with_capacity(total_parts);
    let mut file = std::fs::File::open(file_path)?;

    for part_number in 1..=total_parts {
        let mut chunk = vec![0u8; chunk_size as usize];
        let bytes_read = file.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        chunk.truncate(bytes_read);

        let action = bucket.upload_part(Some(&creds), key, part_number as u16, &upload_id);
        let url = action.sign(config.ttl);
        let response = ensure_s3_success(get_s3_client().put(url).body(chunk).send()?)?;

        let etag = extract_etag(response.headers()).ok_or_else(|| {
            OneIoError::NotSupported("Missing ETag in UploadPart response".into())
        })?;
        parts.push(etag);
    }

    // 3. Complete multipart upload
    let action = bucket.complete_multipart_upload(
        Some(&creds),
        key,
        &upload_id,
        parts.iter().map(|s| s.as_str()),
    );
    let url = action.sign(config.ttl);
    let body = action.body();
    let response = match get_s3_client()
        .post(url)
        .header("content-type", "application/xml")
        .body(body)
        .send()
    {
        Ok(response) => response,
        Err(e) => {
            abort_multipart_upload(&bucket, &creds, key, &upload_id, config.ttl);
            return Err(e.into());
        }
    };

    if let Err(e) = ensure_s3_success(response) {
        abort_multipart_upload(&bucket, &creds, key, &upload_id, config.ttl);
        return Err(e);
    }

    Ok(())
}

fn abort_multipart_upload(
    bucket: &rusty_s3::Bucket,
    creds: &rusty_s3::Credentials,
    key: &str,
    upload_id: &str,
    ttl: Duration,
) {
    let action = bucket.abort_multipart_upload(Some(creds), key, upload_id);
    let url = action.sign(ttl);
    let _ = get_s3_client().delete(url).send();
}

/// Copies an object within the same S3 bucket.
///
/// Uses AWS Signature V4 with Authorization header (not presigned URL).
/// This is required by some S3-compatible services like Cloudflare R2
/// that reject presigned URLs for CopyObject operations.
pub fn s3_copy(bucket: &str, src_key: &str, dst_key: &str) -> Result<(), OneIoError> {
    let config = config::S3Config::from_env(bucket)?;
    let bucket_obj = config.rusty_bucket()?;

    // Get the base URL for the destination object
    let url = bucket_obj
        .object_url(dst_key)
        .map_err(|e| OneIoError::NotSupported(format!("Invalid destination key: {e}")))?;
    let url_str = url.as_str();

    // Extract host and path for signing
    let host = url
        .host_str()
        .ok_or_else(|| OneIoError::NotSupported("Invalid URL: no host".to_string()))?;
    let canonical_uri = url.path();

    // Build x-amz-copy-source header value (/bucket/key)
    let copy_source = format!(
        "/{}/{}",
        config.bucket,
        utf8_percent_encode(src_key, COPY_SOURCE_ENCODE_SET)
    );

    // Generate timestamp and datestamp
    let now = std::time::SystemTime::now();
    let datetime = format_timestamp(now);
    let datestamp = datetime[..8].to_string();

    // Empty payload hash for COPY (no request body)
    let payload_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    // Build signed headers list (alphabetical order for canonical request)
    let mut signed_headers = vec![
        ("host", host),
        ("x-amz-content-sha256", payload_hash),
        ("x-amz-copy-source", copy_source.as_str()),
        ("x-amz-date", datetime.as_str()),
    ];

    // Add session token if present
    if let Some(ref token) = config.credentials.session_token {
        signed_headers.push(("x-amz-security-token", token.as_str()));
    }

    let signed_headers_str = signed_headers
        .iter()
        .map(|(k, _)| *k)
        .collect::<Vec<_>>()
        .join(";");

    // Build canonical headers string
    let canonical_headers = signed_headers
        .iter()
        .map(|(k, v)| format!("{}:{}\n", k.to_lowercase(), v))
        .collect::<String>();

    // Build canonical request (empty query string for header-based auth)
    let canonical_request = format!(
        "PUT\n{}\n\n{}\n{}\n{}",
        canonical_uri, canonical_headers, signed_headers_str, payload_hash
    );

    // Build string to sign
    let credential_scope = format!("{}/{}/s3/aws4_request", datestamp, config.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        datetime,
        credential_scope,
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    // Calculate signature
    let signing_key =
        derive_signing_key(&config.credentials.secret_key, &datestamp, &config.region);
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    // Build Authorization header
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        config.credentials.access_key, credential_scope, signed_headers_str, signature
    );

    // Build and send request
    let mut request_builder = get_s3_client()
        .put(url_str)
        .header("host", host)
        .header("x-amz-date", datetime)
        .header("x-amz-content-sha256", payload_hash)
        .header("x-amz-copy-source", copy_source)
        .header("Authorization", authorization.clone());

    // Add session token if present
    if let Some(token) = &config.credentials.session_token {
        request_builder = request_builder.header("x-amz-security-token", token);
    }

    ensure_s3_success(request_builder.send()?)?;
    Ok(())
}

/// Format system time as ISO 8601 timestamp (YYYYMMDD'T'HHMMSS'Z').
fn format_timestamp(time: std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Simple UTC conversion (no leap second handling needed for AWS SigV4)
    let days_since_epoch = secs / 86400;
    let seconds_of_day = secs % 86400;

    let (year, month, day) = epoch_days_to_ymd(days_since_epoch);
    let hour = (seconds_of_day / 3600) % 24;
    let minute = (seconds_of_day / 60) % 60;
    let second = seconds_of_day % 60;

    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        year, month, day, hour, minute, second
    )
}

/// Convert days since Unix epoch to year, month, day.
fn epoch_days_to_ymd(mut days: u64) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let month_lengths = if is_leap_year(year) {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &len in &month_lengths {
        if days < len {
            break;
        }
        days -= len;
        month += 1;
    }

    (year, month, days as u32 + 1)
}

/// Check if a year is a leap year.
fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// Derive AWS Signature V4 signing key.
fn derive_signing_key(secret_key: &str, datestamp: &str, region: &str) -> Vec<u8> {
    let k_date = hmac_sha256(
        format!("AWS4{}", secret_key).as_bytes(),
        datestamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    hmac_sha256(&k_service, b"aws4_request")
}

/// Compute HMAC-SHA256.
fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Deletes an object from an S3 bucket.
pub fn s3_delete(bucket: &str, key: &str) -> Result<(), OneIoError> {
    let config = config::S3Config::from_env(bucket)?;
    let bucket_obj = config.rusty_bucket()?;
    let creds = config.rusty_credentials();
    let action = bucket_obj.delete_object(Some(&creds), key);
    let url = action.sign(config.ttl);
    ensure_s3_success(get_s3_client().delete(url).send()?)?;
    Ok(())
}

/// Retrieves the head object result for a given bucket and path in Amazon S3.
pub fn s3_stats(bucket: &str, key: &str) -> Result<S3ObjectMetadata, OneIoError> {
    let config = config::S3Config::from_env(bucket)?;
    let bucket_obj = config.rusty_bucket()?;
    let creds = config.rusty_credentials();
    let action = bucket_obj.head_object(Some(&creds), key);
    let url = action.sign(config.ttl);
    let response = get_s3_client().head(url).send()?;

    if response.status().is_success() {
        let content_length = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let last_modified = response
            .headers()
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let etag = extract_etag(response.headers());

        Ok(S3ObjectMetadata {
            content_length,
            content_type,
            last_modified,
            etag,
        })
    } else {
        Err(s3_error_from_response(response))
    }
}

/// Check if a file exists in an S3 bucket.
pub fn s3_exists(bucket: &str, key: &str) -> Result<bool, OneIoError> {
    match s3_stats(bucket, key) {
        Ok(_) => Ok(true),
        Err(OneIoError::NotSupported(msg)) if msg.contains("not found") => Ok(false),
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
    let config = config::S3Config::from_env(bucket)?;
    let bucket_obj = config.rusty_bucket()?;
    let creds = config.rusty_credentials();

    let fixed_delimiter = match dirs && delimiter.is_none() {
        true => Some("/"),
        false => delimiter.as_deref(),
    };

    let mut result = Vec::new();
    let mut continuation_token: Option<String> = None;

    loop {
        let mut action = bucket_obj.list_objects_v2(Some(&creds));
        action.with_prefix(prefix);
        if let Some(delim) = fixed_delimiter {
            action.with_delimiter(delim);
        }
        if let Some(token) = &continuation_token {
            action.with_continuation_token(token);
        }

        let url = action.sign(config.ttl);
        let response = ensure_s3_success(get_s3_client().get(url).send()?)?;

        let parsed = rusty_s3::actions::ListObjectsV2::parse_response(response.text()?.as_bytes())
            .map_err(|e| OneIoError::Network(Box::new(e)))?;

        if dirs {
            result.extend(
                parsed
                    .common_prefixes
                    .into_iter()
                    .map(|p| decode_s3_path(&p.prefix)),
            );
        } else {
            result.extend(parsed.contents.into_iter().map(|c| decode_s3_path(&c.key)));
        }

        match parsed.next_continuation_token {
            Some(token) => continuation_token = Some(token),
            None => break,
        }
    }

    Ok(result)
}

/// Check an S3 HTTP response for errors and preserve the response body for callers.
fn ensure_s3_success(response: Response) -> Result<Response, OneIoError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(s3_error_from_response(response))
    }
}

/// Extract ETag from response headers.
fn extract_etag(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("etag")
        .or_else(|| headers.get("ETag"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string())
}

fn decode_s3_path(path: &str) -> String {
    percent_decode_str(path).decode_utf8_lossy().into_owned()
}

#[derive(Debug, Default)]
struct ParsedS3Error {
    code: Option<String>,
    message: Option<String>,
    key: Option<String>,
    bucket_name: Option<String>,
}

fn s3_error_from_response(response: Response) -> OneIoError {
    let status = response.status().as_u16();
    let body = response.text().ok();

    if let Some(parsed) = body.as_deref().and_then(parse_s3_error_xml) {
        return map_parsed_s3_error(status, parsed);
    }

    match status {
        404 => OneIoError::NotSupported("Object not found".to_string()),
        403 => OneIoError::NotSupported("Access denied".to_string()),
        code => OneIoError::Status {
            service: "S3",
            code,
        },
    }
}

fn map_parsed_s3_error(status: u16, parsed: ParsedS3Error) -> OneIoError {
    let code = parsed.code.unwrap_or_else(|| format!("S3Status{status}"));
    let message = parsed
        .message
        .unwrap_or_else(|| format!("S3 request failed with status {status}"));

    match code.as_str() {
        "NoSuchKey" => {
            let key = parsed.key.unwrap_or_default();
            if key.is_empty() {
                OneIoError::NotSupported("Object not found".to_string())
            } else {
                OneIoError::NotSupported(format!("Object not found: {key}"))
            }
        }
        "NoSuchBucket" => {
            let bucket = parsed.bucket_name.unwrap_or_default();
            if bucket.is_empty() {
                OneIoError::NotSupported("Bucket not found".to_string())
            } else {
                OneIoError::NotSupported(format!("Bucket not found: {bucket}"))
            }
        }
        "AccessDenied" => OneIoError::NotSupported(format!("Access denied: {message}")),
        "InvalidAccessKeyId" | "SignatureDoesNotMatch" => {
            OneIoError::NotSupported(format!("{code}: {message}"))
        }
        _ => OneIoError::NotSupported(format!("{code}: {message}")),
    }
}

fn parse_s3_error_xml(body: &str) -> Option<ParsedS3Error> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut parsed = ParsedS3Error::default();
    let mut current_field: Option<&[u8]> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                current_field = Some(match e.name().as_ref() {
                    b"Code" => b"Code",
                    b"Message" => b"Message",
                    b"Key" => b"Key",
                    b"BucketName" => b"BucketName",
                    _ => b"",
                });
            }
            Ok(Event::Text(e)) => {
                let Some(field) = current_field.take() else {
                    continue;
                };
                if field.is_empty() {
                    continue;
                }

                let value = match e.decode() {
                    Ok(value) => value.into_owned(),
                    Err(_) => return None,
                };

                match field {
                    b"Code" => parsed.code = Some(value),
                    b"Message" => parsed.message = Some(value),
                    b"Key" => parsed.key = Some(value),
                    b"BucketName" => parsed.bucket_name = Some(value),
                    _ => {}
                }
            }
            Ok(Event::End(_)) => current_field = None,
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    if parsed.code.is_some() || parsed.message.is_some() {
        Some(parsed)
    } else {
        None
    }
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
    fn test_extract_etag() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "etag",
            reqwest::header::HeaderValue::from_static("\"abc123\""),
        );
        assert_eq!(extract_etag(&headers), Some("abc123".to_string()));

        let mut headers2 = reqwest::header::HeaderMap::new();
        headers2.insert(
            "ETag",
            reqwest::header::HeaderValue::from_static("\"def456\""),
        );
        assert_eq!(extract_etag(&headers2), Some("def456".to_string()));
    }

    #[test]
    fn test_decode_s3_path() {
        assert_eq!(
            decode_s3_path("test%2Fpath%20file.txt"),
            "test/path file.txt"
        );
    }

    #[test]
    fn test_parse_s3_error_xml() {
        let parsed = parse_s3_error_xml(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <Error>
              <Code>NoSuchKey</Code>
              <Message>The specified key does not exist.</Message>
              <Key>test-file.txt</Key>
            </Error>"#,
        )
        .unwrap();

        assert_eq!(parsed.code.as_deref(), Some("NoSuchKey"));
        assert_eq!(
            parsed.message.as_deref(),
            Some("The specified key does not exist.")
        );
        assert_eq!(parsed.key.as_deref(), Some("test-file.txt"));
    }

    #[test]
    fn test_calculate_chunk_size() {
        let chunk_size = 8 * 1024 * 1024; // 8MB default

        // 0 bytes -> 8MB default chunk (default > min), 0 parts
        let (cs, tp) = calculate_chunk_size(0, chunk_size);
        assert_eq!(cs, 8 * 1024 * 1024);
        assert_eq!(tp, 0);

        // 1 byte -> 8MB chunk, 1 part
        let (cs, tp) = calculate_chunk_size(1, chunk_size);
        assert_eq!(cs, 8 * 1024 * 1024);
        assert_eq!(tp, 1);

        // 5MB - 1 -> 8MB chunk, 1 part
        let (cs, tp) = calculate_chunk_size(5 * 1024 * 1024 - 1, chunk_size);
        assert_eq!(cs, 8 * 1024 * 1024);
        assert_eq!(tp, 1);

        // Exactly 5MB -> 8MB chunk, 1 part
        let (cs, tp) = calculate_chunk_size(5 * 1024 * 1024, chunk_size);
        assert_eq!(cs, 8 * 1024 * 1024);
        assert_eq!(tp, 1);

        // 5MB + 1 -> 8MB chunk (default), 1 part
        let (cs, tp) = calculate_chunk_size(5 * 1024 * 1024 + 1, chunk_size);
        assert_eq!(cs, 8 * 1024 * 1024);
        assert_eq!(tp, 1);

        // 10MB -> 8MB chunk, 2 parts
        let (cs, tp) = calculate_chunk_size(10 * 1024 * 1024, chunk_size);
        assert_eq!(cs, 8 * 1024 * 1024);
        assert_eq!(tp, 2);

        // 80MB -> 8MB chunk, 10 parts
        let (cs, tp) = calculate_chunk_size(80 * 1024 * 1024, chunk_size);
        assert_eq!(cs, 8 * 1024 * 1024);
        assert_eq!(tp, 10);

        // Very large file: 100GB -> chunk size auto-increases to stay under 10,000 parts
        let hundred_gb = 100u64 * 1024 * 1024 * 1024;
        let (cs, tp) = calculate_chunk_size(hundred_gb, chunk_size);
        assert!(tp <= 10_000);
        assert!(cs >= 8 * 1024 * 1024);
        assert_eq!(tp, ((hundred_gb + cs - 1) / cs) as usize);
    }
}
