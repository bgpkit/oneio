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

/// Normal delay
#[cfg(not(test))]
const BASE_RETRY_DELAY_MS: u64 = 200;

/// Small delay for testing
#[cfg(test)]
const BASE_RETRY_DELAY_MS: u64 = 1;

/// Parses the starting byte offset from a `Content-Range` header value.
///
/// The header has the form `bytes <start>-<end>/<total>` (RFC 9110 §14.4);
/// only `<start>` is needed to confirm where the server resumed. Returns
/// `None` if the value is not in the expected form.
fn parse_content_range_start(value: &str) -> Option<u64> {
    // "bytes 5-9/10" -> "5-9/10" -> "5"
    let range = value.strip_prefix("bytes ")?;
    let start = range.split_once('-')?.0;
    start.trim().parse().ok()
}

/// An HTTP reader that automatically resumes downloads using Range requests
/// when the underlying connection is dropped.
///
/// The byte stream presented to the consumer is contiguous — reconnections
/// are invisible to layers above (e.g., decompressors).
pub(crate) struct ResumableHttpReader {
    client: Client,
    url: String,
    response: Response,
    /// Total raw bytes successfully read so far.
    offset: u64,
}

/// Outcome of an attempt to resume the download from the current offset.
enum Resume {
    /// The stream was replaced; read again from the new response.
    Resumed,
    /// The requested range is unsatisfiable; treat as a clean EOF.
    Eof,
    /// The server ignored the Range request; resuming is not possible.
    Unsupported,
    /// Every reconnection attempt failed to reach the server.
    Failed,
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

    /// Reconnects and resumes the download from `self.offset`.
    ///
    /// The request itself is retried up to `MAX_RETRIES` times with exponential
    /// backoff to ride out transient connection failures. Returns `Err` only
    /// when the server replies but its response is malformed (missing/invalid
    /// `Content-Range`, or a start offset that does not match the request),
    /// since continuing to read would corrupt the stream.
    fn resume(&mut self) -> io::Result<Resume> {
        for attempt in 0..MAX_RETRIES {
            let resp = match self
                .client
                .get(&self.url)
                .header(reqwest::header::RANGE, format!("bytes={}-", self.offset))
                .send()
            {
                Ok(resp) => resp,
                // Couldn't reach the server — back off and try again.
                Err(_) => {
                    let backoff_ms = BASE_RETRY_DELAY_MS.saturating_mul(1u64 << attempt.min(4));
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                    continue;
                }
            };

            return match resp.status() {
                // Offset is at or past end of file.
                reqwest::StatusCode::RANGE_NOT_SATISFIABLE => Ok(Resume::Eof),
                // Server honored the Range request.
                reqwest::StatusCode::PARTIAL_CONTENT => {
                    self.accept_resumed_response(resp)?;
                    Ok(Resume::Resumed)
                }
                // Anything else means the Range request was ignored.
                _ => Ok(Resume::Unsupported),
            };
        }

        Ok(Resume::Failed)
    }

    /// Validates a `206 Partial Content` response and installs it as the current
    /// stream. Fails if the server resumed from a wrong offset or a modified
    /// resource, which would corrupt the stream.
    fn accept_resumed_response(&mut self, resp: Response) -> io::Result<()> {
        let content_range = resp
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "resumed response is missing the Content-Range header",
                )
            })?
            .to_str()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let start = parse_content_range_start(content_range).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("malformed Content-Range header: {content_range}"),
            )
        })?;

        if start != self.offset {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("server resumed at byte {start}, expected {}", self.offset),
            ));
        }

        if let Some(last_modified) = self.response.headers().get(reqwest::header::LAST_MODIFIED) {
            let resume_modified = resp.headers().get(reqwest::header::LAST_MODIFIED).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "resumed resource does not have a last-modified header, but original resource did"
                )
            })?;
            if resume_modified != last_modified {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "resumed resource Last-Modified <{}> does not match the original <{}>",
                        String::from_utf8_lossy(resume_modified.as_bytes()),
                        String::from_utf8_lossy(last_modified.as_bytes())
                    ),
                ));
            }
        }

        self.response = resp;
        Ok(())
    }
}

impl Read for ResumableHttpReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Number of consecutive reconnections that delivered no new bytes.
        // Reading real data returns immediately (so the next `read` call starts
        // fresh), meaning this only fires when a server keeps reconnecting us
        // without making progress — e.g. repeated empty `206` responses — which
        // would otherwise loop forever.
        let mut stalled_retries = 0u32;

        loop {
            match self.response.read(buf) {
                Ok(0) => return Ok(0),
                Ok(n) => {
                    self.offset += n as u64;
                    return Ok(n);
                }
                Err(original_err) => {
                    // Connection was reset/dropped — attempt to resume with Range.
                    if stalled_retries >= MAX_RETRIES {
                        return Err(original_err);
                    }
                    stalled_retries += 1;

                    match self.resume()? {
                        // Read again from the freshly reconnected response.
                        Resume::Resumed => continue,
                        // Nothing more to read.
                        Resume::Eof => return Ok(0),
                        // Can't resume — surface the original failure.
                        Resume::Unsupported | Resume::Failed => return Err(original_err),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {

    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::{
        io::prelude::*,
        net::{TcpListener, TcpStream},
        thread,
    };

    use crate::resumable_http::{ResumableHttpReader, MAX_RETRIES};

    /// Reads a full HTTP request header block (up to and including the blank
    /// CRLF line) from a stream. Reading byte-by-byte avoids consuming past the
    /// header block and works regardless of how the request is split across TCP
    /// reads. Returns whatever was read if the peer closes the connection early.
    fn read_request(stream: &mut TcpStream) -> String {
        let mut data = Vec::new();
        let mut byte = [0u8; 1];
        while let Ok(1) = stream.read(&mut byte) {
            data.push(byte[0]);
            if data.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8_lossy(&data).to_string()
    }

    // Check reader works normally when there is no need to resume
    #[test]
    fn no_drop() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();

            // Read the request to prevent reqwest from throwing an error
            read_request(&mut stream);

            let response = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n1234567890";
            stream.write_all(response.as_bytes()).unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);

        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();

        assert_eq!(buf.as_str(), "1234567890");
        handle.join().unwrap();
    }

    // Check reader resumes when server drops connection
    #[test]
    fn drop_resume() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            let response_part1 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=5-"));

            let response_part2 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 5-9/10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\n\r\n67890";
            stream2.write_all(response_part2.as_bytes()).unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);

        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();

        assert_eq!(buf.as_str(), "1234567890");
        handle.join().unwrap();
    }

    // Check reader returns an error when the server does not support Ranges
    #[test]
    fn range_not_supported_is_err() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            let response_part1 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            read_request(&mut stream2);

            // A server not supporting ranges respond with a 200
            let response_part2 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n1234567890";
            stream2.write_all(response_part2.as_bytes()).unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);

        let mut buf = String::new();
        assert!(reader.read_to_string(&mut buf).is_err());
        handle.join().unwrap();
    }

    // Check out of bound range requests are treated as EOF
    #[test]
    fn range_oob_is_eof() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            // Declare more bytes than we actually send, then drop the connection
            // so the client is forced to resume from offset 10.
            let response_part1 = "HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n1234567890";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            // The resume request starts past the end of the real content, so the
            // server reports 416 which the reader must treat as a clean EOF.
            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=10-"));

            let response_part2 = "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\n\r\n";
            stream2.write_all(response_part2.as_bytes()).unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);

        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap();
        assert_eq!(buf.as_str(), "1234567890");
        handle.join().unwrap();
    }

    // Check reader returns error when resuming reading a resource that was modified
    #[test]
    fn new_last_modified_is_err() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            let response_part1 =
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=5-"));

            let response_part2 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 5-9/10\r\nLast-Modified: Tue, 15 Nov 1995 12:45:26 GMT\r\n\r\n67890";
            stream2.write_all(response_part2.as_bytes()).unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);

        let mut buf = String::new();
        assert!(reader.read_to_string(&mut buf).is_err());

        handle.join().unwrap();
    }

    // Check reader returns error when the resumed response drops the
    // Last-Modified header that the original response carried
    #[test]
    fn missing_last_modified_on_resume_is_err() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            let response_part1 =
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=5-"));

            // The resumed response omits the Last-Modified header entirely.
            let response_part2 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 5-9/10\r\n\r\n67890";
            stream2.write_all(response_part2.as_bytes()).unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);

        let mut buf = String::new();
        assert!(reader.read_to_string(&mut buf).is_err());

        handle.join().unwrap();
    }

    // Check reader retries until MAX_RETRIES and then return an error
    #[test]
    fn max_retries_exhausted_is_err() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        // Set up a thread-safe counter to track how many times the client reconnects
        let attempts_counter = Arc::new(AtomicUsize::new(0));
        let server_counter = attempts_counter.clone();

        let handle = thread::spawn(move || {
            // Handle the initial successful request
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);
            let response_part1 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1); // Drop to trigger the client's retry logic

            // Loop indefinitely to prove read stop comes from the client
            loop {
                let (mut stream, _) = listener.accept().unwrap();
                let req = read_request(&mut stream);

                // Look for the "poison pill" to know when to gracefully shut down
                if req.is_empty() || req.starts_with("STOP") {
                    break;
                }

                server_counter.fetch_add(1, Ordering::SeqCst);
                drop(stream);
            }
        });

        // Setup the client and initial request
        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);
        let mut buf = String::new();

        // Reader should exhaust its retries and fail.
        assert!(reader.read_to_string(&mut buf).is_err());

        // Reader has given up. Now, send a dummy "STOP" request to unblock
        // the server's listener.accept() call so the thread can die cleanly.
        if let Ok(mut wake_stream) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
            let _ = wake_stream.write_all(b"STOP");
        }

        handle.join().unwrap();

        // Assert that the client stopped exactly when it was supposed to
        assert_eq!(
            attempts_counter.load(Ordering::SeqCst),
            MAX_RETRIES as usize
        );
    }

    // Check reader does not loop forever when the server keeps returning a 206
    // whose body never arrives.
    #[test]
    fn no_data_206() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        // Set up a thread-safe counter to track how many times the client reconnects
        let attempts_counter = Arc::new(AtomicUsize::new(0));
        let server_counter = attempts_counter.clone();

        let handle = thread::spawn(move || {
            // Handle the initial successful request
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);
            let response_part1 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1); // Drop to trigger the client's retry logic

            // Every resume gets a well-formed 206 whose declared body never arrives.
            let response_206 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 5-9/10\r\n\r\n";
            loop {
                let (mut stream, _) = listener.accept().unwrap();
                let req = read_request(&mut stream);

                // Look for the "poison pill" to know when to gracefully shut down
                if req.is_empty() || req.starts_with("STOP") {
                    break;
                }

                server_counter.fetch_add(1, Ordering::SeqCst);
                stream.write_all(response_206.as_bytes()).unwrap();
                drop(stream);
            }
        });

        // Setup the client and initial request
        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);
        let mut buf = String::new();

        // Reader should give up instead of spinning forever.
        assert!(reader.read_to_string(&mut buf).is_err());

        // Reader has given up. Now, send a dummy "STOP" request to unblock
        // the server's listener.accept() call so the thread can die cleanly.
        if let Ok(mut wake_stream) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
            let _ = wake_stream.write_all(b"STOP");
        }

        handle.join().unwrap();

        // The reader stopped after exactly MAX_RETRIES no-progress attempts.
        assert_eq!(
            attempts_counter.load(Ordering::SeqCst),
            MAX_RETRIES as usize
        );
    }

    // Check `download()` resumes when the server drops the connection mid-body,
    // so the file written to disk contains the complete content. This exercises
    // the resumable wiring through the download path (not just a direct reader).
    #[test]
    fn download_resumes_after_drop() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            let response_part1 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=5-"));

            let response_part2 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 5-9/10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\n\r\n67890";
            stream2.write_all(response_part2.as_bytes()).unwrap();
        });

        let local_path = std::env::temp_dir().join(format!("oneio_download_resume_{}.txt", port));
        let local = local_path.to_str().unwrap();

        crate::OneIo::new().unwrap().download(&url, local).unwrap();

        let content = std::fs::read_to_string(&local_path).unwrap();
        let _ = std::fs::remove_file(&local_path);

        assert_eq!(content.as_str(), "1234567890");
        handle.join().unwrap();
    }
}
