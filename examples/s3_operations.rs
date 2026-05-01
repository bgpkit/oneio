use oneio::{s3_copy, s3_delete, s3_download, s3_exists, s3_list, s3_reader, s3_stats, s3_upload};
use std::io::Read;

/// This example shows how to use all S3 operations and outputs detailed error info.
///
/// You need to set the following environment variables (e.g., in .env):
/// - AWS_ACCESS_KEY_ID
/// - AWS_SECRET_ACCESS_KEY
/// - AWS_REGION (e.g. "us-east-1") (use "auto" for Cloudflare R2)
/// - AWS_ENDPOINT
/// - ONEIO_TEST_BUCKET (optional, defaults to "oneio-test")
fn main() {
    tracing_subscriber::fmt::init();

    let bucket = std::env::var("ONEIO_TEST_BUCKET").unwrap_or_else(|_| "oneio-test".to_string());
    let test_prefix = format!(
        "test-{}/",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );

    println!("Bucket: {}", bucket);
    println!("Test prefix: {}", test_prefix);

    // 1. Upload
    println!("\n=== 1. Upload ===");
    match s3_upload(&bucket, &format!("{}README.md", test_prefix), "README.md") {
        Ok(()) => println!("Upload OK"),
        Err(e) => println!("Upload FAILED: {:?}", e),
    }

    // 2. Read
    println!("\n=== 2. Read ===");
    match s3_reader(&bucket, &format!("{}README.md", test_prefix)) {
        Ok(mut reader) => {
            let mut content = String::new();
            match reader.read_to_string(&mut content) {
                Ok(n) => println!("Read OK: {} bytes", n),
                Err(e) => println!("Read FAILED: {:?}", e),
            }
        }
        Err(e) => println!("Reader FAILED: {:?}", e),
    }

    // 3. Download
    println!("\n=== 3. Download ===");
    match s3_download(
        &bucket,
        &format!("{}README.md", test_prefix),
        "/tmp/oneio-test-download.md",
    ) {
        Ok(()) => println!("Download OK"),
        Err(e) => println!("Download FAILED: {:?}", e),
    }

    // 4. Stats
    println!("\n=== 4. Stats ===");
    match s3_stats(&bucket, &format!("{}README.md", test_prefix)) {
        Ok(stats) => println!("Stats OK: {:?}", stats),
        Err(e) => println!("Stats FAILED: {:?}", e),
    }

    // 5. Exists
    println!("\n=== 5. Exists ===");
    match s3_exists(&bucket, &format!("{}README.md", test_prefix)) {
        Ok(true) => println!("Exists OK: true"),
        Ok(false) => println!("Exists OK: false"),
        Err(e) => println!("Exists FAILED: {:?}", e),
    }

    // 6. Copy (the problematic one)
    println!("\n=== 6. Copy ===");
    match s3_copy(
        &bucket,
        &format!("{}README.md", test_prefix),
        &format!("{}README-temporary.md", test_prefix),
    ) {
        Ok(()) => println!("Copy OK"),
        Err(e) => {
            println!("Copy FAILED: {:?}", e);
            println!("\nNOTE: If you see 'Access denied', verify your R2 token has:");
            println!("  - Object Read & Write permissions (not just Admin)");
            println!("  - Or try 'Admin Read & Write' to rule out permission issues");
        }
    }

    // 7. Verify copy
    println!("\n=== 7. Verify copy ===");
    match s3_exists(&bucket, &format!("{}README-temporary.md", test_prefix)) {
        Ok(true) => println!("Copy exists: true"),
        Ok(false) => println!("Copy exists: false"),
        Err(e) => println!("Exists check FAILED: {:?}", e),
    }

    // 8. List
    println!("\n=== 8. List ===");
    match s3_list(&bucket, &test_prefix, None, false) {
        Ok(keys) => println!("List OK: {} keys found\n  {:?}", keys.len(), keys),
        Err(e) => println!("List FAILED: {:?}", e),
    }

    // 9. Delete temp
    println!("\n=== 9. Delete ===");
    match s3_delete(&bucket, &format!("{}README-temporary.md", test_prefix)) {
        Ok(()) => println!("Delete OK"),
        Err(e) => println!("Delete FAILED: {:?}", e),
    }

    // 10. List after delete
    println!("\n=== 10. List after delete ===");
    match s3_list(&bucket, &test_prefix, None, false) {
        Ok(keys) => println!("List OK: {} keys remain", keys.len()),
        Err(e) => println!("List FAILED: {:?}", e),
    }

    // Cleanup uploaded files
    println!("\n=== Cleanup ===");
    let _ = s3_delete(&bucket, &format!("{}README.md", test_prefix));
    let _ = s3_delete(&bucket, &format!("{}README.md.gz", test_prefix));

    println!("\nDone!");
}
