//! S3 integration tests against Cloudflare R2.
//!
//! These tests require R2 credentials in environment variables:
//! - AWS_ACCESS_KEY_ID
//! - AWS_SECRET_ACCESS_KEY
//! - AWS_ENDPOINT (e.g., https://xxx.r2.cloudflarestorage.com)
//! - AWS_REGION (use "auto" for R2)
//! - ONEIO_TEST_BUCKET (test bucket name, default: "oneio-test")
//!
//! Optional configuration:
//! - ONEIO_S3_CHUNK_SIZE (multipart part size in bytes, default: 8MB)
//! - ONEIO_S3_MULTIPART_THRESHOLD (file size threshold for multipart, default: 5MB)
//!
//! Run with: cargo test --features s3 -- --ignored --test-threads=1

use std::io::Read;
use std::sync::{Mutex, OnceLock};

// Serial execution lock to prevent test collisions
static S3_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn test_bucket() -> String {
    std::env::var("ONEIO_TEST_BUCKET").unwrap_or_else(|_| "oneio-test".to_string())
}

fn begin_s3_test() -> (String, std::sync::MutexGuard<'static, ()>) {
    let guard = S3_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    (test_bucket(), guard)
}

fn test_prefix(name: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("test-{ts}-{name}/")
}

fn generate_test_data(size: usize, seed: &str) -> Vec<u8> {
    let seed_bytes = seed.as_bytes();
    (0..size)
        .map(|i| seed_bytes[i % seed_bytes.len()])
        .collect()
}

fn create_temp_file(data: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "oneio_test_{}.bin",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, data).unwrap();
    path
}

fn cleanup_test_objects(bucket: &str, prefix: &str) {
    if let Ok(keys) = oneio::s3_list(bucket, prefix, None, false) {
        for key in keys {
            let _ = oneio::s3_delete(bucket, &key);
        }
    }
}

fn assert_stream_matches(bucket: &str, key: &str, expected: &[u8]) {
    let mut reader = oneio::s3_reader(bucket, key).unwrap();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut offset = 0;

    loop {
        let bytes_read = reader.read(&mut buffer).unwrap();
        if bytes_read == 0 {
            break;
        }
        assert_eq!(
            &buffer[..bytes_read],
            &expected[offset..offset + bytes_read]
        );
        offset += bytes_read;
    }

    assert_eq!(offset, expected.len());
}

// ========== Upload Tests ==========

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_single_put_small() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("single-put-small");
    let key = format!("{prefix}small-file.txt");
    let data = generate_test_data(1024, "small");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    // Verify by downloading
    let mut downloaded = Vec::new();
    oneio::s3_reader(&bucket, &key)
        .unwrap()
        .read_to_end(&mut downloaded)
        .unwrap();
    assert_eq!(downloaded, data);

    cleanup_test_objects(&bucket, &prefix);
}

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_single_put_just_under_5mb() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("single-put-just-under-5mb");
    let key = format!("{prefix}just-under-5mb.bin");
    let size = 5 * 1024 * 1024 - 1;
    let data = generate_test_data(size, "5mbfile");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    let stats = oneio::s3_stats(&bucket, &key).unwrap();
    assert_eq!(stats.content_length, size as u64);
    assert_stream_matches(&bucket, &key, &data);

    cleanup_test_objects(&bucket, &prefix);
}

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_multipart_just_over_5mb() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("multipart-just-over-5mb");
    let key = format!("{prefix}just-over-5mb.bin");
    let size = 5 * 1024 * 1024 + 1;
    let data = generate_test_data(size, "5mbplus1");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    let stats = oneio::s3_stats(&bucket, &key).unwrap();
    assert_eq!(stats.content_length, size as u64);
    assert_stream_matches(&bucket, &key, &data);

    cleanup_test_objects(&bucket, &prefix);
}

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_multipart_10mb() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("multipart-10mb");
    let key = format!("{prefix}multipart-10mb.bin");
    let size = 10 * 1024 * 1024;
    let data = generate_test_data(size, "10mbfile");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    let stats = oneio::s3_stats(&bucket, &key).unwrap();
    assert_eq!(stats.content_length, size as u64);
    assert_stream_matches(&bucket, &key, &data);

    cleanup_test_objects(&bucket, &prefix);
}

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_multipart_80mb() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("multipart-80mb");
    let key = format!("{prefix}multipart-80mb.bin");
    let size = 80 * 1024 * 1024;
    let data = generate_test_data(size, "80mbfile");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    let stats = oneio::s3_stats(&bucket, &key).unwrap();
    assert_eq!(stats.content_length, size as u64);
    assert_stream_matches(&bucket, &key, &data);

    cleanup_test_objects(&bucket, &prefix);
}

// ========== Download Tests ==========

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_download() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("download");
    let key = format!("{prefix}download-test.bin");
    let data = generate_test_data(1024 * 1024, "download");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    let download_path = create_temp_file(b"");
    oneio::s3_download(&bucket, &key, download_path.to_str().unwrap()).unwrap();

    let downloaded = std::fs::read(&download_path).unwrap();
    assert_eq!(downloaded, data);

    cleanup_test_objects(&bucket, &prefix);
}

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_reader_streaming() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("reader-streaming");
    let key = format!("{prefix}stream-test.bin");
    let data = generate_test_data(2 * 1024 * 1024, "stream");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    let mut reader = oneio::s3_reader(&bucket, &key).unwrap();
    let mut downloaded = Vec::new();
    reader.read_to_end(&mut downloaded).unwrap();
    assert_eq!(downloaded, data);

    cleanup_test_objects(&bucket, &prefix);
}

// ========== List Tests ==========

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_list_objects() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("list-objects");

    // Upload 3 files
    for i in 0..3 {
        let key = format!("{prefix}file-{i}.txt");
        let data = generate_test_data(1024, &format!("list{i}"));
        let temp_path = create_temp_file(&data);
        oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();
    }

    let keys = oneio::s3_list(&bucket, &prefix, None, false).unwrap();
    assert_eq!(keys.len(), 3);
    for key in &keys {
        assert!(key.starts_with(&prefix));
    }

    cleanup_test_objects(&bucket, &prefix);
}

// ========== Metadata Tests ==========

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_head_object() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("head-object");
    let key = format!("{prefix}head-test.bin");
    let data = generate_test_data(5 * 1024 * 1024, "headtest");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    let stats = oneio::s3_stats(&bucket, &key).unwrap();
    assert_eq!(stats.content_length, data.len() as u64);
    assert!(stats.etag.is_some());

    cleanup_test_objects(&bucket, &prefix);
}

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_exists() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("exists");
    let key = format!("{prefix}exists-test.bin");
    let data = generate_test_data(1024, "exists");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();

    assert!(oneio::s3_exists(&bucket, &key).unwrap());
    assert!(!oneio::s3_exists(&bucket, &format!("{prefix}nonexistent")).unwrap());

    cleanup_test_objects(&bucket, &prefix);
}

// ========== Copy and Delete Tests ==========

// NOTE: This test verifies s3_copy works on S3-compatible services.
// Uses rusty-s3 for proper AWS Signature V4 signing.
#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_copy() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("copy");
    let src_key = format!("{prefix}copy-src.bin");
    let dst_key = format!("{prefix}copy-dst.bin");
    let data = generate_test_data(1024 * 1024, "copytest");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &src_key, temp_path.to_str().unwrap()).unwrap();
    oneio::s3_copy(&bucket, &src_key, &dst_key).unwrap();

    assert!(oneio::s3_exists(&bucket, &src_key).unwrap());
    assert!(oneio::s3_exists(&bucket, &dst_key).unwrap());

    cleanup_test_objects(&bucket, &prefix);
}

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_delete() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("delete");
    let key = format!("{prefix}delete-test.bin");
    let data = generate_test_data(1024, "delete");
    let temp_path = create_temp_file(&data);

    oneio::s3_upload(&bucket, &key, temp_path.to_str().unwrap()).unwrap();
    assert!(oneio::s3_exists(&bucket, &key).unwrap());

    oneio::s3_delete(&bucket, &key).unwrap();
    assert!(!oneio::s3_exists(&bucket, &key).unwrap());

    cleanup_test_objects(&bucket, &prefix);
}

// ========== Error Tests ==========

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_error_404() {
    let (bucket, _guard) = begin_s3_test();
    let prefix = test_prefix("error-404");
    let key = format!("{prefix}definitely-not-here.bin");

    let result = oneio::s3_stats(&bucket, &key);
    assert!(result.is_err());
}
