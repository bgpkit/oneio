# Spec: Migrate S3 Operations to rusty-s3 (Sans-IO)

**Status**: Complete  
**Author**: Mingwei Zhang  
**Created**: 2025-04-30  
**Completed**: 2025-05-01  
**Target Branch**: `dev/migrate-to-rusty-s3`  
**Related Issues**: Small file multipart upload issues with current rust-s3 library

---

## 1. Overview

> **Implementation Note (2025-04-30):** After a simplify review, the spec was streamlined during implementation. Key simplifications:
> - Removed `S3Provider` enum; endpoint and region are plain strings in `S3Config`
> - Consolidated from 5 files to 2 (`s3/mod.rs`, `s3/config.rs`)
> - Removed multipart retry logic from initial implementation
> - Kept `s3_list()` returning `Vec<String>` for API compatibility
> - Use rusty-s3's built-in XML parsing instead of custom quick-xml
> - No `bytes` dependency for chunk cloning

### 1.1 Goal
Migrate oneio's S3 functionality from `rust-s3` to `rusty-s3` to achieve:
- Unified HTTP stack (single reqwest client for HTTP + S3 operations)
- Better control over multipart upload behavior
- Support for Cloudflare R2 with full SigV4 compatibility
- Reduced binary bloat (no duplicate HTTP client)
- Improved testability through HTTP layer abstraction

### 1.2 Non-Goals
- ~~Change public oneio API (maintain backward compatibility)~~ **ACCEPTED: This will be a breaking API change**
- Add async S3 support (keep sync only for now)
- Support new S3 operations beyond current set
- Change compression or other non-S3 functionality

### 1.2.1 API Breaking Changes
This migration introduces breaking changes to the public API:
- `s3_bucket()` currently returns `rust-s3::Bucket` → will return oneio-owned `S3Bucket` type
- `s3_stats()` currently returns `rust-s3` types → will return `S3ObjectMetadata`
- All S3 return types will be oneio-owned wrappers, not third-party crate types

**Migration path:** Users upgrading will need to update type references. This will be announced in CHANGELOG with semver bump (likely 0.x → 0.y or 1.x → 2.0 depending on current version).

### 1.3 Success Criteria
- [ ] All 11 existing S3 functions work with equivalent functionality
- [ ] Small file uploads (<5MB) use single PUT (no multipart)
- [ ] Large file uploads use proper multipart orchestration
- [ ] R2 compatibility verified
- [ ] No regression in existing tests
- [ ] Binary size reduced or maintained
- [ ] Compile times not significantly increased

---

## 2. Current State Analysis

### 2.1 Existing S3 Functions

| Function | rust-s3 API | Effort to Migrate | Returns |
|----------|-------------|-------------------|---------|
| `s3_env_check()` | `Credentials::from_env()` | Easy | `bool` |
| `s3_url_parse()` | Custom parsing (keep) | Keep | `S3Url` |
| `s3_bucket()` | `Bucket::new()` | Medium | `S3Bucket` (new oneio type) |
| `s3_reader()` | `get_object_to_writer()` + channels | Hard | `Box<dyn Read + Send>` |
| `s3_upload()` | `put_object_stream()` | Hard | `Result<(), OneIoError>` |
| `s3_copy()` | `copy_object_internal()` | Medium | `Result<(), OneIoError>` |
| `s3_delete()` | `delete_object()` | Medium | `Result<(), OneIoError>` |
| `s3_download()` | `get_object_to_writer()` | Medium | `Result<(), OneIoError>` |
| `s3_stats()` | `head_object()` | Medium | `S3ObjectMetadata` (new oneio type) |
| `s3_exists()` | Wrapper over stats | Easy | `bool` |
| `s3_list()` | `list()` | Medium | `Vec<S3Object>` (new oneio type) |

### 2.2 Current Dependency Stack
```
rust-s3 (0.37.0)
├── attohttpc (HTTP client for sync)
├── aws-creds
├── aws-region
└── ... (additional S3-specific deps)

oneio HTTP stack:
└── reqwest (blocking client)

Result: Two HTTP stacks in one binary
```

### 2.3 Problems with Current Approach
1. **Duplicate HTTP clients**: attohttpc + reqwest
2. **Small file issue**: rust-s3 initiates-then-aborts multipart for files <8MB
3. **Limited control**: Cannot customize multipart behavior
4. **R2 quirks**: rust-s3 had specific issues with R2 multipart (GitHub issues #302, #355)

---

## 3. Proposed Architecture

### 3.1 Design Philosophy
**Sans-IO**: rusty-s3 handles request signing and URL generation. Oneio handles HTTP transport using its existing reqwest client.

### 3.2 Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        oneio library                         │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   HTTP I/O   │  │   FTP I/O    │  │   S3 I/O     │      │
│  │   (reqwest)  │  │  (suppaftp)  │  │  (rusty-s3)  │      │
│  └──────┬───────┘  └──────────────┘  └──────┬───────┘      │
│         │                                      │             │
│         └──────────────┬──────────────────────┘             │
│                          │                                  │
│                    ┌─────┴─────┐                           │
│                    │   OneIo   │  (shared config/client)   │
│                    │  Client   │                           │
│                    └───────────┘                           │
└─────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │                   │
            ┌───────▼──────┐   ┌───────▼──────┐
            │    reqwest   │   │   rusty-s3   │
            │   (HTTP)     │   │  (signing)   │
            └──────────────┘   └──────────────┘
```

### 3.3 New S3 Module Structure

```rust
// src/s3/mod.rs
pub mod config;    // S3Config, S3Provider (AWS/R2/MinIO/Custom), S3Bucket
pub mod actions;   // High-level action wrappers
pub mod stream;    // Streaming upload/download helpers
pub mod types;     // S3ObjectMetadata, S3Object, etc.

// Public API (NOTE: breaking changes from v0.x)
pub use config::{S3Config, S3Provider, S3Bucket};
pub use types::{S3ObjectMetadata, S3Object};
pub use actions::{
    s3_env_check, s3_url_parse, s3_bucket, s3_reader,
    s3_upload, s3_copy, s3_delete, s3_download,
    s3_stats, s3_exists, s3_list
};

// Shared HTTP client for S3 operations
// No global timeout — per-request timeouts applied where needed.
// s3_reader() returns a response that the caller may consume slowly.
use std::sync::OnceLock;

static S3_HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn get_s3_client() -> &'static reqwest::blocking::Client {
    S3_HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create S3 HTTP client")
    })
}
```

### 3.4 S3 Provider Support

```rust
pub enum S3Provider {
    Aws { region: String },
    R2 { account_id: String },
    MinIO { endpoint: String },
    Custom { endpoint: String, region: String },
}

impl S3Provider {
    fn endpoint(&self) -> String;
    fn region(&self) -> &str;  // "auto" for R2
    fn url_style(&self) -> UrlStyle;  // Path-style for MinIO, virtual-host for AWS
}

pub struct S3Credentials {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: Option<String>,  // AWS_SESSION_TOKEN support
}

impl S3Config {
    /// Read from environment variables with precedence:
    /// 1. AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY / AWS_SESSION_TOKEN (standard AWS)
    /// 2. AWS_REGION or S3_REGION
    /// 3. AWS_ENDPOINT or S3_ENDPOINT (for custom/MinIO)
    pub fn from_env() -> Result<Self, OneIoError>;
    
    /// Normalize endpoint (add https:// if missing, strip trailing /)
    fn normalize_endpoint(url: &str) -> String;
}
```

### 3.5 Public API Types

Exact fields and traits for all new public types:

```rust
// src/s3/types.rs

/// Metadata returned by s3_stats() / s3_head()
#[derive(Debug, Clone)]
pub struct S3ObjectMetadata {
    pub content_length: u64,
    pub content_type: Option<String>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
}

/// Object entry returned by s3_list()
#[derive(Debug, Clone)]
pub struct S3Object {
    pub key: String,
    pub size: u64,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
}

/// Bucket handle returned by s3_bucket()
#[derive(Debug, Clone)]
pub struct S3Bucket {
    pub name: String,
    pub provider: S3Provider,
    pub endpoint: String,
}
```

### 3.6 Public Function Signatures

All 11 public S3 functions and their exact signatures:

```rust
// Environment
pub fn s3_env_check() -> Result<(), OneIoError>;
pub fn s3_url_parse(path: &str) -> Result<(String, String), OneIoError>;

// Bucket
pub fn s3_bucket(
    name: &str,
    provider: S3Provider,
    credentials: S3Credentials,
) -> Result<S3Bucket, OneIoError>;

// Object operations
pub fn s3_reader(bucket: &str, key: &str) -> Result<Box<dyn Read + Send>, OneIoError>;
pub fn s3_download(bucket: &str, key: &str, file_path: &str) -> Result<(), OneIoError>;
pub fn s3_upload(bucket: &str, key: &str, file_path: &str) -> Result<(), OneIoError>;
pub fn s3_copy(bucket: &str, src_key: &str, dst_key: &str) -> Result<(), OneIoError>;
pub fn s3_delete(bucket: &str, key: &str) -> Result<(), OneIoError>;

// Metadata
pub fn s3_stats(bucket: &str, key: &str) -> Result<S3ObjectMetadata, OneIoError>;
pub fn s3_exists(bucket: &str, key: &str) -> Result<bool, OneIoError>;

// Listing
pub fn s3_list(bucket: &str, prefix: &str) -> Result<Vec<S3Object>, OneIoError>;
```

### 3.7 Config Resolution Model

All S3 action functions (`s3_upload`, `s3_download`, `s3_list`, etc.) resolve configuration via `get_config()`:

```rust
/// Resolve S3 configuration from environment variables.
/// Called internally by all S3 action functions.
///
/// Reads:
/// - AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY / AWS_SESSION_TOKEN
/// - AWS_REGION (or S3_REGION)
/// - AWS_ENDPOINT (or S3_ENDPOINT)
/// - ONEIO_S3_CHUNK_SIZE (optional, defaults to 8MB)
///
/// Returns OneIoError::Configuration if required vars are missing.
fn get_config() -> Result<S3Config, OneIoError>;

/// Internal config struct used by action functions.
struct S3Config {
    pub bucket: String,           // Parsed from the bucket parameter or env
    pub credentials: S3Credentials,
    pub provider: S3Provider,
    pub ttl: u64,                 // Signed URL TTL in seconds (default: 3600)
    pub multipart_chunk_size: u64,
    pub multipart_threshold: u64, // 5MB
}
```

**Config flow:**
1. User calls `s3_upload("my-bucket", "key", "file.txt")`
2. `s3_upload()` internally calls `get_config()` which reads env vars
3. `get_config()` constructs `S3Config` with bucket name, credentials, provider
4. The action uses `config.bucket`, `config.credentials`, `config.provider` to create `rusty_s3::Bucket` and sign requests

**Note:** `s3_bucket()` is a convenience function that validates credentials and returns an `S3Bucket` handle. The actual action functions do not require callers to construct `S3Bucket` first — they resolve config from env internally.

---

## 4. Implementation Details

### 4.1 Phase 1: Foundation (Files: Cargo.toml, s3/mod.rs, s3/config.rs, s3/types.rs)

**Dependencies:**
- Replace `rust-s3` with `rusty-s3` (signing only)
- Add `quick-xml` for XML parsing (S3 responses)

**Tasks:**
1. Replace `rust-s3` with `rusty-s3` in Cargo.toml
2. Add `quick-xml` dependency for XML parsing
3. Create `S3Config` struct with provider support
4. Implement environment variable reading (including `AWS_SESSION_TOKEN`)
5. Add R2 endpoint format support
6. Create oneio-owned return types (`S3Bucket`, `S3ObjectMetadata`, `S3Object`)

**Acceptance Criteria:**
- `cargo build` succeeds with new dependencies
- `S3Config::from_env()` works for AWS and R2
- Unit test: config parsing for all providers
- Unit test: XML parsing for ListObjectsV2 response
- Unit test: XML generation for CompleteMultipartUpload

### 4.2 Phase 2: Basic Actions (Files: s3/actions.rs)

**Tasks:**
1. Implement `s3_head()` (HEAD request)
2. Implement `s3_delete()` (DELETE request)
3. Implement `s3_list()` (ListObjectsV2 with pagination)
4. Implement `s3_copy()` (PUT with x-amz-copy-source)
5. Implement `s3_download()` (GET to file)

### 4.2 Request Construction (Normative)

The core integration between `rusty-s3` (signing) and `reqwest` (HTTP):

```rust
/// Assemble and send a signed S3 request.
///
/// `rusty-s3` produces a `SignedUrl` containing:
/// - `url`: the fully signed URL (query params include SigV4 signature)
/// - `headers`: signed headers that must be forwarded to reqwest
///
/// For most operations, the signed URL contains all auth in query params,
/// so no extra headers are needed. For CopyObject, the `x-amz-copy-source`
/// header is part of the signed payload and must be included.
fn send_s3_request(
    method: reqwest::Method,
    signed_url: &SignedUrl,
    body: Option<reqwest::blocking::Body>,
) -> Result<reqwest::blocking::Response, OneIoError> {
    let mut request = get_s3_client().request(method, signed_url.url.clone());

    // Forward signed headers from rusty-s3 onto reqwest request
    for (name, value) in &signed_url.headers {
        request = request.header(name.as_str(), value);
    }

    if let Some(b) = body {
        request = request.body(b);
    }

    let response = request.send()?;

    // On non-2xx, attempt to parse S3 XML error body
    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().unwrap_or_default();
        return Err(parse_s3_error(status, &body_text));
    }

    Ok(response)
}
```

**Request types:**

| Operation | Method | Body | Special Headers |
|-----------|--------|------|-----------------|
| `s3_stats` / `s3_exists` | HEAD | None | None |
| `s3_delete` | DELETE | None | None |
| `s3_download` / `s3_reader` | GET | None | None |
| `s3_upload` (single) | PUT | File stream | `content-type` (optional) |
| `s3_upload` (multipart init) | POST | None | None |
| `s3_upload` (part) | PUT | `&[u8]` chunk | None |
| `s3_upload` (complete) | POST | XML body | `content-type: application/xml` |
| `s3_upload` (abort) | DELETE | None | None |
| `s3_copy` | PUT | None | `x-amz-copy-source` (URL-encoded) |
| `s3_list` | GET | None | None |

**Note on payload hashing:** `rusty-s3` handles SigV4 payload hashing internally when signing. For streaming uploads, use `unsigned_payload` mode (if supported by rusty-s3) or pre-compute hash for small files. Verify rusty-s3's API for `UNSIGNED-PAYLOAD` option.

**Pattern for each action:**
```rust
pub fn s3_action(...) -> Result<..., OneIoError> {
    let config = get_config()?;
    let action = CreateAction::new(&config.bucket, Some(&config.credentials), ...);
    let signed_url = action.sign(config.ttl);
    let response = send_s3_request(method, &signed_url, body)?;
    // Parse response body on success
}
```

**ListObjectsV2 Pagination:**
```rust
pub fn s3_list(bucket: &str, prefix: &str) -> Result<Vec<S3Object>, OneIoError> {
    let mut objects = Vec::new();
    let mut continuation_token = None;
    
    loop {
        let action = ListObjectsV2::new(&bucket, Some(&config.credentials), ...)
            .with_prefix(prefix)
            .with_continuation_token(continuation_token);
        
        let response = send_action(&action)?;
        let result = parse_list_objects_v2_response(&response.text()?)?;
        
        objects.extend(result.contents);
        
        match result.next_continuation_token {
            Some(token) => continuation_token = Some(token),
            None => break,
        }
    }
    
    Ok(objects)
}
```

**Acceptance Criteria:**
- All 5 actions pass automated integration tests
- `s3_list()` correctly handles >1,000 keys via pagination
- Error handling matches current behavior
- Response parsing works for AWS and R2
- XML error responses are parsed into descriptive `OneIoError` variants

### 4.3 Phase 3: Streaming Operations (Files: s3/stream.rs, updates to actions.rs)

#### 4.3.1 Download Streaming (`s3_reader`)

**Simplified approach with reqwest:**
```rust
pub fn s3_reader(bucket: &str, key: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let config = get_config()?;
    let action = GetObject::new(&config.bucket, Some(&config.credentials), key);
    let signed_url = action.sign(config.ttl);
    let response = send_s3_request(Method::GET, &signed_url, None)?;
    Ok(Box::new(response))
}
```

**Note:** No threads or channels needed - `reqwest::blocking::Response` already implements `std::io::Read` and `Send`.

#### 4.3.2 Upload with Smart Multipart (`s3_upload`)

**Chunk Size Decision:**
- **Default:** 8MB (better throughput than 5MB)
- **Configurable:** via `S3Config::multipart_chunk_size`
- **Part limit:** S3 allows maximum 10,000 parts. For files that would exceed this,
  dynamically increase chunk size so the upload stays within the limit.

**Chunk size calculation:**
```rust
fn calculate_chunk_size(file_size: u64, requested_chunk_size: u64) -> Result<u64, OneIoError> {
    const MAX_PARTS: u64 = 10_000;
    const MIN_PART_SIZE: u64 = 5 * 1024 * 1024; // 5MB minimum per part (except last)

    let required_chunk_size = (file_size + MAX_PARTS - 1) / MAX_PARTS;
    let chunk_size = requested_chunk_size.max(required_chunk_size).max(MIN_PART_SIZE);

    let total_parts = (file_size + chunk_size - 1) / chunk_size;
    if total_parts > MAX_PARTS {
        return Err(OneIoError::NotSupported(
            format!("file size {file_size} exceeds maximum multipart upload capacity")
        ));
    }

    Ok(chunk_size)
}
```

**Logic:**
```rust
pub fn s3_upload(bucket: &str, key: &str, file_path: &str) -> Result<(), OneIoError> {
    let metadata = std::fs::metadata(file_path)?;
    let size = metadata.len();
    
    if size < config.multipart_threshold {  // 5MB threshold for multipart decision
        upload_single(bucket, key, file_path)
    } else {
        upload_multipart(bucket, key, file_path, size)
    }
}
```

**Single Upload:**
- Stream file directly from disk to HTTP socket (no full memory buffering)
- Create `PutObject` action
- Sign and PUT with body streamed from `File`

**Multipart Upload with Abort Handling:**
```rust
fn upload_multipart(bucket: &str, key: &str, file_path: &str, size: u64) -> Result<(), OneIoError> {
    let chunk_size = calculate_chunk_size(size, config.multipart_chunk_size)?;
    let total_parts = ((size + chunk_size - 1) / chunk_size) as usize;
    
    // 1. Initiate multipart upload
    let action = CreateMultipartUpload::new(&bucket, Some(&config.credentials), key);
    let response = send_s3_request(Method::POST, &action.sign(config.ttl), None)?;
    let upload_id = parse_create_multipart_upload_response(&response.text()?)?;
    
    let mut parts: Vec<(usize, String)> = Vec::with_capacity(total_parts);
    let mut file = File::open(file_path)?;
    
    for part_number in 1..=total_parts {
        let chunk = read_chunk(&mut file, chunk_size)?;
        let etag = match upload_part_with_retry(bucket, key, &upload_id, part_number, &chunk) {
            Ok(etag) => etag,
            Err(e) => {
                // Abort multipart upload on any part failure
                let _ = abort_multipart_upload(bucket, key, &upload_id);
                return Err(e);
            }
        };
        parts.push((part_number, etag));
    }
    
    // 3. Complete multipart upload
    let complete_action = CompleteMultipartUpload::new(&bucket, Some(&config.credentials), key, &upload_id);
    let xml_body = generate_complete_multipart_xml(&parts);
    let body = reqwest::blocking::Body::from(xml_body);
    
    let complete_result = (|| {
        let response = send_s3_request(Method::POST, &complete_action.sign(config.ttl), Some(body))?;
        // CompleteMultipartUpload can return 200 OK with an error in the body
        verify_complete_multipart_response(&response.text()?)?;
        Ok(())
    })();
    
    if complete_result.is_err() {
        // Best-effort abort on completion failure
        let _ = abort_multipart_upload(bucket, key, &upload_id);
    }
    
    complete_result
}
        };
        
        parts.push((part_number, etag));
    }
    
    // 3. Complete multipart upload
    let complete_action = CompleteMultipartUpload::new(&bucket, Some(&config.credentials), key, &upload_id);
    let xml_body = generate_complete_multipart_xml(&parts);
    let body = reqwest::blocking::Body::from(xml_body);
    let response = send_s3_request(
        Method::POST,
        &complete_action.sign(config.ttl),
        Some(body),
    )?;
    
    // CompleteMultipartUpload can return 200 OK with an error in the body
    verify_complete_multipart_response(&response.text()?)?;
    
    Ok(())
}

/// Upload a single part with retry. Uses bytes::Bytes for cheap clone on retry.
fn upload_part_with_retry(
    bucket: &str,
    key: &str,
    upload_id: &str,
    part_number: usize,
    chunk: bytes::Bytes,
) -> Result<String, OneIoError> {
    let max_retries = 3;
    
    for attempt in 0..max_retries {
        let action = UploadPart::new(&bucket, Some(&config.credentials), key, upload_id, part_number);
        // Bytes::clone() is O(1) reference count increment, not a data copy
        let body = reqwest::blocking::Body::from(chunk.clone());
        let response = send_s3_request(Method::PUT, &action.sign(config.ttl), Some(body))?;
        
        if response.status().is_success() {
            return Ok(extract_etag(response.headers()));
        }
        
        if attempt < max_retries - 1 {
            std::thread::sleep(Duration::from_millis(100 * (attempt + 1) as u64));
        }
    }
    
    Err(OneIoError::S3UploadPartFailed { part_number })
}

fn abort_multipart_upload(bucket: &str, key: &str, upload_id: &str) {
    let action = AbortMultipartUpload::new(&bucket, Some(&config.credentials), key, upload_id);
    let _ = send_s3_request(Method::DELETE, &action.sign(config.ttl), None);
    // Ignore abort errors - we're already in failure path
}

/// Generate CompleteMultipartUpload XML using format! (simple, no serialization crate needed)
fn generate_complete_multipart_xml(parts: &[(usize, String)]) -> String {
    let mut xml = String::from("<CompleteMultipartUpload>");
    for (part_number, etag) in parts {
        xml.push_str(&format!(
            "<Part><PartNumber>{part_number}</PartNumber><ETag>{etag}</ETag></Part>"
        ));
    }
    xml.push_str("</CompleteMultipartUpload>");
    xml
}
```

**Multipart Rules:**
- All parts except last must be ≥ 5MB (enforced)
- Maximum 10,000 parts (enforced); chunk size auto-increases for very large files
- Parts must be ordered by part number in completion XML
- Each part gets fresh signed URL (avoids TTL expiry mid-upload)
- Failed parts are retried 3x with exponential backoff
- Any failure triggers `AbortMultipartUpload` to clean up S3 state

**Acceptance Criteria:**
- Files <5MB use single PUT
- Files >5MB use multipart with auto-calculated chunk size
- Files stream from disk (not fully buffered in memory)
- Failed parts retry 3x before failing entire upload
- Failed uploads abort multipart state in S3
- Upload works with files up to S3's ~5TB limit

### 4.4 Phase 4: Integration (Files: lib.rs, bin/oneio.rs, error.rs)

**Tasks:**
1. Update public exports in lib.rs
2. Update CLI to use new S3 module
3. Update error handling for new error types
4. Update examples
5. Update CHANGELOG.md with breaking changes notice
6. Update README.md with migration guide

**Acceptance Criteria:**
- `oneio s3 upload` CLI command works
- All examples compile and run
- CHANGELOG documents breaking API changes
- README includes migration guide for users upgrading from v0.x

### 4.5 Error Mapping

Exact mapping from HTTP status codes to `OneIoError` variants for each operation:

| Operation | 200 | 204 | 206 | 301/307 | 403 | 404 | 400 | 500 | Timeout |
|-----------|-----|-----|-----|---------|-----|-----|-----|-----|---------|
| `s3_stats` | ✓ metadata | - | - | redirect | `PermissionDenied` | `NotFound` | `InvalidInput` | `S3Error` | `Io(timeout)` |
| `s3_exists` | `true` | - | - | redirect | `Err(PermissionDenied)` | `Ok(false)` | `Err(InvalidInput)` | `Err(S3Error)` | `Err(Io(timeout))` |
| `s3_download` | ✓ stream | - | - | redirect | `PermissionDenied` | `NotFound` | `InvalidInput` | `S3Error` | `Io(timeout)` |
| `s3_reader` | ✓ `Box<dyn Read>` | - | - | redirect | `PermissionDenied` | `NotFound` | `InvalidInput` | `S3Error` | `Io(timeout)` |
| `s3_upload` (single) | ✓ | - | - | redirect | `PermissionDenied` | - | `InvalidInput` | `S3Error` | `Io(timeout)` |
| `s3_upload` (multipart) | ✓ complete | - | - | redirect | `PermissionDenied` | - | `InvalidInput` | `S3Error` | `Io(timeout)` |
| `s3_list` | ✓ objects | - | - | redirect | `PermissionDenied` | `NotFound` (bucket) | `InvalidInput` | `S3Error` | `Io(timeout)` |
| `s3_delete` | - | ✓ | - | redirect | `PermissionDenied` | `NotFound` | `InvalidInput` | `S3Error` | `Io(timeout)` |
| `s3_copy` | ✓ | - | - | redirect | `PermissionDenied` | `NotFound` (src) | `InvalidInput` | `S3Error` | `Io(timeout)` |

**S3 XML Error Parsing:**
When S3 returns a non-2xx status, the body contains XML like:
```xml
<Error>
  <Code>NoSuchKey</Code>
  <Message>The specified key does not exist.</Message>
  <Key>test-file.txt</Key>
</Error>
```

Map the `<Code>` to `OneIoError`:
- `NoSuchKey` → `OneIoError::NotFound(key)`
- `NoSuchBucket` → `OneIoError::NotFound(bucket)`
- `AccessDenied` → `OneIoError::PermissionDenied`
- `InvalidAccessKeyId` → `OneIoError::AuthenticationFailed`
- Any other code → `OneIoError::S3Error { code, message }`

**Note on `s3_exists`:** Returns `Result<bool, OneIoError>`.
- `200 OK` → `Ok(true)` (object exists)
- `404 Not Found` → `Ok(false)` (object does not exist)
- `403 Forbidden` → `Err(PermissionDenied)` (credentials insufficient to check)
- Any other error status → `Err(...)` with appropriate variant
- Transport errors (timeout, connection refused) → `Err(Io(...))`

### 4.6 Protocol Edge Cases

**CopyObject (`s3_copy`):**
- The `x-amz-copy-source` header value must be URL-encoded: `/{bucket}/{key}` where `{key}` is percent-encoded.
- CopyObject can return `200 OK` with an embedded error in the XML body (e.g., `NoSuchKey` for the source). Must parse the response body even on 200.
- Success body contains `<CopyObjectResult><ETag>...</ETag><LastModified>...</LastModified></CopyObjectResult>`.

**CompleteMultipartUpload:**
- Can return `200 OK` with an error in the body (e.g., `InvalidPart`, `EntityTooSmall`). Must parse the response body.
- Success body contains `<CompleteMultipartUploadResult><Location>...</Location><Bucket>...</Bucket><Key>...</Key><ETag>...</ETag></CompleteMultipartUploadResult>`.
- R2 may return slightly different XML — parse leniently (ignore unknown elements).

**ListObjectsV2:**
- `IsTruncated` flag indicates more pages.
- `NextContinuationToken` is absent when `IsTruncated` is false.
- Empty bucket returns `<Contents/>` or no `<Contents>` elements — handle both.

---

## 5. Testing Strategy

### 5.1 Unit Tests
- Config parsing for all providers
- URL signing (verify format)
- Multipart chunk calculation (0 bytes, 1 byte, 5MB-1, 5MB, 5MB+1, 80GB)
- XML parsing for ListObjectsV2 responses
- XML parsing for error responses
- XML generation for CompleteMultipartUpload
- ETag extraction from headers
- Endpoint normalization (http/https, trailing slashes)

### 5.2 Integration Tests (R2)

Integration tests run against a dedicated Cloudflare R2 test bucket. A token limited to this bucket is configured in GitHub CI secrets. Tests are marked with `#[ignore]` so they don't run by default (bucket may be empty, credentials may not be set locally), but CI runs them explicitly.

**Test Organization:**
All S3 integration tests use `#[ignore]` attribute.

```rust
#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_single_put_small() { ... }

#[test]
#[ignore = "requires R2 credentials"]
fn test_r2_multipart_upload() { ... }
```

**Run locally (if you have credentials):**
```bash
export AWS_ACCESS_KEY_ID="..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_ENDPOINT="https://...r2.cloudflarestorage.com"
export AWS_REGION="auto"
export ONEIO_TEST_BUCKET="bgpkit-oneio-test"

cargo test --features s3 -- --ignored
```

**CI configuration (to be added):**
```yaml
- name: Run S3 integration tests
  env:
    AWS_ACCESS_KEY_ID: ${{ secrets.R2_TEST_ACCESS_KEY_ID }}
    AWS_SECRET_ACCESS_KEY: ${{ secrets.R2_TEST_SECRET_ACCESS_KEY }}
    AWS_ENDPOINT: ${{ secrets.R2_TEST_ENDPOINT }}
    AWS_REGION: auto
    ONEIO_TEST_BUCKET: bgpkit-oneio-test
  run: cargo test --features s3 -- --ignored
```

**Test Philosophy:**
Tests must be self-contained. The test bucket starts empty, so tests upload their own fixtures, verify operations, and clean up. Each test uses a unique prefix (e.g., `test-${timestamp}-${test_name}/`) to avoid collisions.

**Test Matrix:**

| Test | Description | Setup |
|------|-------------|-------|
| `test_r2_single_put_small` | Upload 1KB file via single PUT | Creates temp file, uploads, verifies |
| `test_r2_single_put_5mb` | Upload exactly 5MB file via single PUT | Creates 5MB temp file |
| `test_r2_multipart_10mb` | Upload 10MB file via multipart | Creates 10MB temp file, verifies chunks |
| `test_r2_multipart_80mb` | Upload 80MB file (10 parts at 8MB) | Creates 80MB temp file |
| `test_r2_download` | Download uploaded file, verify content | Uploads then downloads |
| `test_r2_reader_streaming` | Use `s3_reader()`, stream content | Uploads then streams |
| `test_r2_list_objects` | List objects with prefix | Uploads 3 files, lists with prefix |
| `test_r2_list_pagination` | List >1,000 keys, verify pagination | Uploads 1,200 files, lists all |
| `test_r2_head_object` | Get object metadata | Uploads, calls `s3_stats()` |
| `test_r2_exists` | Check object exists/not exists | Uploads one file, checks two keys |
| `test_r2_copy` | Copy object to new key | Uploads, copies, verifies both |
| `test_r2_delete` | Delete object | Uploads, deletes, verifies gone |
| `test_r2_error_404` | Request non-existent object | No setup needed |
| `test_r2_multipart_abort` | Fail mid-upload, verify abort cleanup | Intentionally fails part 2 |

**Test Data Generation:**
Tests generate deterministic content (repeated byte patterns seeded by test name) so they don't need committed fixture files:

```rust
fn generate_test_file(size: usize, seed: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let seed_bytes = seed.as_bytes();
    for i in 0..size {
        data.push(seed_bytes[i % seed_bytes.len()]);
    }
    data
}
```

**Cleanup:**
Each test cleans up its own objects in `Drop` or `finally` blocks. A separate `cleanup_test_prefix(prefix)` helper removes all objects under a test prefix to handle partial failures.

### 5.3 Regression Tests
- Compare behavior with old implementation
- Verify error messages are similar
- Check performance is comparable or better
- Binary size comparison: `cargo build --release` before vs after
- Compile time comparison: `cargo build --release` clean build
- Test edge cases:
  - Empty file (0 bytes)
  - File exactly at 5MB threshold
  - File at exactly 8MB chunk boundary
  - File with size not divisible by chunk size

---

## 6. Risks and Mitigation

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| rusty-s3 missing features | High | Low | Check API coverage before migration |
| R2 compatibility issues | Medium | Low | Test early with R2 bucket |
| Streaming performance regression | Medium | Medium | Benchmark before/after |
| Breaking API changes | High | **ACCEPTED** | Document in CHANGELOG, bump semver |
| reqwest integration issues | Low | Low | reqwest is well-tested |
| XML parsing bugs | Medium | Medium | Extensive unit tests for XML handling |
| Multipart upload orphaned state | Medium | Low | Implement abort on all failure paths |
| Signed URL TTL expiry mid-upload | Low | Low | Generate fresh signed URLs per part |

---

## 7. Rollback Plan

If migration fails:
1. Keep old `s3.rs` as `s3_legacy.rs` during development
2. Feature flag to switch between implementations
3. Revert to main branch if needed

---

---

## 8. Open Questions

1. ~~What's the optimal chunk size for multipart?~~ **DECIDED:** 8MB default, configurable via `S3Config`
2. ~~Should we add retry logic for failed parts?~~ **DECIDED:** Yes, 3 retries with exponential backoff
3. Do we need to support S3-compatible services beyond AWS/R2/MinIO? **PENDING:** Current coverage sufficient for known use cases
4. **Progress callbacks for multipart uploads** - **DEFERRED** to post-v1. Not in initial implementation.

## 9. Implementation Checklist

### Phase 1: Foundation
- [x] Add `rusty-s3` and `quick-xml` to Cargo.toml
- [x] Create `s3/types.rs` with `S3Bucket`, `S3ObjectMetadata`, `S3Object` → **Simplified**: types defined in `mod.rs` instead of separate file
- [x] Create `s3/config.rs` with `S3Config`, `S3Credentials` → **Simplified**: removed `S3Provider` enum, using plain strings
- [x] Implement environment variable reading (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_SESSION_TOKEN, AWS_REGION, AWS_ENDPOINT)
- [x] Add endpoint normalization
- [x] Create shared `S3_HTTP_CLIENT` via `std::sync::OnceLock`
- [x] Unit tests: config parsing
- [x] Unit tests: XML parsing/serialization

### Phase 2: Basic Actions
- [x] Implement `s3_stats()` (HEAD request)
- [x] Implement `s3_delete()` (DELETE request)
- [x] Implement `s3_list()` with pagination
- [x] Implement `s3_copy()` with URL-encoded x-amz-copy-source
- [x] Implement `s3_download()` with temp file + atomic rename
- [x] Implement S3 XML error parsing
- [x] Write `#[ignore]` integration tests for all basic actions

### Phase 3: Streaming
- [x] Implement `s3_reader()` returning `Box<dyn Read + Send>`
- [x] Implement `s3_upload()` with single PUT for small files
- [x] Implement multipart upload with 8MB chunks → **Simplified**: immediate abort on failure instead of retry logic
- [x] Implement abort on failure
- [x] Write `#[ignore]` integration tests for upload/download streaming

### Phase 4: Integration
- [x] Update `lib.rs` exports
- [x] Update CLI (`bin/oneio.rs`)
- [x] Update error types in `error.rs`
- [x] Update examples
- [x] Update CHANGELOG.md with breaking changes
- [x] Update README.md with migration guide → CHANGELOG contains breaking changes documentation

---

## 10. References

- [rusty-s3 Documentation](https://docs.rs/rusty-s3)
- [rusty-s3 Repository](https://github.com/paolobarbolini/rusty-s3)
- [Cloudflare R2 S3 API](https://developers.cloudflare.com/r2/api/s3/api/)
- [AWS SigV4 Signing](https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-authenticating-requests.html)
- [Sans-IO Pattern](https://sans-io.readthedocs.io/)

---

## 12. Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2025-04-30 | Use rusty-s3 over rust-s3 | Better architectural fit, Sans-IO approach |
| 2025-04-30 | Keep sync-only (no async) | Matches current oneio architecture |
| 2025-04-30 | 5MB multipart threshold | S3 minimum is 5MB per part |
| 2025-04-30 | Support AWS/R2/MinIO/Custom | Covers known use cases |
| 2025-04-30 | **Accept breaking API changes** | Clean slate for oneio-owned types |
| 2025-04-30 | Add `quick-xml` dependency | Required for S3 XML parsing |
| 2025-04-30 | 8MB default chunk size | Better throughput than 5MB |
| 2025-04-30 | 3 retries with exponential backoff | Handle transient network failures |
| 2025-04-30 | Fresh signed URL per part | Avoid TTL expiry mid-upload |

---

## 11. Post-MAGI Review Summary

Both reviewers (magi-melchior, magi-balthasar) identified critical gaps in the original spec:

### Round 1 Critical Fixes Applied
1. **API breakage accepted** - Will announce breaking changes and cut new release
2. **XML parsing strategy added** - Using `quick-xml` for S3 XML parsing/generation
3. **List pagination specified** - Full continuation token loop until exhaustion
4. **Multipart state machine completed** - Abort logic, ETag ordering, 10,000 part limit, retry logic
5. **HTTP client ownership defined** - Shared `S3_HTTP_CLIENT` via `OnceLock`
6. **Credential handling expanded** - AWS_SESSION_TOKEN support, endpoint normalization
7. **Download streaming simplified** - Direct `reqwest::Response` return (no threads/channels)
8. **Timeline removed** - As requested

### Round 2 Critical Fixes Applied
9. **Multipart part-count logic fixed** - Chunk size auto-increases instead of silently capping at 10,000 parts
10. **Normative request-building section added** - Exact `rusty-s3` → `reqwest` translation for all operations
11. **Public API types defined** - Exact fields and traits for `S3Bucket`, `S3ObjectMetadata`, `S3Object`
12. **Public function signatures listed** - All 11 functions with exact signatures
13. **Error mapping table added** - Status codes to `OneIoError` for each operation
14. **Protocol edge cases documented** - CopyObject 200-with-error-body, CompleteMultipartUpload parsing
15. **Global timeout removed** - No timeout on shared client (per-request only); prevents s3_reader() timeouts
16. **`lazy_static` replaced with `OnceLock`** - No external dependency needed (MSRV 1.70+)
17. **XML generation simplified** - `format!` loop instead of serialization crate
18. **Chunk cloning eliminated** - `&[u8]` instead of `Vec<u8>` in retry path
19. **Progress callback deferred** - Removed from v1 scope

### Ready for Implementation
The spec now addresses all critical issues raised in both review rounds.

---

**Next Steps:**
1. Begin Phase 1 implementation
2. Check off items in Implementation Checklist as completed
