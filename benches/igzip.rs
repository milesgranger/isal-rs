use criterion::{criterion_group, criterion_main, Criterion};
use isal::igzip;
use std::io::{self, Cursor};

static DATA: [&'static [u8]; 25] = [
    include_bytes!("data/COPYING"),
    include_bytes!("data/alice29.txt"),
    include_bytes!("data/asyoulik.txt"),
    include_bytes!("data/fireworks.jpeg"),
    include_bytes!("data/geo.protodata"),
    include_bytes!("data/html"),
    include_bytes!("data/html_x_4"),
    include_bytes!("data/kppkn.gtb"),
    include_bytes!("data/lcet10.txt"),
    include_bytes!("data/Mark.Twain-Tom.Sawyer.txt"),
    include_bytes!("data/mr"),
    include_bytes!("data/ooffice"),
    include_bytes!("data/osdb"),
    include_bytes!("data/paper-100k.pdf"),
    include_bytes!("data/plrabn12.txt"),
    include_bytes!("data/sao"),
    include_bytes!("data/urls.10K"),
    include_bytes!("data/xml"),
    include_bytes!("data/x-ray"),
    include_bytes!("data/reymont"),
    include_bytes!("data/dickens"),
    include_bytes!("data/samba"),
    include_bytes!("data/nci"),
    include_bytes!("data/mozilla"),
    include_bytes!("data/webster"),
];

// Get a subset of the data, using all of it takes a very long time
// so can mix-match when in doubt or debugging. As of now, it selects
// a decent mix I think, last one being a larger file 'webster' @ ~40M
fn get_data() -> [&'static [u8]; 6] {
    [DATA[1], DATA[5], DATA[10], DATA[15], DATA[20], DATA[24]]
}

fn gzip_read_encoder(c: &mut Criterion) {
    let datas = get_data();
    c.bench_function("igzip::read::GzipEncoder", |b| {
        b.iter(|| {
            for data in datas {
                let mut output = vec![];
                let mut encoder =
                    igzip::read::GzipEncoder::new(data, igzip::CompressionLevel::Three);
                let _ = io::copy(&mut encoder, &mut output).unwrap();
            }
        })
    });
}
fn flate2_gzip_read_encoder(c: &mut Criterion) {
    let datas = get_data();
    c.bench_function("flate2::read::GzEncoder", |b| {
        b.iter(|| {
            for data in datas {
                let mut output = vec![];
                let mut encoder = flate2::read::GzEncoder::new(data, flate2::Compression::best());
                let _ = io::copy(&mut encoder, &mut output).unwrap();
            }
        })
    });
}

fn gzip_write_encoder(c: &mut Criterion) {
    let datas = get_data();
    c.bench_function("igzip::write::GzipEncoder", |b| {
        b.iter(|| {
            for data in datas {
                let mut output = vec![];
                let mut encoder =
                    igzip::write::GzipEncoder::new(&mut output, igzip::CompressionLevel::Three);
                let _ = io::copy(&mut Cursor::new(&data), &mut encoder).unwrap();
            }
        })
    });
}
fn flate2_gzip_write_encoder(c: &mut Criterion) {
    let datas = get_data();
    c.bench_function("flate2::write::GzEncoder", |b| {
        b.iter(|| {
            for data in datas {
                let mut output = vec![];
                let mut encoder =
                    flate2::write::GzEncoder::new(&mut output, flate2::Compression::best());
                let _ = io::copy(&mut Cursor::new(&data), &mut encoder).unwrap();
            }
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets =
        gzip_read_encoder,
        flate2_gzip_read_encoder,
        gzip_write_encoder,
        flate2_gzip_write_encoder
}
criterion_main!(benches);
