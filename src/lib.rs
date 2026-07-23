/*!
Unified I/O for compressed files from any source.

OneIO provides a single interface for reading and writing files with any
compression format, from local disk or remote locations (HTTP, FTP, S3).

# Quick Start

```toml
[dependencies]
oneio = "0.23"
```

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use oneio;

// Read a remote compressed file
let content = oneio::read_to_string_lossy("https://example.com/data.txt.gz")?;
# Ok(())
# }
```

# Feature Selection

Enable only what you need:

| Feature | Description |
|---------|-------------|
| `gz` | Gzip compression |
| `bz` | Bzip2 compression |
| `lz` | LZ4 compression |
| `xz` | XZ compression |
| `zstd` | Zstandard compression |
| `http` | HTTP/HTTPS support |
| `reqwest-gzip` | Opt-in HTTP gzip content-encoding (advertises `Accept-Encoding: gzip`, transparently decodes responses) |
| `ftp` | FTP support |
| `s3` | S3-compatible storage |
| `async` | Async I/O support |
| `json` | JSON deserialization |
| `digest` | SHA256 hashing |
| `cli` | Command-line tool |

**Example: Minimal setup for local files**
```toml
[dependencies]
oneio = { version = "0.23", default-features = false, features = ["gz"] }
```

**Example: HTTPS with custom TLS for corporate proxies**
```toml
[dependencies]
oneio = { version = "0.23", default-features = false, features = ["http", "native-tls", "gz"] }
```

# Core API

## Reading

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
// Read entire file to string
let content = oneio::read_to_string_lossy("data.txt")?;

// Read lines
for line in oneio::read_lines_lossy("data.txt")? {
    println!("{}", line?);
}

// Get a reader for streaming
let mut reader = oneio::get_reader("data.txt.gz")?;
# Ok(())
# }
```

## Writing

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use std::io::Write;

let mut writer = oneio::get_writer("output.txt.gz")?;
writer.write_all(b"Hello")?;
// Compression finalized on drop
# Ok(())
# }
```

## Reusable Client

For multiple requests with shared configuration:

```rust,no_run
# #[cfg(feature = "http")]
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use oneio::OneIo;

let client = OneIo::builder()
    .header_str("Authorization", "Bearer token")
    .timeout(std::time::Duration::from_secs(30))
    .build()?;

let data1 = client.read_to_string_lossy("https://api.example.com/1.json")?;
let data2 = client.read_to_string_lossy("https://api.example.com/2.json")?;
# Ok(())
# }
# #[cfg(not(feature = "http"))]
# fn main() {}
```

# Compression

Automatic detection by file extension:

| Extension | Algorithm |
|-----------|-----------|
| `.gz` | Gzip |
| `.bz2` | Bzip2 |
| `.lz4` | LZ4 |
| `.xz` | XZ |
| `.zst` | Zstandard |

Override detection for URLs with query parameters:

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use oneio::OneIo;

let client = OneIo::new()?;
let reader = client.get_reader_with_type(
    "https://api.example.com/data?format=gz",
    "gz"
)?;
# Ok(())
# }
```

# Protocols

- **Local**: `/path/to/file.txt`
- **HTTP/HTTPS**: `https://example.com/file.txt.gz`
- **FTP**: `ftp://ftp.example.com/file.txt` (requires `ftp` feature)
- **S3**: `s3://bucket/key` (requires `s3` feature)

# Async API

Enable the `async` feature:

```rust
# #[cfg(feature = "async")]
# async fn example() -> Result<(), oneio::OneIoError> {
let content = oneio::read_to_string_lossy_async("https://example.com/data.txt").await?;
# Ok(())
# }
```

Async compression support: `gz`, `bz`, `zstd`
LZ4 and XZ return `NotSupported` error.

# Error Handling

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use oneio::OneIoError;

match oneio::get_reader("file.txt") {
    Ok(reader) => { /* ... */ }
    Err(OneIoError::Io(e)) => { /* filesystem error */ }
    Err(OneIoError::Network(e)) => { /* network error */ }
    Err(OneIoError::NotSupported(msg)) => { /* feature not enabled */ }
    _ => { /* future error variants */ }
}
# Ok(())
# }
```

# Environment Variables

## General

- `ONEIO_ACCEPT_INVALID_CERTS=true` - Accept invalid TLS certificates (development only)
- `ONEIO_CA_BUNDLE=/path/to/ca.pem` - Add custom CA certificate to trust store

## S3 (requires `s3` feature)

Required:
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `AWS_REGION` - Use `"auto"` for Cloudflare R2
- `AWS_ENDPOINT` - e.g. `https://xxx.r2.cloudflarestorage.com`

Optional:
- `AWS_SESSION_TOKEN` - Temporary session token
- `ONEIO_S3_CHUNK_SIZE` - Multipart part size in bytes (default: 8MB)
- `ONEIO_S3_MULTIPART_THRESHOLD` - File size threshold for multipart upload (default: 5MB)

R2 supports single PUT uploads up to 300 MiB. The default threshold of 5MB
(the S3 minimum part size) uses single-PUT for small files and multipart
for larger files where retry-per-part improves reliability.

# TLS and Corporate Proxies

For environments with custom TLS certificates (Cloudflare WARP, corporate proxies):

1. Use `native-tls` feature to use the OS trust store:
   ```toml
   features = ["http", "native-tls"]
   ```

2. Or add certificates programmatically:
   ```rust,no_run
   # #[cfg(feature = "http")]
   # fn main() -> Result<(), Box<dyn std::error::Error>> {
   # use oneio::OneIo;
   let client = OneIo::builder()
       .add_root_certificate_pem(&std::fs::read("ca.pem")?)?
       .build()?;
   # Ok(())
   # }
   # #[cfg(not(feature = "http"))]
   # fn main() {}
   ```

3. Or via environment variable:
   ```bash
   export ONEIO_CA_BUNDLE=/path/to/ca.pem
   ```
*/

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/bgpkit/assets/main/logos/icon-transparent.png",
    html_favicon_url = "https://raw.githubusercontent.com/bgpkit/assets/main/logos/favicon.ico"
)]

mod builder;
mod client;
mod compression;
mod error;
mod progress;

pub use builder::OneIoBuilder;
pub use client::OneIo;
pub use error::OneIoError;

/// Re-export of the exact `reqwest` crate oneio is built against.
///
/// HTTP response types (e.g. [`OneIo::get_http_reader_raw`] returning
/// `reqwest::blocking::Response`) appear in oneio's public API. This re-export
/// lets downstream crates name those types (`oneio::reqwest::StatusCode`,
/// `oneio::reqwest::header`, ...) without declaring their own reqwest
/// dependency, avoiding version skew between the reqwest oneio uses and the
/// reqwest a consumer imports.
///
/// Note: this makes reqwest part of oneio's public API contract; a reqwest
/// major-version bump is a breaking oneio change.
///
/// # Example: conditional GET with HTTP validators
///
/// ```rust,no_run
/// # #[cfg(feature = "http")]
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oneio::reqwest::StatusCode;
/// use oneio::OneIo;
///
/// let client = OneIo::builder()
///     .header_str("If-None-Match", "\"some-etag\"")
///     .build()?;
/// let response = client.get_http_reader_raw("https://example.com/data.json")?;
/// if response.status() == StatusCode::NOT_MODIFIED {
///     // cached copy is still current; skip re-processing
/// } else {
///     let etag = response
///         .headers()
///         .get("etag")
///         .and_then(|v| v.to_str().ok())
///         .map(String::from);
///     // read `response` (implements `std::io::Read`) and store `etag`
/// }
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "http"))]
/// # fn main() {}
/// ```
#[cfg(feature = "http")]
pub use reqwest;

#[cfg(feature = "async")]
pub mod async_reader;
#[cfg(feature = "rustls")]
pub mod crypto;
#[cfg(feature = "digest")]
pub mod digest;
#[cfg(any(feature = "http", feature = "ftp"))]
pub(crate) mod remote;
#[cfg(feature = "s3")]
pub mod s3;

// Re-export all s3 functions
#[cfg(feature = "s3")]
pub use s3::*;

// Re-export all digest functions
#[cfg(feature = "digest")]
pub use digest::*;

use std::fs::File;
use std::io::{BufWriter, Read, Write};

// Internal helpers

/// Extracts the protocol from a given path.
pub(crate) fn get_protocol(path: &str) -> Option<&str> {
    path.split_once("://").map(|(protocol, _)| protocol)
}

/// Extract the file extension, ignoring URL query params and fragments.
pub(crate) fn file_extension(path: &str) -> &str {
    let path = path.split('?').next().unwrap_or(path);
    let path = path.split('#').next().unwrap_or(path);
    path.rsplit('.').next().unwrap_or("")
}

/// Creates a raw writer without compression.
pub(crate) fn get_writer_raw_impl(path: &str) -> Result<BufWriter<File>, OneIoError> {
    let path = std::path::Path::new(path);
    if let Some(prefix) = path.parent() {
        std::fs::create_dir_all(prefix)?;
    }
    let output_file = BufWriter::new(File::create(path)?);
    Ok(output_file)
}

/// Creates a raw reader for local files.
#[allow(dead_code)]
pub(crate) fn get_reader_raw_impl(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    let file = File::open(path)?;
    Ok(Box::new(std::io::BufReader::new(file)))
}

/// Gets a reader for the given file path.
pub fn get_reader(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    builder::default_oneio()?.get_reader(path)
}

/// Returns a writer for the given file path with the corresponding compression.
pub fn get_writer(path: &str) -> Result<Box<dyn Write>, OneIoError> {
    builder::default_oneio()?.get_writer(path)
}

/// Checks whether a local or remote path exists.
pub fn exists(path: &str) -> Result<bool, OneIoError> {
    builder::default_oneio()?.exists(path)
}

/// Reads the full contents of a file or URL into a string.
#[deprecated(since = "0.23.0", note = "Use read_to_string_lossy or read_to_bytes")]
#[allow(deprecated)]
pub fn read_to_string(path: &str) -> Result<String, OneIoError> {
    builder::default_oneio()?.read_to_string(path)
}

/// Reads the full contents of a file or URL into a string,
/// replacing invalid UTF-8 sequences with `U+FFFD`.
pub fn read_to_string_lossy(path: &str) -> Result<String, OneIoError> {
    builder::default_oneio()?.read_to_string_lossy(path)
}

/// Reads the full contents of a file or URL into raw bytes.
pub fn read_to_bytes(path: &str) -> Result<Vec<u8>, OneIoError> {
    builder::default_oneio()?.read_to_bytes(path)
}

/// Reads and deserializes JSON into the requested type.
#[cfg(feature = "json")]
pub fn read_json_struct<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, OneIoError> {
    builder::default_oneio()?.read_json_struct(path)
}

/// Returns an iterator over lines from the provided path.
#[deprecated(
    since = "0.23.0",
    note = "Use read_lines_lossy for lossy text, read_to_bytes for byte-perfect whole-file reads, or get_reader for byte streaming"
)]
#[allow(deprecated)]
pub fn read_lines(
    path: &str,
) -> Result<std::io::Lines<std::io::BufReader<Box<dyn Read + Send>>>, OneIoError> {
    builder::default_oneio()?.read_lines(path)
}

/// Like [`read_lines`], but invalid UTF-8 sequences are replaced with
/// `U+FFFD` instead of producing `Err(io::ErrorKind::InvalidData)`.
pub fn read_lines_lossy(
    path: &str,
) -> Result<impl Iterator<Item = std::io::Result<String>> + Send, OneIoError> {
    builder::default_oneio()?.read_lines_lossy(path)
}

/// Downloads a remote resource to a local path.
pub fn download(remote: &str, local: &str) -> Result<(), OneIoError> {
    builder::default_oneio()?.download(remote, local)
}

/// Creates a reader backed by a local cache file.
pub fn get_cache_reader(
    path: &str,
    cache_dir: &str,
    cache_file_name: Option<String>,
    force_cache: bool,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    builder::default_oneio()?.get_cache_reader(path, cache_dir, cache_file_name, force_cache)
}

/// Gets an async reader for the given file path.
#[cfg(feature = "async")]
pub async fn get_reader_async(
    path: &str,
) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, OneIoError> {
    async_reader::get_reader_async(path).await
}

/// Reads the entire content of a file asynchronously into a string.
#[deprecated(
    since = "0.23.0",
    note = "Use read_to_string_lossy_async or read_to_bytes_async"
)]
#[allow(deprecated)]
#[cfg(feature = "async")]
pub async fn read_to_string_async(path: &str) -> Result<String, OneIoError> {
    async_reader::read_to_string_async(path).await
}

/// Reads the entire content of a file asynchronously into a string,
/// replacing invalid UTF-8 sequences with `U+FFFD`.
#[cfg(feature = "async")]
pub async fn read_to_string_lossy_async(path: &str) -> Result<String, OneIoError> {
    async_reader::read_to_string_lossy_async(path).await
}

/// Reads the entire content of a file asynchronously into raw bytes.
#[cfg(feature = "async")]
pub async fn read_to_bytes_async(path: &str) -> Result<Vec<u8>, OneIoError> {
    async_reader::read_to_bytes_async(path).await
}

/// Downloads a file asynchronously from a URL to a local path.
#[cfg(feature = "async")]
pub async fn download_async(url: &str, path: &str) -> Result<(), OneIoError> {
    async_reader::download_async(url, path).await
}
