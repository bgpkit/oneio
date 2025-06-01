use thiserror::Error;

#[derive(Debug, Error)]
pub enum OneIoError {
    #[cfg(feature = "http")]
    #[error("remote IO error: {0}")]
    RemoteIoError(#[from] reqwest::Error),
    #[cfg(feature = "ftp")]
    #[error("FTP error: {0}")]
    FptError(#[from] suppaftp::FtpError),
    #[cfg(feature = "json")]
    #[error("JSON object parsing error: {0}")]
    JsonParsingError(#[from] serde_json::Error),
    #[error("End-of-file error: {0}")]
    EofError(std::io::Error),
    #[error("IO error: {0}")]
    IoError(std::io::Error),
    #[error("Not supported error: {0}")]
    NotSupported(String),
    #[error("Cache IO error: {0}")]
    CacheIoError(String),

    #[cfg(feature = "s3")]
    #[error("S3 IO error: {0}")]
    S3IoError(#[from] s3::error::S3Error),
    #[cfg(feature = "s3")]
    #[error("S3 credential error: {0}")]
    S3CredentialError(#[from] s3::creds::error::CredentialsError),
    #[cfg(feature = "s3")]
    #[error("S3 invalid url: {0}")]
    S3UrlError(String),
    #[cfg(feature = "s3")]
    #[error("S3 region error: {0}")]
    S3RegionError(#[from] s3::region::error::RegionError),
    #[cfg(feature = "s3")]
    #[error("S3 download error: code {0}")]
    S3DownloadError(u16),
}

impl From<std::io::Error> for OneIoError {
    fn from(io_error: std::io::Error) -> Self {
        match io_error.kind() {
            std::io::ErrorKind::UnexpectedEof => OneIoError::EofError(io_error),
            _ => OneIoError::IoError(io_error),
        }
    }
}
