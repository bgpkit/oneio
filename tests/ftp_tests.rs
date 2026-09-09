//! FTP integration tests against a real, public FTP server.
//!
//! These tests require network access and run against `ftp.radb.net` (RADB),
//! the same host that bgpkit-commons uses for IRR database downloads.
//! They are `#[ignore]`d by default; run manually with:
//!
//! ```bash
//! cargo test --test ftp_tests --features ftp -- --ignored --nocapture
//! ```

use std::io::Read;

/// Stream the first bytes of a real file over anonymous FTP and verify that
/// data actually flows through the suppaftp-based reader.
#[test]
#[ignore]
fn read_radb_file_header() {
    // radb.db.gz is the full RADB IRR database dump (large). We only read the
    // first 64 bytes to verify connect + login + RETR streaming without
    // downloading the whole file.
    let mut reader = oneio::get_reader("ftp://ftp.radb.net/radb/dbase/radb.db.gz")
        .expect("failed to open FTP reader");
    let mut buf = [0u8; 64];
    let n = reader
        .read(&mut buf)
        .expect("failed to read from FTP stream");
    assert!(n > 0, "expected at least some bytes from the FTP server");

    let head = String::from_utf8_lossy(&buf[..n]);
    // RADB dumps start with IRR text (comments or aut-num objects); after gzip
    // decompression the first bytes must be printable ASCII, not binary.
    assert!(
        head.chars()
            .all(|c| c.is_ascii_graphic() || c.is_ascii_whitespace()),
        "unexpected binary data at start of decompressed stream: {head:?}"
    );
}
