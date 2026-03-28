# OneIO Test Plan

## Overview

This document outlines the current test coverage and planned improvements for the OneIO library. The goal is to ensure critical functionality is well-tested while identifying gaps for future work.

**Current Status:** 51 tests passing (`--all-features`). All Phase 1 and Phase 2 items are implemented. See coverage gaps for remaining low-priority work.

**Test Organization:**
- Unit tests embedded in source files (src/*.rs)
- Integration tests in `tests/` directory
- Doc tests in lib.rs

---

## Current Test Inventory

### 1. Unit Tests (in source files)

| File | Test | Description |
|------|------|-------------|
| `src/crypto.rs` | `test_ensure_default_provider` | Verifies crypto provider initialization |
| `src/crypto.rs` | `test_provider_installed` | Confirms provider is available after init |
| `src/s3.rs` | `test_s3_url_parse` | Tests S3 URL parsing (bucket/key extraction) |
| `src/s3.rs` | `test_s3_upload_nonexistent_file_early_validation` | Validates early file existence check |
| `src/s3.rs` | `test_stream_reader_reads_in_order` | Tests streaming reader ordering |
| `src/s3.rs` | `test_stream_reader_propagates_error` | Tests error handling in stream reader |

**Total: 6 tests**

### 2. Integration Tests (tests/basic_integration.rs)

| Test | Feature(s) | Description |
|------|------------|-------------|
| `test_local_files` | core, gz, bz | Read local files (txt, gz, bz2) |
| `test_writers` | core, gz, bz | Write and read back compressed files |
| `test_remote_files` | http, gz, bz | Read remote HTTP files |
| `test_404_handling` | http | Tests 404 error handling and exists() |
| `test_oneio_builder_reuses_default_headers` | http | Verifies header reuse across requests |
| `test_oneio_builder_accepts_root_certificate` | rustls\|native-tls | Tests custom CA certificate loading |
| `test_file_extension_plain` | core | Tests file extension detection |
| `test_file_extension_strips_query_params` | gz | Tests URL query param stripping |
| `test_get_reader_with_type_plain` | core | Tests explicit no-compression override |
| `test_get_reader_with_type_gz_override` | gz | Tests explicit gzip override |
| `test_get_reader_with_type_bz2_override` | bz | Tests explicit bz2 override |
| `test_builder_timeout_builds_successfully` | http | Tests timeout configuration |
| `test_builder_configure_http_escape_hatch` | http | Tests escape hatch configuration |
| `test_builder_no_proxy_builds_successfully` | http | Tests no_proxy configuration |
| `test_download_with_retry_succeeds_on_first_attempt` | http | Tests retry on successful download |
| `test_download_with_retry_exhausts_retries_on_bad_url` | http | Tests retry exhaustion on bad URL |

**Total: 45 tests** (includes Phase 1 and Phase 2 additions)

### 3. Async Integration Tests (tests/async_integration.rs)

| Test | Feature(s) | Description |
|------|------------|-------------|
| `async_read_local_plain` | async, core | Async local file reading |
| `async_read_local_gzip` | async, gz | Async gzip file reading |
| `async_read_http_plain` | async, http | Async HTTP file reading |
| `async_read_http_gzip` | async, http, gz | Async HTTP gzip reading |
| `async_download_http_to_file` | async, http | Async download to file |
| `async_download_preserves_compressed_bytes` | async, gz | Verifies byte preservation |

**Total: 6 tests**

### 4. Doc Tests (src/lib.rs)

| Test | Description | Status |
|------|-------------|--------|
| Feature guide example (line 45) | Code example | Ignored |
| Feature guide example (line 51) | Code example | Ignored |
| Feature guide example (line 61) | Code example | Ignored |

**Total: 3 ignored (examples only)**

---

## Coverage Gaps

### Critical Priority (Block Release if Not Fixed)

**None identified** - Core functionality is adequately tested for release.

### High Priority — ✅ Implemented

| Gap | Status | Tests Added |
|-----|--------|-------------|
| **LZ4/XZ/Zstd compression** | ✅ Done | `test_local_lz4/xz/zstd`, `test_write_lz4/xz/zstd`, `test_get_reader_with_type_lz4/xz/zstd_override` |
| **Progress tracking** | ✅ Done | `test_get_reader_with_progress_fires_callback`, `test_get_reader_with_progress_local_no_total` |
| **Cache reader** | ✅ Done | `test_cache_reader_creates_cache_file`, `_reuses_existing_cache`, `_force_refreshes_cache`, `_creates_missing_cache_dir` |
| **JSON parsing** | ✅ Done | `test_read_json_struct_local`, `test_read_json_struct_invalid_returns_error` |
| **Content length detection** | ✅ Done | `test_get_content_length_local_file`, `test_get_content_length_http_with_content_length_header` |

Note: implementing LZ4/Zstd write tests revealed a bug — `lz4::Encoder` has no `Drop` impl and requires an explicit `finish()` call. Fixed by adding a `Lz4Writer` wrapper in `compression.rs`.

### Medium Priority — ✅ Implemented

| Gap | Status | Tests Added |
|-----|--------|-------------|
| **Error variants** | ✅ Done | `test_invalid_certificate_error_variant`, `test_invalid_certificate_der_error_variant`, `test_network_error_on_refused_connection` |
| **Writer variations** | ✅ Done | `test_get_writer_raw_creates_uncompressed_file`, `test_get_writer_raw_creates_parent_dirs` |
| **Environment variables** | ✅ Done | `test_oneio_ca_bundle_env_var_valid_path`, `_missing_path`, `test_oneio_accept_invalid_certs_env_var` |
| **Digest/SHA256** | ✅ Done | `test_get_sha256_digest_known_file`, `test_get_sha256_digest_missing_file_returns_error` |
| **FTP protocol** | Low - Requires running FTP server | Skipped |
| **S3 operations** | Low - Requires credentials | Skipped |

### Low Priority (Future Work)

| Gap | Impact | Notes |
|-----|--------|-------|
| **Proxy configuration** | Low | Would require mock proxy server |
| **Redirect policy** | Low | Test redirect following |
| **CLI tests** | Low | Would require external tooling or integration testing framework |

---

## Test Implementation Roadmap

### Phase 1: High Priority Tests — ✅ Complete

- [x] LZ4/XZ/Zstd compression: read, write, and explicit type override
- [x] Progress tracking: callback fires with correct bytes/total, local and HTTP
- [x] Cache reader: creation, reuse, force-refresh, nested directory creation
- [x] JSON parsing: valid struct deserialization, invalid input returns error
- [x] Content length: local file metadata, HTTP with Content-Length header

### Phase 2: Medium Priority Tests — ✅ Complete

- [x] Error variants: `InvalidCertificate` (PEM + DER), network error on refused connection
- [x] Environment variables: `ONEIO_CA_BUNDLE` (valid + missing path), `ONEIO_ACCEPT_INVALID_CERTS`
- [x] SHA256 digest: known hash assertion, missing file returns error
- [x] Writer: `get_writer_raw` creates uncompressed file, creates nested parent dirs

### Phase 3: Integration & Infrastructure (Ongoing)

- [ ] Consider S3 integration tests with mock server (LocalStack)
- [ ] Consider CLI tests with assert_cmd or similar
- [ ] Add property-based tests for compression round-trips
- [ ] Add benchmarks to CI to prevent performance regressions

---

## Test Infrastructure Improvements

### Current State
- Tests use real HTTP requests (spaces.bgpkit.org)
- Tests require network connectivity
- Some tests gracefully handle network failures (async tests)

### Proposed Improvements

1. **Mock HTTP Server for Tests**
   - Use `mockito` or similar for HTTP tests
   - More reliable, faster, works offline
   - Can test edge cases (slow responses, errors)

2. **Test Categorization**
   - Add `#[ignore]` to network-dependent tests by default
   - Create test profiles: `cargo test --lib` (fast, offline) vs `cargo test --all-features` (full)

3. **CI Improvements**
   - Run tests with different feature combinations
   - Test minimal features (`--no-default-features`)
   - Test each compression format individually

4. **Coverage Reporting**
   - Add `cargo-tarpaulin` to CI
   - Set minimum coverage threshold

---

## Testing Guidelines

### When Adding New Features

1. **Unit tests** for internal logic (in src/*.rs)
2. **Integration tests** for public API (in tests/)
3. **Feature-gate tests** appropriately (#[cfg(feature = "...")])
4. **Test both success and failure paths**
5. **Mock external services** when possible

### Test Data

- Use `tests/test_data.txt` as base test content
- Use compressed variants (`.gz`, `.bz2`, etc.) for compression tests
- Create temporary files in `tests/` with `_tmp_` prefix and clean up

### Network Tests

- Prefer local mock servers over real network calls
- If using real network, handle failures gracefully (don't panic)
- Use spaces.bgpkit.org for integration tests (stable endpoints)

---

## Summary

**Current Coverage:** ✅ All planned phases complete
- Core I/O: ✅ Well tested
- HTTP/HTTPS: ✅ Well tested
- Compression (gz, bz, lz4, xz, zstd): ✅ Well tested
- Builder API: ✅ Well tested
- Async: ✅ Well tested
- Progress tracking: ✅ Well tested
- Cache reader: ✅ Well tested
- JSON parsing: ✅ Well tested
- SHA256 digest: ✅ Well tested
- Error variants: ✅ Well tested
- Environment variables: ✅ Well tested

**Remaining gaps (low priority):**
- FTP protocol (requires running FTP server)
- S3 live integration (requires credentials; unit tests for URL parsing and streaming already exist)
- CLI tests (requires assert_cmd or similar)
- Proxy / redirect policy tests (require mock proxy server)

**Total Tests:** 51 (`--all-features`): 6 unit + 45 integration + 6 async
