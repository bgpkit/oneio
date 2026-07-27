//! Reproducer for S3 multipart upload failure on large files.
//!
//! This test uploads a ~1GB file (128 parts × 8MB) to trigger the same
//! multipart path that fails in production (Railway broker backup).
//!
//! Run with:
//!   cargo test --features s3 --test s3_large_upload -- --ignored --nocapture

#![cfg(feature = "s3")]

use std::env;
use std::fs;
use std::time::Instant;

#[test]
#[ignore]
fn test_large_multipart_upload() {
    let _ = dotenvy::dotenv();

    let bucket = env::var("ONEIO_TEST_BUCKET").expect("ONEIO_TEST_BUCKET not set");
    let test_file =
        env::var("ONEIO_S3_TEST_FILE").unwrap_or_else(|_| "/tmp/test_upload_large.bin".to_string());

    let metadata = fs::metadata(&test_file).unwrap_or_else(|e| {
        panic!("Test file {test_file} not accessible: {e}. Set ONEIO_S3_TEST_FILE or create {test_file}");
    });
    let size = metadata.len();
    println!(
        "Test file: {test_file} ({:.1} MB)",
        size as f64 / 1024.0 / 1024.0
    );
    println!(
        "Expected parts: {} (at 8MB chunks)",
        (size + 8 * 1024 * 1024 - 1) / (8 * 1024 * 1024)
    );

    let key = format!("test-large-upload/{}.bin", std::process::id());
    println!("Uploading to s3://{bucket}/{key}");

    let start = Instant::now();
    let result = oneio::s3_upload(&bucket, &key, &test_file);
    let elapsed = start.elapsed();

    match &result {
        Ok(()) => {
            println!("✅ Upload succeeded in {:.1}s", elapsed.as_secs_f64());

            // Verify: download stats
            match oneio::s3_stats(&bucket, &key) {
                Ok(stats) => {
                    println!("Remote object: {} bytes", stats.content_length);
                    assert_eq!(stats.content_length, size, "Remote size mismatch");
                    println!("✅ Size verified: {} bytes", size);
                }
                Err(e) => {
                    println!("⚠️ Upload succeeded but s3_stats failed: {e}");
                }
            }

            // Cleanup
            let _ = oneio::s3_delete(&bucket, &key);
            println!("Cleaned up s3://{bucket}/{key}");
        }
        Err(e) => {
            println!("❌ Upload FAILED after {:.1}s", elapsed.as_secs_f64());
            println!("Error: {e}");
            println!("\nError chain:");
            let mut source: Option<&dyn std::error::Error> = std::error::Error::source(e);
            while let Some(s) = source {
                println!("  └── {s}");
                source = std::error::Error::source(s);
            }
            panic!("Upload failed: {e}");
        }
    }
}

#[test]
#[ignore]
fn test_small_upload_baseline() {
    let _ = dotenvy::dotenv();

    let bucket = env::var("ONEIO_TEST_BUCKET").expect("ONEIO_TEST_BUCKET not set");

    // Create a small 1MB file (under 5MB multipart threshold)
    let test_file = "/tmp/test_upload_small.bin";
    let size = 1024 * 1024; // 1MB
    let data = vec![0x42u8; size];
    fs::write(test_file, &data).expect("failed to write test file");

    let key = format!("test-small-upload/{}.bin", std::process::id());
    println!("Uploading 1MB file to s3://{bucket}/{key}");

    let start = Instant::now();
    let result = oneio::s3_upload(&bucket, &key, test_file);
    let elapsed = start.elapsed();

    match &result {
        Ok(()) => println!("✅ Small upload succeeded in {:.1}s", elapsed.as_secs_f64()),
        Err(e) => {
            println!("❌ Small upload FAILED: {e}");
            panic!("Small upload failed: {e}");
        }
    }

    let _ = oneio::s3_delete(&bucket, &key);
    let _ = fs::remove_file(test_file);
}
