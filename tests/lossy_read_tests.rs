//! Tests for lossy UTF-8 reading and byte-perfect reads.
//!
//! These validate the `read_lines_lossy`, `read_to_string_lossy`, `read_to_bytes`,
//! and async variants added to handle real-world non-UTF-8 data.

use std::io::Write;
use std::path::PathBuf;

const LATIN1_BYTES: &[u8] = b"valid\nbad: \xf3\nnext\n";
const CRLF_BYTES: &[u8] = b"line1\r\nline2\r\n";
const BARE_CR_BYTES: &[u8] = b"line1\rline2\r";
const NO_NEWLINE_BYTES: &[u8] = b"no newline";
const EMPTY_BYTES: &[u8] = b"";
const ALL_INVALID_BYTES: &[u8] = b"\xff\xfe\xfd";
const MIXED_INVALID_BYTES: &[u8] = b"hello\n\xffworld\n";

fn write_temp(bytes: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "oneio_lossy_test_{}_{}.txt",
        std::process::id(),
        id
    ));
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    path
}

// ─────────────────────────────────────────────
// Unit-style tests via private helper
// ─────────────────────────────────────────────

#[test]
fn test_lossy_lines_basic() {
    let cursor = std::io::Cursor::new(b"line1\nline2\n");
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines, vec!["line1", "line2"]);
}

#[test]
fn test_lossy_lines_latin1() {
    let cursor = std::io::Cursor::new(LATIN1_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "valid");
    assert_eq!(lines[1], "bad: \u{FFFD}");
    assert_eq!(lines[2], "next");
}

#[test]
fn test_lossy_lines_crlf() {
    let cursor = std::io::Cursor::new(CRLF_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "line1");
    assert_eq!(lines[1], "line2");
    assert!(!lines[0].ends_with('\r'));
}

#[test]
fn test_lossy_lines_bare_cr() {
    // Files with bare \r and no \n are read as a single line (same as BufRead::lines())
    let cursor = std::io::Cursor::new(BARE_CR_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "line1\rline2\r");
}

#[test]
fn test_lossy_lines_no_trailing_newline() {
    let cursor = std::io::Cursor::new(NO_NEWLINE_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "no newline");
}

#[test]
fn test_lossy_lines_empty() {
    let cursor = std::io::Cursor::new(EMPTY_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert!(lines.is_empty());
}

#[test]
fn test_lossy_lines_all_invalid() {
    let cursor = std::io::Cursor::new(ALL_INVALID_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "\u{FFFD}\u{FFFD}\u{FFFD}");
}

#[test]
fn test_lossy_lines_mixed_invalid() {
    let cursor = std::io::Cursor::new(MIXED_INVALID_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "hello");
    assert_eq!(lines[1], "\u{FFFD}world");
}

#[test]
fn test_lossy_lines_continuation_past_bad_byte() {
    let cursor = std::io::Cursor::new(LATIN1_BYTES);
    let lines: Vec<String> = oneio::OneIo::new()
        .unwrap()
        .to_lines_lossy(Box::new(cursor))
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 3, "iterator should continue past bad byte");
}

// ─────────────────────────────────────────────
// Integration tests via public API
// ─────────────────────────────────────────────

#[test]
fn test_read_lines_lossy_latin1() {
    let path = write_temp(LATIN1_BYTES);
    let lines: Vec<String> = oneio::read_lines_lossy(path.to_str().unwrap())
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], "bad: \u{FFFD}");
}

#[test]
fn test_read_lines_lossy_crlf() {
    let path = write_temp(CRLF_BYTES);
    let lines: Vec<String> = oneio::read_lines_lossy(path.to_str().unwrap())
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines[0], "line1");
    assert!(!lines[0].ends_with('\r'));
}

#[test]
fn test_read_lines_lossy_empty() {
    let path = write_temp(EMPTY_BYTES);
    let lines: Vec<String> = oneio::read_lines_lossy(path.to_str().unwrap())
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(lines.is_empty());
}

#[test]
fn test_read_to_string_lossy_latin1() {
    let path = write_temp(LATIN1_BYTES);
    let content = oneio::read_to_string_lossy(path.to_str().unwrap()).unwrap();
    assert!(content.contains('\u{FFFD}'));
    assert!(content.contains("valid"));
    assert!(content.contains("next"));
}

#[test]
fn test_read_to_string_lossy_valid_utf8() {
    let valid = b"hello\nworld\n";
    let path = write_temp(valid);
    let lossy = oneio::read_to_string_lossy(path.to_str().unwrap()).unwrap();
    #[allow(deprecated)]
    let strict = oneio::read_to_string(path.to_str().unwrap()).unwrap();
    assert_eq!(lossy, strict, "lossy should match strict on valid UTF-8");
}

#[test]
fn test_read_to_bytes_roundtrip() {
    let path = write_temp(LATIN1_BYTES);
    let bytes = oneio::read_to_bytes(path.to_str().unwrap()).unwrap();
    assert_eq!(bytes, LATIN1_BYTES);
}

#[test]
fn test_read_to_bytes_empty() {
    let path = write_temp(EMPTY_BYTES);
    let bytes = oneio::read_to_bytes(path.to_str().unwrap()).unwrap();
    assert!(bytes.is_empty());
}

#[test]
fn test_client_read_lines_lossy() {
    let client = oneio::OneIo::new().unwrap();
    let path = write_temp(LATIN1_BYTES);
    let lines: Vec<String> = client
        .read_lines_lossy(path.to_str().unwrap())
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], "bad: \u{FFFD}");
}

#[test]
fn test_client_to_lines_lossy() {
    let client = oneio::OneIo::new().unwrap();
    let path = write_temp(LATIN1_BYTES);
    let reader = client.get_reader(path.to_str().unwrap()).unwrap();
    let lines: Vec<String> = client.to_lines_lossy(reader).map(|r| r.unwrap()).collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1], "bad: \u{FFFD}");
}

#[test]
fn test_client_read_to_bytes() {
    let client = oneio::OneIo::new().unwrap();
    let path = write_temp(LATIN1_BYTES);
    let bytes = client.read_to_bytes(path.to_str().unwrap()).unwrap();
    assert_eq!(bytes, LATIN1_BYTES);
}

// ─────────────────────────────────────────────
// Legacy strict behavior (deprecated but functional)
// ─────────────────────────────────────────────

#[test]
#[allow(deprecated)]
fn test_read_lines_strict_still_fails() {
    let path = write_temp(LATIN1_BYTES);
    let mut lines = oneio::read_lines(path.to_str().unwrap()).unwrap();
    assert_eq!(lines.next().unwrap().unwrap(), "valid");
    let second = lines.next().unwrap();
    assert!(
        second.is_err(),
        "strict read_lines should fail on Latin-1 byte"
    );
}

#[test]
#[allow(deprecated)]
fn test_read_to_string_strict_still_fails() {
    let path = write_temp(LATIN1_BYTES);
    let result = oneio::read_to_string(path.to_str().unwrap());
    assert!(
        result.is_err(),
        "strict read_to_string should fail on Latin-1 byte"
    );
}

// ─────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────

#[test]
fn test_lines_with_embedded_nul() {
    let bytes = b"hello\x00world\n";
    let path = write_temp(bytes);
    let lines: Vec<String> = oneio::read_lines_lossy(path.to_str().unwrap())
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "hello\x00world");
}

#[test]
fn test_send_trait() {
    let client = oneio::OneIo::new().unwrap();
    let path = write_temp(LATIN1_BYTES);
    let lines = client.read_lines_lossy(path.to_str().unwrap()).unwrap();
    // Move to another thread — should compile if Send
    let handle = std::thread::spawn(move || {
        let collected: Vec<String> = lines.map(|r| r.unwrap()).collect();
        assert_eq!(collected.len(), 3);
    });
    handle.join().unwrap();
}

#[test]
fn test_no_leak_on_early_drop() {
    let client = oneio::OneIo::new().unwrap();
    let path = write_temp(LATIN1_BYTES);
    let mut lines = client.read_lines_lossy(path.to_str().unwrap()).unwrap();
    // Read only first line, then drop
    assert_eq!(lines.next().unwrap().unwrap(), "valid");
    drop(lines);
    // If this doesn't hang or panic, the test passes
}
