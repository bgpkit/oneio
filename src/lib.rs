/*!
Unified I/O for compressed files from any source.

OneIO provides a single interface for reading and writing files with any
compression format, from local disk or remote locations (HTTP, FTP, S3).

# Quick Start

```toml
[dependencies]
oneio = "0.20"
```

```rust,ignore
use oneio;

// Read a remote compressed file
let content = oneio::read_to_string("https://example.com/data.txt.gz")?;
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
| `ftp` | FTP support |
| `s3` | S3-compatible storage |
| `async` | Async I/O support |
| `json` | JSON deserialization |
| `digest` | SHA256 hashing |
| `cli` | Command-line tool |

**Example: Minimal setup for local files**
```toml
[dependencies]
oneio = { version = "0.20", default-features = false, features = ["gz"] }
```

**Example: HTTPS with custom TLS for corporate proxies**
```toml
[dependencies]
oneio = { version = "0.20", default-features = false, features = ["http", "native-tls", "gz"] }
```

# Core API

## Reading

```rust,ignore
// Read entire file to string
let content = oneio::read_to_string("data.txt")?;

// Read lines
for line in oneio::read_lines("data.txt")? {
    println!("{}", line?);
}

// Get a reader for streaming
let mut reader = oneio::get_reader("data.txt.gz")?;
```

## Writing

```rust,ignore
use std::io::Write;

let mut writer = oneio::get_writer("output.txt.gz")?;
writer.write_all(b"Hello")?;
// Compression finalized on drop
```

## Reusable Client

For multiple requests with shared configuration:

```rust,ignore
use oneio::OneIo;

let client = OneIo::builder()
    .header_str("Authorization", "Bearer token")
    .timeout(std::time::Duration::from_secs(30))
    .build()?;

let data1 = client.read_to_string("https://api.example.com/1.json")?;
let data2 = client.read_to_string("https://api.example.com/2.json")?;
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

```rust,ignore
use oneio::OneIo;

let client = OneIo::new()?;
let reader = client.get_reader_with_type(
    "https://api.example.com/data?format=gz",
    "gz"
)?;
```

# Protocols

- **Local**: `/path/to/file.txt`
- **HTTP/HTTPS**: `https://example.com/file.txt.gz`
- **FTP**: `ftp://ftp.example.com/file.txt` (requires `ftp` feature)
- **S3**: `s3://bucket/key` (requires `s3` feature)

# Async API

Enable the `async` feature:

```rust,ignore
let content = oneio::read_to_string_async("https://example.com/data.txt").await?;
```

Async compression support: `gz`, `bz`, `zstd`
LZ4 and XZ return `NotSupported` error.

# Error Handling

```rust,ignore
use oneio::OneIoError;

match oneio::get_reader("file.txt") {
    Ok(reader) => { /* ... */ }
    Err(OneIoError::Io(e)) => { /* filesystem error */ }
    Err(OneIoError::Network(e)) => { /* network error */ }
    Err(OneIoError::NotSupported(msg)) => { /* feature not enabled */ }
    _ => { /* future error variants */ }
}
```

# Environment Variables

- `ONEIO_ACCEPT_INVALID_CERTS=true` - Accept invalid TLS certificates (development only)
- `ONEIO_CA_BUNDLE=/path/to/ca.pem` - Add custom CA certificate to trust store

# TLS and Corporate Proxies

For environments with custom TLS certificates (Cloudflare WARP, corporate proxies):

1. Use `native-tls` feature to use the OS trust store:
   ```toml
   features = ["http", "native-tls"]
   ```

2. Or add certificates programmatically:
   ```rust,ignore
   let client = OneIo::builder()
       .add_root_certificate_pem(&std::fs::read("ca.pem")?)?
       .build()?;
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
pub fn read_to_string(path: &str) -> Result<String, OneIoError> {
    builder::default_oneio()?.read_to_string(path)
}

/// Reads and deserializes JSON into the requested type.
#[cfg(feature = "json")]
pub fn read_json_struct<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, OneIoError> {
    builder::default_oneio()?.read_json_struct(path)
}

/// Returns an iterator over lines from the provided path.
pub fn read_lines(
    path: &str,
) -> Result<std::io::Lines<std::io::BufReader<Box<dyn Read + Send>>>, OneIoError> {
    builder::default_oneio()?.read_lines(path)
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
#[cfg(feature = "async")]
pub async fn read_to_string_async(path: &str) -> Result<String, OneIoError> {
    async_reader::read_to_string_async(path).await
}

/// Downloads a file asynchronously from a URL to a local path.
#[cfg(feature = "async")]
pub async fn download_async(url: &str, path: &str) -> Result<(), OneIoError> {
    async_reader::download_async(url, path).await
}
