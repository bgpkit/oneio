//! End-to-end test for `get_resumable_http_reader` against a real remote file.
//!
//! Unlike the raw-socket unit tests in `src/resumable_http.rs` (which script
//! exact responses), this test puts a local proxy in front of a real upstream
//! server. The proxy kills the connection mid-body on the first request, then
//! forwards the reader's Range request to the real upstream — so the 206,
//! `Content-Range`, `ETag`, and `Last-Modified` the reader validates against
//! all come from the actual server.
//!
//! The payload is bzip2-compressed, so a corrupted splice would fail
//! decompression — an independent integrity check on top of content equality.
//!
//! Requires network access to data.bgpkit.com. Run with:
//!
//! ```bash
//! cargo test --features https,bz --test resumable_real_e2e -- --ignored --nocapture
//! ```

#![cfg(all(feature = "http", feature = "bz"))]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

const UPSTREAM: &str = "https://data.bgpkit.com/as2rel/as2rel-latest.json.bz2";
/// Kill the first response after this many body bytes.
const DROP_AFTER: usize = 256 * 1024;

/// Headers extracted from one client request.
struct ClientRequest {
    range: Option<String>,
    accept_encoding: Option<String>,
}

/// Reads the client's request header block and extracts the headers the test
/// cares about.
fn read_request(stream: &mut TcpStream) -> ClientRequest {
    let mut data = Vec::new();
    let mut byte = [0u8; 1];
    while stream.read(&mut byte).ok() == Some(1) {
        data.push(byte[0]);
        if data.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let text = String::from_utf8_lossy(&data);
    let header = |name: &str| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case(name)
                .then(|| value.trim().to_string())
        })
    };
    ClientRequest {
        range: header("range"),
        accept_encoding: header("accept-encoding"),
    }
}

/// Handles one proxied connection: fetches the real upstream (forwarding any
/// Range header), relays status + validator headers + body. If `kill_mid_body`
/// is set, abruptly closes the connection after `DROP_AFTER` body bytes.
fn handle_connection(
    mut stream: TcpStream,
    kill_mid_body: bool,
    requests: Arc<Mutex<Vec<ClientRequest>>>,
) {
    let client_req = read_request(&mut stream);
    let range = client_req.range.clone();
    requests.lock().unwrap().push(client_req);

    let client = reqwest::blocking::Client::new();
    let mut req = client.get(UPSTREAM);
    if let Some(r) = &range {
        req = req.header(reqwest::header::RANGE, r);
    }
    let mut resp = match req.send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("proxy: upstream request failed: {e}");
            return;
        }
    };

    let status = resp.status();
    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        status.as_u16(),
        status.canonical_reason().unwrap_or("Status")
    );
    for h in [
        reqwest::header::CONTENT_LENGTH,
        reqwest::header::CONTENT_RANGE,
        reqwest::header::ETAG,
        reqwest::header::LAST_MODIFIED,
    ] {
        if let Some(v) = resp.headers().get(&h) {
            head.push_str(&format!("{}: {}\r\n", h.as_str(), v.to_str().unwrap()));
        }
    }
    head.push_str("Connection: close\r\n\r\n");
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }

    let mut buf = [0u8; 8192];
    let mut sent = 0usize;
    loop {
        let n = match resp.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                eprintln!("proxy: upstream body read failed: {e}");
                return;
            }
        };
        if kill_mid_body && sent + n > DROP_AFTER {
            let _ = stream.write_all(&buf[..DROP_AFTER - sent]);
            let _ = stream.flush();
            // Abrupt close: the client sees an incomplete body.
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
        if stream.write_all(&buf[..n]).is_err() {
            return;
        }
        sent += n;
    }
}

#[test]
#[ignore = "requires network access to data.bgpkit.com"]
fn resumable_reader_real_file_end_to_end() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_log = requests.clone();
    let connection_count = Arc::new(AtomicUsize::new(0));
    let count_log = connection_count.clone();

    let server = thread::spawn(move || {
        // Expect exactly 2 connections: the dropped one and the resume.
        for _ in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            let kill = count_log.fetch_add(1, Ordering::SeqCst) == 0;
            handle_connection(stream, kill, requests_log.clone());
        }
    });

    // Reference bytes: the real file read directly, decompressed by oneio.
    let mut reference = Vec::new();
    oneio::get_reader(UPSTREAM)
        .unwrap()
        .read_to_end(&mut reference)
        .unwrap();
    assert!(!reference.is_empty());

    // The drop point must land inside the compressed body, or no resume
    // would be triggered.
    let compressed_len: u64 = reqwest::blocking::Client::new()
        .head(UPSTREAM)
        .send()
        .unwrap()
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .expect("upstream must report a Content-Length");
    assert!(
        (DROP_AFTER as u64) < compressed_len,
        "test file ({compressed_len} bytes) must be larger than the drop point ({DROP_AFTER})"
    );

    // The reader under test: same real bytes through the dropping proxy.
    let proxy_url = format!("http://127.0.0.1:{port}/as2rel-latest.json.bz2");
    let mut resumed = Vec::new();
    oneio::get_resumable_http_reader(&proxy_url)
        .unwrap()
        .read_to_end(&mut resumed)
        .unwrap();

    server.join().unwrap();

    // A resume actually happened: exactly two requests, the second carrying
    // `Range: bytes={DROP_AFTER}-`.
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "expected initial request plus one resume"
    );
    assert!(requests[0].range.is_none());
    assert_eq!(
        requests[1].range.as_deref(),
        Some(format!("bytes={DROP_AFTER}-").as_str()),
        "resume must continue from the drop offset"
    );
    // Both requests must pin identity encoding, so byte ranges refer to the
    // stored representation even when reqwest's `gzip` feature is enabled.
    for (i, req) in requests.iter().enumerate() {
        assert_eq!(
            req.accept_encoding.as_deref(),
            Some("identity"),
            "request {i} must send Accept-Encoding: identity"
        );
    }

    // The bz2-decoded content matches the reference — bz2 integrity plus
    // byte equality means the splice was exact.
    assert_eq!(resumed.len(), reference.len());
    assert_eq!(resumed, reference);
    println!(
        "OK: resumed after a drop at {DROP_AFTER} bytes; \
         decompressed {} bytes identical to reference",
        reference.len()
    );
}
