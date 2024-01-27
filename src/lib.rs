//! OneIO creates file reader and writer that just works.
//!
//! ## Usage and Feature Flags
//!
//! Enable all compression algorithms, and handle remote files (default)
//! ```toml
//! oneio = "0.15"
//! ```
//!
//! Select from supported feature flags
//! ```toml
//! oneio = {version = "0.15", features = ["remote", "gz"]}
//! ```
//!
//! Supported feature flags:
//! - `all` (default): all flags (`["gz", "bz", "lz", "remote", "json"]`)
//! - `remote`: allow reading from remote files, including http(s) and ftp
//! - `remote-rustls`: use `rustls` instead of `native-tls` for remote files via https
//! - `rustls`: use `rustls` instead of `native-tls` for remote files via https or S3, if either are enabled
//! - `gz`: support `gzip` files
//! - `bz`: support `bzip2` files
//! - `lz`: support `lz4` files
//! - `json`: allow reading JSON content into structs directly
//!
//! ## Reader
//!
//! The returned reader implements BufRead, and handles decompression from the following types:
//! - `gzip`: files ending with `gz` or `gzip`
//! - `bzip2`: files ending with `bz` or `bz2`
//! - `lz4`: files ending with `lz4` or `lz`
//!
//! It also handles reading from remote or local files transparently.
//!
//! ### Examples
//!
//! Read all into string:
//! ```rust
//! const TEST_TEXT: &str = "OneIO test file.
//! This is a test.";
//!
//! let mut reader = oneio::get_reader("https://spaces.bgpkit.org/oneio/test_data.txt.gz").unwrap();
//!
//! let mut text = "".to_string();
//! reader.read_to_string(&mut text).unwrap();
//! assert_eq!(text.as_str(), TEST_TEXT);
//! ```
//!
//! Read into lines:
//! ```rust
//! use std::io::BufRead;
//! use std::io::BufReader;
//! const TEST_TEXT: &str = "OneIO test file.
//! This is a test.";
//!
//! let lines = oneio::read_lines("https://spaces.bgpkit.org/oneio/test_data.txt.gz").unwrap()
//!     .map(|line| line.unwrap()).collect::<Vec<String>>();
//!
//! assert_eq!(lines.len(), 2);
//! assert_eq!(lines[0].as_str(), "OneIO test file.");
//! assert_eq!(lines[1].as_str(), "This is a test.");
//! ```
//!
//! Cache data first, then read
//! ```rust
//! use std::io::BufRead;
//! use std::io::BufReader;
//! const TEST_TEXT: &str = "OneIO test file.
//! This is a test.";
//!
//! let reader = BufReader::new(oneio::get_cache_reader("https://spaces.bgpkit.org/oneio/test_data.txt.gz", "/tmp/oneio/cache/", None, false).unwrap());
//! let lines = reader.lines().into_iter().map(|line| line.unwrap()).collect::<Vec<String>>();
//!
//! assert_eq!(lines.len(), 2);
//! assert_eq!(lines[0].as_str(), "OneIO test file.");
//! assert_eq!(lines[1].as_str(), "This is a test.");
//!
//! let mut reader = oneio::get_reader("/tmp/oneio/cache/test_data.txt.gz").unwrap();
//! let mut text = "".to_string();
//! reader.read_to_string(&mut text).unwrap();
//! assert_eq!(text.as_str(), TEST_TEXT);
//! ```
//!
//! Read remote content with custom headers
//! ```no_run
//! use std::collections::HashMap;
//! let mut reader = oneio::get_remote_reader(
//!   "https://SOME_REMOTE_RESOURCE_PROTECTED_BY_ACCESS_TOKEN",
//!   HashMap::from([("X-Custom-Auth-Key".to_string(), "TOKEN".to_string())])
//! ).unwrap();
//! let mut text = "".to_string();
//! reader.read_to_string(&mut text).unwrap();
//! println!("{}", text);
//! ```
//!
//! ## Writer
//!
//! [get_writer] returns a generic writer that implements [Write], and handles decompression from the following types:
//! - `gzip`: files ending with `gz` or `gzip`
//! - `bzip2`: files ending with `bz` or `bz2`
//!
//! **Note: lz4 writer is not currently supported.**
//!
//! ### Example
//!
//! ```rust
//! let to_read_file = "https://spaces.bgpkit.org/oneio/test_data.txt.gz";
//! let to_write_file = "/tmp/test_write.txt.bz2";
//!
//! // read text from remote gzip file
//! let mut text = "".to_string();
//! oneio::get_reader(to_read_file).unwrap().read_to_string(&mut text).unwrap();
//!
//! // write the same text to a local bz2 file
//! let mut writer = oneio::get_writer(to_write_file).unwrap();
//! writer.write_all(text.as_ref()).unwrap();
//! drop(writer);
//!
//! // read from the newly-generated bz2 file
//! let mut new_text = "".to_string();
//! oneio::get_reader(to_write_file).unwrap().read_to_string(&mut new_text).unwrap();
//!
//! // compare the decompressed content of the remote and local files
//! assert_eq!(text.as_str(), new_text.as_str());
//! std::fs::remove_file(to_write_file).unwrap();
//! ```

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/bgpkit/assets/main/logos/icon-transparent.png",
    html_favicon_url = "https://raw.githubusercontent.com/bgpkit/assets/main/logos/favicon.ico"
)]

mod error;
mod oneio;

pub use error::OneIoError;

#[cfg(feature = "digest")]
pub use crate::oneio::digest::*;
#[cfg(feature = "remote")]
pub use crate::oneio::remote::*;
#[cfg(feature = "s3")]
pub use crate::oneio::s3::*;

pub use crate::oneio::*;
