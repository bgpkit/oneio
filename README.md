# OneIO - all-in-one IO library for Rust

[![Rust](https://github.com/bgpkit/oneio/actions/workflows/rust.yml/badge.svg)](https://github.com/bgpkit/oneio/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/oneio)](https://crates.io/crates/oneio)
[![Docs.rs](https://docs.rs/oneio/badge.svg)](https://docs.rs/oneio)
[![License](https://img.shields.io/crates/l/oneio)](https://raw.githubusercontent.com/bgpkit/oneio/main/LICENSE)

OneIO is a Rust library that provides a unified IO interface for synchronously reading and writing
to and from data files from different sources and compressions.

### Usage and Feature Flags

Enable all compression algorithms and handle remote files (default)

```toml
oneio = "0.18"
```

Select from supported feature flags

```toml
oneio = { version = "0.18", default-features = false, features = ["remote", "gz"] }
```

Default flags include `lib-core` and `rustls`.

#### Core features: `lib-core`

`lib-core` core features include:

- `remote`: allow reading from remote files, including http(s) and ftp
    - `http`: support reading from http(s) remote files using `reqwest` crate
    - `ftp`: support reading from ftp remote files using `suppaftp` crate
- `compressions`: support all compression algorithms
    - `gz`: support `gzip` files using `flate2` crate
    - `bz`: support `bzip2` files using `bzip2` crate
    - `lz`: support `lz4` files using `lz4` crate
    - `xz`: support `xz` files using `xz2` crate (requires xz library installed)
    - `zstd`: support `zst` files using `zstd` crate
- `json`: allow reading JSON content into structs with `serde` and `serde_json`

#### TLS choice: `rustls` or `native-tls`

Users can choose between `rustls` or `native-tls` as their TLS library. We use `rustls` as the basic library.

Users can also choose to accept invalid certificates (not recommending) by setting `ONEIO_ACCEPT_INVALID_CERTS=true`
environment variable.

#### Optional features: `cli`, `s3`, `digest`

- `s3`: allow reading from AWS S3 compatible buckets
- `cli`: build commandline program `oneio`, uses the following features
    - `lib-core`, `rustls`, `s3` for core functionalities
    - `clap`, `tracing` for CLI basics
- `digest` for generating SHA256 digest string

#### Selecting some compression algorithms

Users can also manually opt-in to specific compression algorithms. For example, to work with only local `gzip`
and `bzip2` files:

```toml
oneio = { version = "0.18", default-features = false, features = ["gz", "bz"] }
```

### Use `oneio` commandline tool

OneIO comes with a commandline tool, `oneio`, that opens and reads local/remote files
to terminal and handles decompression automatically. This can be useful if you want to
read some compressed plain-text files from a local or remote source.

```
oneio reads files from local or remote locations with any compression

Usage: oneio [OPTIONS] [FILE] [COMMAND]

Commands:
  s3      S3-related subcommands
  digest  Generate SHA256 digest
  help    Print this message or the help of the given subcommand(s)

Arguments:
  [FILE]  file to open, remote or local

Options:
  -d, --download                 download the file to current directory, similar to run `wget`
  -o, --outfile <OUTFILE>        output file path
      --cache-dir <CACHE_DIR>    cache reading to specified directory
      --cache-force              force re-caching if local cache already exists
      --cache-file <CACHE_FILE>  specify cache file name
  -s, --stats                    read through the file and only print out stats
  -h, --help                     Print help
  -V, --version                  Print version
```

You can specify a data file location after `oneio`. The following command
prints out the raw HTML file from <https://bgpkit.com>.

```bash
oneio https://bgpkit.com
```

Here is another example of using `oneio` to read a remote compressed JSON file,
pipe it to `jq` and count the number of JSON objects in the array.

```bash
$ oneio https://data.bgpkit.com/peer-stats/as2rel-latest.json.bz2 | jq '.|length'
802861
```

You can also directly download a file with the `--download` (or `-d`) flag.

```bash
$ oneio -d https://archive.routeviews.org/route-views.amsix/bgpdata/2022.11/RIBS/rib.20221107.0400.bz2
file successfully downloaded to rib.20221107.0400.bz2

$ ls -lh rib.20221107.0400.bz2
-rw-r--r--  1 mingwei  staff   122M Nov  7 16:17 rib.20221107.0400.bz2

$ monocle parse rib.20221107.0400.bz2 |head -n5
A|1667793600|185.1.167.24|3214|0.0.0.0/0|3214 1299|IGP|185.1.167.24|0|0|3214:3001|NAG||
A|1667793600|80.249.211.155|61955|0.0.0.0/0|61955 50629|IGP|80.249.211.155|0|0||NAG||
A|1667793600|80.249.213.223|267613|0.0.0.0/0|267613 1299|IGP|80.249.213.223|0|0|5469:6000|NAG||
A|1667793600|185.1.167.62|212483|1.0.0.0/24|212483 13335|IGP|152.89.170.244|0|0|13335:10028 13335:19000 13335:20050 13335:20500 13335:20530 lg:212483:1:104|NAG|13335|108.162.243.9
A|1667793600|80.249.210.28|39120|1.0.0.0/24|39120 13335|IGP|80.249.210.28|0|0|13335:10020 13335:19020 13335:20050 13335:20500 13335:20530|AG|13335|141.101.65.254
```

### Use OneIO Reader as a Library

The returned reader implements BufRead, and handles decompression from the following types:

- `gzip`: files ending with `gz` or `gzip`
- `bzip2`: files ending with `bz` or `bz2`
- `lz4`: files ending with `lz4` or `lz`
- `xz`: files ending with `xz` or `xz2`
- `zstd`: files ending with `zst` or `zstd`

It also handles reading from remote or local files transparently.

#### Examples

Read all into string:

```rust
const TEST_TEXT: &str = "OneIO test file.
This is a test.";

let mut reader = oneio::get_reader("https://spaces.bgpkit.org/oneio/test_data.txt.gz").unwrap();
let mut text = "".to_string();
reader.read_to_string(&mut text).unwrap();
assert_eq!(text.as_str(), TEST_TEXT);
```

Read into lines:

```rust
use std::io::BufRead;

const TEST_TEXT: &str = "OneIO test file.
This is a test.";

let lines = oneio::read_lines("https://spaces.bgpkit.org/oneio/test_data.txt.gz").unwrap()
.map(|line| line.unwrap()).collect::<Vec<String>>();
assert_eq!(lines.len(), 2);
assert_eq!(lines[0].as_str(), "OneIO test file.");
assert_eq!(lines[1].as_str(), "This is a test.");
```

### Use OneIO Writer as a Library

[get_writer] returns a generic writer that implements [std::io::Write], and handles decompression from the following types:

- `gzip`: files ending with `gz` or `gzip`
- `bzip2`: files ending with `bz` or `bz2`

**Note: lz4 writer is not currently supported.**

#### Example

##### Common IO operations

```rust
let to_read_file = "https://spaces.bgpkit.org/oneio/test_data.txt.gz";
let to_write_file = "/tmp/test_write.txt.bz2";

// read text from remote gzip file
let mut text = "".to_string();
oneio::get_reader(to_read_file).unwrap().read_to_string(&mut text).unwrap();

// write the same text to a local bz2 file
let mut writer = oneio::get_writer(to_write_file).unwrap();
writer.write_all(text.as_ref()).unwrap();
drop(writer);

// read from the newly generated bz2 file
let mut new_text = "".to_string();
oneio::get_reader(to_write_file).unwrap().read_to_string(&mut new_text).unwrap();

// compare the decompressed content of the remote and local files
assert_eq!(text.as_str(), new_text.as_str());
std::fs::remove_file(to_write_file).unwrap();
```

##### Read remote content with custom headers

```rust
use std::collections::HashMap;
use reqwest::header::HeaderMap;

let client = oneio::create_client_with_headers([("X-Custom-Auth-Key", "TOKEN")]).unwrap();
let mut reader = oneio::get_http_reader(
  "https://SOME_REMOTE_RESOURCE_PROTECTED_BY_ACCESS_TOKEN",
  Some(client),
).unwrap();
let mut text = "".to_string();
reader.read_to_string(&mut text).unwrap();
println!("{}", text);
```

##### Download remote file to local directory

```rust
oneio::download(
    "https://data.ris.ripe.net/rrc18/2022.11/updates.20221107.2325.gz",
    "updates.gz",
    None
).unwrap();
```

##### S3-related operations (needs `s3` feature flag)

```rust
use oneio::s3::*;

// upload to S3
s3_upload("oneio-test", "test/README.md", "README.md").unwrap();

// read directly from S3
let mut content = String::new();
s3_reader("oneio-test", "test/README.md")
    .unwrap()
    .read_to_string(&mut content)
    .unwrap();
println!("{}", content);

// download from S3
s3_download("oneio-test", "test/README.md", "test/README-2.md").unwrap();

// get S3 file stats
let res = s3_stats("oneio-test", "test/README.md").unwrap();
dbg!(res);

// error if file does not exist
let res = s3_stats("oneio-test", "test/README___NON_EXISTS.md");
assert!(res.is_err());

// copy S3 file to a different location
let res = s3_copy("oneio-test", "test/README.md", "test/README-temporary.md");
assert!(res.is_ok());
assert_eq!(
    true,
    s3_exists("oneio-test", "test/README-temporary.md").unwrap()
);

// delete temporary copied S3 file
let res = s3_delete("oneio-test", "test/README-temporary.md");
assert!(res.is_ok());
assert_eq!(
    false,
    s3_exists("oneio-test", "test/README-temporary.md").unwrap()
);

// list S3 files
let res = s3_list("oneio-test", "test/", Some("/".to_string()), false).unwrap();

assert_eq!(
    false,
    s3_exists("oneio-test", "test/README___NON_EXISTS.md").unwrap()
);
assert_eq!(true, s3_exists("oneio-test", "test/README.md").unwrap());
```

## Built with ❤️ by BGPKIT Team

<a href="https://bgpkit.com"><img src="https://bgpkit.com/Original%20Logo%20Cropped.png" alt="https://bgpkit.com/favicon.ico" width="200"/></a>

## License

MIT
