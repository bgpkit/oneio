# OneIO - all-in-one IO library for Rust

[![Rust](https://github.com/bgpkit/oneio/actions/workflows/rust.yml/badge.svg)](https://github.com/bgpkit/oneio/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/oneio)](https://crates.io/crates/oneio)
[![Docs.rs](https://docs.rs/oneio/badge.svg)](https://docs.rs/oneio)
[![License](https://img.shields.io/crates/l/oneio)](https://raw.githubusercontent.com/bgpkit/oneio/main/LICENSE)

OneIO is a Rust library providing unified IO operations for reading and writing compressed
files from local and remote sources with both synchronous and asynchronous support.

### Quick Start

```toml
oneio = "0.19"  # Default: gz, bz, http
```

### Feature Selection Guide

#### Common Use Cases

**Local files only:**
```toml
oneio = { version = "0.19", default-features = false, features = ["gz", "bz"] }
```

**HTTP/HTTPS with compression**:
```toml
oneio = { version = "0.19", default-features = false, features = ["http", "gz", "zstd"] }
```

**S3-compatible storage (AWS S3, R2, MinIO)**:
```toml
oneio = { version = "0.19", default-features = false, features = ["s3", "http", "gz"] }
```
Note: S3 requires the `http` feature.

**Async operations**:
```toml
oneio = { version = "0.19", features = ["async"] }
```

#### Available Features

**Compression** (choose only what you need):
- `gz` - Gzip via flate2
- `bz` - Bzip2
- `lz` - LZ4
- `xz` - XZ
- `zstd` - Zstandard (balanced)

**Protocols**:
- `http` - HTTP/HTTPS support
- `ftp` - FTP support (requires `http`)
- `s3` - S3-compatible storage

**Additional**:
- `async` - Async support (limited to gz, bz, zstd for compression)
- `json` - JSON parsing
- `digest` - SHA256 digest calculation
- `cli` - Command-line tool

**TLS Backend** (mutually exclusive):
- `rustls` - Pure Rust TLS (default)
- `native-tls` - Platform native TLS

Environment: Set `ONEIO_ACCEPT_INVALID_CERTS=true` to accept invalid certificates.

### Usages

#### Reading Files

Read all content into a string:

```rust
use oneio;

const TEST_TEXT: &str = "OneIO test file.\nThis is a test.";

// Works with compression and remote files automatically
let content = oneio::read_to_string("https://spaces.bgpkit.org/oneio/test_data.txt.gz")?;
assert_eq!(content.trim(), TEST_TEXT);
```

Read line by line:

```rust
use oneio;

let lines = oneio::read_lines("https://spaces.bgpkit.org/oneio/test_data.txt.gz")?
    .map(|line| line.unwrap())
    .collect::<Vec<String>>();

assert_eq!(lines.len(), 2);
assert_eq!(lines[0], "OneIO test file.");
assert_eq!(lines[1], "This is a test.");
```

Get a reader for streaming:

```rust
use oneio;
use std::io::Read;

let mut reader = oneio::get_reader("tests/test_data.txt.gz")?;
let mut buffer = Vec::new();
reader.read_to_end(&mut buffer)?;
```

#### Writing Files

Write with automatic compression:

```rust
use oneio;
use std::io::Write;

let mut writer = oneio::get_writer("output.txt.gz")?;
writer.write_all(b"Hello, compressed world!")?;
drop(writer); // Important: close the writer

// Read it back
let content = oneio::read_to_string("output.txt.gz")?;
assert_eq!(content, "Hello, compressed world!");
```

#### Remote Files with Custom Headers

```rust
use oneio;

let client = oneio::create_client_with_headers([("Authorization", "Bearer TOKEN")])?;
let mut reader = oneio::get_http_reader(
    "https://api.example.com/protected/data.json.gz",
    Some(client)
)?;

let content = std::io::read_to_string(&mut reader)?;
println!("{}", content);
```

#### Progress Tracking
Track download/read progress with callbacks:

```rust
use oneio;

let (mut reader, total_size) = oneio::get_reader_with_progress(
    "https://example.com/largefile.gz",
    |bytes_read, total_bytes| {
        match total_bytes {
            Some(total) => {
                let percent = (bytes_read as f64 / total as f64) * 100.0;
                println!("Progress: {:.1}%", percent);
            }
            None => println!("Downloaded: {} bytes", bytes_read),
        }
    }
)?;
```

#### Async Support (Feature: `async`)

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


### Supported Formats

#### Compression Detection

OneIO detects compression algorithm by the file extensions:

- **Gzip**: `.gz`, `.gzip`
- **Bzip2**: `.bz`, `.bz2`
- **LZ4**: `.lz4`, `.lz`
- **XZ**: `.xz`, `.xz2`
- **Zstandard**: `.zst`, `.zstd`

#### Protocol Support
- **Local files**: `/path/to/file.txt`
- **HTTP/HTTPS**: `https://example.com/file.txt.gz`
- **FTP**: `ftp://ftp.example.com/file.txt` (requires `ftp` feature)
- **S3**: `s3://bucket/path/file.txt` (requires `s3` feature)

### Command Line Tool

Install the CLI tool:

```bash
cargo install oneio --features cli
```

Basic usage:

```bash
# Read and print a remote compressed file
oneio https://example.com/data.txt.gz

# Download a file
oneio -d https://example.com/largefile.bz2

# Pipe to other tools
oneio https://api.example.com/data.json.gz | jq '.results | length'
```

### S3 Operations (Feature: `s3`)

```rust
use oneio::s3::*;

// Direct S3 operations
s3_upload("my-bucket", "path/to/file.txt", "local/file.txt")?;
s3_download("my-bucket", "path/to/file.txt", "downloaded.txt")?;

// Read S3 directly
let content = oneio::read_to_string("s3://my-bucket/path/to/file.txt")?;

// Check existence and get metadata
if s3_exists("my-bucket", "path/to/file.txt")? {
    let stats = s3_stats("my-bucket", "path/to/file.txt")?;
    println!("Size: {} bytes", stats.content_length.unwrap_or(0));
}

// List objects
let objects = s3_list("my-bucket", "path/", Some("/".to_string()), false)?;
```

### Error Handling

Three error types in v0.19:

```rust
use oneio::OneIoError;

match oneio::get_reader("file.txt") {
    Ok(reader) => { /* use reader */ },
    Err(OneIoError::Io(e)) => { /* filesystem error */ },
    Err(OneIoError::Network(e)) => { /* network error */ },
    Err(OneIoError::NotSupported(msg)) => { /* feature not compiled */ },
}
```

## License

MIT
