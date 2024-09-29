# isal-rs

[![CI](https://github.com/milesgranger/isal-rs/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/milesgranger/isal-rs/actions/workflows/CI.yml)
[![Latest version](https://img.shields.io/crates/v/isal-rs.svg)](https://crates.io/crates/isal-rs)
[![Documentation](https://docs.rs/isal-rs/badge.svg)](https://docs.rs/isal-rs)
![License](https://img.shields.io/crates/l/isal-rs.svg)

Rust bindings for [isa-l](https://github.com/intel/isa-l)

---

Supports the following codecs using the ISA-L library under the hood:

- GZIP 
  - `isal::igzip::read::GzipEncoder/GzipDecoder`
  - `isal::igzip::write::GzipEncoder/GzipDecoder`
- DEFLATE
  - `isal::igzip::read::DeflateEncoder/DeflateDecoder`
  - `isal::igzip::write::DeflateEncoder/DeflateDecoder`
- ZLIB
  - `isal::igzip::read::ZlibEncoder/ZlibDecoder`
  - `isal::igzip::write::ZlibEncoder/ZlibDecoder`
  - TODO:
    - [ ] Support an 'unsafe' setting where one can ignore step of verifying Adler32 checksum.

Or can use functions of `de/compress` and `de/compress_into`

---

Building requires some system tools like `autotools`, `nasm`, `make`, and anything the official ISA-L repo suggests. 
On Windows the build is invoked with `nmake`, other systems use the `./autogen.sh` and `./configure` setups.

---

### Examples:

#### Functions like `compress_into` and `decompress`
(Similar functionality with `compress` and `decompress_into`)
```rust
use isal::igzip::{CompressionLevel, Codec, compress_into, decompress};

let mut compressed = vec![0u8; 100];
let nbytes = compress_into(b"foobar", &mut compressed, CompressionLevel::Three, Codec::Gzip).unwrap();

let decompressed = decompress(&compressed[..nbytes], Codec::Gzip).unwrap();
assert_eq!(decompressed.as_slice(), b"foobar");
```

#### Compress using the Encoder/Decoder structs implementing `io::Read`

```rust
use std::{io, io::Read};
use isal::igzip::{read::{Encoder, GzipEncoder}, CompressionLevel, decompress, Codec};

let data = b"Hello, World!".to_vec();

// Note these two encoders are equivelent...
let mut encoder = GzipEncoder::new(data.as_slice(), CompressionLevel::Three);
let mut encoder = Encoder::new(data.as_slice(), CompressionLevel::Three, Codec::Gzip);

// Number of compressed bytes written to `output`
let mut compressed = vec![];
let n = io::copy(&mut encoder, &mut compressed).unwrap();
assert_eq!(n as usize, compressed.len());

let decompressed = decompress(compressed.as_slice(), Codec::Gzip).unwrap();
assert_eq!(decompressed.as_slice(), data);
```

#### Decompress using the Encoder/Decoder structs implementing `io::Write`

```rust
use std::{io, io::Write};
use isal::igzip::{write::Decoder, CompressionLevel, compress, Codec};

let data = b"Hello, World!".to_vec();
let compressed = compress(io::Cursor::new(data.as_slice()), CompressionLevel::Three, Codec::Gzip).unwrap();

let mut decompressed = vec![];
let mut decoder = Decoder::new(&mut decompressed, Codec::Gzip);

// Number of compressed bytes written to `output`
let n = io::copy(&mut io::Cursor::new(&compressed), &mut decoder).unwrap();
assert_eq!(n as usize, compressed.len());

assert_eq!(decompressed.as_slice(), data);
```

---

### Benchmarks

Checkout the [README](./benches/README.md) in the benches directory.

---

### Versioning: 
Versions are specified in normal SemVer format, and a trailing "`+<< commit hash >>`" to indicate
which commit in [isa-l](https://github.com/intel/isa-l) the crate is built against. ie: `0.1.0+62519d9`
