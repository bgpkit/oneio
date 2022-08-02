use std::io::BufRead;
use oneio;

const TEST_TEXT: &str = "OneIO test file.
This is a test.";

fn test_read( file_path: &str ) {
    let mut reader = oneio::get_reader(file_path).unwrap();

    let mut text = "".to_string();
    reader.read_to_string(&mut text).unwrap();
    assert_eq!(text.as_str(), TEST_TEXT);

    let buf_reader = oneio::get_buf_reader(file_path).unwrap();
    let lines = buf_reader.lines().into_iter().map(|line| line.unwrap()).collect::<Vec<String>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].as_str(), "OneIO test file.");
    assert_eq!(lines[1].as_str(), "This is a test.");
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
