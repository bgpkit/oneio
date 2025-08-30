# OneIO Feature Simplification Plan (2025)

This document tracks the v0.19.0 simplification of OneIO's feature flags and API surface. The goal is to reduce complexity while maintaining the library's "dead-simple to use" philosophy.

## Current Status: Planning Phase

## Key Changes

### 1. Feature Flag Simplification
Remove complex hierarchy (`lib-core`, `remote`, `compressions`) and replace with flat, intuitive features:

```toml
# New simplified features
default = ["gz", "bz", "http"]
gz = ["flate2"]
bz = ["bzip2"] 
lz = ["lz4"]
xz = ["xz2"]
zstd = ["dep:zstd"]
http = ["reqwest/default"]
ftp = ["http", "suppaftp"]
s3 = ["rust-s3"]
json = ["serde", "serde_json"]
digest = ["ring", "hex"]
```

### 2. Error Simplification
Reduce from 10+ error variants to just 3:
- `Io(std::io::Error)` - File system errors
- `Network(Box<dyn Error>)` - Network/remote errors  
- `NotSupported(String)` - Feature not compiled

### 3. Remove Unnecessary Code
- Delete build.rs entirely
- Remove OneIOCompression trait (use direct match statements)
- Fix unsafe string operations in path parsing

## Migration Guide

```toml
# Before (v0.18)
oneio = { version = "0.18", features = ["lib-core", "rustls"] }

# After (v0.19)  
oneio = { version = "0.19", features = ["gz", "bz", "http"] }
```

## New Features (Included in v0.19)

### 4. Async Support
Add minimal async functions for downstream testing:
```rust
#[cfg(feature = "async")]
pub async fn get_reader_async(path: &str) -> Result<impl AsyncRead, OneIoError>

#[cfg(feature = "async")]  
pub async fn read_to_string_async(path: &str) -> Result<String, OneIoError>

#[cfg(feature = "async")]
pub async fn download_async(url: &str, path: &str) -> Result<(), OneIoError>
```

**Feature flag:**
```toml
async = ["tokio", "async-compression", "futures"]
```

### 5. Progress Tracking
Add progress support with early failure if size cannot be determined:
```rust
pub fn get_reader_with_progress<F>(
    path: &str,
    progress: F
) -> Result<(Box<dyn Read + Send>, u64), OneIoError>
where
    F: Fn(u64, u64) + Send + 'static  // (bytes_read, total_bytes)
```

**Key features:**
- Fails early if total size cannot be determined (no Content-Length, etc.)
- Tracks raw bytes read (before decompression)
- Returns both reader and total file size
- Works with: local files, HTTP with Content-Length, FTP, S3
- Fails for: streaming endpoints, chunked transfer without Content-Length

## Implementation Checklist

### Core Simplifications
- [ ] Update Cargo.toml with new feature structure
- [ ] Simplify error.rs to 3 variants
- [ ] Remove build.rs
- [ ] Update compression module to use direct matching

### New Features (for downstream testing)
- [ ] Implement async functions with `async` feature flag
- [ ] Add progress tracking with size determination
- [ ] Add `get_content_length()` helper for all protocols

### Testing & Documentation  
- [ ] Test all feature combinations including new features
- [ ] Add async examples and documentation
- [ ] Add progress tracking examples
- [ ] Write migration guide
- [ ] Update README with new capabilities

## Rationale for v0.19 Scope

**Why include new features:**
- Downstream libraries need async/progress for upcoming releases
- Better to test simplifications and new features together
- Avoids multiple breaking change cycles
- New features are additive-only (no breaking changes)

**Risk mitigation:**
- New features behind feature flags (`async`, new function for progress)
- Can be disabled if issues found during testing
- Core simplifications remain the primary focus