# Spec: Concurrent S3 multipart upload

**Status**: Complete
**Author**: Mingwei Zhang
**Created**: 2026-10-23
**Target Branch**: `dev/s3-concurrent-multipart`

## 1. Overview

- Goal: make `oneio::s3_upload` upload multi-GB files at link speed instead of
  one part at a time, and stop a single stalled part request from costing the
  caller a 300 s timeout before anything is retried.
- Non-goals: async API, conditional writes, progress callbacks, changing
  `s3_upload`'s signature or the `S3Config` struct.
- Success criteria:
  - [x] 4.3 GB upload to Cloudflare R2 completes in well under the 6 minutes
        the sequential path needs (measured 90 s with 4 workers, 420 s with 1)
  - [x] part order in `CompleteMultipartUpload` is always part-number order
  - [x] one failing part aborts the multipart upload and returns that error
  - [x] `cargo fmt --check`, `cargo clippy --all-features -- -D warnings`,
        `cargo test --all-features` all pass
  - [x] no new dependency

## 2. Current State

`upload_multipart` uploads parts strictly sequentially, one 8 MiB part per
request, each with a hardcoded 300 s request timeout. Measured against
Cloudflare R2, one TCP stream caps at about 12 MiB/s while four concurrent
streams reach about 90 MiB/s, so the sequential loop leaves most of the link
idle. When a part request stalls,
nothing else is in flight, so the whole upload waits for the timeout.

## 3. Proposed Solution

- Keep the blocking design. Concurrency comes from `std::thread::scope`.
- N worker threads (default 4) share an `AtomicUsize` part counter; each worker
  opens its own `File` handle and reads only its own byte range, so there is no
  shared seek or buffer state.
- Each worker signs its own part URL and uploads with the existing retry
  helper; the ETag is written into a pre-sized slot `Vec<Option<String>>` at the
  part index, so the completion list is ordered by construction.
- On the first unrecoverable error a shared `AtomicBool` stops the other
  workers at their next part boundary; after all workers join, the multipart
  upload is aborted exactly once and the first error is returned.
- Tuning is read from the environment through cached `OnceLock`s, mirroring the
  existing `s3_retry_config` pattern, so no public struct or signature changes:
  - `ONEIO_S3_MAX_CONCURRENCY` (default 4, clamped to 1..=16)
  - `ONEIO_S3_REQUEST_TIMEOUT_SECS` (default 300, unchanged behavior)
- Memory: peak per upload is `concurrency * part_size` (4 x 8 MiB default).

## 4. Implementation Plan

1. Add `s3_upload_tuning()` (concurrency + request timeout) next to
   `s3_retry_config`; replace the `S3_UPLOAD_REQUEST_TIMEOUT` constant uses.
2. Rewrite `upload_multipart` around a scoped worker pool; extract
   `collect_part_etags` so ordering and missing-part handling are unit-testable.
3. Unit tests: concurrency clamp, ETag ordering, missing part error.
4. Verify against R2 with a 4.3 GB local file.

## 5. Testing Strategy

- Unit: pure helpers only (no network).
- Integration: `tests/s3_integration.rs` multipart cases (5 MB, 10 MB, 80 MB)
  already cover correctness against R2; they now exercise the concurrent path.
- Manual: 4.3 GB upload, compare wall clock with the sequential measurement
  (420 s) and check the object size with a HEAD (both runs stored exactly
  4,334,649,344 bytes).

## 6. Risks

- More sockets per upload: default 4, bounded, and only for multipart.
- A part failing late can leave other parts already uploaded: acceptable, the
  upload is aborted and S3 bills parts only until then.
- `thread::scope` panics propagate: workers return `Result` instead of panicking
  so the abort path always runs.
- Env-based tuning is cached: a test or embedding process cannot change it after
  the first upload, same as the existing retry settings.

## 7. Decision Log

- 2026-10-23: kept the blocking API; concurrency via `thread::scope` rather than
  adding tokio, because oneio's S3 path has no async runtime and pulling one in
  would be a much larger change than the problem requires.
- 2026-10-23: tuned via environment rather than new `S3Config` fields, because
  `S3Config` has public fields and adding one breaks struct literals.
- 2026-10-23: no per-part logging in the library; oneio has no logging facade in
  library code today, so callers time the whole call instead.
- 2026-10-23: four workers by default, 16 as the ceiling; a failing part is
  reported after the other workers stop at their next part boundary, so the
  caller never waits for parts queued behind a failure.
