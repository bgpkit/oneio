use std::fs;
use std::io::Write;
use std::path::PathBuf;

const TARGET_CORPUS_SIZE: usize = 16 * 1024 * 1024;

pub fn build_text_corpus() -> Vec<u8> {
    let mut data = Vec::with_capacity(TARGET_CORPUS_SIZE);
    let mut seq = 0_u64;

    while data.len() < TARGET_CORPUS_SIZE {
        writeln!(
            &mut data,
            "{seq},AS{:05},AS{:05},peer=route-views.eqix,next-hop=192.0.2.{},med={},local-pref={},community={}:{}",
            (seq % 64512) + 100,
            ((seq * 7) % 64512) + 100,
            (seq % 254) + 1,
            seq % 1000,
            100 + (seq % 200),
            64512 + (seq % 64),
            100 + (seq % 4096)
        )
        .unwrap();
        seq += 1;
    }

    data.truncate(TARGET_CORPUS_SIZE);
    data
}

pub fn write_fixture(name: &str, bytes: &[u8]) -> PathBuf {
    let fixture_dir = PathBuf::from("target/bench-fixtures");
    fs::create_dir_all(&fixture_dir).unwrap();

    let path = fixture_dir.join(name);
    fs::write(&path, bytes).unwrap();
    path
}
