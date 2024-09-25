use std::{fs, io::Cursor};

use isal::igzip;

fn get_data() -> std::result::Result<Vec<u8>, std::io::Error> {
    fs::read(format!(
        "{}/../../pyrus-cramjam/benchmarks/data/html_x_4",
        env!("CARGO_MANIFEST_DIR")
    ))
}

fn main() {
    let data = get_data().unwrap();
    for _ in 0..1000 {
        let _v = igzip::compress(
            Cursor::new(&data),
            igzip::CompressionLevel::Three,
            isal::igzip::Codec::Gzip,
        )
        .unwrap();
    }
}
