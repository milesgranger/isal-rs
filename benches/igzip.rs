use criterion::{criterion_group, criterion_main, Criterion};
use isal::igzip;
use rand::Rng;
use std::io::{self, Cursor};

use rand::seq::SliceRandom;
use rand::thread_rng;

// Generate some semi-random data
fn generate_random_sentence() -> Vec<u8> {
    let words = [
        "the",
        "quick",
        "brown",
        "fox",
        "jumps",
        "over",
        "the",
        "lazy",
        "dog",
        "hello",
        "world",
        "data",
        "compression",
        "test",
        "rust",
        "benchmark",
    ];
    let mut rng = thread_rng();
    // sentences 5 - 15 words long
    let sentence_length = 5 + (rng.gen_range(0..10));
    let sentence: Vec<&str> = (0..sentence_length)
        .map(|_| *words.choose(&mut rng).unwrap())
        .collect();
    (sentence.join(" ") + ".").as_bytes().to_vec()
}

fn get_data() -> Vec<u8> {
    (0..10_000)
        .map(|_| generate_random_sentence())
        .flatten()
        .collect::<Vec<u8>>()
}

fn gzip_read_encoder(c: &mut Criterion) {
    let data = get_data();
    c.bench_function("igzip::read::GzipEncoder", |b| {
        b.iter(|| {
            let mut output = vec![];
            let mut encoder =
                igzip::read::GzipEncoder::new(data.as_slice(), igzip::CompressionLevel::Three);
            let _ = io::copy(&mut encoder, &mut output).unwrap();
        })
    });
}
fn flate2_gzip_read_encoder(c: &mut Criterion) {
    let data = get_data();
    c.bench_function("flate2::read::GzEncoder", |b| {
        b.iter(|| {
            let mut output = vec![];
            let mut encoder =
                flate2::read::GzEncoder::new(data.as_slice(), flate2::Compression::best());
            let _ = io::copy(&mut encoder, &mut output).unwrap();
        })
    });
}

fn gzip_write_encoder(c: &mut Criterion) {
    let data = get_data();
    c.bench_function("igzip::write::GzipEncoder", |b| {
        b.iter(|| {
            let mut output = vec![];
            let mut encoder =
                igzip::write::GzipEncoder::new(&mut output, igzip::CompressionLevel::Three);
            let _ = io::copy(&mut Cursor::new(&data), &mut encoder).unwrap();
        })
    });
}
fn flate2_gzip_write_encoder(c: &mut Criterion) {
    let data = get_data();
    c.bench_function("flate2::write::GzEncoder", |b| {
        b.iter(|| {
            let mut output = vec![];
            let mut encoder =
                flate2::write::GzEncoder::new(&mut output, flate2::Compression::best());
            let _ = io::copy(&mut Cursor::new(&data), &mut encoder).unwrap();
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(50);
    targets =
        gzip_read_encoder,
        flate2_gzip_read_encoder,
        gzip_write_encoder,
        flate2_gzip_write_encoder
}
criterion_main!(benches);
