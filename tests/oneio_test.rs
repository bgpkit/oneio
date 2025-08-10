use oneio;
use std::io::Write;

const TEST_TEXT: &str = "OneIO test file.
This is a test.";

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

fn test_read_cache(file_path: &str) {
    let cache_file_name = file_path
        .split('/')
        .collect::<Vec<&str>>()
        .into_iter()
        .last()
        .unwrap()
        .to_string();

    let cache_file_path = format!("/tmp/{}", cache_file_name);

    let _ = std::fs::remove_file(cache_file_path.as_str());

    // read from remote then cache
    let mut reader =
        oneio::get_cache_reader(file_path, "/tmp", Some(cache_file_name), true).unwrap();
    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);
    drop(reader);

    // read directly from cache
    let mut reader = oneio::get_reader(cache_file_path.as_str()).unwrap();
    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);
    drop(reader);

    // read directly from remote
    let mut reader = oneio::get_reader(file_path).unwrap();
    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);
    drop(reader);
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
fn test_reader_local() {
    test_read("tests/test_data.txt");
    test_read("tests/test_data.txt.gz");
    test_read("tests/test_data.txt.bz2");
    test_read("tests/test_data.txt.lz4");
    test_read("tests/test_data.txt.xz");
    test_read("tests/test_data.txt.zst");
}

#[test]
fn test_reader_remote() {
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt");
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.gz");
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.bz2");
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.lz4");
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.xz");
}

#[test]
fn test_writer() {
    test_write("tests/test_write_data.txt", "tests/test_data.txt");
    test_write("tests/test_write_data.txt.gz", "tests/test_data.txt.gz");
    test_write("tests/test_write_data.txt.bz2", "tests/test_data.txt.bz2");
    test_write("tests/test_write_data.txt.zst", "tests/test_data.txt.zst");
    // lz4 writer is not currently supported
}

#[test]
fn test_cache_reader() {
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt");
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt.gz");
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt.bz2");
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt.lz4");
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt.xz");
}

#[test]
fn test_read_json_struct() {
    #[derive(serde::Deserialize, Debug)]
    struct Data {
        purpose: String,
        version: u32,
        meta: SubData,
    }
    #[derive(serde::Deserialize, Debug)]
    struct SubData {
        float: f64,
        success: bool,
    }

    let data =
        oneio::read_json_struct::<Data>("https://spaces.bgpkit.org/oneio/test_data.json").unwrap();

    assert_eq!(data.purpose, "test".to_string());
    assert_eq!(data.version, 1);
    assert_eq!(data.meta.float, 1.1);
    assert_eq!(data.meta.success, true);
}

#[test]
fn test_read_404() {
    let reader = oneio::get_reader("https://spaces.bgpkit.org/oneio/test_data_NOT_EXIST.json");
    assert!(reader.is_err());
    assert!(!oneio::exists("https://spaces.bgpkit.org/oneio/test_data_NOT_EXIST.json").unwrap());

    let reader = oneio::get_reader("https://spaces.bgpkit.org/oneio/test_data.json");
    assert!(reader.is_ok());
    assert!(oneio::exists("https://spaces.bgpkit.org/oneio/test_data.json").unwrap());
}

#[test]
fn test_progress_tracking_local() {
    use std::io::Read;
    use std::sync::{Arc, Mutex};

    // Track progress calls
    let progress_calls = Arc::new(Mutex::new(Vec::<(u64, u64)>::new()));
    let calls_clone = progress_calls.clone();

    // Test with a local compressed file
    let (mut reader, total_size) = oneio::get_reader_with_progress(
        "tests/test_data.txt.gz",
        move |bytes_read, total_bytes| {
            calls_clone.lock().unwrap().push((bytes_read, total_bytes));
        },
    )
    .unwrap();

    assert!(total_size > 0, "Total size should be greater than 0");

    // Read the entire file
    let mut content = String::new();
    reader.read_to_string(&mut content).unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());

    // Check that progress was tracked
    let calls = progress_calls.lock().unwrap();
    assert!(
        !calls.is_empty(),
        "Progress callback should have been called"
    );

    // Verify progress calls are reasonable
    let (last_bytes, last_total) = calls.last().unwrap();
    assert_eq!(*last_total, total_size, "Total should match in callbacks");
    assert!(
        *last_bytes <= total_size,
        "Bytes read should not exceed total"
    );
    assert!(*last_bytes > 0, "Should have read some bytes");
}

#[test]
fn test_progress_tracking_remote() {
    use std::io::Read;
    use std::sync::{Arc, Mutex};

    // Track progress calls
    let progress_calls = Arc::new(Mutex::new(Vec::<(u64, u64)>::new()));
    let calls_clone = progress_calls.clone();

    // Test with a remote file that has Content-Length
    let result = oneio::get_reader_with_progress(
        "https://spaces.bgpkit.org/oneio/test_data.txt",
        move |bytes_read, total_bytes| {
            calls_clone.lock().unwrap().push((bytes_read, total_bytes));
        },
    );

    match result {
        Ok((mut reader, total_size)) => {
            assert!(total_size > 0, "Total size should be greater than 0");

            // Read the file
            let mut content = String::new();
            reader.read_to_string(&mut content).unwrap();
            assert_eq!(content.trim(), TEST_TEXT.trim());

            // Check progress tracking
            let calls = progress_calls.lock().unwrap();
            assert!(
                !calls.is_empty(),
                "Progress callback should have been called"
            );

            let (last_bytes, last_total) = calls.last().unwrap();
            assert_eq!(*last_total, total_size);
            assert!(*last_bytes <= total_size);
        }
        Err(oneio::OneIoError::NotSupported(_)) => {
            // Server doesn't provide Content-Length, which is expected behavior
            println!("Remote server doesn't provide Content-Length - this is expected");
        }
        Err(e) => panic!("Unexpected error: {:?}", e),
    }
}

#[test]
fn test_progress_tracking_content_length_missing() {
    // Test with a URL that likely doesn't provide Content-Length
    let result = oneio::get_reader_with_progress(
        "https://httpbin.org/stream/10", // This endpoint doesn't provide Content-Length
        |_bytes_read, _total_bytes| {
            // This callback should never be called
        },
    );

    // Should fail with NotSupported error
    match result {
        Err(oneio::OneIoError::NotSupported(msg)) => {
            assert!(msg.contains("Content-Length") || msg.contains("file size"));
        }
        Err(e) => panic!("Expected NotSupported error, got: {:?}", e),
        Ok(_) => panic!("Expected error when Content-Length is missing"),
    }
}

#[test]
fn test_get_content_length_local() {
    // Test local file content length
    let size = oneio::get_content_length("tests/test_data.txt.gz").unwrap();
    assert!(size > 0, "Local file should have a size greater than 0");

    // Verify it matches filesystem metadata
    let metadata = std::fs::metadata("tests/test_data.txt.gz").unwrap();
    assert_eq!(
        size,
        metadata.len(),
        "Content length should match file metadata"
    );
}

// ================================
// ASYNC TESTS (Phase 3)
// ================================

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_reader_local() {
    use tokio::io::AsyncReadExt;

    // Test async reading with different compression formats
    let mut reader = oneio::get_reader_async("tests/test_data.txt")
        .await
        .unwrap();
    let mut content = String::new();
    reader.read_to_string(&mut content).await.unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());

    // Test with gzip
    let mut reader = oneio::get_reader_async("tests/test_data.txt.gz")
        .await
        .unwrap();
    let mut content = String::new();
    reader.read_to_string(&mut content).await.unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());

    // Test with bzip2
    let mut reader = oneio::get_reader_async("tests/test_data.txt.bz2")
        .await
        .unwrap();
    let mut content = String::new();
    reader.read_to_string(&mut content).await.unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());

    // Test with zstd
    let mut reader = oneio::get_reader_async("tests/test_data.txt.zst")
        .await
        .unwrap();
    let mut content = String::new();
    reader.read_to_string(&mut content).await.unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_read_to_string() {
    // Test async read_to_string with compression
    let content = oneio::read_to_string_async("tests/test_data.txt.gz")
        .await
        .unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());

    // Test with different formats
    let content = oneio::read_to_string_async("tests/test_data.txt.bz2")
        .await
        .unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());

    let content = oneio::read_to_string_async("tests/test_data.txt.zst")
        .await
        .unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_download() {
    use std::path::Path;

    // Test async download
    let download_path = "/tmp/test_async_download.txt";

    // Clean up any existing file
    let _ = std::fs::remove_file(download_path);

    // Download with async
    oneio::download_async(
        "https://spaces.bgpkit.org/oneio/test_data.txt",
        download_path,
    )
    .await
    .unwrap();

    // Verify the file was downloaded
    assert!(Path::new(download_path).exists());

    // Verify content
    let content = std::fs::read_to_string(download_path).unwrap();
    assert_eq!(content.trim(), TEST_TEXT.trim());

    // Clean up
    let _ = std::fs::remove_file(download_path);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_unsupported_compression() {
    // Test that unsupported async compression formats return appropriate errors

    // LZ4 should return NotSupported for async when lz feature is enabled
    #[cfg(feature = "lz")]
    {
        match oneio::get_reader_async("tests/test_data.txt.lz4").await {
            Err(oneio::OneIoError::NotSupported(msg)) => {
                assert!(msg.contains("LZ4") || msg.contains("not yet supported"));
            }
            Ok(_) => panic!("Expected LZ4 async to be unsupported"),
            Err(e) => panic!("Expected NotSupported error, got: {:?}", e),
        }
    }

    // XZ should return NotSupported for async when xz feature is enabled
    #[cfg(feature = "xz")]
    {
        match oneio::get_reader_async("tests/test_data.txt.xz").await {
            Err(oneio::OneIoError::NotSupported(msg)) => {
                assert!(msg.contains("XZ") || msg.contains("not yet supported"));
            }
            Ok(_) => panic!("Expected XZ async to be unsupported"),
            Err(e) => panic!("Expected NotSupported error, got: {:?}", e),
        }
    }

    // When features are not enabled, compression should be treated as no compression
    #[cfg(not(feature = "lz"))]
    {
        // Without lz feature, .lz4 files are treated as uncompressed
        match oneio::get_reader_async("tests/test_data.txt.lz4").await {
            Ok(_) => {} // This is expected - treated as uncompressed file
            Err(e) => println!("Note: LZ4 test without feature enabled: {:?}", e),
        }
    }

    #[cfg(not(feature = "xz"))]
    {
        // Without xz feature, .xz files are treated as uncompressed
        match oneio::get_reader_async("tests/test_data.txt.xz").await {
            Ok(_) => {} // This is expected - treated as uncompressed file
            Err(e) => println!("Note: XZ test without feature enabled: {:?}", e),
        }
    }
}

#[cfg(feature = "async")]
#[tokio::test]
async fn test_async_remote_http() {
    use tokio::io::AsyncReadExt;

    // Test async HTTP reading
    match oneio::get_reader_async("https://spaces.bgpkit.org/oneio/test_data.txt").await {
        Ok(mut reader) => {
            let mut content = String::new();
            reader.read_to_string(&mut content).await.unwrap();
            assert_eq!(content.trim(), TEST_TEXT.trim());
        }
        Err(e) => {
            // Network issues are acceptable in tests
            println!("Async HTTP test skipped due to network error: {:?}", e);
        }
    }
}
