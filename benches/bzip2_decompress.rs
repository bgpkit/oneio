mod common;

use std::hint::black_box;
use std::io::{Read, Write};

use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use bzip2::Compression;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn build_bzip2_fixture() -> (Vec<u8>, usize, String) {
    let corpus = common::build_text_corpus();
    let mut encoder = BzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&corpus).unwrap();
    let compressed = encoder.finish().unwrap();
    let fixture = common::write_fixture("bzip2.txt.bz2", &compressed);

    (
        compressed,
        corpus.len(),
        fixture.to_string_lossy().into_owned(),
    )
}

fn bench_bzip2_decompress(c: &mut Criterion) {
    let (input, output_len, fixture_path) = build_bzip2_fixture();

    let mut group = c.benchmark_group("bzip2_decompress");
    group.throughput(Throughput::Bytes(output_len as u64));

    group.bench_function("raw_decoder", |b| {
        b.iter(|| {
            let mut reader = BzDecoder::new(input.as_slice());
            let mut out = Vec::with_capacity(output_len);
            reader.read_to_end(&mut out).unwrap();
            black_box(out.len())
        })
    });

    group.bench_function("oneio_get_reader", |b| {
        b.iter(|| {
            let mut reader = oneio::get_reader(&fixture_path).unwrap();
            let mut out = Vec::with_capacity(output_len);
            reader.read_to_end(&mut out).unwrap();
            black_box(out.len())
        })
    });

    group.finish();
}

criterion_group!(benches, bench_bzip2_decompress);
criterion_main!(benches);
