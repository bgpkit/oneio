# Changelog

All notable changes to this project will be documented in this file.

## v0.15.10 - 2024-01-26

### Hot fix

- fixed issue where `oneio s3 list BUCKET PREFIX` command not showing files match the prefix unless they are on the same directory as the prefix
- fixed issue where running `oneio` without argument causing program to panic

## v0.15.9 - 2024-01-26

### Highlights

- add support for installing via [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
- `s3_list` now accepts a new forth parameter, `dirs` boolean flag, to allow returning only the matching directories
- add `oneio s3 list` and `oneio s3 upload` commands to the CLI

### Breaking changes

The signature of `s3_list` function is now changed to:
```rust
pub fn s3_list(
    bucket: &str,
    prefix: &str,
    delimiter: Option<String>,
    dirs: bool,
) -> Result<Vec<String>, OneIoError> {}
```
This includes changes of:
1. `delimiter` changed from `Option<&str>` to `Option<String>`
2. new `dirs` boolean flag to allow returning only matching directories in the specified prefix
    - the `delimiter` is also automatically forced to `Some("/".to_string())` if `dirs` is specified and `delimiter` is specified as `None`.
 
### Misc

- "cargo publish" process is now automated with GitHub actions
- test process now makes sure `s3` modules' doc-tests must compile

## v0.15.8 - 2023-12-18

### Highlights

* Added `vendored-openssl` flag to enable GitHub actions builds for different systems.
* GitHub releases automatically builds CLI binary for macOS (Universal), and linux (arm and amd64)

## v0.15.8-beta.1 - 2023-12-16

### Highlights

* GitHub actions uses vendored openssl instead of system openssl.

## v0.15.7 - 2023-12-16

### Highlights

* Module Refactoring: A dedicated module has been created for remote and utils. (ec80e0236170f13e9eec2450eeaa8334e255a1ee)
* Improvements in Caching Control: The HTTP caching is now controllable and disabled for CLI requests. (69de57c5f9a7003edecde2fe6641c438abe233a5)
* Improved Error Handling: We have improved error handling in line reads. The application no longer attempts to read further if a line read fails, preventing any stalls. (fd1352fa2cb701e3fb336a4b6f99014d76d64788)

## v0.15.6 - 2023-12-16

### Added

- support getting oneio reader directly by supplying an S3 URL: https://github.com/bgpkit/oneio/pull/31
