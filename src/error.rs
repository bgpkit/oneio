use thiserror::Error;

#[derive(Debug, Error)]
pub enum OneIoError {
    #[cfg(feature = "remote")]
    #[error("remote IO error: {0}")]
    RemoteIoError(#[from] reqwest::Error),
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
}

impl From<std::io::Error> for OneIoError {
    fn from(io_error: std::io::Error) -> Self {
        match io_error.kind() {
            std::io::ErrorKind::UnexpectedEof => OneIoError::EofError(io_error),
            _ => OneIoError::IoError(io_error),
        }
    }
}
