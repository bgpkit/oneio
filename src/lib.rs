/*!
OneIO is a Rust library providing unified IO operations for reading and writing compressed
files from local and remote sources with both synchronous and asynchronous support.

## Quick Start

```toml
oneio = "0.20"  # Default: gz, bz, https
```

## Feature Selection Guide

### Common Use Cases

**Local files only:**
```toml
oneio = { version = "0.20", default-features = false, features = ["gz", "bz"] }
```

**HTTPS with default rustls**:
```toml
oneio = { version = "0.20", default-features = false, features = ["https", "gz"] }
```

**HTTPS with custom TLS backend**:
```toml
# With native-tls (for WARP/corporate proxies)
oneio = { version = "0.20", default-features = false, features = ["http", "native-tls", "gz"] }
```

### Working with Corporate Proxies (Cloudflare WARP, etc.)

If you're behind a corporate proxy or VPN like Cloudflare WARP that uses custom TLS certificates:

```toml
oneio = { version = "0.20", default-features = false, features = ["http", "native-tls", "gz"] }
```

The `native-tls` feature uses your operating system's TLS stack with its trust store, which
includes custom corporate certificates. This works for both HTTP/HTTPS and S3 operations.

## Examples

### Reading Files

```rust,ignore
let content = oneio::read_to_string("https://example.com/data.txt.gz")?;
```

### Reusable OneIo Clients

```rust,ignore
let oneio = oneio::OneIo::builder()
    .header_str("Authorization", "Bearer TOKEN")
    .build()?;

let content = oneio.read_to_string("https://api.example.com/data.json.gz")?;
```

### Async Support

```rust,ignore
let content = oneio::read_to_string_async("https://example.com/data.json.gz").await?;
```

## Environment Variables

- `ONEIO_ACCEPT_INVALID_CERTS=true` - Accept invalid TLS certificates (insecure, for development only)
- `ONEIO_CA_BUNDLE=/path/to/ca.pem` - Add custom CA certificate to trust store
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
