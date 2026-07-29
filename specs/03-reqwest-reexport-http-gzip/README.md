# Spec: reqwest Re-export and Opt-in HTTP gzip

**Status**: Complete
**Author**: Mingwei Zhang
**Created**: 2026-07-22
**Target Branch**: `dev/reqwest-reexport-http-gzip`

## 1. Overview

Re-export `reqwest` so downstream crates can name the HTTP types exposed through
oneio's public API, and add an opt-in `reqwest-gzip` feature that enables
reqwest's transparent gzip content-encoding support.

**Non-goals:**
- Changing any default behavior — the gzip feature is off unless explicitly enabled
- Brotli/deflate content-encoding — can follow the same pattern later if requested
- Replacing oneio's suffix-based file decompression (`gz` family) — orthogonal concern
- Wrapping or abstracting reqwest's API — the goal is to expose it, not hide it

**Success criteria:**
- [ ] `oneio::reqwest` resolves when the `http` feature is enabled (e.g.
      `oneio::reqwest::StatusCode::NOT_MODIFIED` compiles downstream)
- [ ] With `reqwest-gzip`, requests advertise `Accept-Encoding: gzip` and
      `Content-Encoding: gzip` responses are transparently decoded
- [ ] Without `reqwest-gzip`, dependency tree is unchanged (no gzip decoder crates)
- [ ] Conditional-GET workflow (send `If-None-Match`, read `304` status and
      `ETag`/`Last-Modified` response headers) is possible using only oneio's API
- [ ] All quality gates pass: fmt, build, test, clippy (`-D warnings`)

## 2. Current State

`OneIo::get_http_reader_raw()` (since 0.21) returns `reqwest::blocking::Response`
in its public signature, and `OneIo::builder()` / `OneIo::http_client()` expose
reqwest types as well. However, reqwest is an internal dependency: downstream
crates cannot `use reqwest::...` unless they declare reqwest themselves, which
risks version skew (two reqwest major versions in the tree; the `Response` they
receive is not the `Response` they imported). Working around it via type
inference (`.status().as_u16() == 304`) is fragile and implicit.

Separately, oneio's reqwest build enables `blocking`, `http2`, `charset`,
`stream` — but not `gzip`. Requests therefore do not advertise
`Accept-Encoding: gzip`, and `Content-Encoding: gzip` responses are not decoded.
oneio's own decompression is inferred from the URL suffix, so payload-gzipped
endpoints like `https://rpki.cloudflare.com/rpki.json` (~97 MB plain, ~4.6 MB
gzipped) cannot benefit.

Concrete downstream need: bgpkit-commons#33 (conditional RPKI loading) currently
adds a direct reqwest dependency solely to name these types and enable gzip.

## 3. Proposed Solution

### Re-export

```rust
#[cfg(feature = "http")]
pub use reqwest;
```

Downstream then uses `oneio::reqwest::{blocking::Client, StatusCode, header}`
guaranteed to be the exact reqwest oneio was built against.

**Key decision — full re-export vs curated module:** full `pub use reqwest`
(chosen) is simpler and the type already leaks via `get_http_reader_raw`; a
curated `oneio::http` module would not actually reduce the semver exposure.

**Semver implication (documented in the PR):** reqwest becomes part of oneio's
public API contract. A future reqwest 0.13 upgrade becomes a breaking oneio
change (same trade-off as tokio↔bytes). Acceptable: oneio is pre-1.0 and
already exposes reqwest types in signatures.

### Feature flag

```toml
reqwest-gzip = ["http", "reqwest/gzip"]
```

reqwest's `gzip` feature is purely additive: it advertises
`Accept-Encoding: gzip` and transparently decodes gzipped responses. No oneio
code changes or `#[cfg]` gating are required — the flag only forwards.

**Key decision — naming:** `reqwest-gzip` (not `gzip`) to avoid confusion with
the existing `gz`/`gz-*` family, which is suffix-based file decompression.
Pattern extends naturally to `reqwest-brotli` / `reqwest-deflate` later.

## 4. Implementation Plan

1. **Cargo.toml**: add `reqwest-gzip = ["http", "reqwest/gzip"]` feature.
   Acceptance: `cargo tree --no-default-features --features reqwest-gzip` shows
   a gzip decoder backend; without the feature it does not.
2. **src/lib.rs**: add the gated re-export with doc comment; add `reqwest-gzip`
   row to the feature table in the crate docs.
   Acceptance: doc test using `oneio::reqwest::StatusCode` compiles.
3. **tests/**: gzip integration test (mock server, gzipped body with
   `Content-Encoding: gzip`, assert transparent decode) gated on
   `reqwest-gzip`; conditional-GET example test using the re-export.
   Acceptance: tests pass under `--features reqwest-gzip` and are absent/skipped
   otherwise.
4. **Docs/CI/changelog**: README feature mention, CI build/clippy legs for the
   new feature, CHANGELOG entry under Unreleased.
   Acceptance: CI matrix covers `reqwest-gzip`.

## 5. Testing Strategy

- Integration test with in-process `TcpListener` mock server (pattern already
  used in `tests/basic_integration.rs`):
  - serve a gzip-compressed body (compressed with flate2, already in-tree via
    the `gz` features) with `Content-Encoding: gzip`; assert decoded content
    matches and the request carried `Accept-Encoding: gzip`
  - serve `304 Not Modified` after an initial `200` with `ETag`; assert the
    client can read status and validators via `oneio::reqwest` types
- Doc test on the re-export (compile-time check downstream usage works)

## 6. Risks

- **Semver coupling**: reqwest version becomes public API → documented in
  CHANGELOG and PR; mitigated by oneio being pre-1.0 with reqwest already in
  public signatures.
- **Feature unification surprises**: a downstream crate enabling
  `reqwest/gzip` itself would silently change oneio's wire behavior (gzip is
  additive) — this is standard Cargo feature semantics, noted in docs.
- **MSRV**: reqwest's gzip feature pulls `async-compression`/`flate2`; flate2
  is already a oneio dependency via the `gz` features, so MSRV is unaffected.

## 7. Decision Log

- 2026-07-22: Full `pub use reqwest` over curated re-export module (the type
  already leaks; curation adds maintenance without reducing exposure)
- 2026-07-22: Feature named `reqwest-gzip` to disambiguate from the
  suffix-based `gz` family; passthrough-only, no cfg-gated code
