//! A wrapper around an HTTP response that transparently retries using Range
//! requests when the connection is dropped (e.g., server send-timeout).
//!
//! This is critical for applications that read from multiple remote streams
//! concurrently: when one stream is paused while another is being consumed,
//! the server may close the idle connection. This reader detects the failure
//! and reconnects from where it left off.

use reqwest::{
    blocking::{Client, Response},
    header::HeaderValue,
};
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
    let mut parts = value.split_whitespace();
    let unit = parts.next()?;
    if !unit.eq_ignore_ascii_case("bytes") {
        return None;
    }
    let range = parts.next()?;
    let start = range.split_once('-')?.0;
    start.trim().parse().ok()
}

/// How to interpret a `416 Range Not Satisfiable` reply to a resume request.
enum RangeNotSatisfiable {
    /// The resume offset is at or past the known content length, so the body
    /// was already fully read — treat the 416 as a clean end of stream.
    Complete,
    /// The resume offset is below the known content length, or the length is
    /// unknown. The body is incomplete and returning it would be a silent
    /// truncation, so the read must fail.
    Incomplete,
}

/// Decides whether a `416` at the current offset means the stream is complete
/// or truncated. A 416 is only EOF when we know the total size and have already
/// read at least that many bytes; otherwise it signals an incomplete transfer.
fn classify_range_not_satisfiable(offset: u64, content_length: Option<u64>) -> RangeNotSatisfiable {
    match content_length {
        Some(len) if offset >= len => RangeNotSatisfiable::Complete,
        _ => RangeNotSatisfiable::Incomplete,
    }
}

/// Result of comparing the `ETag` / `Last-Modified` validators of the original
/// and resumed responses to decide whether they describe the same resource.
enum ValidatorCheck {
    /// The validators agree, or the original response advertised none.
    Match,
    /// A validator present in both responses changed: the resource was modified
    /// mid-transfer, so the two byte ranges must not be spliced together.
    Modified,
    /// The original advertised validators but the resume echoed none of them,
    /// so the resource cannot be confirmed unchanged.
    Unverifiable,
}

/// Compares the original and resumed validators.
///
/// For every validator type present in both responses the values must be equal;
/// a single mismatch means the resource changed. If the original advertised any
/// validator, at least one of them must reappear on the resume, otherwise the
/// match cannot be confirmed. When the original had no validator at all there is
/// nothing to compare, so the resume is optimistically accepted (documented
/// residual risk).
///
/// Note: weak `ETag`s (`W/"..."`) are compared verbatim. They are not strictly
/// reliable for byte-range validation, but a matching weak validator is still a
/// useful signal in practice.
fn compare_validators(
    original_last_modified: Option<&HeaderValue>,
    original_etag: Option<&HeaderValue>,
    resume_last_modified: Option<&HeaderValue>,
    resume_etag: Option<&HeaderValue>,
) -> ValidatorCheck {
    // Nothing to compare against — accept by policy.
    if original_last_modified.is_none() && original_etag.is_none() {
        return ValidatorCheck::Match;
    }

    let mut shared = false;

    if let (Some(original), Some(resume)) = (original_etag, resume_etag) {
        shared = true;
        if original != resume {
            return ValidatorCheck::Modified;
        }
    }
    if let (Some(original), Some(resume)) = (original_last_modified, resume_last_modified) {
        shared = true;
        if original != resume {
            return ValidatorCheck::Modified;
        }
    }

    if shared {
        ValidatorCheck::Match
    } else {
        ValidatorCheck::Unverifiable
    }
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
    /// The original response's `Content-Length`, used to validate the resumed
    /// response's EOF.
    content_length: Option<u64>,
    /// The original response's `Last-Modified` header value.
    last_modified: Option<HeaderValue>,
    /// The original response's `ETag` header value.
    etag: Option<HeaderValue>,
}

/// Outcome of an attempt to resume the download from the current offset.
enum Resume {
    /// The stream was replaced; read again from the new response.
    Resumed,
    /// The stream has been consumed cleanly.
    Eof,
    /// The server ignored the Range request; resuming is not possible.
    Unsupported,
    /// Every reconnection attempt failed to reach the server.
    Failed,
}

impl ResumableHttpReader {
    pub fn new(client: Client, url: String, response: Response) -> Self {
        let content_length: Option<u64> = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok());

        let last_modified = response
            .headers()
            .get(reqwest::header::LAST_MODIFIED)
            .cloned();

        let etag = response.headers().get(reqwest::header::ETAG).cloned();

        Self {
            client,
            url,
            response,
            offset: 0,
            content_length,
            last_modified,
            etag,
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
                // Pin identity explicitly: Range offsets apply to the stored
                // representation, so the body must not be transport-encoded.
                // reqwest's opt-in `gzip` feature currently skips ranged
                // requests, but this guards against that default changing.
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
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
                // If we haven't delivered any bytes yet, a plain restart is safe
                reqwest::StatusCode::OK if self.offset == 0 => {
                    self.response = resp;
                    Ok(Resume::Resumed)
                }

                reqwest::StatusCode::RANGE_NOT_SATISFIABLE => {
                    match classify_range_not_satisfiable(self.offset, self.content_length) {
                        // Already read the whole declared body — clean EOF.
                        RangeNotSatisfiable::Complete => Ok(Resume::Eof),
                        // Server can't give us the rest and we're short of the
                        // expected length: fail loudly instead of truncating.
                        RangeNotSatisfiable::Incomplete => Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            format!(
                                "server returned 416 Range Not Satisfiable while resuming at byte {}, \
                                 but the transfer is incomplete (declared content length: {}); \
                                 refusing to return truncated data",
                                self.offset,
                                self.content_length
                                    .map(|len| len.to_string())
                                    .unwrap_or_else(|| "unknown".to_string()),
                            ),
                        )),
                    }
                }

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

        let resume_last_modified = resp.headers().get(reqwest::header::LAST_MODIFIED);
        let resume_etag = resp.headers().get(reqwest::header::ETAG);
        match compare_validators(
            self.last_modified.as_ref(),
            self.etag.as_ref(),
            resume_last_modified,
            resume_etag,
        ) {
            ValidatorCheck::Match => {}
            ValidatorCheck::Modified => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "resumed resource validators (ETag/Last-Modified) do not match the original; \
                     the resource changed mid-transfer",
                ));
            }
            ValidatorCheck::Unverifiable => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "resumed response omitted the ETag/Last-Modified validators the original \
                     provided; cannot confirm the resource is unchanged",
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
mod tests {

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

            let response_part1 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\nETag: \"v1\"\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=5-"));

            let response_part2 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 5-9/10\r\nLast-Modified: Tue, 15 Nov 1994 12:45:26 GMT\r\nETag: \"v1\"\r\n\r\n67890";
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

    // Check reader resumes from a 200 only if offset is zero
    // A server might reply with a 200 to a range request asking for the whole content
    #[test]
    fn resume_200_null_offset() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            let response_part1 =
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\n";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=0-"));

            let response_part2 = "HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n1234567890";
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

    // Check reader returns an error when the server does not support ranges
    // In that case the server returns a 200 and the reader offset is nonzero
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

    // Check that out-of-bounds range requests are treated as an error
    #[test]
    fn range_oob_is_err() {
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
            // server reports 416 which the reader must treat as an error.
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
        assert!(reader.read_to_string(&mut buf).is_err());
        handle.join().unwrap();
    }

    // The 416-as-EOF branch (resuming exactly at/after the declared length)
    // cannot be reached over a real socket: once reqwest has delivered all
    // Content-Length bytes it reports a clean EOF, so no resume is ever issued
    // at that offset. It is defensive code, so the decision is unit-tested
    // directly instead of end-to-end.
    #[test]
    fn classify_416_only_eof_when_complete() {
        use crate::resumable_http::{classify_range_not_satisfiable, RangeNotSatisfiable};

        // Offset at or past a known length -> complete (EOF).
        assert!(matches!(
            classify_range_not_satisfiable(10, Some(10)),
            RangeNotSatisfiable::Complete
        ));
        assert!(matches!(
            classify_range_not_satisfiable(11, Some(10)),
            RangeNotSatisfiable::Complete
        ));

        // Offset below a known length -> incomplete (error).
        assert!(matches!(
            classify_range_not_satisfiable(5, Some(10)),
            RangeNotSatisfiable::Incomplete
        ));

        // Unknown length -> incomplete (error).
        assert!(matches!(
            classify_range_not_satisfiable(5, None),
            RangeNotSatisfiable::Incomplete
        ));
    }

    // Exhaustively check the validator comparison logic, which drives whether a
    // resumed response is accepted, rejected as modified, or rejected as
    // unverifiable.
    #[test]
    fn compare_validators_decision_table() {
        use crate::resumable_http::{compare_validators, ValidatorCheck};
        use reqwest::header::HeaderValue;

        let etag_a = HeaderValue::from_static("\"v1\"");
        let etag_b = HeaderValue::from_static("\"v2\"");
        let lm_a = HeaderValue::from_static("Tue, 15 Nov 1994 12:45:26 GMT");
        let lm_b = HeaderValue::from_static("Tue, 15 Nov 1995 12:45:26 GMT");

        // No original validators -> accept whatever the resume carries.
        assert!(matches!(
            compare_validators(None, None, Some(&lm_b), Some(&etag_b)),
            ValidatorCheck::Match
        ));

        // Matching single validator -> match.
        assert!(matches!(
            compare_validators(Some(&lm_a), None, Some(&lm_a), None),
            ValidatorCheck::Match
        ));
        assert!(matches!(
            compare_validators(None, Some(&etag_a), None, Some(&etag_a)),
            ValidatorCheck::Match
        ));

        // Mismatching single validator -> modified.
        assert!(matches!(
            compare_validators(Some(&lm_a), None, Some(&lm_b), None),
            ValidatorCheck::Modified
        ));
        assert!(matches!(
            compare_validators(None, Some(&etag_a), None, Some(&etag_b)),
            ValidatorCheck::Modified
        ));

        // Original advertised one validator, resume echoes it plus an extra one
        // we never had -> match (the extra is ignored, not a reason to reject).
        assert!(matches!(
            compare_validators(None, Some(&etag_a), Some(&lm_a), Some(&etag_a)),
            ValidatorCheck::Match
        ));
        assert!(matches!(
            compare_validators(Some(&lm_a), None, Some(&lm_a), Some(&etag_a)),
            ValidatorCheck::Match
        ));

        // Original had both, resume echoes only the matching ETag -> match.
        assert!(matches!(
            compare_validators(Some(&lm_a), Some(&etag_a), None, Some(&etag_a)),
            ValidatorCheck::Match
        ));

        // Original had both, resume echoes a matching ETag but a changed
        // Last-Modified -> modified (any shared mismatch fails).
        assert!(matches!(
            compare_validators(Some(&lm_a), Some(&etag_a), Some(&lm_b), Some(&etag_a)),
            ValidatorCheck::Modified
        ));

        // Original had validators, resume echoes none of them -> unverifiable.
        assert!(matches!(
            compare_validators(Some(&lm_a), Some(&etag_a), None, None),
            ValidatorCheck::Unverifiable
        ));

        // Original had one validator, resume returns only a different-type
        // validator -> unverifiable (nothing shared to compare).
        assert!(matches!(
            compare_validators(Some(&lm_a), None, None, Some(&etag_a)),
            ValidatorCheck::Unverifiable
        ));
    }

    // Check reader returns an error when resuming reading a resource that was modified
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

    // Check reader returns an error when the resumed response advertises an ETag
    // that differs from the original (the resource changed mid-transfer)
    #[test]
    fn etag_mismatch_is_err() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/data.txt", port);

        let handle = thread::spawn(move || {
            let (mut stream1, _) = listener.accept().unwrap();
            read_request(&mut stream1);

            let response_part1 =
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"v1\"\r\n\r\n12345";
            stream1.write_all(response_part1.as_bytes()).unwrap();
            drop(stream1);

            let (mut stream2, _) = listener.accept().unwrap();
            let req = read_request(&mut stream2);
            assert!(req.to_ascii_lowercase().contains("range: bytes=5-"));

            // The resumed response advertises a different ETag.
            let response_part2 = "HTTP/1.1 206 Partial Content\r\nContent-Length: 5\r\nContent-Range: bytes 5-9/10\r\nETag: \"v2\"\r\n\r\n67890";
            stream2.write_all(response_part2.as_bytes()).unwrap();
        });

        let client = reqwest::blocking::Client::new();
        let resp = client.get(&url).send().unwrap();
        let mut reader = ResumableHttpReader::new(client, url, resp);

        let mut buf = String::new();
        assert!(reader.read_to_string(&mut buf).is_err());

        handle.join().unwrap();
    }

    // Check reader returns an error when the resumed response drops the
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

    // Check reader retries until MAX_RETRIES and then returns an error
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
}
