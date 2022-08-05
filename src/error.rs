use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum OneIoErrorKind {
    #[cfg(feature="remote")]
    RemoteIoError(reqwest::Error),
    EofError(std::io::Error),
    IoError(std::io::Error),
    NotSupported(String),
    CacheIoError(String),
}

#[derive(Debug)]
pub struct OneIoError {
    pub kind: OneIoErrorKind,
}

impl Display for OneIoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = match &self.kind {
            #[cfg(feature="remote")]
            OneIoErrorKind::RemoteIoError(e) => {e.to_string()}
            OneIoErrorKind::EofError(e) => {e.to_string()}
            OneIoErrorKind::IoError(e) => {e.to_string()}
            OneIoErrorKind::NotSupported(msg) => {msg.clone()}
            OneIoErrorKind::CacheIoError(msg) => {msg.clone()}
        };
        write!(f, "error: {}", msg)
    }
}

impl Error for OneIoError {

}

#[cfg(feature="remote")]
impl From<reqwest::Error> for OneIoError {
    fn from(error: reqwest::Error) -> Self {
        OneIoError{
            kind: OneIoErrorKind::RemoteIoError(error)
        }
    }
}

impl From<std::io::Error> for OneIoError {
    fn from(io_error: std::io::Error) -> Self {
        OneIoError {
            kind: match io_error.kind() {
                std::io::ErrorKind::UnexpectedEof => { OneIoErrorKind::EofError(io_error)}
                _ => OneIoErrorKind::IoError(io_error)
            }
        }
    }
}

