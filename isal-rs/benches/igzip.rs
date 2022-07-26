use criterion::{criterion_group, criterion_main, Criterion};
use isal_rs::igzip::{self, read::Encoder};
use std::{fs, io};

fn get_data() -> std::result::Result<Vec<u8>, std::io::Error> {
    fs::read(format!(
        "{}/../../pyrus-cramjam/benchmarks/data/html_x_4",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn igzip_compress(c: &mut Criterion) {
    let data = get_data().unwrap();
    c.bench_function("igzip::compress", |b| {
        b.iter(|| {
            let _ = igzip::compress(&data, igzip::CompressionLevel::Three, true).unwrap();
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

criterion_group!(benches, igzip_compress, igzip_encoder);
criterion_main!(benches);
