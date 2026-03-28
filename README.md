# OneIO - all-in-one IO library for Rust

[![Rust](https://github.com/bgpkit/oneio/actions/workflows/rust.yml/badge.svg)](https://github.com/bgpkit/oneio/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/oneio)](https://crates.io/crates/oneio)
[![Docs.rs](https://docs.rs/oneio/badge.svg)](https://docs.rs/oneio)
[![License](https://img.shields.io/crates/l/oneio)](https://raw.githubusercontent.com/bgpkit/oneio/main/LICENSE)

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
# With rustls
oneio = { version = "0.20", default-features = false, features = ["http", "rustls", "gz"] }

# With native-tls (recommended for corporate proxies/VPNs)
oneio = { version = "0.20", default-features = false, features = ["http", "native-tls", "gz"] }
```

**S3-compatible storage**:
```toml
oneio = { version = "0.20", default-features = false, features = ["s3", "https", "gz"] }
```

**Async operations**:
```toml
oneio = { version = "0.20", features = ["async"] }
```

### Available Features

**Compression** (choose only what you need):
- `gz` - Gzip via flate2
- `bz` - Bzip2
- `lz` - LZ4
- `xz` - XZ
- `zstd` - Zstandard (balanced)

**Protocols**:
- `http` - HTTP-only support (no TLS)
- `https` - HTTP/HTTPS with rustls TLS backend (equivalent to `http` + `rustls`)
- `ftp` - FTP support (requires `http` + TLS backend)
- `s3` - S3-compatible storage

**TLS Backends** (for HTTPS - mutually exclusive):
- `rustls` - Pure Rust TLS (use with `http`). Uses both system certificates and bundled Mozilla certificates for maximum compatibility.
- `native-tls` - Platform native TLS (use with `http`). **Recommended for corporate proxies and VPNs** (Cloudflare WARP, etc.) as it uses the OS trust store.

**Additional**:
- `async` - Async support (limited to gz, bz, zstd for compression)
- `json` - JSON parsing
- `digest` - SHA256 digest calculation
- `cli` - Command-line tool

### Working with Corporate Proxies (Cloudflare WARP, etc.)

If you're behind a corporate proxy or VPN like Cloudflare WARP that uses custom TLS certificates:

```toml
[dependencies]
oneio = { version = "0.20", default-features = false, features = ["http", "native-tls", "gz"] }
```

The `native-tls` feature uses your operating system's TLS stack with its trust store, which includes custom corporate certificates. This works for both HTTP/HTTPS and S3 operations.

Alternatively, you can add custom CA certificates:

```rust
use oneio::OneIo;

let oneio = OneIo::builder()
    .add_root_certificate_pem(&std::fs::read("company-ca.pem")?)?
    .build()?;
```

Or set the `ONEIO_CA_BUNDLE` environment variable:
```bash
export ONEIO_CA_BUNDLE=/path/to/company-ca.pem
```

**Environment Variables:**
- `ONEIO_ACCEPT_INVALID_CERTS=true` - Accept invalid TLS certificates (insecure, for development only)
- `ONEIO_CA_BUNDLE=/path/to/ca.pem` - Add custom CA certificate to trust store

## Library Usage

### Basic Reading and Writing

Read all content into a string (works with compression and remote files automatically):

```rust
use oneio;

let content = oneio::read_to_string("https://spaces.bgpkit.org/oneio/test_data.txt.gz")?;
println!("{}", content);
```

Read line by line:

```rust
use oneio;

let lines = oneio::read_lines("https://spaces.bgpkit.org/oneio/test_data.txt.gz")?
    .map(|line| line.unwrap())
    .collect::<Vec<String>>();

for line in lines {
    println!("{}", line);
}
```

Get a reader for streaming:

```rust
use oneio;
use std::io::Read;

let mut reader = oneio::get_reader("tests/test_data.txt.gz")?;
let mut buffer = Vec::new();
reader.read_to_end(&mut buffer)?;
```

Write with automatic compression:

```rust
use oneio;
use std::io::Write;

let mut writer = oneio::get_writer("output.txt.gz")?;
writer.write_all(b"Hello, compressed world!")?;
drop(writer); // Important: close the writer

// Read it back
let content = oneio::read_to_string("output.txt.gz")?;
```

### Reusable OneIo Clients

The `OneIo` client allows you to configure headers, TLS certificates, timeouts, and other options once, then reuse the configuration across multiple operations:

```rust
use oneio::OneIo;
use reqwest::header::{HeaderName, HeaderValue};

// Build a reusable client with custom headers and certificates
let oneio = OneIo::builder()
    .header_str("Authorization", "Bearer TOKEN")
    .add_root_certificate_pem(&std::fs::read("company-ca.pem")?)?
    .timeout(std::time::Duration::from_secs(30))
    .connect_timeout(std::time::Duration::from_secs(10))
    .build()?;

// Reuse the same configuration for multiple requests
let content1 = oneio.read_to_string("https://api.example.com/data1.json")?;
let content2 = oneio.read_to_string("https://api.example.com/data2.json")?;
```

**Builder Methods:**
- `.header(name, value)` - Add a typed header (infallible, uses `HeaderName` and `HeaderValue`)
- `.header_str(name, value)` - Add a string header (panics on invalid input)
- `.user_agent(value)` - Set User-Agent header
- `.add_root_certificate_pem(pem)` - Add custom CA certificate (PEM format)
- `.add_root_certificate_der(der)` - Add custom CA certificate (DER format)
- `.danger_accept_invalid_certs(true)` - Accept invalid certificates
- `.timeout(duration)` - Set request timeout
- `.connect_timeout(duration)` - Set connection timeout
- `.proxy(proxy)` - Set HTTP proxy
- `.no_proxy()` - Disable system proxy
- `.redirect(policy)` - Set redirect policy
- `.configure_http(f)` - Escape hatch for direct reqwest configuration

### Compression Override

For URLs with query parameters or non-standard extensions, use explicit compression type:

```rust
use oneio::OneIo;

let oneio = OneIo::new()?;

// URL has query params, so we specify compression explicitly
let reader = oneio.get_reader_with_type(
    "https://api.example.com/data?format=gzip",
    "gz"
)?;
```

### Progress Tracking

Track download/read progress with callbacks:

```rust
use oneio::OneIo;

let oneio = OneIo::new()?;

// Callback receives (bytes_read, total_bytes).
// total_bytes is 0 when the server does not provide a Content-Length.
// The returned Option<u64> is Some(total) when the size was known upfront.
let (mut reader, total_size) = oneio.get_reader_with_progress(
    "https://example.com/largefile.gz",
    |bytes_read, total_bytes| {
        if total_bytes > 0 {
            let percent = (bytes_read as f64 / total_bytes as f64) * 100.0;
            println!("Progress: {:.1}%", percent);
        } else {
            println!("Downloaded: {} bytes", bytes_read);
        }
    }
)?;
```

### Async Support (Feature: `async`)

```rust
use oneio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let content = oneio::read_to_string_async("https://example.com/data.json.gz").await?;

    oneio::download_async(
        "https://example.com/data.csv.gz",
        "local_data.csv.gz"
    ).await?;

    Ok(())
}
```

Note: Async compression is limited to gz, bz, zstd. LZ4/XZ return `NotSupported`.

### S3 Operations (Feature: `s3`)

```rust
use oneio::s3::*;

// Direct S3 operations
s3_upload("my-bucket", "path/to/file.txt", "local/file.txt")?;
s3_download("my-bucket", "path/to/file.txt", "downloaded.txt")?;

// Read S3 directly using OneIO
let oneio = oneio::OneIo::new()?;
let content = oneio.read_to_string("s3://my-bucket/path/to/file.txt")?;

// Check existence and get metadata
if s3_exists("my-bucket", "path/to/file.txt")? {
    let stats = s3_stats("my-bucket", "path/to/file.txt")?;
    println!("Size: {} bytes", stats.content_length.unwrap_or(0));
}

// List objects
let objects = s3_list("my-bucket", "path/", Some("/".to_string()), false)?;
```

Required environment variables for S3:
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `AWS_REGION` (use "auto" for Cloudflare R2)
- `AWS_ENDPOINT`

### Error Handling

OneIO uses a simplified error enum with `#[non_exhaustive]` for forward compatibility:

```rust
use oneio::OneIoError;

match oneio::get_reader("file.txt") {
    Ok(reader) => { /* use reader */ },
    Err(OneIoError::Io(e)) => { /* filesystem error */ },
    Err(OneIoError::Network(e)) => { /* network error */ },
    Err(OneIoError::NetworkWithContext { source, url }) => {
        // Network error with URL context for debugging
        eprintln!("Failed to fetch {}: {}", url, source);
    }
    Err(OneIoError::Status { service, code }) => { /* remote status error */ },
    Err(OneIoError::InvalidCertificate(msg)) => { /* TLS cert error */ },
    Err(OneIoError::NotSupported(msg)) => { /* feature not compiled */ },
    _ => { /* handle future error variants */ }
}
```

### Crypto Provider Initialization (Rustls)

When using HTTPS, S3, or FTP features with rustls, oneio automatically initializes a crypto provider (AWS-LC or ring) on first use. For more control, initialize it explicitly:

```rust
use oneio;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize crypto provider explicitly at startup
    oneio::crypto::ensure_default_provider()?;

    // Now all HTTPS/S3/FTP operations will work
    let content = oneio::read_to_string("https://example.com/data.txt")?;

    Ok(())
}
```

## Command Line Tool

Install the CLI tool:

```bash
cargo install oneio --features cli
```

### Basic Usage

**Read and print a remote compressed file:**
```bash
$ oneio https://spaces.bgpkit.org/oneio/test_data.txt.gz
OneIO test file.
This is a test.
```

**Read local compressed file:**
```bash
$ oneio tests/test_data.txt.gz
OneIO test file.
This is a test.
```

**Get file statistics:**
```bash
$ oneio tests/test_data.txt --stats
lines: 	 2
chars: 	 31
```

### Download with Progress Bar

Download a file with automatic progress bar (shown when stderr is a terminal):
```bash
$ oneio -d https://example.com/largefile.bz2
downloaded to largefile.bz2
```

When stderr is piped or redirected the progress bar is suppressed.

### Custom HTTP Headers

Add custom headers for API authentication:
```bash
$ oneio -H "Authorization: Bearer TOKEN" -H "X-Custom-Header: value" https://api.example.com/data.json
```

### Compression Override

For URLs with query parameters where extension detection fails:
```bash
$ oneio --compression gz "https://api.example.com/data?format=gzip"
```

### Caching

Cache remote files locally for repeated reads:
```bash
$ oneio --cache-dir /tmp/cache https://example.com/largefile.gz
# Second read uses cache
$ oneio --cache-dir /tmp/cache https://example.com/largefile.gz
```

Force re-download even if cache exists:
```bash
$ oneio --cache-dir /tmp/cache --cache-force https://example.com/largefile.gz
```

### S3 Operations

**Upload file to S3:**
```bash
$ oneio s3 upload local-file.txt my-bucket path/in/s3.txt
uploaded to s3://my-bucket/path/in/s3.txt
```

**Download file from S3:**
```bash
$ oneio s3 download my-bucket path/in/s3.txt -o local-file.txt
downloaded s3://my-bucket/path/in/s3.txt to local-file.txt
```

**List S3 bucket:**
```bash
$ oneio s3 list my-bucket path/ --delimiter "/"
```

**List directories only:**
```bash
$ oneio s3 list my-bucket path/ --dirs
```

### Generate SHA256 Digest

```bash
$ oneio digest tests/test_data.txt
a3f5c8e9d2b1... (64 hex characters)
```

### CLI Help Output

```
$ oneio --help
oneio reads files from local or remote locations with any compression

Usage: oneio [OPTIONS] [FILE] [COMMAND]

Commands:
  s3      S3-related subcommands
  digest  Generate SHA256 digest
  help    Print this message or the given subcommand(s)

Arguments:
  [FILE]  file to open, remote or local

Options:
  -d, --download                   download the file to the current directory
  -o, --outfile <OUTFILE>          output file path
      --cache-dir <CACHE_DIR>      cache reading to a specified directory
      --cache-force                force re-caching if a local cache already exists
      --cache-file <CACHE_FILE>    specify cache file name
  -s, --stats                      read through the file and only print out stats
  -H, --header <HEADERS>           Add HTTP header (format: "Name: Value"), can be repeated
      --compression <COMPRESSION>  Override compression type (gz, bz2, lz4, xz, zst)
  -h, --help                       Print help
  -V, --version                    Print version
```

## Supported Formats

### Compression Detection

OneIO detects compression algorithm by the file extensions:

- **Gzip**: `.gz`, `.gzip`, `.tgz`
- **Bzip2**: `.bz`, `.bz2`
- **LZ4**: `.lz4`, `.lz`
- **XZ**: `.xz`, `.xz2`, `.lzma`
- **Zstandard**: `.zst`, `.zstd`

For URLs with query parameters, use `--compression` flag or `get_reader_with_type()`.

### Protocol Support
- **Local files**: `/path/to/file.txt`
- **HTTP/HTTPS**: `https://example.com/file.txt.gz`
- **FTP**: `ftp://ftp.example.com/file.txt` (requires `ftp` feature)
- **S3**: `s3://bucket/path/file.txt` (requires `s3` feature)

## License

MIT
