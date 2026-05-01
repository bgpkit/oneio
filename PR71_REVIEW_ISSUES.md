# PR #71 Review Issues

PR: https://github.com/bgpkit/oneio/pull/71  
Title: feat: migrate S3 from rust-s3 to rusty-s3  
Review date: 2026-05-01

---

## Open Issues

### 1. `epoch_days_to_ymd` linear year loop

**File**: `src/s3/mod.rs:502–511`

The SigV4 timestamp helper iterates year-by-year from 1970 on every `s3_copy` call. A closed-form Gregorian formula would be simpler and remove the loop.

**Status**: Deferred — for dates near 2026, the loop is only ~56 iterations, which is negligible. Can be revisited if this becomes a hot path.

---

## Copilot Comments — Not Actionable

- **`rusty_s3::S3Action` unused import** (`mod.rs:31`): Copilot flagged this, but it is a trait import required to bring `.sign()` into scope for all action objects. The code compiles without warnings.
- **`quick-xml` API incompatibility** (`mod.rs:773`): Copilot suggested `read_event_into` / `unescape()`, but the current code (`read_event()` / `e.decode()`) compiles and passes tests against quick-xml 0.38.

---

## Resolved

All issues from earlier review rounds have been addressed:

### Round 6 (2026-05-01)

| Issue | Resolution |
|---|---|
| `s3_copy` empty host in non-default-port branch | Fixed: `unwrap_or("")` replaced with `ok_or_else` |
| `s3` feature forces `https` (rustls), blocking `native-tls` | Fixed: `s3` now depends on `http` instead of `https` |
| `s3_stats` silently defaults `content_length` to 0 | Fixed: returns `OneIoError::NotSupported` when header is missing/unparseable |
| Integration test temp files never deleted | Fixed: `create_temp_file` replaced with `TempFile` RAII wrapper that cleans up on drop |
| `std::mem::take` loses chunk buffer capacity | Fixed: `std::mem::replace(&mut chunk, Vec::with_capacity(...))` preserves capacity |
| `s3_copy` extra HEAD request before copy | Fixed: no longer present in current code — copy attempts directly |

### Earlier Rounds

| Issue | Resolution |
|---|---|
| `map_parsed_s3_error` all arms identical — XML error detail discarded | Fixed: `message`, `key`, `bucket_name` now used in error output |
| `chunk.clone()` copies 8MB per part | Fixed: replaced with `std::mem::take` |
| `authorization.clone()` unnecessary | Fixed: passed by value |
| `extract_etag` redundant `or_else("ETag")` fallback | Fixed: removed |
| `s3_exists` fragile string-prefix matching for 404 | Fixed: direct HTTP status match |
| `content_length == 0` rejects empty objects in `client.rs` | Fixed: returns `stats.content_length` directly |
| `upload_multipart` `file.read()` could return short reads | Fixed: `take(chunk_size).read_to_end()` |
| Chunk buffer re-allocated per iteration | Fixed: buffer declared outside loop, cleared per iteration |
| `dotenv` loaded on every S3 call | Fixed: guarded by `DOTENV_INIT: OnceLock<()>` |
| `Debug` leaks `secret_key` / `session_token` | Fixed: custom `Debug` impl redacts both |
| Missing rustls crypto provider init | Fixed: `ensure_default_provider()` called in `get_s3_client()` |
| Version not bumped for breaking changes | Will be bumped in separate release commit |
| CompleteMultipartUpload / CopyObject false-success on 200+embedded error | Fixed: body parsed for `<Error>` on HTTP 200 |
| Multipart upload leak on part failure | Fixed: `abort_multipart_upload` called on all failure paths |
| S3 client ignoring `ONEIO_CA_BUNDLE` / `ONEIO_ACCEPT_INVALID_CERTS` | Fixed: honored in `get_s3_client()` |
| `upload_single` buffering entire file in memory | Fixed: streams `File` directly |
| `s3_copy` dropping non-default ports | Fixed: includes port in `Host` header |
| VirtualHost breaking dotted bucket names | Fixed: falls back to path-style |

(End of file)
