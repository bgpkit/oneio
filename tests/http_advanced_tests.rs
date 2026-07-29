//! Integration tests for HTTP advanced features:
//! - `reqwest` re-export (`oneio::reqwest`) for naming HTTP types downstream
//! - `reqwest-gzip` feature: transparent gzip content-encoding
//!
//! Uses an in-process mock HTTP server; no external network access required.

#![cfg(feature = "http")]

use std::io::{Read, Write};

/// Spawn a minimal HTTP/1.1 server that answers one canned response per
/// incoming connection, in order. Returns the base URL and a handle that
/// yields the raw request texts it received.
fn mock_server(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::net::TcpListener;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();

            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let bytes_read = stream.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..bytes_read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            requests.push(String::from_utf8(request).unwrap());

            stream.write_all(&response).unwrap();
            stream.flush().unwrap();
        }
        requests
    });

    (format!("http://{addr}"), handle)
}

fn http_response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut response = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    response.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    let mut response = response.into_bytes();
    response.extend_from_slice(body);
    response
}

#[test]
fn test_reqwest_reexport_conditional_get() {
    use oneio::reqwest::StatusCode;
    use oneio::OneIo;

    let body = b"fresh data";
    let (base_url, server) = mock_server(vec![
        http_response(
            "200 OK",
            &[
                ("ETag", "\"v1\""),
                ("Last-Modified", "Wed, 01 Jan 2025 00:00:00 GMT"),
            ],
            body,
        ),
        http_response("304 Not Modified", &[], &[]),
    ]);
    let url = format!("{base_url}/data.txt");

    // Initial unconditional fetch: 200 with validators exposed via re-exported types.
    let client = OneIo::new().unwrap();
    let response = client.get_http_reader_raw(&url).unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response
        .headers()
        .get(oneio::reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    assert_eq!(etag.as_deref(), Some("\"v1\""));

    // Conditional fetch with the validator: 304 maps to "keep cached copy".
    let client = OneIo::builder()
        .header_str("If-None-Match", etag.as_deref().unwrap())
        .build()
        .unwrap();
    let response = client.get_http_reader_raw(&url).unwrap();
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1].to_lowercase().contains("if-none-match: \"v1\""),
        "second request should carry If-None-Match, got: {}",
        requests[1]
    );
}

/// gzip-compressed bytes of `GZIP_TEST_TEXT` (pre-compressed so this test does
/// not require a compression feature to be enabled).
#[cfg(feature = "reqwest-gzip")]
const GZIP_TEST_TEXT: &str = "OneIO test file.\nThis is gzip-encoded content.";

#[cfg(feature = "reqwest-gzip")]
const GZIP_TEST_BODY: [u8; 64] = [
    31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 243, 207, 75, 245, 244, 87, 40, 73, 45, 46, 81, 72, 203,
    204, 73, 213, 227, 10, 201, 200, 44, 86, 0, 162, 244, 170, 204, 2, 221, 212, 188, 228, 252,
    148, 212, 20, 133, 228, 252, 188, 146, 212, 188, 18, 61, 0, 97, 22, 228, 9, 46, 0, 0, 0,
];

#[cfg(feature = "reqwest-gzip")]
#[test]
fn test_reqwest_gzip_transparent_decode() {
    let (base_url, server) = mock_server(vec![http_response(
        "200 OK",
        &[("Content-Encoding", "gzip")],
        &GZIP_TEST_BODY,
    )]);
    // No compression suffix in the URL: decoding must come from the
    // Content-Encoding header, not oneio's suffix-based decompression.
    let url = format!("{base_url}/data.txt");

    let text = oneio::read_to_string_lossy(&url).unwrap();
    assert_eq!(text.as_str(), GZIP_TEST_TEXT);

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0].to_lowercase().contains("accept-encoding: gzip"),
        "request should advertise Accept-Encoding: gzip, got: {}",
        requests[0]
    );
}
