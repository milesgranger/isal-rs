use criterion::{criterion_group, criterion_main, Criterion};
use isal_rs::igzip;
use std::fs;

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

criterion_group!(benches, igzip_compress);
criterion_main!(benches);
