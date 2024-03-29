use criterion::{criterion_group, criterion_main, Criterion};
use isal_rs::igzip::{self, read::Encoder};
use std::{fs, io};

fn get_data() -> std::result::Result<Vec<u8>, std::io::Error> {
    fs::read(format!("{}/test-data/html_x_4", env!("CARGO_MANIFEST_DIR")))
}

fn igzip_compress(c: &mut Criterion) {
    let data = get_data().unwrap();
    c.bench_function("igzip::compress", |b| {
        b.iter(|| {
            let _ = igzip::compress(&data, igzip::CompressionLevel::Three, true).unwrap();
        })
    });
}

fn igzip_compress_into(c: &mut Criterion) {
    let data = get_data().unwrap();
    let compressed_len = igzip::compress(&data, igzip::CompressionLevel::Three, true)
        .unwrap()
        .len();
    let mut compressed = vec![0; compressed_len];
    c.bench_function("igzip::compress_into", |b| {
        b.iter(|| {
            let _ =
                igzip::compress_into(&data, &mut compressed, igzip::CompressionLevel::Three, true)
                    .unwrap();
        })
    });
}

fn igzip_decompress(c: &mut Criterion) {
    let data = get_data().unwrap();
    let compressed = igzip::compress(&data, igzip::CompressionLevel::Three, true).unwrap();
    c.bench_function("igzip::decompress", |b| {
        b.iter(|| {
            let _ = igzip::decompress(&compressed).unwrap();
        })
    });
}

fn igzip_decompress_into(c: &mut Criterion) {
    let data = get_data().unwrap();
    let compressed = igzip::compress(&data, igzip::CompressionLevel::Three, true).unwrap();
    let mut decompressed = vec![0; data.len()];
    c.bench_function("igzip::decompress_into", |b| {
        b.iter(|| {
            let _ = igzip::decompress_into(&compressed, &mut decompressed).unwrap();
        })
    });
}

fn igzip_roundtrip(c: &mut Criterion) {
    let data = get_data().unwrap();
    c.bench_function("igzip::roundtrip", |b| {
        b.iter(|| {
            let compressed = igzip::compress(&data, igzip::CompressionLevel::Three, true).unwrap();
            let _ = igzip::decompress(&compressed).unwrap();
        })
    });
}

fn igzip_roundtrip_into(c: &mut Criterion) {
    let data = get_data().unwrap();
    let compressed_len = igzip::compress(&data, igzip::CompressionLevel::Three, true)
        .unwrap()
        .len();

    let mut decompressed = vec![0; data.len()];
    let mut compressed = vec![0; compressed_len];

    c.bench_function("igzip::roundtrip_into", |b| {
        b.iter(|| {
            let _ =
                igzip::compress_into(&data, &mut compressed, igzip::CompressionLevel::Three, true)
                    .unwrap();
            let _ = igzip::decompress_into(&compressed, &mut decompressed).unwrap();
        })
    });
}

fn igzip_encoder(c: &mut Criterion) {
    let data = get_data().unwrap();
    c.bench_function("igzip::read::Encoder", |b| {
        b.iter(|| {
            let mut output = vec![];
            let mut encoder = Encoder::new(data.as_slice(), igzip::CompressionLevel::Three, true);
            let _ = io::copy(&mut encoder, &mut output).unwrap();
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(1_000);
    targets =
        igzip_compress,
        igzip_compress_into,
        igzip_decompress,
        igzip_decompress_into,
        igzip_roundtrip,
        igzip_roundtrip_into,
        igzip_encoder
}
criterion_main!(benches);
