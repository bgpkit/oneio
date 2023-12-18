# Changelog

All notable changes to this project will be documented in this file.

## v0.15.8-beta.2 - 2023-12-16

### Highlights
* GitHub actions uses vendored openssl instead of system openssl. 

## v0.15.8-beta.1 - 2023-12-16

### Highlights
* CLI application now depends on Rustls instead of native openssl. This will allow us to build a binaries for more platforms.

## v0.15.7 - 2023-12-16

### Highlights

* Module Refactoring: A dedicated module has been created for remote and utils. (ec80e0236170f13e9eec2450eeaa8334e255a1ee)
* Improvements in Caching Control: The HTTP caching is now controllable and disabled for CLI requests. (69de57c5f9a7003edecde2fe6641c438abe233a5)
* Improved Error Handling: We have improved error handling in line reads. The application no longer attempts to read further if a line read fails, preventing any stalls. (fd1352fa2cb701e3fb336a4b6f99014d76d64788)

## v0.15.6 - 2023-12-16

### Added

- support getting oneio reader directly by supplying an S3 URL: https://github.com/bgpkit/oneio/pull/31