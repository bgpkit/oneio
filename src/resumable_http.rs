//! A wrapper around an HTTP response that transparently retries using Range
//! requests when the connection is dropped (e.g., server send-timeout).
//!
//! This is critical for applications that read from multiple remote streams
//! concurrently: when one stream is paused while another is being consumed,
//! the server may close the idle connection. This reader detects the failure
//! and reconnects from where it left off.

use reqwest::blocking::{Client, Response};
use std::io::{self, Read};

/// Maximum number of consecutive retry attempts before giving up.
const MAX_RETRIES: u32 = 5;

/// An HTTP reader that automatically resumes downloads using Range requests
/// when the underlying connection is dropped.
///
/// The byte stream presented to the consumer is contiguous — reconnections
/// are invisible to layers above (e.g., decompressors).
pub(crate) struct ResumableHttpReader {
    client: Client,
    url: String,
    response: Response,
    /// Total raw (compressed) bytes successfully read so far.
    offset: u64,
}

impl ResumableHttpReader {
    pub fn new(client: Client, url: String, response: Response) -> Self {
        Self {
            client,
            url,
            response,
            offset: 0,
        }
    }
}

impl Read for ResumableHttpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.response.read(buf) {
            Ok(0) => Ok(0),
            Ok(n) => {
                self.offset += n as u64;
                Ok(n)
            }
            Err(original_err) => {
                // Connection was reset/dropped — attempt to resume with Range
                for attempt in 1..=MAX_RETRIES {
                    let backoff_ms = 200u64.saturating_mul(1u64 << attempt.min(4));
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));

                    let result = self
                        .client
                        .get(&self.url)
                        .header("Range", format!("bytes={}-", self.offset))
                        .send();

                    match result {
                        Ok(resp) if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT => {
                            // Successfully resumed
                            self.response = resp;
                            return self.read(buf);
                        }
                        Ok(resp) if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE => {
                            // Offset is at or past end of file — treat as EOF
                            return Ok(0);
                        }
                        Ok(_) | Err(_) => {
                            // Server doesn't support Range or other error — keep retrying
                            continue;
                        }
                    }
                }

                // All retries exhausted — propagate original error
                Err(original_err)
            }
        }
    }
}
