//! S3 configuration and credentials.

use crate::OneIoError;

/// S3 credentials.
#[derive(Debug, Clone)]
pub struct S3Credentials {
    /// Access key ID.
    pub access_key: String,
    /// Secret access key.
    pub secret_key: String,
    /// Optional session token.
    pub session_token: Option<String>,
}

impl S3Credentials {
    /// Create credentials from environment variables.
    ///
    /// Reads:
    /// - AWS_ACCESS_KEY_ID
    /// - AWS_SECRET_ACCESS_KEY
    /// - AWS_SESSION_TOKEN (optional)
    pub fn from_env() -> Result<Self, OneIoError> {
        dotenvy::dotenv().ok();

        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .map_err(|_| OneIoError::NotSupported("AWS_ACCESS_KEY_ID not set".to_string()))?;
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .map_err(|_| OneIoError::NotSupported("AWS_SECRET_ACCESS_KEY not set".to_string()))?;
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();

        Ok(S3Credentials {
            access_key,
            secret_key,
            session_token,
        })
    }
}

/// S3 configuration used by action functions.
#[derive(Debug, Clone)]
pub struct S3Config {
    /// Bucket name.
    pub bucket: String,
    /// Credentials.
    pub credentials: S3Credentials,
    /// Endpoint URL.
    pub endpoint: String,
    /// Region.
    pub region: String,
    /// Signed URL TTL in seconds (default: 3600).
    pub ttl: std::time::Duration,
    /// Multipart chunk size in bytes (default: 8MB).
    pub multipart_chunk_size: u64,
    /// Multipart threshold in bytes (default: 5MB).
    pub multipart_threshold: u64,
}

impl S3Config {
    /// Create S3Config from environment variables for a given bucket.
    pub fn from_env(bucket: &str) -> Result<Self, OneIoError> {
        dotenvy::dotenv().ok();

        let credentials = S3Credentials::from_env()?;

        // Region: AWS_REGION or S3_REGION
        let region = std::env::var("AWS_REGION")
            .or_else(|_| std::env::var("S3_REGION"))
            .unwrap_or_else(|_| "us-east-1".to_string());

        // Endpoint: AWS_ENDPOINT or S3_ENDPOINT
        let endpoint = std::env::var("AWS_ENDPOINT")
            .or_else(|_| std::env::var("S3_ENDPOINT"))
            .unwrap_or_else(|_| format!("https://s3.{region}.amazonaws.com"));

        // Normalize endpoint
        let endpoint = normalize_endpoint(&endpoint);

        // Chunk size from env (default: 8MB)
        let multipart_chunk_size = std::env::var("ONEIO_S3_CHUNK_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8 * 1024 * 1024);

        // Multipart threshold from env (default: same as chunk size)
        // Files smaller than this use single PUT; larger files use multipart.
        let multipart_threshold = std::env::var("ONEIO_S3_MULTIPART_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(multipart_chunk_size);

        Ok(S3Config {
            bucket: bucket.to_string(),
            credentials,
            endpoint,
            region,
            ttl: std::time::Duration::from_secs(3600),
            multipart_chunk_size,
            multipart_threshold,
        })
    }

    /// Convert to rusty_s3 credentials.
    pub fn rusty_credentials(&self) -> rusty_s3::Credentials {
        match &self.credentials.session_token {
            Some(token) => rusty_s3::Credentials::new_with_token(
                &self.credentials.access_key,
                &self.credentials.secret_key,
                token,
            ),
            None => rusty_s3::Credentials::new(
                &self.credentials.access_key,
                &self.credentials.secret_key,
            ),
        }
    }

    /// Build a rusty_s3 Bucket from this config.
    pub fn rusty_bucket(&self) -> Result<rusty_s3::Bucket, OneIoError> {
        let endpoint = self
            .endpoint
            .parse()
            .map_err(|e| OneIoError::NotSupported(format!("Invalid S3 endpoint: {e}")))?;

        // Use path-style for non-AWS endpoints (MinIO, R2, custom)
        let is_aws = self.endpoint.contains("amazonaws.com");
        let url_style = if is_aws {
            rusty_s3::UrlStyle::VirtualHost
        } else {
            rusty_s3::UrlStyle::Path
        };

        rusty_s3::Bucket::new(
            endpoint,
            url_style,
            self.bucket.clone(),
            self.region.clone(),
        )
        .map_err(|e| OneIoError::NotSupported(format!("Invalid S3 bucket config: {e:?}")))
    }
}

/// Normalize an endpoint URL.
///
/// - Adds https:// if no scheme is present.
/// - Strips trailing slashes.
pub(crate) fn normalize_endpoint(url: &str) -> String {
    let url = url.trim();
    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    url.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_endpoint() {
        assert_eq!(normalize_endpoint("example.com"), "https://example.com");
        assert_eq!(
            normalize_endpoint("http://example.com"),
            "http://example.com"
        );
        assert_eq!(
            normalize_endpoint("https://example.com/"),
            "https://example.com"
        );
        assert_eq!(
            normalize_endpoint("https://example.com/path/"),
            "https://example.com/path"
        );
    }
}
