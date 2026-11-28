use thiserror::Error;

/// Error type for OneIO operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OneIoError {
    /// All IO-related errors (file system, EOF, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// All network/remote operation errors (HTTP, FTP, S3, JSON parsing)
    ///
    /// The wrapped error is exposed as [`std::error::Error::source`]. Without that marker a
    /// consumer that formats an error chain, such as anyhow's `{:#}`, printed only reqwest's
    /// "error sending request for url (...)" and never the reason underneath it, so a fetch that
    /// failed with a connection reset and one that timed out looked identical in a log line and in
    /// a database column.
    #[error("{0}")]
    Network(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// Network error with URL context for debugging
    #[error("{url}: {source}")]
    NetworkWithContext {
        source: Box<dyn std::error::Error + Send + Sync>,
        url: String,
    },

    /// Structured status errors from remote services
    #[error("{service} status error: {code}{}", .message.as_deref().map(|m| format!(": {m}")).unwrap_or_default())]
    Status {
        service: &'static str,
        code: u16,
        message: Option<String>,
    },

    /// Invalid header name or value
    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    /// Invalid certificate data
    #[error("Invalid certificate: {0}")]
    InvalidCertificate(String),

    /// Feature not supported/compiled
    #[error("Not supported: {0}")]
    NotSupported(String),
}

// Convert various network-related errors to Network variant
#[cfg(feature = "http")]
impl From<reqwest::Error> for OneIoError {
    fn from(err: reqwest::Error) -> Self {
        OneIoError::Network(Box::new(err))
    }
}

#[cfg(feature = "ftp")]
impl From<suppaftp::FtpError> for OneIoError {
    fn from(err: suppaftp::FtpError) -> Self {
        OneIoError::Network(Box::new(err))
    }
}

#[cfg(feature = "json")]
impl From<serde_json::Error> for OneIoError {
    fn from(err: serde_json::Error) -> Self {
        OneIoError::Network(Box::new(err))
    }
}

#[cfg(test)]
mod tests {
    use super::OneIoError;
    use std::error::Error;

    /// The wrapped network error has to stay reachable: a consumer that formats the chain needs the
    /// cause, not only reqwest's one-line summary, or "connection reset" and "timed out" read the
    /// same in a log line.
    #[test]
    fn a_network_error_exposes_the_wrapped_cause() {
        let inner = std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "connection reset by peer",
        );
        let error = OneIoError::Network(Box::new(inner));
        let source = error.source().expect("the wrapped error is the source");
        assert!(source.to_string().contains("connection reset by peer"));

        let mut chain = vec![error.to_string()];
        let mut current = error.source();
        while let Some(item) = current {
            chain.push(item.to_string());
            current = item.source();
        }
        assert!(
            chain.len() >= 2,
            "the cause has to survive in the chain: {chain:?}"
        );
    }
}
