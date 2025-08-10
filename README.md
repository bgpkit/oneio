# OneIO - all-in-one IO library for Rust

[![Rust](https://github.com/bgpkit/oneio/actions/workflows/rust.yml/badge.svg)](https://github.com/bgpkit/oneio/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/oneio)](https://crates.io/crates/oneio)
[![Docs.rs](https://docs.rs/oneio/badge.svg)](https://docs.rs/oneio)
[![License](https://img.shields.io/crates/l/oneio)](https://raw.githubusercontent.com/bgpkit/oneio/main/LICENSE)

OneIO is a Rust library that provides a unified IO interface for reading and writing
data files from different sources and compressions, with support for both synchronous
and asynchronous operations.

### Usage and Feature Flags

Enable default features (gzip, bzip2, HTTP support):

```toml
oneio = "0.19"
```

Select from supported feature flags:

```toml
oneio = { version = "0.19", default-features = false, features = ["gz", "http", "async"] }
```

### Feature Flags

OneIO v0.19 uses a simplified, flat feature structure:

#### Default Features
- `gz`: Support gzip compression using `flate2` crate
- `bz`: Support bzip2 compression using `bzip2` crate
- `http`: Support HTTP(S) remote files using `reqwest` crate

#### Optional Compression Features
- `lz`: Support LZ4 compression using `lz4` crate
- `xz`: Support XZ compression using `xz2` crate (requires xz library)
- `zstd`: Support Zstandard compression using `zstd` crate

#### Optional Protocol Features
- `ftp`: Support FTP remote files using `suppaftp` crate (requires `http`)
- `s3`: Support AWS S3 compatible buckets using `rust-s3` crate

#### Other Features
- `json`: Enable JSON parsing with `serde` and `serde_json`
- `digest`: Enable SHA256 digest generation using `ring` crate
- `async`: Enable async support with `tokio` and `async-compression`
- `cli`: Build the `oneio` command-line tool

#### TLS Configuration (Advanced)
- `rustls`: Use rustls for TLS (default when TLS is needed)
- `native-tls`: Use native TLS instead of rustls

Set `ONEIO_ACCEPT_INVALID_CERTS=true` to accept invalid certificates (not recommended).

### New in v0.19

#### Progress Tracking
Track download/read progress with callbacks, works with both known and unknown file sizes.
Progress tracking now provides better error handling and distinguishes between unknown size
(streaming endpoints) and failed size determination:

```rust
use oneio;

let (mut reader, total_size) = oneio::get_reader_with_progress(
    "https://example.com/largefile.gz",
    |bytes_read, total_bytes| {
        if total_bytes > 0 {
            let percent = (bytes_read as f64 / total_bytes as f64) * 100.0;
            println!("Progress: {:.1}% ({}/{})", percent, bytes_read, total_bytes);
        } else {
            println!("Downloaded: {} bytes (size unknown)", bytes_read);
        }
    }
)?;

// total_size is None when file size cannot be determined
match total_size {
    Some(size) => println!("File size: {} bytes", size),
    None => println!("File size: unknown (streaming)"),
}

let content = std::io::read_to_string(&mut reader)?;
```

#### Async Support (Feature: `async`)
Asynchronous file operations with automatic compression:

```rust
use oneio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Async reading with automatic decompression
    let content = oneio::read_to_string_async("https://example.com/data.json.gz").await?;
    println!("Content: {}", content);

    // Async download
    oneio::download_async(
        "https://example.com/data.csv.gz",
        "local_data.csv.gz"
    ).await?;

    Ok(())
}
```

**Note**: Async compression support varies by format:
- ✅ Supported: gzip, bzip2, zstd
- ❌ Not supported: LZ4, XZ (returns `NotSupported` error)

### Basic Usage

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

### Supported Formats

#### Compression Detection
Compression is detected automatically by file extension:

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

// Upload local file to S3
s3_upload("my-bucket", "path/to/file.txt", "local/file.txt")?;

// Read directly from S3
let content = oneio::read_to_string("s3://my-bucket/path/to/file.txt")?;

// Download from S3
s3_download("my-bucket", "path/to/file.txt", "downloaded.txt")?;

// Check if S3 object exists (improved error handling)
if s3_exists("my-bucket", "path/to/file.txt")? {
    println!("File exists!");
}

// Get object metadata
let stats = s3_stats("my-bucket", "path/to/file.txt")?;
println!("Size: {} bytes", stats.content_length.unwrap_or(0));

// List objects
let objects = s3_list("my-bucket", "path/", Some("/".to_string()), false)?;
for obj in objects {
    println!("Found: {}", obj);
}
```

### Error Handling

OneIO v0.19 uses a simplified error system with three main types:

```rust
use oneio::OneIoError;

match oneio::get_reader("nonexistent.txt") {
    Ok(reader) => { /* use reader */ },
    Err(OneIoError::Io(e)) => println!("IO error: {}", e),
    Err(OneIoError::Network(e)) => println!("Network error: {}", e),
    Err(OneIoError::NotSupported(msg)) => println!("Not supported: {}", msg),
}
```

## Built with ❤️ by BGPKIT Team

<a href="https://bgpkit.com"><img src="https://bgpkit.com/Original%20Logo%20Cropped.png" alt="https://bgpkit.com/favicon.ico" width="200"/></a>

## License

MIT
