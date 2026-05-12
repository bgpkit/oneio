# Spec: Lossy Read Operations

**Status**: Draft (Revised after MAGI review)
**Author**: Mingwei Zhang
**Created**: 2025-05-12
**Target Branch**: `dev/lossy-read`

## 1. Overview

Add lossy UTF-8 reading variants and byte-perfect helpers, deprecate strict-UTF-8 APIs that fail on real-world data containing non-UTF-8 bytes.

**Non-goals:**
- Full encoding detection (e.g., CP1252, Shift-JIS) — out of scope
- Changing existing APIs to be lossy by default — backward incompatible
- Async line iteration — not requested, `read_to_string_lossy_async` is sufficient

**Success criteria:**
- [ ] `read_lines_lossy` no longer aborts on Latin-1 or other non-UTF-8 data
- [ ] CLI no longer exits on non-UTF-8 input
- [ ] Existing strict APIs deprecated with clear migration paths
- [ ] All internal code compiles without deprecation warnings
- [ ] Tests cover lossy replacement, byte-perfect round-trip, and legacy failure mode

## 2. Current State

`OneIo::read_lines`, `OneIo::read_to_string`, and `read_to_string_async` use Rust's strict UTF-8 reading (`BufRead::lines()`, `Read::read_to_string()`, `AsyncReadExt::read_to_string()`). Any invalid byte sequence yields `io::Error(InvalidData)` and stops iteration or fails the entire read.

Real-world impact: The RIPE IRR database (~280 MB) contains Latin-1 `0xf3` bytes in `descr:` fields. `read_lines` fails after ~1700 lines with `"stream did not contain valid UTF-8"`.

The CLI (`src/bin/oneio.rs`) manually wraps a resolved reader in `BufReader` and calls `.lines()`, so it also dies on the first bad byte with exit code 1.

There is **no built-in lossy line iterator in `std::io`**. `BufRead::lines()` is strict UTF-8 only, and `BufRead::split(b'\n')` leaves trailing `\r` on CRLF input while also double-allocating per line. A custom iterator is required.

## 3. Proposed Solution

### Core type: `impl Iterator` helper

A private helper using `std::iter::from_fn` that mirrors `std::io::Lines` semantics but replaces invalid UTF-8 with `U+FFFD`. Uses a reusable `Vec<u8>` buffer and `read_until` for single-allocation-per-line performance.

```rust
fn lossy_lines<B: BufRead>(mut buf: B) -> impl Iterator<Item = io::Result<String>> {
    let mut bytes = Vec::new();
    std::iter::from_fn(move || {
        bytes.clear();
        match buf.read_until(b'\n', &mut bytes) {
            Ok(0) => None,
            Ok(_) => {
                // Match BufRead::lines() semantics
                if bytes.ends_with(b"\n") {
                    bytes.pop();
                    if bytes.ends_with(b"\r") {
                        bytes.pop();
                    }
                }
                Some(Ok(String::from_utf8_lossy(&bytes).into_owned()))
            }
            Err(e) => Some(Err(e)),
        }
    })
}
```

**Why `impl Iterator` over a public struct or `Box<dyn Iterator>`:**

| Approach | CRLF handling | Allocations/line | API surface | Verdict |
|----------|---------------|------------------|-------------|---------|
| `BufRead::split(b'\n')` + `from_utf8_lossy` | Broken (leaves `\r`) | 2 (Vec + String) | `Box<dyn Iterator>` | Rejected |
| `Box<dyn Iterator<Item = io::Result<String>>>` | Same as above | 2 | Opaque, dynamic dispatch | Rejected |
| **`impl Iterator` via `from_fn`** | Correct | 1 | Minimal — no new public types | **Accepted** |
| `LossyLines` public struct | Correct | 1 | Adds permanent public type | Rejected |

**Tradeoff:** `impl Iterator` cannot be named in struct fields. In practice, 100% of `read_lines` callers in this codebase use it directly in a `for` loop or `.map().collect()` chain, not stored in structs. If a namable type is needed later, a public struct can be added without breaking existing `impl Iterator` consumers.

### New public APIs

| API | Location | Description |
|-----|----------|-------------|
| `OneIo::to_lines_lossy` | `src/client.rs` | Converts `Box<dyn Read + Send>` into `impl Iterator<Item = io::Result<String>>` |
| `OneIo::read_lines_lossy` | `src/client.rs` | `get_reader` + `to_lines_lossy` |
| `OneIo::read_to_string_lossy` | `src/client.rs` | `read_to_end` + `String::from_utf8_lossy` |
| `OneIo::read_to_bytes` | `src/client.rs` | `read_to_end`, returns `Vec<u8>` |
| `oneio::read_lines_lossy` | `src/lib.rs` | Module-level convenience |
| `oneio::read_to_string_lossy` | `src/lib.rs` | Module-level convenience |
| `oneio::read_to_bytes` | `src/lib.rs` | Module-level convenience |
| `oneio::read_to_string_lossy_async` | `src/async_reader.rs` | Async `read_to_end` + lossy conversion |
| `oneio::read_to_bytes_async` | `src/async_reader.rs` | Async byte-perfect read |

### Deprecated APIs

Attach `#[deprecated(since = "0.23.0", note = "...")]`:

- `OneIo::read_lines` → *"Use `read_lines_lossy` for lossy text, `read_to_bytes` for byte-perfect whole-file reads, or `get_reader` for byte streaming"*
- `OneIo::read_to_string` → *"Use `read_to_string_lossy` or `read_to_bytes`"*
- `oneio::read_lines` → same as above
- `oneio::read_to_string` → same as above
- `oneio::read_to_string_async` → *"Use `read_to_string_lossy_async` or `read_to_bytes_async`"*

### CLI change

Add a `--strict-utf8` flag and default to lossy reading:

```rust
/// Fail on invalid UTF-8 instead of replacing with U+FFFD
#[clap(long)]
strict_utf8: bool,
```

Replace the inline `reader.lines()` loop:

```rust
// Before
let reader = Box::new(BufReader::new(reader_result?));
for line in reader.lines() { ... }

// After
let lines = if cli.strict_utf8 {
    Box::new(BufReader::new(reader_result?).lines())
        as Box<dyn Iterator<Item = io::Result<String>>>
} else {
    oneio.to_lines_lossy(reader_result?)
};
for line in lines { ... }
```

## 4. Breaking Changes

### Library API

No breaking changes. All existing functions remain unchanged with identical signatures and behavior. Strict-UTF-8 APIs are **deprecated** with `#[deprecated]` but not removed.

| API | Change Type | Detail |
|-----|-------------|--------|
| `OneIo::read_lines` | Deprecated (warning only) | Still strict UTF-8, still works |
| `OneIo::read_to_string` | Deprecated (warning only) | Still strict UTF-8, still works |
| `oneio::read_lines` | Deprecated (warning only) | Still strict UTF-8, still works |
| `oneio::read_to_string` | Deprecated (warning only) | Still strict UTF-8, still works |
| `oneio::read_to_string_async` | Deprecated (warning only) | Still strict UTF-8, still works |
| `OneIo::read_lines_lossy` | **New** | Returns `impl Iterator<Item = io::Result<String>>` |
| `OneIo::read_to_string_lossy` | **New** | Returns `Result<String, OneIoError>` |
| `OneIo::read_to_bytes` | **New** | Returns `Result<Vec<u8>, OneIoError>` |
| `oneio::read_lines_lossy` | **New** | Returns `impl Iterator<Item = io::Result<String>>` |
| `oneio::read_to_string_lossy` | **New** | Returns `Result<String, OneIoError>` |
| `oneio::read_to_bytes` | **New** | Returns `Result<Vec<u8>, OneIoError>` |
| `oneio::read_to_string_lossy_async` | **New** | Async, returns `String` |
| `oneio::read_to_bytes_async` | **New** | Async, returns `Vec<u8>` |

### CLI Binary

**Behavioral breaking change.** The CLI previously exited with code 1 when encountering invalid UTF-8 in the input stream. After this change, the CLI silently replaces invalid byte sequences with `U+FFFD` and continues processing.

Impact on users:
- **Shell scripts** using `set -e` or checking `$?` will no longer catch encoding errors via exit code
- **Data pipelines** that relied on `oneio` failing on non-UTF-8 data will now receive lossy output instead of an error
- **Log processing** will now complete on partially-invalid files instead of aborting mid-stream

This is an intentional change: the CLI is a data pipeline tool, and hard failure on encoding is unexpected for non-programmers. Users who need strict validation can use the new `--strict-utf8` CLI flag.

## 6. Implementation Plan

| Phase | Task | Acceptance Criteria |
|-------|------|-------------------|
| 1 | Write spec | This document exists and is reviewed |
| 2 | Add private `lossy_lines` helper | Compiles, handles `\r\n`, `\n`, and no-trailing-newline correctly |
| 3 | Add `OneIo::to_lines_lossy` | Returns `impl Iterator`, used by `read_lines_lossy` and CLI |
| 4 | Add `read_lines_lossy`, `read_to_string_lossy`, `read_to_bytes` | All have module-level wrappers |
| 5 | Add async variants | `read_to_string_lossy_async`, `read_to_bytes_async` |
| 6 | Deprecate strict APIs | `#[deprecated]` on 5 functions, zero internal warnings |
| 7 | Migrate CLI to lossy | CLI uses `to_lines_lossy`, no manual `BufReader` |
| 8 | Migrate tests/examples | All internal callers use new APIs |
| 9 | Add encoding test | Test file with `0xf3` validates lossy and bytes behavior |
| 10 | Update docs/CHANGELOG | `lib.rs` examples, `README.md`, `CHANGELOG.md` updated |

## 7. Testing Strategy

### 7.1 Test Fixtures

All tests create fixtures programmatically (no checked-in binary files) to avoid cross-platform encoding issues:

```rust
const LATIN1_BYTES: &[u8] = b"valid\nbad: \xf3\nnext\n";
const CRLF_BYTES: &[u8] = b"line1\r\nline2\r\n";
const BARE_CR_BYTES: &[u8] = b"line1\rline2\r";
const NO_NEWLINE_BYTES: &[u8] = b"no newline";
const EMPTY_BYTES: &[u8] = b"";
const ALL_INVALID_BYTES: &[u8] = b"\xff\xfe\xfd";
const MIXED_INVALID_BYTES: &[u8] = b"hello\n\xffworld\n";
const UTF8_WITH_BOM: &[u8] = b"\xef\xbb\xbfhello\nworld\n";
```

### 7.2 Unit Tests (`tests/lossy_lines_tests.rs` or inline in `src/client.rs`)

Test the private `lossy_lines` helper directly using `std::io::Cursor`:

| # | Test Name | Input | Expected Result |
|---|-----------|-------|-----------------|
| 1 | `test_lossy_lines_basic` | `b"line1\nline2\n"` | 2 lines: `"line1"`, `"line2"` |
| 2 | `test_lossy_lines_latin1` | `LATIN1_BYTES` | 3 lines: `"valid"`, `"bad: \u{FFFD}"`, `"next"` |
| 3 | `test_lossy_lines_crlf` | `CRLF_BYTES` | 2 lines: `"line1"`, `"line2"` (no trailing `\r`) |
| 4 | `test_lossy_lines_bare_cr` | `BARE_CR_BYTES` | 2 lines: `"line1\r"`, `"line2\r"` (bare `\r` preserved) |
| 5 | `test_lossy_lines_no_trailing_newline` | `NO_NEWLINE_BYTES` | 1 line: `"no newline"` |
| 6 | `test_lossy_lines_empty` | `EMPTY_BYTES` | 0 lines |
| 7 | `test_lossy_lines_all_invalid` | `ALL_INVALID_BYTES` | 1 line: `"\u{FFFD}\u{FFFD}\u{FFFD}"` |
| 8 | `test_lossy_lines_mixed_invalid` | `MIXED_INVALID_BYTES` | 2 lines: `"hello"`, `"\u{FFFD}world"` |
| 9 | `test_lossy_lines_utf8_bom` | `UTF8_WITH_BOM` | 2 lines: BOM preserved in first line `"\u{FEFF}hello"` |
| 10 | `test_lossy_lines_single_long_line` | `b"x"` repeated 1MB + `b"\n"` | 1 line, 1MB of `"x"` |

### 7.3 Integration Tests (`tests/basic_integration.rs` additions)

Test the public API surface via temp files:

| # | Test Name | API Under Test | Input | Expected Result |
|---|-----------|----------------|-------|-----------------|
| 11 | `test_read_lines_lossy_latin1` | `oneio::read_lines_lossy` | Temp file with `LATIN1_BYTES` | 3 lines, middle contains `\u{FFFD}` |
| 12 | `test_read_lines_lossy_crlf` | `oneio::read_lines_lossy` | Temp file with `CRLF_BYTES` | 2 lines, no `\r` |
| 13 | `test_read_lines_lossy_empty` | `oneio::read_lines_lossy` | Empty temp file | 0 lines |
| 14 | `test_read_lines_lossy_continuation` | `oneio::read_lines_lossy` | `LATIN1_BYTES` + more valid lines after | All lines yielded, not truncated at bad byte |
| 15 | `test_read_to_string_lossy_latin1` | `oneio::read_to_string_lossy` | Temp file with `LATIN1_BYTES` | String containing `\u{FFFD}` |
| 16 | `test_read_to_string_lossy_valid_utf8` | `oneio::read_to_string_lossy` | Valid UTF-8 file | Exact same content as `read_to_string` |
| 17 | `test_read_to_bytes_roundtrip` | `oneio::read_to_bytes` | Temp file with `LATIN1_BYTES` | Exact `LATIN1_BYTES` returned |
| 18 | `test_read_to_bytes_empty` | `oneio::read_to_bytes` | Empty temp file | Empty `Vec<u8>` |
| 19 | `test_read_lines_strict_still_fails` | `oneio::read_lines` | Temp file with `LATIN1_BYTES` | `Err(InvalidData)` on second `next()` |
| 20 | `test_read_to_string_strict_still_fails` | `oneio::read_to_string` | Temp file with `LATIN1_BYTES` | `Err(InvalidData)` |
| 21 | `test_client_read_lines_lossy` | `OneIo::read_lines_lossy` | Via `OneIo::new()` client | Same as test 11 |
| 22 | `test_client_to_lines_lossy` | `OneIo::to_lines_lossy` | Via `get_reader` + `to_lines_lossy` | Same as test 11 |
| 23 | `test_client_read_to_bytes` | `OneIo::read_to_bytes` | Via `OneIo::new()` client | Same as test 17 |

### 7.4 Async Tests (`tests/async_integration.rs` additions)

| # | Test Name | API Under Test | Input | Expected Result |
|---|-----------|----------------|-------|-----------------|
| 24 | `test_read_to_string_lossy_async_latin1` | `oneio::read_to_string_lossy_async` | Temp file with `LATIN1_BYTES` | String containing `\u{FFFD}` |
| 25 | `test_read_to_bytes_async_roundtrip` | `oneio::read_to_bytes_async` | Temp file with `LATIN1_BYTES` | Exact `LATIN1_BYTES` returned |
| 26 | `test_read_to_string_async_strict_still_fails` | `oneio::read_to_string_async` | Temp file with `LATIN1_BYTES` | `Err(InvalidData)` |

### 7.5 CLI Tests

| # | Test Name | Command | Input | Expected Result |
|---|-----------|---------|-------|-----------------|
| 27 | `cli_lossy_default` | `cargo run -- --stats file_with_0xf3` | File with Latin-1 byte | Prints line count, exits 0 |
| 28 | `cli_lossy_output` | `cargo run -- file_with_0xf3` | File with Latin-1 byte | Prints all lines with `\u{FFFD}` in output, exits 0 |
| 29 | `cli_strict_utf8_fails` | `cargo run -- --strict-utf8 --stats file_with_0xf3` | File with Latin-1 byte | Exits with non-zero code |
| 30 | `cli_strict_utf8_success` | `cargo run -- --strict-utf8 --stats valid_utf8_file` | Valid UTF-8 file | Prints line count, exits 0 |
| 31 | `cli_crlf_handling` | `cargo run -- file_with_crlf` | File with `\r\n` | Output lines do not contain `\r` |
| 32 | `cli_compression_with_lossy` | `cargo run -- file_with_0xf3.gz` | Gzipped file with Latin-1 | Decompresses and prints with `\u{FFFD}`, exits 0 |

### 7.6 Edge Case & Stress Tests

| # | Test Name | Scenario | Expected Result |
|---|-----------|----------|-----------------|
| 33 | `test_very_large_file` | 100MB file with scattered invalid bytes | Completes without OOM, all lines yielded |
| 34 | `test_concurrent_reads` | Multiple threads calling `read_lines_lossy` on different files | No data races, all iterators independent |
| 35 | `test_send_trait` | Verify `read_lines_lossy` result can be moved across threads | Compiles (required for `Send` usage) |
| 36 | `test_no_leak_on_early_drop` | Drop iterator after 1 line | No memory leak, file handle released |
| 37 | `test_lines_with_embedded_nul` | `b"hello\x00world\n"` | Line contains embedded NUL, lossy conversion not triggered (NUL is valid UTF-8) |
| 38 | `test_max_line_length` | Single line > BufReader capacity (8KB default) | Correctly reads full line, not truncated |

### 7.7 Deprecation & Compatibility Tests

| # | Test Name | Scenario | Expected Result |
|---|-----------|----------|-----------------|
| 39 | `test_deprecated_api_compiles` | Use `#[allow(deprecated)]` on `read_lines` call | Compiles without error |
| 40 | `test_zero_internal_deprecation_warnings` | Build with `--all-features` | No deprecation warnings from library's own code |
| 41 | `test_new_api_no_deprecation` | Use `read_lines_lossy` | No deprecation warning |

### 7.8 Test Infrastructure

**Helper function for temp files:**
```rust
fn write_temp(bytes: &[u8]) -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file
}
```

**CI integration:**
- All new tests run with `cargo test --all-features`
- Encoding tests run on Linux, macOS, and Windows CI runners
- CLI tests use shell scripts in `.github/workflows/cli-tests.sh`

## 8. Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Deprecation warnings in external dependents | High | Clear deprecation messages with direct replacement names |
| `read_to_string_lossy` allocates entire file into memory | Low | Same cost as `String::from_utf8_lossy`; document streaming recommendation |
| Test fixture encoding issues on Windows | Low | Write test fixtures in code (hex literals), don't check in binary files |
| `impl Iterator` helper diverges from future `BufRead::lines()` behavior | Low | `lines()` semantics are stable; tests lock expected behavior |

## 9. Design Discussion: Should We Expect Unicode?

A central question: is `read_lines` failing on non-UTF-8 actually a bug, or a feature?

### Arguments for strict UTF-8 (current behavior)

- **Rust `String` requires UTF-8** — Any non-UTF-8 byte that slips through is technically invalid `String` data
- **Data quality validation** — For JSON, configs, or protocol text, invalid UTF-8 genuinely means corrupted/malformed input
- **Caller expectations** — Some users may rely on `read_lines` as an implicit validation step; silently lossy conversion could hide data corruption
- **HTTP content-type** — Servers often declare `charset=utf-8`; failure on invalid bytes honors that contract

### Arguments against strict UTF-8 (the issue #72 perspective)

- **Real-world data is messy** — RIPE IRR (`0xf3` Latin-1), legacy logs (CP1252), mail archives (ISO-8859-1), scraped HTML frequently contain non-UTF-8 bytes
- **No encoding metadata in compressed files** — `.gz`, `.bz2`, `.zst` provide no charset information; assuming UTF-8 is just a guess
- **Failure mode is unhelpful** — Stopping mid-stream with `InvalidData` gives the caller no way to recover or salvage the remaining 99% of valid data
- **OneIO is a general I/O library, not a validator** — Its job is to get bytes from A to B; strictness is the caller's concern if they need it
- **100% of `read_lines` usage in this codebase** — Used in `for` loops or `.map().collect()`; none store the iterator or depend on strictness for validation

### Conclusion

For a general-purpose I/O library, **assuming Unicode is not safe**. The default should tolerate non-UTF-8 data (via lossy replacement) because:
1. The failure case (stopping mid-stream) is worse than the replacement case (`U+FFFD` in one field)
2. Callers who need strict validation can opt in via the (now deprecated but functional) `read_lines` or via `--strict-utf8` in the CLI
3. This matches the Rust ecosystem convention: `String::from_utf8_lossy` exists precisely because real-world bytes are not always valid UTF-8

The deprecation of strict APIs signals that lossy tolerance is the preferred default for general I/O, while keeping strict paths available for callers who need validation.

## 10. Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2025-05-12 | Add lossy variants | Real-world data (RIPE IRR, logs) contains non-UTF-8 bytes |
| 2025-05-12 | Deprecate rather than change default | Backward compatibility; callers may rely on strict UTF-8 validation |
| 2025-05-12 | No encoding detection dependency | Out of scope; `String::from_utf8_lossy` is the standard Rust lossy conversion |
| 2025-05-12 | CLI defaults to lossy without a flag | CLI is a data pipeline tool, hard failure on encoding is unexpected |
| 2025-05-12 | `impl Iterator` via `from_fn` over public struct or `Box<dyn Iterator>` | Minimal API surface — no new public types, no dynamic dispatch, correct CRLF |
| 2025-05-12 | `read_until` over `split(b'\n')` | `split` leaves `\r` on CRLF and allocates twice per line |
| 2025-05-12 | `--strict-utf8` CLI flag | Provides escape hatch for users who need strict validation in the CLI |
