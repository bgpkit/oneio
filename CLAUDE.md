# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

OneIO is a Rust library that provides unified IO operations for reading and writing compressed files from local and remote sources (HTTP, FTP, S3). It's built with a modular feature flag system allowing users to include only what they need.

## Build and Development Commands

### Core Commands
- `cargo build` - Build with default features (lib-core + rustls)
- `cargo build --no-default-features` - Build without any features
- `cargo build --all-features` - Build with all features enabled
- `cargo test` - Run all tests with default features
- `cargo test --all-features` - Run tests with all features

### Code Quality Checks (Run before committing)
- `cargo fmt` - Format code
- `cargo fmt --check` - Check formatting without modifying
- `cargo clippy --all-features -- -D warnings` - Run clippy on all features
- `cargo clippy --no-default-features` - Run clippy with no features

### Testing Specific Features
- `cargo test --features s3` - Test S3 functionality
- `cargo build --no-default-features --features lib-core,native-tls` - Build with native-tls instead of rustls
- `cargo build --no-default-features --features gz,bz` - Build with only specific compression support

### Documentation
- `cargo readme > README.md` - Regenerate README from lib.rs documentation (run after changing lib.rs docs)

## Architecture and Key Components

### Module Structure
- `src/lib.rs` - Main library entry point with comprehensive documentation
- `src/error.rs` - Unified error handling using thiserror
- `src/oneio/mod.rs` - Core IO operations (get_reader, get_writer)
- `src/oneio/compressions/` - Compression implementations (gzip, bzip2, lz4, xz, zstd)
- `src/oneio/remote.rs` - HTTP/FTP remote file handling
- `src/oneio/s3.rs` - S3 operations (requires `s3` feature)
- `src/oneio/utils.rs` - Utility functions (read_lines, read_to_string, download, etc.)
- `src/bin/oneio.rs` - CLI binary implementation (requires `cli` feature)

### Feature Flag System

The library uses a sophisticated feature flag system to control dependencies:

**Core Features (`lib-core`):**
- `remote` - HTTP/FTP support
- `compressions` - All compression algorithms
- `json` - JSON parsing support

**TLS Backends (mutually exclusive):**
- `rustls` (default) - Pure Rust TLS
- `native-tls` - Platform native TLS

**Compression Algorithms (can be selected individually):**
- `gz` - Gzip support via flate2
- `bz` - Bzip2 support
- `lz` - LZ4 support
- `xz` - XZ support (requires system xz library)
- `zstd` - Zstandard support

**Optional Features:**
- `s3` - AWS S3 compatible storage
- `cli` - Command-line tool
- `digest` - SHA256 digest generation

### Key Design Patterns

1. **Protocol Detection**: Uses URL protocol prefix (http://, ftp://, s3://) to route to appropriate reader/writer
2. **Compression Detection**: Uses file extension to automatically apply compression/decompression
3. **Unified Interface**: All readers return `Box<dyn Read + Send>`, writers return `Box<dyn Write + Send>`
4. **Build-time Validation**: `build.rs` ensures TLS backend is selected when remote features are enabled

### Testing Infrastructure

Tests are in `tests/oneio_test.rs` and cover:
- Local file reading with all compression formats
- Remote HTTP reading
- Cache functionality
- JSON deserialization
- 404 error handling
- Writer functionality

Test data files are stored in `tests/` directory with various compression formats.

### Environment Variables

- `ONEIO_ACCEPT_INVALID_CERTS` - Set to "true" to accept invalid TLS certificates
- AWS credentials for S3 support:
  - `AWS_ACCESS_KEY_ID`
  - `AWS_SECRET_ACCESS_KEY`
  - `AWS_REGION` (use "auto" for Cloudflare R2)
  - `AWS_ENDPOINT`

## Development Progress Tracking

### Active Development Sessions
When working on multi-phase implementations or significant features:

1. **Create PLAN.md** - Temporary file to track session progress
   ```markdown
   # OneIO v0.X.Y Implementation Plan
   - Track current phase and completed tasks
   - Document breaking changes as discovered
   - Include testing checklist
   - Delete after release
   ```

2. **Update progress daily** - Keep PLAN.md current during active development
3. **Delete PLAN.md after release** - It's temporary, not permanent documentation

### Using PLAN.md Template
- Phase-based organization for large changes
- ✅ Completed / 🔄 In Progress / ⏳ Pending status indicators
- Breaking changes section for each phase
- Migration guide examples
- Comprehensive testing checklist

## Common Development Patterns

### Adding New Compression Format
1. Add feature flag in `Cargo.toml`
2. Implement reader/writer in `src/oneio/compressions/`
3. Update `get_compression_reader/writer` in compressions module
4. Add test data files and update tests

### Working with Remote Files
The library abstracts remote/local file access - just pass a URL or file path to `get_reader()` or `get_writer()`.