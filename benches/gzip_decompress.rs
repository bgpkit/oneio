mod common;

use std::hint::black_box;
use std::io::{Read, Write};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

#[cfg(feature = "gz-miniz")]
const GZIP_BACKEND: &str = "miniz_oxide";
#[cfg(all(not(feature = "gz-miniz"), feature = "gz-zlib-rs"))]
const GZIP_BACKEND: &str = "zlib-rs";
#[cfg(all(
    not(feature = "gz-miniz"),
    not(feature = "gz-zlib-rs"),
    feature = "gz-zlib-ng"
))]
const GZIP_BACKEND: &str = "zlib-ng";
#[cfg(all(
    not(feature = "gz-miniz"),
    not(feature = "gz-zlib-rs"),
    not(feature = "gz-zlib-ng"),
    feature = "gz-zlib-cloudflare"
))]
const GZIP_BACKEND: &str = "cloudflare-zlib";

fn build_gzip_fixture() -> (Vec<u8>, usize, String) {
    let corpus = common::build_text_corpus();
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&corpus).unwrap();
    let compressed = encoder.finish().unwrap();
    let fixture = common::write_fixture(&format!("gzip-{GZIP_BACKEND}.txt.gz"), &compressed);

    (
        compressed,
        corpus.len(),
        fixture.to_string_lossy().into_owned(),
    )
}

fn bench_gzip_decompress(c: &mut Criterion) {
    let (input, output_len, fixture_path) = build_gzip_fixture();

    let mut group = c.benchmark_group("gzip_decompress");
    group.throughput(Throughput::Bytes(output_len as u64));

    group.bench_function(format!("raw_decoder/{GZIP_BACKEND}"), |b| {
        b.iter(|| {
            let mut reader = GzDecoder::new(input.as_slice());
            let mut out = Vec::with_capacity(output_len);
            reader.read_to_end(&mut out).unwrap();
            black_box(out.len())
        })
    });

    group.bench_function(format!("oneio_get_reader/{GZIP_BACKEND}"), |b| {
        b.iter(|| {
            let mut reader = oneio::get_reader(&fixture_path).unwrap();
            let mut out = Vec::with_capacity(output_len);
            reader.read_to_end(&mut out).unwrap();
            black_box(out.len())
        })
    });

    group.finish();
}

criterion_group!(benches, bench_gzip_decompress);
criterion_main!(benches);
