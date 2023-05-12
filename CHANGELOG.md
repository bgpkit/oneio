# CHANGELOG

## V0.10.1: Bug fix, S3 object exists

### New 

- [[#15](https://github.com/bgpkit/oneio/pull/15)]: fixes #14 where `.env` missing would trigger panic
- [[#16](https://github.com/bgpkit/oneio/pull/16)]: add `s3_exists` to check if an object exists in S3

## V0.10.0: S3 operations

### New

- [[#13](https://github.com/bgpkit/oneio/pull/13)]: Add S3-related functionalities

Example:
```rust
use oneio::{s3_download, s3_list, s3_reader, s3_stats, s3_upload};
use std::io::Read;

/// This example shows how to upload a file to S3 and read it back.
///
/// You need to set the following environment variables (e.g. in .env):
/// - AWS_ACCESS_KEY_ID
/// - AWS_SECRET_ACCESS_KEY
/// - AWS_REGION (e.g. "us-east-1") (use "auto" for Cloudflare R2)
/// - AWS_ENDPOINT
fn main() {
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

    // list S3 files
    let res = s3_list("oneio-test", "test/", Some("/")).unwrap();
    dbg!(res);
}
```

## V0.9.0: error on 4XX, 5XX codes

### Breaking change

- [[#11](https://github.com/bgpkit/oneio/pull/11)]: The remote file openers will now return an error if the remote file returns a 4XX or 5XX code.

## V0.8.1: `impl Send`, format, custom error

### Revision
- [[#10](https://github.com/bgpkit/oneio/pull/10)]: fix confusing `cache_file_name` vs `cache_file_path` issue

## V0.8.0: `impl Send`, format, custom error

### New
- [[#7](https://github.com/bgpkit/oneio/pull/7)]: add `impl Send` for all reader functions
  - from `Box<dyn Read>` to `Box<dyn Read + Send`
  - this allows the reader to be used across threads

### Refactor

- [[#8](https://github.com/bgpkit/oneio/pull/8)]: refactor custom Errors to use `thiserror` for implementation
- [[#9](https://github.com/bgpkit/oneio/pull/9)]: apply `rustfmt` and enforce formatting in CI checks



## V0.7.1: add `read_lines`

### New

- [[#6](https://github.com/bgpkit/oneio/pull/6)]: add `read_lines()` utility function

## V0.7.0: `Read` instead of `BufRead`

### Breaking change

- [[#5](https://github.com/bgpkit/oneio/pull/5)]: returns `Box<Read>` instead of `Box<BufRead>`



## V0.6.0: `read_to_string` and `read_json_struct`

### New

- [[#4](https://github.com/bgpkit/oneio/pull/4)]: add `read_to_string` and `read_json_struct` utility functions
  - `read_to_string(FILE_PATH)`: returns a String from a read file
  - `read_json_struct::<DataStruct>(FILE_PATH)`: returns a parsed user-provided `DataStruct` struct from read file

### Fixes

- [[#3](https://github.com/bgpkit/oneio/pull/3)]: fix build with `--no-default-features` (credits to [@yu-re-ka](https://github.com/yu-re-ka))
  - also added `cargo build --no-default-features` to CI build process to catch future issues like this
  - 


## V0.5.0: `download` function

## New 

- [[#2](https://github.com/bgpkit/oneio/pull/2)]: added `download` function to allow downloading a file directly

## V0.4.0: custom headers

## New

- [[994563c](https://github.com/bgpkit/oneio/commit/994563cb00b344ab94f1ee6617e574d689327c2e)]: added `get_remote_reader` function that allows specifying custom HTTP headers with a `HashMap<String, String>`



## V0.3.0: cached reader

## New

- [[#1](https://github.com/bgpkit/oneio/pull/1)]: added `get_cache_reader` to allow caching read content to a specified local directory