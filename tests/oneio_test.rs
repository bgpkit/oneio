use std::collections::HashMap;
use std::io::{BufRead, Write};
use oneio;

const TEST_TEXT: &str = "OneIO test file.
This is a test.";

fn test_read( file_path: &str ) {
    let mut reader = oneio::get_reader(file_path).unwrap();

    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);

    let reader = oneio::get_reader(file_path).unwrap();
    let lines = reader.lines().into_iter().map(|line| line.unwrap()).collect::<Vec<String>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].as_str(), "OneIO test file.");
    assert_eq!(lines[1].as_str(), "This is a test.");
}

fn test_read_cache( file_path: &str ) {

    let cache_file_name = format!("./{}", file_path.split('/').collect::<Vec<&str>>().into_iter().last().unwrap().to_string());

    let _ = std::fs::remove_file(cache_file_name.as_str());

    // read from remote then cache
    let mut reader = oneio::get_cache_reader(file_path, "./", None, true).unwrap();
    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);
    drop(reader);

    // read directly from cache
    let mut reader = oneio::get_reader(cache_file_name.as_str()).unwrap();
    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);
    drop(reader);

    // read directly from remote
    let mut reader = oneio::get_reader(file_path).unwrap();
    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);
    drop(reader);
}

fn test_write ( to_write_file: &str , to_read_file: &str) {
    let mut text = "".to_string();
    oneio::get_reader(to_read_file).unwrap().read_to_string(&mut text).unwrap();

    let mut writer = oneio::get_writer(to_write_file).unwrap();
    writer.write_all(text.as_ref()).unwrap();
    drop(writer);

    let mut new_text = "".to_string();
    oneio::get_reader(to_write_file).unwrap().read_to_string(&mut new_text).unwrap();

    assert_eq!(text.as_str(), new_text.as_str());
    std::fs::remove_file(to_write_file).unwrap();
}


#[test]
fn test_reader_local() {
    test_read("tests/test_data.txt");
    test_read("tests/test_data.txt.gz");
    test_read("tests/test_data.txt.bz2");
    test_read("tests/test_data.txt.lz4");
}

#[test]
fn test_reader_remote() {
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt");
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.gz");
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.bz2");
    test_read("https://spaces.bgpkit.org/oneio/test_data.txt.lz4");
}

#[test]
fn test_reader_remote_with_header() {
    let mut reader = oneio::get_remote_reader(
        "https://bgp-datasets.radar-cfdata-org.workers.dev/caida/as2org/20050801.as-org2info.jsonl.gz",
        HashMap::from([("X-Custom-Auth-Key".to_string(), "vDe94ID5qAHC5YMtHdHexoyk7".to_string())])
    ).unwrap();

    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    println!("{}", text);
}

#[test]
fn test_writer() {
    test_write("tests/test_write_data.txt", "tests/test_data.txt");
    test_write("tests/test_write_data.txt.gz", "tests/test_data.txt.gz");
    test_write("tests/test_write_data.txt.bz2", "tests/test_data.txt.bz2");
    // lz4 writer is not currently supported
}

#[test]
fn test_cache_reader() {
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt");
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt.gz");
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt.bz2");
    test_read_cache("https://spaces.bgpkit.org/oneio/test_data.txt.lz4");
}