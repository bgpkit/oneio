// Async integration tests for oneio async feature
// These tests are only compiled/run when the `async` feature is enabled.

#![cfg(feature = "async")]

use tokio::io::AsyncReadExt;

const TEST_TEXT: &str = "OneIO test file.\nThis is a test.";

#[tokio::test]
async fn async_read_local_plain() {
    let mut reader = oneio::get_reader_async("tests/test_data.txt")
        .await
        .unwrap();
    let mut content = String::new();
    reader.read_to_string(&mut content).await.unwrap();
    assert_eq!(content, TEST_TEXT);
}

#[cfg(feature = "any_gz")]
#[tokio::test]
async fn async_read_local_gzip() {
    let mut reader = oneio::get_reader_async("tests/test_data.txt.gz")
        .await
        .unwrap();
    let mut content = String::new();
    reader.read_to_string(&mut content).await.unwrap();
    assert_eq!(content, TEST_TEXT);
}

#[cfg(feature = "http")]
#[tokio::test]
async fn async_read_http_plain() {
    // Use remote small test data provided by project
    match oneio::get_reader_async("https://spaces.bgpkit.org/oneio/test_data.txt").await {
        Ok(mut reader) => {
            let mut content = String::new();
            reader.read_to_string(&mut content).await.unwrap();
            assert_eq!(content.trim(), TEST_TEXT);
        }
        Err(e) => {
            // Network may be unavailable in CI; don't fail entire suite.
            eprintln!("async_read_http_plain skipped due to error: {e}");
        }
    }
}

#[cfg(all(feature = "http", feature = "any_gz"))]
#[tokio::test]
async fn async_read_http_gzip() {
    match oneio::get_reader_async("https://spaces.bgpkit.org/oneio/test_data.txt.gz").await {
        Ok(mut reader) => {
            let mut content = String::new();
            reader.read_to_string(&mut content).await.unwrap();
            assert_eq!(content.trim(), TEST_TEXT);
        }
        Err(e) => {
            eprintln!("async_read_http_gzip skipped due to error: {e}");
        }
    }
}

#[cfg(feature = "http")]
#[tokio::test]
async fn async_download_http_to_file() {
    // Verify download_async writes content to a local file
    let tmp_path = "tests/_tmp_async_download.txt";
    // Clean up any previous run
    let _ = std::fs::remove_file(tmp_path);

    match oneio::download_async("https://spaces.bgpkit.org/oneio/test_data.txt", tmp_path).await {
        Ok(()) => {
            let text = std::fs::read_to_string(tmp_path).unwrap();
            assert_eq!(text.trim(), TEST_TEXT);
        }
        Err(e) => {
            eprintln!("async_download_http_to_file skipped due to error: {e}");
        }
    }

    let _ = std::fs::remove_file(tmp_path);
}

fn tmp_path(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "oneio_async_test_{}_{}_{name}",
        std::process::id(),
        id
    ))
}

#[tokio::test]
async fn async_read_to_string_lossy_latin1() {
    let tmp_path = tmp_path("lossy");
    let _ = std::fs::remove_file(&tmp_path);
    std::fs::write(&tmp_path, b"valid\nbad: \xf3\nnext\n").unwrap();

    let content = oneio::read_to_string_lossy_async(tmp_path.to_str().unwrap())
        .await
        .unwrap();
    assert!(content.contains('\u{FFFD}'));
    assert!(content.contains("valid"));
    assert!(content.contains("next"));

    let _ = std::fs::remove_file(&tmp_path);
}

#[tokio::test]
async fn async_read_to_bytes_roundtrip() {
    let tmp_path = tmp_path("bytes");
    let _ = std::fs::remove_file(&tmp_path);
    let expected = b"valid\nbad: \xf3\nnext\n";
    std::fs::write(&tmp_path, expected).unwrap();

    let bytes = oneio::read_to_bytes_async(tmp_path.to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(bytes, expected);

    let _ = std::fs::remove_file(&tmp_path);
}

#[tokio::test]
#[allow(deprecated)]
async fn async_read_to_string_async_strict_still_fails() {
    let tmp_path = tmp_path("strict");
    let _ = std::fs::remove_file(&tmp_path);
    std::fs::write(&tmp_path, b"valid\nbad: \xf3\nnext\n").unwrap();

    let result = oneio::read_to_string_async(tmp_path.to_str().unwrap()).await;
    assert!(
        result.is_err(),
        "strict async read_to_string should fail on Latin-1 byte"
    );

    let _ = std::fs::remove_file(&tmp_path);
}

#[cfg(feature = "any_gz")]
#[tokio::test]
async fn async_download_preserves_compressed_bytes() {
    let tmp_path = "tests/_tmp_async_download.txt.gz";
    let _ = std::fs::remove_file(tmp_path);

    oneio::download_async("tests/test_data.txt.gz", tmp_path)
        .await
        .unwrap();

    let original = std::fs::read("tests/test_data.txt.gz").unwrap();
    let downloaded = std::fs::read(tmp_path).unwrap();
    assert_eq!(downloaded, original);

    let _ = std::fs::remove_file(tmp_path);
}
