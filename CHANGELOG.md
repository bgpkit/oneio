# CHANGELOG

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