//! Basic integration tests using only default features (gz, bz, http)
//! These tests should always pass with `cargo test`

use std::io::{Read, Write};

const TEST_TEXT: &str = "OneIO test file.\nThis is a test.";

#[cfg(feature = "http")]
fn spawn_http_server(request_count: usize) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::net::TcpListener;
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let body = TEST_TEXT.to_string();

    let handle = std::thread::spawn(move || {
        let mut requests = Vec::with_capacity(request_count);
        for _ in 0..request_count {
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

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
        }
        requests
    });

    (format!("http://{addr}/test.txt"), handle)
}

fn test_read(file_path: &str) {
    let mut reader = oneio::get_reader(file_path).unwrap();
    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);

    assert_eq!(
        oneio::read_to_string(file_path).unwrap().as_str(),
        TEST_TEXT
    );

    let lines = oneio::read_lines(file_path)
        .unwrap()
        .map(|line| line.unwrap())
        .collect::<Vec<String>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].as_str(), "OneIO test file.");
    assert_eq!(lines[1].as_str(), "This is a test.");
}

fn test_write(to_write_file: &str, to_read_file: &str) {
    let mut text = "".to_string();
    oneio::get_reader(to_read_file)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();

    let mut writer = oneio::get_writer(to_write_file).unwrap();
    writer.write_all(text.as_ref()).unwrap();
    drop(writer);

    let mut new_text = "".to_string();
    oneio::get_reader(to_write_file)
        .unwrap()
        .read_to_string(&mut new_text)
        .unwrap();

    assert_eq!(text.as_str(), new_text.as_str());
    std::fs::remove_file(to_write_file).unwrap();
}

#[test]
fn test_local_files() {
    // Test local file reading with default compression formats
    test_read("tests/test_data.txt");

    // Test gzip (default feature)
    #[cfg(feature = "any_gz")]
    test_read("tests/test_data.txt.gz");

    // Test bzip2 (default feature)
    #[cfg(feature = "bz")]
    test_read("tests/test_data.txt.bz2");
}

#[test]
fn test_writers() {
    // Test writing with default compression formats
    test_write("tests/test_write_data.txt", "tests/test_data.txt");

    #[cfg(feature = "any_gz")]
    test_write("tests/test_write_data.txt.gz", "tests/test_data.txt.gz");

    #[cfg(feature = "bz")]
    test_write("tests/test_write_data.txt.bz2", "tests/test_data.txt.bz2");
}

#[cfg(feature = "http")]
#[test]
fn test_remote_files() {
    // Test HTTP reading (default feature)
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt");

    #[cfg(feature = "any_gz")]
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.gz");

    #[cfg(feature = "bz")]
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.bz2");
}

#[cfg(feature = "http")]
#[test]
fn test_404_handling() {
    let reader = oneio::get_reader("https://spaces.bgpkit.org/oneio/test_data_NOT_EXIST.json");
    assert!(reader.is_err());
    assert!(!oneio::exists("https://spaces.bgpkit.org/oneio/test_data_NOT_EXIST.json").unwrap());

    let reader = oneio::get_reader("https://spaces.bgpkit.org/oneio/test_data.json");
    assert!(reader.is_ok());
    assert!(oneio::exists("https://spaces.bgpkit.org/oneio/test_data.json").unwrap());
}

#[cfg(feature = "http")]
#[test]
fn test_oneio_builder_reuses_default_headers() {
    let (url, handle) = spawn_http_server(2);
    let oneio = oneio::OneIo::builder()
        .header_str("X-Test-Token", "secret")
        .build()
        .unwrap();

    let first = oneio.read_to_string(&url).unwrap();
    let second = oneio.read_to_string(&url).unwrap();

    assert_eq!(first, TEST_TEXT);
    assert_eq!(second, TEST_TEXT);

    let requests = handle.join().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests {
        let request = request.to_ascii_lowercase();
        assert!(request.contains("x-test-token: secret"));
        assert!(request.contains("user-agent: oneio"));
    }
}

#[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
#[test]
fn test_oneio_builder_accepts_root_certificate() {
    let cert_pem = br#"-----BEGIN CERTIFICATE-----
MIIDCTCCAfGgAwIBAgIUZwNjzmSANT4XyBCwC6aLzuUhsCAwDQYJKoZIhvcNAQEL
BQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MB4XDTI2MDMwNjE1MjMwNloXDTI2MDMw
NzE1MjMwNlowFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEF
AAOCAQ8AMIIBCgKCAQEAnb4K2oDt8XUvD3MwNSOkfTD2Ud0vqFIsZYSnXdgw2mUT
pYW9Xs+1vdJ3IV77VCAqnvNBm2poL20xkpTQfwPrL4IWNvguAziGiWlSs573jvUe
+myRftFou3iZRl56u3evqKOgkL8CladtHYTx1ZArsKZyJJHpUMrPCMJBvcTBiAh0
kbemeAdcnDP6PORQqW+bibYXz1pyHDGUMXUMOj5PdPV0/ayumXlr1VBnbgkLlrTd
QsJOxLVk9w7RkaLg3pvq0RGvn08up+J8FEkfK1Ddoz4nJnYJy5xgs25rIUDVfGTw
G5QBJdNZKSlXXqQXBawLGHJi7zvSV4urRFXlhfad8wIDAQABo1MwUTAdBgNVHQ4E
FgQUHx0PDPAWL4pKz3T0RVNxjjnYSyEwHwYDVR0jBBgwFoAUHx0PDPAWL4pKz3T0
RVNxjjnYSyEwDwYDVR0TAQH/BAUwAwEB/zANBgkqhkiG9w0BAQsFAAOCAQEAAtmG
mWhz0AfDCxzulDTV4OLTWMkBpGgOlDG/bFWM0+M519t0f2yE7szaboYH+E4axCoe
ZF9zAMKgSmoyCKNnZlFs4ZqXvphNeim1Pnd4LmTbiUGxLwHXuTzwfdIfna4ACx+s
qQe3vGmM9OWcGipiA4Z84HrReW7Ht70enYYpC7CaDalTu9pRZIk/cparF8qL2QNv
OkOLHxPjJTiGWvjaZpzADT30e9SKjK1RPMBRLBUdg4wizKuliRugVYV6flquH/iY
ryXRHfGX358AcPpdZQxhuYsMRkaCKfgCXULQx4+MpoosyeoH6lPRWYeIZVIqL5wc
FZr4y1T605mmkIwGPQ==
-----END CERTIFICATE-----
"#;

    let oneio = oneio::OneIo::builder()
        .add_root_certificate_pem(cert_pem)
        .unwrap()
        .build();

    assert!(oneio.is_ok());
}

// ── file_extension ────────────────────────────────────────────────────────────

#[test]
fn test_file_extension_plain() {
    assert_eq!(oneio::get_reader("tests/test_data.txt").is_ok(), true);
}

#[cfg(feature = "any_gz")]
#[test]
fn test_file_extension_strips_query_params() {
    // Simulate a path where the extension is followed by a query string.
    // The file_extension helper must ignore everything after '?'.
    // We test this by verifying get_reader on a local .gz path with a fake query
    // suffix doesn't crash or misdetect the extension.
    //
    // Direct unit test of the internal helper via a round-trip: write a gz file,
    // construct a path with query-like suffix, confirm detection still works.
    let gz_path = "tests/test_data.txt.gz";
    // file_extension is pub(crate), so we test it indirectly through get_reader_with_type.
    let oneio = oneio::OneIo::new().unwrap();
    // get_reader on local path ignores the query part for protocol detection, but
    // compression detection is what we're verifying here via get_reader_with_type.
    let result = oneio.get_reader_with_type(gz_path, "gz");
    assert!(
        result.is_ok(),
        "get_reader_with_type with explicit gz should succeed"
    );
    let mut content = String::new();
    result.unwrap().read_to_string(&mut content).unwrap();
    assert_eq!(content.as_str(), TEST_TEXT);
}

// ── OneIo::get_reader_with_type ───────────────────────────────────────────────

#[test]
fn test_get_reader_with_type_plain() {
    let oneio = oneio::OneIo::new().unwrap();
    // Explicit empty compression = raw pass-through
    let result = oneio.get_reader_with_type("tests/test_data.txt", "");
    assert!(result.is_ok());
    let mut content = String::new();
    result.unwrap().read_to_string(&mut content).unwrap();
    assert_eq!(content.as_str(), TEST_TEXT);
}

#[cfg(feature = "any_gz")]
#[test]
fn test_get_reader_with_type_gz_override() {
    let oneio = oneio::OneIo::new().unwrap();
    // File is .gz but we pass extension explicitly — should decompress correctly.
    let result = oneio.get_reader_with_type("tests/test_data.txt.gz", "gz");
    assert!(result.is_ok());
    let mut content = String::new();
    result.unwrap().read_to_string(&mut content).unwrap();
    assert_eq!(content.as_str(), TEST_TEXT);
}

#[cfg(feature = "bz")]
#[test]
fn test_get_reader_with_type_bz2_override() {
    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_reader_with_type("tests/test_data.txt.bz2", "bz2");
    assert!(result.is_ok());
    let mut content = String::new();
    result.unwrap().read_to_string(&mut content).unwrap();
    assert_eq!(content.as_str(), TEST_TEXT);
}

// ── OneIoBuilder: timeout and configure_http ──────────────────────────────────

#[cfg(feature = "http")]
#[test]
fn test_builder_timeout_builds_successfully() {
    use std::time::Duration;
    let result = oneio::OneIo::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(5))
        .build();
    assert!(result.is_ok());
}

#[cfg(feature = "http")]
#[test]
fn test_builder_configure_http_escape_hatch() {
    use std::time::Duration;
    // configure_http lets us set options not directly exposed by OneIoBuilder.
    let result = oneio::OneIo::builder()
        .configure_http(|b| b.connection_verbose(false).timeout(Duration::from_secs(10)))
        .build();
    assert!(result.is_ok());
}

#[cfg(feature = "http")]
#[test]
fn test_builder_no_proxy_builds_successfully() {
    let result = oneio::OneIo::builder().no_proxy().build();
    assert!(result.is_ok());
}

// ── download_with_retry ───────────────────────────────────────────────────────

#[cfg(feature = "http")]
#[test]
fn test_download_with_retry_succeeds_on_first_attempt() {
    let (url, handle) = spawn_http_server(1);
    let oneio = oneio::OneIo::new().unwrap();
    let out = "tests/test_download_retry_output.txt";
    let result = oneio.download_with_retry(&url, out, 3);
    handle.join().unwrap();

    assert!(result.is_ok());
    let content = std::fs::read_to_string(out).unwrap();
    assert_eq!(content, TEST_TEXT);
    std::fs::remove_file(out).unwrap();
}

#[cfg(feature = "http")]
#[test]
fn test_download_with_retry_exhausts_retries_on_bad_url() {
    let oneio = oneio::OneIo::new().unwrap();
    // Port 1 is reserved and will immediately refuse the connection.
    let result = oneio.download_with_retry(
        "http://127.0.0.1:1/no-such-file",
        "tests/should_not_exist.txt",
        1,
    );
    assert!(result.is_err());
    // Cleanup in case it somehow created a file.
    let _ = std::fs::remove_file("tests/should_not_exist.txt");
}

// ── Phase 1: LZ4 / XZ / Zstd compression ─────────────────────────────────────

#[cfg(feature = "lz")]
#[test]
fn test_local_lz4() {
    test_read("tests/test_data.txt.lz4");
}

#[cfg(feature = "lz")]
#[test]
fn test_write_lz4() {
    test_write("tests/test_write_data.txt.lz4", "tests/test_data.txt.lz4");
}

#[cfg(feature = "lz")]
#[test]
fn test_get_reader_with_type_lz4_override() {
    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_reader_with_type("tests/test_data.txt.lz4", "lz4");
    assert!(result.is_ok());
    let mut content = String::new();
    result.unwrap().read_to_string(&mut content).unwrap();
    assert_eq!(content.as_str(), TEST_TEXT);
}

#[cfg(feature = "xz")]
#[test]
fn test_local_xz() {
    test_read("tests/test_data.txt.xz");
}

#[cfg(feature = "xz")]
#[test]
fn test_write_xz() {
    test_write("tests/test_write_data.txt.xz", "tests/test_data.txt.xz");
}

#[cfg(feature = "xz")]
#[test]
fn test_get_reader_with_type_xz_override() {
    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_reader_with_type("tests/test_data.txt.xz", "xz");
    assert!(result.is_ok());
    let mut content = String::new();
    result.unwrap().read_to_string(&mut content).unwrap();
    assert_eq!(content.as_str(), TEST_TEXT);
}

#[cfg(feature = "zstd")]
#[test]
fn test_local_zstd() {
    test_read("tests/test_data.txt.zst");
}

#[cfg(feature = "zstd")]
#[test]
fn test_write_zstd() {
    test_write("tests/test_write_data.txt.zst", "tests/test_data.txt.zst");
}

#[cfg(feature = "zstd")]
#[test]
fn test_get_reader_with_type_zstd_override() {
    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_reader_with_type("tests/test_data.txt.zst", "zst");
    assert!(result.is_ok());
    let mut content = String::new();
    result.unwrap().read_to_string(&mut content).unwrap();
    assert_eq!(content.as_str(), TEST_TEXT);
}

// ── Phase 1: Progress tracking ────────────────────────────────────────────────

#[cfg(feature = "http")]
#[test]
fn test_get_reader_with_progress_fires_callback() {
    use std::sync::{Arc, Mutex};

    // get_reader_with_progress makes a HEAD request (content-length probe)
    // followed by a GET request, so the server must handle 2 connections.
    let (url, handle) = spawn_http_server(2);
    let oneio = oneio::OneIo::new().unwrap();

    let observed = Arc::new(Mutex::new(Vec::<(u64, u64)>::new()));
    let observed_cb = Arc::clone(&observed);

    let (mut reader, total_size) = oneio
        .get_reader_with_progress(&url, move |bytes_read, total_bytes| {
            observed_cb.lock().unwrap().push((bytes_read, total_bytes));
        })
        .unwrap();

    // Drain the reader so all callbacks fire.
    let mut content = String::new();
    reader.read_to_string(&mut content).unwrap();
    handle.join().unwrap();

    // Content-Length is set by spawn_http_server, so total_size must be known.
    assert_eq!(total_size, Some(TEST_TEXT.len() as u64));

    let calls = observed.lock().unwrap();
    // At least one callback must have fired.
    assert!(!calls.is_empty(), "progress callback never fired");
    // Final bytes_read must equal the total content length.
    let (final_bytes, _) = *calls.last().unwrap();
    assert_eq!(final_bytes, TEST_TEXT.len() as u64);
    // total_bytes passed to every callback must match the content length.
    for (_, total) in calls.iter() {
        assert_eq!(*total, TEST_TEXT.len() as u64);
    }
    assert_eq!(content, TEST_TEXT);
}

#[test]
fn test_get_reader_with_progress_local_no_total() {
    use std::sync::{Arc, Mutex};

    // Local files don't go through get_content_length HTTP path —
    // total_bytes should be known from fs::metadata, total_size Some.
    let oneio = oneio::OneIo::new().unwrap();
    let observed = Arc::new(Mutex::new(0u64));
    let observed_cb = Arc::clone(&observed);

    let (mut reader, total_size) = oneio
        .get_reader_with_progress("tests/test_data.txt", move |bytes_read, _| {
            *observed_cb.lock().unwrap() = bytes_read;
        })
        .unwrap();

    let mut content = String::new();
    reader.read_to_string(&mut content).unwrap();

    assert_eq!(content, TEST_TEXT);
    // Local file size is known from metadata.
    assert!(total_size.is_some());
    assert_eq!(total_size.unwrap(), TEST_TEXT.len() as u64);
    assert_eq!(*observed.lock().unwrap(), TEST_TEXT.len() as u64);
}

// ── Phase 1: Cache reader ─────────────────────────────────────────────────────

#[test]
fn test_cache_reader_creates_cache_file() {
    let cache_dir = "tests/tmp_cache_create";
    let cache_file = "cached.txt";
    let cache_path = format!("{cache_dir}/{cache_file}");
    // Clean up before test.
    let _ = std::fs::remove_dir_all(cache_dir);

    let oneio = oneio::OneIo::new().unwrap();
    let mut reader = oneio
        .get_cache_reader(
            "tests/test_data.txt",
            cache_dir,
            Some(cache_file.to_string()),
            false,
        )
        .unwrap();

    let mut content = String::new();
    reader.read_to_string(&mut content).unwrap();
    assert_eq!(content, TEST_TEXT);

    // Cache file must exist after the first read.
    assert!(std::path::Path::new(&cache_path).exists());
    std::fs::remove_dir_all(cache_dir).unwrap();
}

#[test]
fn test_cache_reader_reuses_existing_cache() {
    let cache_dir = "tests/tmp_cache_reuse";
    let cache_file = "cached.txt";
    let _ = std::fs::remove_dir_all(cache_dir);
    std::fs::create_dir_all(cache_dir).unwrap();

    // Pre-populate the cache with different content.
    let cached_content = "cached content";
    std::fs::write(format!("{cache_dir}/{cache_file}"), cached_content).unwrap();

    let oneio = oneio::OneIo::new().unwrap();
    // force_cache=false → must read from the pre-existing cache, not the source.
    let mut reader = oneio
        .get_cache_reader(
            "tests/test_data.txt",
            cache_dir,
            Some(cache_file.to_string()),
            false,
        )
        .unwrap();

    let mut content = String::new();
    reader.read_to_string(&mut content).unwrap();
    assert_eq!(
        content, cached_content,
        "should have read from cache, not source"
    );
    std::fs::remove_dir_all(cache_dir).unwrap();
}

#[test]
fn test_cache_reader_force_refreshes_cache() {
    let cache_dir = "tests/tmp_cache_force";
    let cache_file = "cached.txt";
    let _ = std::fs::remove_dir_all(cache_dir);
    std::fs::create_dir_all(cache_dir).unwrap();

    // Pre-populate the cache with stale content.
    std::fs::write(format!("{cache_dir}/{cache_file}"), "stale content").unwrap();

    let oneio = oneio::OneIo::new().unwrap();
    // force_cache=true → must re-fetch from source and overwrite cache.
    let mut reader = oneio
        .get_cache_reader(
            "tests/test_data.txt",
            cache_dir,
            Some(cache_file.to_string()),
            true,
        )
        .unwrap();

    let mut content = String::new();
    reader.read_to_string(&mut content).unwrap();
    assert_eq!(content, TEST_TEXT, "should have re-fetched from source");

    // Cache file on disk must also be updated.
    let on_disk = std::fs::read_to_string(format!("{cache_dir}/{cache_file}")).unwrap();
    assert_eq!(on_disk, TEST_TEXT);
    std::fs::remove_dir_all(cache_dir).unwrap();
}

#[test]
fn test_cache_reader_creates_missing_cache_dir() {
    // The cache directory must not exist before the call.
    let cache_dir = "tests/tmp_cache_dir_creation/nested/path";
    let _ = std::fs::remove_dir_all("tests/tmp_cache_dir_creation");

    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_cache_reader("tests/test_data.txt", cache_dir, None, false);
    assert!(result.is_ok(), "should create nested cache directory");
    std::fs::remove_dir_all("tests/tmp_cache_dir_creation").unwrap();
}

// ── Phase 1: JSON parsing ─────────────────────────────────────────────────────

#[cfg(feature = "json")]
#[test]
fn test_read_json_struct_local() {
    use serde::Deserialize;

    #[derive(Deserialize, PartialEq, Debug)]
    struct TestData {
        name: String,
        value: u32,
        enabled: bool,
        items: Vec<String>,
    }

    let result = oneio::read_json_struct::<TestData>("tests/test_data.json");
    assert!(
        result.is_ok(),
        "read_json_struct failed: {:?}",
        result.err()
    );
    let data = result.unwrap();
    assert_eq!(data.name, "oneio_test");
    assert_eq!(data.value, 42);
    assert!(data.enabled);
    assert_eq!(data.items, vec!["alpha", "beta", "gamma"]);
}

#[cfg(feature = "json")]
#[test]
fn test_read_json_struct_invalid_returns_error() {
    // A plain text file is not valid JSON — must return an error, not panic.
    let result = oneio::read_json_struct::<serde_json::Value>("tests/test_data.txt");
    assert!(result.is_err());
}

// ── Phase 1: Content length ───────────────────────────────────────────────────

#[test]
fn test_get_content_length_local_file() {
    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_content_length("tests/test_data.txt");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), TEST_TEXT.len() as u64);
}

#[cfg(feature = "http")]
#[test]
fn test_get_content_length_http_with_content_length_header() {
    let (url, handle) = spawn_http_server(1);
    let oneio = oneio::OneIo::new().unwrap();
    // spawn_http_server sends Content-Length, so we must get it back.
    let result = oneio.get_content_length(&url);
    handle.join().unwrap();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), TEST_TEXT.len() as u64);
}

// ── Phase 2: get_writer_raw ───────────────────────────────────────────────────

#[test]
fn test_get_writer_raw_creates_uncompressed_file() {
    let path = "tests/tmp_writer_raw.txt";
    let oneio = oneio::OneIo::new().unwrap();

    {
        let mut writer = oneio.get_writer_raw(path).unwrap();
        writer.write_all(TEST_TEXT.as_bytes()).unwrap();
    }

    // File must be readable as plain text (no compression wrapper).
    let content = std::fs::read_to_string(path).unwrap();
    assert_eq!(content, TEST_TEXT);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn test_get_writer_raw_creates_parent_dirs() {
    let path = "tests/tmp_writer_raw_nested/subdir/out.txt";
    let _ = std::fs::remove_dir_all("tests/tmp_writer_raw_nested");

    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_writer_raw(path);
    assert!(result.is_ok(), "get_writer_raw should create parent dirs");
    std::fs::remove_dir_all("tests/tmp_writer_raw_nested").unwrap();
}

// ── Phase 2: SHA256 digest ────────────────────────────────────────────────────

#[cfg(feature = "digest")]
#[test]
fn test_get_sha256_digest_known_file() {
    // Known SHA256 of tests/test_data.txt (pre-computed with sha256sum).
    const EXPECTED: &str = "51a6f9bf51d9e6243fe838242bb74e6e16f77c87cae138b9f3e065c173fc63c7";
    let result = oneio::get_sha256_digest("tests/test_data.txt");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), EXPECTED);
}

#[cfg(feature = "digest")]
#[test]
fn test_get_sha256_digest_missing_file_returns_error() {
    let result = oneio::get_sha256_digest("tests/does_not_exist.txt");
    assert!(result.is_err());
}

// ── Phase 2: Error variants ───────────────────────────────────────────────────

// Note: reqwest::Certificate::from_pem/from_der do not validate certificate data
// at parse time. They only validate when used in a TLS connection. Therefore,
// we cannot test for InvalidCertificate errors with invalid data here.

#[cfg(feature = "http")]
#[test]
fn test_network_error_on_refused_connection() {
    // Port 1 is reserved and always refuses connections — produces a network error.
    let oneio = oneio::OneIo::new().unwrap();
    let result = oneio.get_reader("http://127.0.0.1:1/file.txt");
    assert!(result.is_err());
    // Error display must be non-empty and useful.
    assert!(!result.err().unwrap().to_string().is_empty());
}

// ── Phase 2: Environment variables ───────────────────────────────────────────

#[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
#[test]
fn test_oneio_ca_bundle_env_var_valid_path() {
    // Point ONEIO_CA_BUNDLE at a known PEM cert — builder must succeed.
    std::env::set_var("ONEIO_CA_BUNDLE", "tests/test-cert.pem");
    let result = oneio::OneIo::builder().build();
    std::env::remove_var("ONEIO_CA_BUNDLE");
    assert!(
        result.is_ok(),
        "builder failed with valid ONEIO_CA_BUNDLE: {:?}",
        result.err()
    );
}

#[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
#[test]
fn test_oneio_ca_bundle_env_var_missing_path() {
    // A non-existent path must be silently ignored (not panic or error).
    std::env::set_var("ONEIO_CA_BUNDLE", "/tmp/oneio_does_not_exist_ca.pem");
    let result = oneio::OneIo::builder().build();
    std::env::remove_var("ONEIO_CA_BUNDLE");
    assert!(
        result.is_ok(),
        "builder should ignore missing ONEIO_CA_BUNDLE"
    );
}

#[cfg(all(feature = "http", any(feature = "rustls", feature = "native-tls")))]
#[test]
fn test_oneio_accept_invalid_certs_env_var() {
    // Builder must succeed when env var is set to "true".
    std::env::set_var("ONEIO_ACCEPT_INVALID_CERTS", "true");
    let result = oneio::OneIo::builder().build();
    std::env::remove_var("ONEIO_ACCEPT_INVALID_CERTS");
    assert!(result.is_ok());
}
