# AGENTS.md - oneio Project

Project-specific agent instructions for the oneio library.

---

## Project Overview

**oneio** is a Rust library providing unified I/O for compressed files from any source (local, HTTP, FTP, S3).

- **Repository**: https://github.com/bgpkit/oneio
- **Documentation**: https://docs.rs/oneio
- **License**: MIT

---

## Development Methodology: Spec-Driven Development

This project follows a **lightweight spec-driven development (SDD)** workflow optimized for single-developer/owner projects.

### When to Write a Spec

| Scenario | Spec Required | Location |
|----------|---------------|----------|
| New feature | Yes | `specs/[NN]-[feature-name]/README.md` |
| Bug fix (complex) | Optional | PR description |
| Bug fix (simple) | No | Commit message |
| Refactoring | Yes (if API changes) | `specs/` |
| Dependency updates | No | CHANGELOG.md entry |

### Spec Format

All specs live in `specs/[NN]-[feature-name]/README.md` where `[NN]` is a zero-padded number (01, 02, 03...).

**Required sections:**

```markdown
# Spec: [Feature Name]

**Status**: Draft | In Progress | Complete  
**Author**: [Name]  
**Created**: [Date]  
**Target Branch**: `dev/[feature-name]`

## 1. Overview
- Goal (one sentence)
- Non-goals (what we're NOT doing)
- Success criteria (checkboxes)

## 2. Current State
- What exists now
- What's broken/missing

## 3. Proposed Solution
- Architecture/design
- Key decisions

## 4. Implementation Plan
- Phases with acceptance criteria
- Estimated time per phase

## 5. Testing Strategy
- Unit tests
- Integration tests

## 6. Risks
- What could go wrong
- Mitigation

## 7. Decision Log
- Date: Decision (rationale)
```

### Spec Workflow

```
1. Write spec → 2. Review → 3. Approve → 4. Implement → 5. Verify → 6. Complete
```

**Review checkpoints:**
- After spec writing: Verify scope is clear
- After each phase: Verify acceptance criteria met
- Before merge: Verify against success criteria

---

## Code Style and Conventions

### Rust Style

- **Formatting**: `cargo fmt` (enforced in CI)
- **Linting**: `cargo clippy --all-features -- -D warnings`
- **Edition**: 2021
- **MSRV**: 1.70+ (be conservative)

### Naming Conventions

```rust
// Functions: verb_phrase
pub fn read_to_string(path: &str) -> Result<String>
pub fn s3_upload(bucket: &str, key: &str, file: &str) -> Result<()>

// Structs: PascalCase
pub struct OneIoBuilder
pub struct S3Config

// Constants: SCREAMING_SNAKE_CASE
const CHUNK_SIZE: usize = 8_388_608;

// Feature flags: lowercase with hyphens
// "s3", "native-tls", "async"
```

### Documentation

- **Public APIs**: All public items must have doc comments
- **Examples**: Include usage examples in doc comments
- **CHANGELOG.md**: Update for user-facing changes

```rust
/// Reads a file to string with automatic decompression.
///
/// # Examples
///
/// ```
/// let content = oneio::read_to_string("data.txt.gz")?;
/// ```
///
/// # Errors
///
/// Returns `OneIoError::NotFound` if file doesn't exist.
pub fn read_to_string(path: &str) -> Result<String, OneIoError> {
    // ...
}
```

---

## Testing Requirements

### Before Committing

Run these commands (also enforced in CI):

```bash
cargo fmt --check
cargo build --no-default-features
cargo build --all-features
cargo test --all-features
cargo clippy --all-features -- -D warnings
cargo clippy --no-default-features
```

### Test Organization

```
tests/
├── unit_tests.rs       # Unit tests for internal functions
├── integration_tests/  # Integration tests by feature
│   ├── http_tests.rs
│   ├── s3_tests.rs
│   └── compression_tests.rs
└── fixtures/           # Test data files
```

### Feature-Specific Testing

| Feature | Test Requirements |
|---------|-------------------|
| `s3` | Requires env vars; mark with `#[cfg(test)]` + `#[ignore]` |
| `http` | Use mock servers when possible |
| `ftp` | Requires FTP server; integration tests only |
| Compression | Test with fixture files |

---

## Feature Flags

### Current Flags

| Flag | Description | Dependencies |
|------|-------------|--------------|
| `default` | `gz` + `bz` + `https` | - |
| `gz` | Gzip support | flate2 |
| `bz` | Bzip2 support | bzip2 |
| `lz` | LZ4 support | lz4 |
| `xz` | XZ support | xz2 |
| `zstd` | Zstd support | zstd |
| `http` | HTTP support | reqwest |
| `https` | HTTPS support | reqwest + rustls |
| `ftp` | FTP support | suppaftp |
| `s3` | S3 support | rusty-s3 |
| `async` | Async I/O | tokio |
| `json` | JSON support | serde + serde_json |
| `digest` | Hashing support | ring |
| `cli` | Command-line tool | clap + tracing |

### Adding New Features

1. Add to `[features]` in Cargo.toml
2. Add to feature table in lib.rs docs
3. Gate code with `#[cfg(feature = "...")]`
4. Update CI to test new feature
5. Document in CHANGELOG.md

---

## Git Workflow

### Branch Naming

```
main                    # Production-ready
dev/[description]       # Feature branches
hotfix/[description]    # Urgent fixes
```

### Commit Messages

**Format:** `<type>: <description>`

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting (no code change)
- `refactor`: Code restructuring
- `perf`: Performance improvement
- `test`: Adding tests
- `chore`: Maintenance tasks

**Examples:**
```
feat: add S3 multipart upload progress callback
fix: correct gzip decompression for empty files
docs: update README with R2 configuration examples
refactor: extract HTTP client to shared module
```

### Commit Guidelines

- **Keep language factual**: Avoid words like "comprehensive", "extensive", "robust"
- **Good**: "Added R2 endpoint support"
- **Bad**: "Added comprehensive R2 endpoint support with robust error handling"
- **Never include**: Co-authored-by messages for AI agents

---

## Pull Request Process

### Before Creating PR

1. Run full test suite
2. Update CHANGELOG.md
3. Update documentation if needed
4. Review your own diff first

### PR Description Template

```markdown
## Summary
Brief description of changes

## Related Spec
Link to spec document if applicable

## Changes
- List of specific changes

## Testing
- How was this tested?

## Checklist
- [ ] Tests pass
- [ ] CHANGELOG.md updated
- [ ] Documentation updated
- [ ] Spec completed (if applicable)
```

---

## Error Handling

### OneIoError Conventions

```rust
pub enum OneIoError {
    // I/O errors
    Io(std::io::Error),
    
    // Protocol-specific
    NotSupported(String),
    Status { service: &'static str, code: u16 },
    
    // Feature not enabled
    #[cfg(not(feature = "s3"))]
    S3NotEnabled,
}
```

### Error Message Guidelines

- Be specific: `"File not found: {path}"` not `"File error"`
- Include context: `"S3 upload failed for {bucket}/{key}: {source}"`
- User-facing errors should suggest solutions

---

## Dependencies

### Adding New Dependencies

1. Check if already in tree: `cargo tree | grep <crate>`
2. Prefer small, focused crates
3. Check MSRV compatibility
4. Check license compatibility (MIT/Apache-2.0 preferred)
5. Update CHANGELOG.md under "Dependencies"

### Version Constraints

```toml
# Use caret for stable APIs
reqwest = "0.12"

# Pin for unstable or critical deps
rusty-s3 = "=0.9.1"

# Use range for flexibility (rarely)
rustls = ">=0.22, <0.24"
```

---

## Performance Considerations

### Memory

- Stream large files (don't buffer entirely)
- Use ` BufReader`/`BufWriter` for I/O
- Reuse HTTP clients (don't create per-request)

### CPU

- Compression: Use default compression levels
- Hashing: Use streaming for large files
- Avoid unnecessary string allocations

### Network

- Enable connection pooling (reqwest does this)
- Respect S3 multipart thresholds (5MB minimum)
- Use appropriate timeout values

---

## Security

### S3/Credentials

- Never log credentials
- Use environment variables or standard credential chains
- Support IAM roles where possible
- Zeroize credentials in memory when possible

### TLS

- Use rustls (not native-tls) by default
- Enable certificate verification
- Support custom CA certificates

### Input Validation

- Validate paths (no directory traversal)
- Validate URLs (parse, don't construct blindly)
- Sanitize user input in error messages

---

## Release Process

### Version Bumping

Follow [SemVer](https://semver.org/):
- **MAJOR**: Breaking API changes
- **MINOR**: New features (backward compatible)
- **PATCH**: Bug fixes

### Release Checklist

- [ ] Version bumped in Cargo.toml
- [ ] CHANGELOG.md updated with version header
- [ ] Git tag created: `git tag vX.Y.Z`
- [ ] Tests pass on CI
- [ ] Documentation builds
- [ ] crates.io publish: `cargo publish`

---

## Communication

### Questions/Decisions

Document significant decisions in:
1. Spec decision log (for features)
2. Code comments (for implementation details)
3. CHANGELOG.md (for user-facing changes)

### External Resources

- **Rust S3 APIs**: rusty-s3 docs, AWS S3 API reference
- **Compression**: flate2, bzip2, zstd crate docs
- **HTTP**: reqwest docs, hyper docs

---

## Tools and Commands

### Development

```bash
# Build with all features
cargo build --all-features

# Run specific test
cargo test --features s3 s3_upload

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-features -- -D warnings

# Generate docs
cargo doc --all-features --open
```

### Debugging

```bash
# Verbose logging
RUST_LOG=debug cargo run --bin oneio --features cli

# Trace-level S3 operations
RUST_LOG=trace cargo test --features s3 -- --nocapture
```

---

*Last updated: 2025-05-01*
