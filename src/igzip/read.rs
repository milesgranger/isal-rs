//! Encoder and Decoder implementing `std::io::Read`
use crate::igzip::*;
use std::io;

/// Streaming compression for input streams implementing `std::io::Read`.
///
/// Notes
/// -----
/// One should consider using `crate::igzip::compress` or `crate::igzip::compress_into` if possible.
/// In that context, we do not need to hold and maintain intermediate buffers for reading and writing.
///
/// Example
/// -------
/// ```
/// use std::{io, io::Read};
/// use isal::igzip::{read::Encoder, CompressionLevel, decompress, Codec};
/// let data = b"Hello, World!".to_vec();
///
/// let mut encoder = Encoder::new(data.as_slice(), CompressionLevel::Three, Codec::Gzip);
/// let mut compressed = vec![];
///
/// // Number of compressed bytes written to `output`
/// let n = io::copy(&mut encoder, &mut compressed).unwrap();
/// assert_eq!(n as usize, compressed.len());
///
/// let decompressed = decompress(compressed.as_slice(), Codec::Gzip).unwrap();
/// assert_eq!(decompressed.as_slice(), data);
/// ```
pub struct Encoder<R: io::Read> {
    inner: R,
    stream: ZStream,
    in_buf: [u8; BUF_SIZE],
}

impl<R: io::Read> Encoder<R> {
    /// Create a new `Encoder` which implements the `std::io::Read` trait.
    pub fn new(reader: R, level: CompressionLevel, codec: Codec) -> Encoder<R> {
        let in_buf = [0_u8; BUF_SIZE];

        let mut zstream = ZStream::new(level, ZStreamKind::Stateful);
        zstream.stream.end_of_stream = 0;
        zstream.stream.flush = FlushFlags::SyncFlush as _;
        zstream.stream.gzip_flag = codec as _;

        Self {
            inner: reader,
            stream: zstream,
            in_buf,
        }
    }

    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
}

impl<R: io::Read> io::Read for Encoder<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check if there is data left in out_buf, otherwise refill; if end state, return 0
        self.stream.stream.avail_out = buf.len() as _;
        self.stream.stream.next_out = buf.as_mut_ptr();

        let mut nbytes = 0;
        while self.stream.stream.avail_out > 0 {
            // Read out next buf len worth to compress; filling intermediate out_buf
            if self.stream.stream.next_in.is_null() {
                self.stream.stream.avail_in = self.inner.read(&mut self.in_buf)? as _;
                self.stream.stream.next_in = self.in_buf.as_mut_ptr();
                self.stream.stream.end_of_stream = (self.stream.stream.avail_in == 0) as _;
            } else {
                let idx = unsafe {
                    self.stream.stream.next_in.offset_from(self.in_buf.as_ptr()) as usize
                };
                self.in_buf
                    .copy_within(idx..idx + self.stream.stream.avail_in as usize, 0);
                let avail_in = self
                    .inner
                    .read(&mut self.in_buf[self.stream.stream.avail_in as usize..])?
                    as usize;
                self.stream.stream.avail_in += avail_in as u32;
                self.stream.stream.end_of_stream = (avail_in == 0) as _;
                self.stream.stream.next_in = self.in_buf.as_mut_ptr();
            }

            // If it's zstate end, we can exit early if no more avail in, otherwise start next by resetting
            if self.stream.stream.internal_state.state == isal::isal_zstate_state_ZSTATE_END {
                if self.stream.stream.avail_in == 0 {
                    println!(
                        "Early exit: {}, avail_in: {}, avail_out: {}",
                        self.stream.stream.internal_state.state,
                        self.stream.stream.avail_in,
                        self.stream.stream.avail_out
                    );
                    return Ok(nbytes);
                } else {
                    unsafe { isal::isal_deflate_reset(&mut self.stream.stream) };
                }
            }

            println!(
                "Before deflate: {}, avail_in: {}, avail_out: {}",
                self.stream.stream.internal_state.state,
                self.stream.stream.avail_in,
                self.stream.stream.avail_out
            );

            self.stream.deflate()?;
            nbytes = buf.len() - self.stream.stream.avail_out as usize;
            println!(
                "\tAfter deflate: {}, avail_in: {}, avail_out: {}, nbytes: {}",
                self.stream.stream.internal_state.state,
                self.stream.stream.avail_in,
                self.stream.stream.avail_out,
                nbytes
            );
        }

        Ok(nbytes)
    }
}

/// Streaming compression for input streams implementing `std::io::Read`.
///
/// Notes
/// -----
/// One should consider using `crate::igzip::decompress` or `crate::igzip::decompress_into` if possible.
/// In that context, we do not need to hold and maintain intermediate buffers for reading and writing.
///
/// Example
/// -------
/// ```
/// use std::{io, io::Read};
/// use isal::igzip::{read::Decoder, CompressionLevel, compress, Codec};
/// let data = b"Hello, World!".to_vec();
///
/// let compressed = compress(data.as_slice(), CompressionLevel::Three, Codec::Gzip).unwrap();
/// let mut decoder = Decoder::new(compressed.as_slice(), Codec::Gzip);
/// let mut decompressed = vec![];
///
/// // Numbeer of compressed bytes written to `output`
/// let n = io::copy(&mut decoder, &mut decompressed).unwrap();
/// assert_eq!(n as usize, data.len());
/// assert_eq!(decompressed.as_slice(), data);
/// ```
pub struct Decoder<R: io::Read> {
    inner: R,
    zst: InflateState,
    in_buf: [u8; BUF_SIZE],
    codec: Codec,
    adler32: u32,
    check_adler32: bool,
}

impl<R: io::Read> Decoder<R> {
    pub fn new(reader: R, codec: Codec) -> Decoder<R> {
        let mut zst = InflateState::new();
        zst.0.crc_flag = codec as _;

        Self {
            inner: reader,
            zst,
            in_buf: [0u8; BUF_SIZE],
            codec,
            adler32: 1,
            check_adler32: true,
        }
    }

    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Configure the checking of adler32 when in ZLib codec mode, has no effect in Deflate or Gzip settings.
    /// defaults to `true`
    ///
    /// It's recommended to always check, so it's purposely left out of the constructor
    /// thus setting it to `false` is made to be a very deliberate action.
    pub fn perform_adler32_check(&mut self, check: bool) {
        self.check_adler32 = check;
    }
}

impl<R: io::Read> io::Read for Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.zst.0.next_out = buf.as_mut_ptr();
        self.zst.0.avail_out = buf.len() as _;

        // keep writing as much as possible to the output buf
        let mut n_bytes = 0;
        let mut avail_in_last = 0;
        while self.zst.0.avail_out > 0 {
            // Keep in_buf full for avail_in
            if self.zst.0.avail_in < self.in_buf.len() as _ {
                // if null, it's our first assignment of compressed data in
                if self.zst.0.next_in.is_null() {
                    debug_assert_eq!(self.zst.0.avail_in, 0);
                    self.zst.0.avail_in = self.inner.read(&mut self.in_buf)? as u32;
                    self.zst.0.next_in = self.in_buf.as_mut_ptr();

                // Otherwise shift current available in section to the front and read more
                } else {
                    let idx =
                        unsafe { self.zst.0.next_in.offset_from(self.in_buf.as_ptr()) as usize };
                    self.in_buf
                        .copy_within(idx..idx + self.zst.0.avail_in as usize, 0);
                    self.zst.0.next_in = self.in_buf.as_mut_ptr();

                    // read some more and update avail_in
                    let n = self
                        .inner
                        .read(&mut self.in_buf[self.zst.0.avail_in as usize..])?;
                    self.zst.0.avail_in += n as u32;
                };

                avail_in_last = self.zst.0.avail_in;

                // Block finished, reset if we have more compressed data, otherwise exit
                if self.zst.block_state() == isal::isal_block_state_ISAL_BLOCK_FINISH {
                    if self.zst.0.avail_in == 0 {
                        break;
                    }
                    self.zst.reset();
                }
            }

            self.zst.step_inflate()?;
            n_bytes = buf.len() - self.zst.0.avail_out as usize;

            // Check adler
            if self.codec == Codec::Zlib
                && self.check_adler32
                && n_bytes >= buf.len()
                && self.zst.block_state() == isal::isal_block_state_ISAL_BLOCK_FINISH
            {
                let decompressed = &buf[..n_bytes];
                self.adler32 = unsafe {
                    isal::isal_adler32(self.adler32, decompressed.as_ptr(), decompressed.len() as _)
                };
                let bytes: [u8; 4] = (&self.in_buf
                    [avail_in_last as usize - 4..avail_in_last as usize])
                    .try_into()
                    .unwrap();
                let e_adler32 = u32::from_be_bytes(bytes);
                if self.adler32 != e_adler32 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        crate::error::Error::DecompressionError(DecompCode::IncorrectChecksum),
                    ));
                }
            }
        }

        Ok(n_bytes)
    }
}

/// Deflate decompression
/// Basically a wrapper to `Encoder` which sets the codec for you.
pub struct DeflateEncoder<R: io::Read> {
    inner: Encoder<R>,
}

impl<R: io::Read> DeflateEncoder<R> {
    pub fn new(reader: R, level: CompressionLevel) -> Self {
        Self {
            inner: Encoder::new(reader, level, Codec::Deflate),
        }
    }
    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner.inner
    }
}

impl<R: io::Read> io::Read for DeflateEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Deflate decompression
/// Basically a wrapper to `Decoder` which sets the codec for you.
pub struct DeflateDecoder<R: io::Read> {
    inner: Decoder<R>,
}

impl<R: io::Read> DeflateDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: Decoder::new(reader, Codec::Deflate),
        }
    }
    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner.inner
    }
}

impl<R: io::Read> io::Read for DeflateDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}
/// Zlib decompression
/// Basically a wrapper to `Encoder` which sets the codec for you.
pub struct ZlibEncoder<R: io::Read> {
    inner: Encoder<R>,
}

impl<R: io::Read> ZlibEncoder<R> {
    pub fn new(reader: R, level: CompressionLevel) -> Self {
        Self {
            inner: Encoder::new(reader, level, Codec::Zlib),
        }
    }
    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner.inner
    }
}

impl<R: io::Read> io::Read for ZlibEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Zlib decompression
/// Basically a wrapper to `Decoder` which sets the codec for you.
pub struct ZlibDecoder<R: io::Read> {
    inner: Decoder<R>,
}

impl<R: io::Read> ZlibDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: Decoder::new(reader, Codec::Zlib),
        }
    }
    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner.inner
    }

    /// Configure the checking of adler32 when in ZLib codec mode, has no effect in Deflate or Gzip settings.
    /// defaults to `true`
    ///
    /// It's recommended to always check, so it's purposely left out of the constructor
    /// thus setting it to `false` is made to be a very deliberate action.
    pub fn perform_adler32_check(&mut self, check: bool) {
        self.inner.check_adler32 = check;
    }
}

impl<R: io::Read> io::Read for ZlibDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}
/// Gzip decompression
/// Basically a wrapper to `Encoder` which sets the codec for you.
pub struct GzipEncoder<R: io::Read> {
    inner: Encoder<R>,
}

impl<R: io::Read> GzipEncoder<R> {
    pub fn new(reader: R, level: CompressionLevel) -> Self {
        Self {
            inner: Encoder::new(reader, level, Codec::Gzip),
        }
    }
    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner.inner
    }
}

impl<R: io::Read> io::Read for GzipEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

/// Gzip decompression
/// Basically a wrapper to `Decoder` which sets the codec for you.
pub struct GzipDecoder<R: io::Read> {
    inner: Decoder<R>,
}

impl<R: io::Read> GzipDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: Decoder::new(reader, Codec::Gzip),
        }
    }
    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut R {
        &mut self.inner.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &R {
        &self.inner.inner
    }
}

impl<R: io::Read> io::Read for GzipDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::igzip::tests::{gen_large_data, same_same};
    use std::io::{self, Cursor};

    #[test]
    fn roundtrip_small() {
        roundtrip(b"foobar")
    }
    #[test]
    fn roundtrip_large() {
        roundtrip(&gen_large_data())
    }
    fn roundtrip(input: &[u8]) {
        let mut encoder = Encoder::new(Cursor::new(&input), CompressionLevel::Three, Codec::Gzip);
        let mut output = vec![];

        let n = io::copy(&mut encoder, &mut output).unwrap();
        assert_eq!(n as usize, output.len());

        let mut decoder = Decoder::new(Cursor::new(output), Codec::Gzip);
        let mut decompressed = vec![];
        let nbytes = io::copy(&mut decoder, &mut decompressed).unwrap();

        assert_eq!(nbytes as usize, input.len());
        assert!(same_same(&input, &decompressed));
    }

    #[test]
    fn basic_compress_small() -> Result<()> {
        basic_compress(b"foobar")
    }
    #[test]
    fn basic_compress_large() -> Result<()> {
        basic_compress(&gen_large_data())
    }
    fn basic_compress(input: &[u8]) -> Result<()> {
        let mut encoder = Encoder::new(Cursor::new(input), CompressionLevel::Three, Codec::Gzip);
        let mut output = vec![];

        let n = io::copy(&mut encoder, &mut output)? as usize;
        let decompressed = decompress(&output[..n], Codec::Gzip)?;

        assert_eq!(input, decompressed.as_slice());
        Ok(())
    }

    #[test]
    fn longer_compress() -> Result<()> {
        // Build input which is greater than BUF_SIZE
        let mut input = vec![];
        for chunk in std::iter::repeat(b"Hello, World!") {
            input.extend_from_slice(chunk);
            if input.len() > BUF_SIZE * 2 {
                break;
            }
        }

        let mut encoder = Encoder::new(Cursor::new(&input), CompressionLevel::Three, Codec::Gzip);
        let mut output = vec![];

        let n = io::copy(&mut encoder, &mut output)? as usize;
        let decompressed = decompress(&output[..n], Codec::Gzip)?;

        assert!(same_same(&input, decompressed.as_slice()));
        Ok(())
    }

    #[test]
    fn basic_decompress_small() -> Result<()> {
        basic_decompress(b"foobar")
    }
    #[test]
    fn basic_decompress_large() -> Result<()> {
        basic_decompress(&gen_large_data())
    }
    fn basic_decompress(input: &[u8]) -> Result<()> {
        let compressed = compress(Cursor::new(input), CompressionLevel::Three, Codec::Gzip)?;

        let mut decoder = Decoder::new(compressed.as_slice(), Codec::Gzip);
        let mut decompressed = vec![];

        let n = io::copy(&mut decoder, &mut decompressed)? as usize;
        assert_eq!(n, decompressed.len());
        assert_eq!(input, decompressed.as_slice());
        Ok(())
    }

    #[test]
    fn longer_decompress() -> Result<()> {
        // Build input which is greater than BUF_SIZE
        let mut input = vec![];
        for chunk in std::iter::repeat(b"Hello, World!") {
            input.extend_from_slice(chunk);
            if input.len() > BUF_SIZE * 3 {
                break;
            }
        }

        let compressed = compress(input.as_slice(), CompressionLevel::Three, Codec::Gzip)?;

        let mut decoder = Decoder::new(compressed.as_slice(), Codec::Gzip);
        let mut decompressed = vec![];

        let n = io::copy(&mut decoder, &mut decompressed)? as usize;
        assert_eq!(n, decompressed.len());
        assert!(same_same(&input, decompressed.as_slice()));

        Ok(())
    }

    #[test]
    fn flate2_gzip_compat_encoder_out_small() {
        flate2_gzip_compat_encoder_out(b"foobar")
    }
    #[test]
    fn flate2_gzip_compat_encoder_out_large() {
        flate2_gzip_compat_encoder_out(&gen_large_data())
    }
    fn flate2_gzip_compat_encoder_out(data: &[u8]) {
        // our encoder
        let mut encoder = Encoder::new(data, CompressionLevel::Three, Codec::Gzip);
        let mut compressed = vec![];
        io::copy(&mut encoder, &mut compressed).unwrap();

        // their decoder
        let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
        let mut decompressed = vec![];
        io::copy(&mut decoder, &mut decompressed).unwrap();

        assert!(same_same(&data, &decompressed));
    }

    #[test]
    fn flate2_gzip_compat_decoder_out_small() {
        flate2_gzip_compat_decoder_out(b"foobar")
    }
    #[test]
    fn flate2_gzip_compat_decoder_out_large() {
        flate2_gzip_compat_decoder_out(&gen_large_data())
    }
    fn flate2_gzip_compat_decoder_out(data: &[u8]) {
        // their encoder
        let mut encoder = flate2::read::GzEncoder::new(data, flate2::Compression::fast());
        let mut compressed = vec![];
        io::copy(&mut encoder, &mut compressed).unwrap();

        // our decoder
        let mut decoder = Decoder::new(compressed.as_slice(), Codec::Gzip);
        let mut decompressed = vec![];
        io::copy(&mut decoder, &mut decompressed).unwrap();

        assert!(same_same(&data, &decompressed));
    }

    #[test]
    fn flate2_deflate_compat_encoder_out_small() {
        flate2_deflate_compat_encoder_out(b"foobar")
    }
    #[test]
    fn flate2_deflate_compat_encoder_out_large() {
        flate2_deflate_compat_encoder_out(&gen_large_data())
    }
    fn flate2_deflate_compat_encoder_out(data: &[u8]) {
        // our encoder
        let mut encoder = DeflateEncoder::new(data, CompressionLevel::Three);
        let mut compressed = vec![];
        io::copy(&mut encoder, &mut compressed).unwrap();

        // their decoder
        let mut decoder = flate2::read::DeflateDecoder::new(compressed.as_slice());
        let mut decompressed = vec![];
        io::copy(&mut decoder, &mut decompressed).unwrap();

        assert!(same_same(&data, &decompressed));
    }

    #[test]
    fn flate2_deflate_compat_decoder_out_small() {
        flate2_deflate_compat_decoder_out(b"foobar")
    }
    #[test]
    fn flate2_deflate_compat_decoder_out_large() {
        flate2_deflate_compat_decoder_out(&gen_large_data())
    }
    fn flate2_deflate_compat_decoder_out(data: &[u8]) {
        // their encoder
        let mut encoder = flate2::read::DeflateEncoder::new(data, flate2::Compression::fast());
        let mut compressed = vec![];
        io::copy(&mut encoder, &mut compressed).unwrap();

        // our decoder
        let mut decoder = DeflateDecoder::new(compressed.as_slice());
        let mut decompressed = vec![];
        io::copy(&mut decoder, &mut decompressed).unwrap();

        assert_eq!(data.len(), decompressed.len());
        assert!(same_same(&data, &decompressed));
    }
}
