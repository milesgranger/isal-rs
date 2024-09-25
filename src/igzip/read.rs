//! Encoder and Decoder implementing `std::io::Read`
use crate::igzip::*;
use mem::MaybeUninit;
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
/// use isal::igzip::{read::Encoder, CompressionLevel, decompress};
/// let data = b"Hello, World!".to_vec();
///
/// let mut encoder = Encoder::new(data.as_slice(), CompressionLevel::Three, true);
/// let mut compressed = vec![];
///
/// // Numbeer of compressed bytes written to `output`
/// let n = io::copy(&mut encoder, &mut compressed).unwrap();
/// assert_eq!(n as usize, compressed.len());
///
/// let decompressed = decompress(io::Cursor::new(compressed)).unwrap();
/// assert_eq!(decompressed.as_slice(), data);
/// ```
pub struct Encoder<R: io::Read> {
    inner: R,
    stream: ZStream,
    in_buf: [u8; BUF_SIZE],
    out_buf: Vec<u8>,
    dsts: usize,
    dste: usize,
}

impl<R: io::Read> Encoder<R> {
    /// Create a new `Encoder` which implements the `std::io::Read` trait.
    pub fn new(reader: R, level: CompressionLevel, is_gzip: bool) -> Encoder<R> {
        let in_buf = [0_u8; BUF_SIZE];
        let out_buf = Vec::with_capacity(BUF_SIZE);

        let mut zstream = ZStream::new(level, ZStreamKind::Stateful);

        zstream.stream.end_of_stream = 0;
        zstream.stream.flush = FlushFlags::SyncFlush as _;
        zstream.stream.gzip_flag = is_gzip as _;

        Self {
            inner: reader,
            stream: zstream,
            in_buf,
            out_buf,
            dste: 0,
            dsts: 0,
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

    // Read data from intermediate output buffer holding compressed output.
    // It's unknown if the output from igzip will fit into buffer passed during read
    // so we hold it here and empty as read calls pass.
    // thanks to: https://github.com/BurntSushi/rust-snappy/blob/f9eb8d49c713adc48732fb95682a201a7b74d39a/src/read.rs#L327
    #[inline(always)]
    fn read_from_out_buf(&mut self, buf: &mut [u8]) -> usize {
        let available_bytes = self.dste - self.dsts;
        let count = std::cmp::min(available_bytes, buf.len());
        buf[..count].copy_from_slice(&self.out_buf[self.dsts..self.dsts + count]);
        self.dsts += count;
        count
    }
}

impl<R: io::Read> io::Read for Encoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check if there is data left in out_buf, otherwise refill; if end state, return 0
        let count = self.read_from_out_buf(buf);
        if count > 0 {
            Ok(count)
        } else if self.stream.stream.internal_state.state != isal::isal_zstate_state_ZSTATE_END {
            // Read out next buf len worth to compress; filling intermediate out_buf
            self.stream.stream.avail_in = self.inner.read(&mut self.in_buf)? as _;
            if self.stream.stream.avail_in < self.in_buf.len() as _ {
                self.stream.stream.end_of_stream = 1;
            }
            self.stream.stream.next_in = self.in_buf.as_mut_ptr();

            let mut n_bytes = 0;
            self.out_buf.truncate(0);

            // compress this chunk into out_buf
            while self.stream.stream.avail_in > 0 {
                self.out_buf.resize(self.out_buf.len() + BUF_SIZE, 0);

                self.stream.stream.avail_out = BUF_SIZE as _;
                self.stream.stream.next_out =
                    self.out_buf[n_bytes..n_bytes + BUF_SIZE].as_mut_ptr();

                self.stream.deflate()?;

                n_bytes += BUF_SIZE - self.stream.stream.avail_out as usize;
            }
            self.out_buf.truncate(n_bytes);
            self.dste = n_bytes;
            self.dsts = 0;

            Ok(self.read_from_out_buf(buf))
        } else {
            Ok(0)
        }
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
/// use isal::igzip::{read::Decoder, CompressionLevel, compress};
/// let data = b"Hello, World!".to_vec();
///
/// let compressed = compress(data.as_slice(), CompressionLevel::Three, true).unwrap();
/// let mut decoder = Decoder::new(compressed.as_slice());
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
    out_buf: Vec<u8>,
    dsts: usize,
    dste: usize,
}

impl<R: io::Read> Decoder<R> {
    pub fn new(reader: R) -> Decoder<R> {
        let mut zst = InflateState::new();
        zst.0.crc_flag = isal::IGZIP_GZIP;

        Self {
            inner: reader,
            zst,
            in_buf: [0u8; BUF_SIZE],
            out_buf: Vec::with_capacity(BUF_SIZE),
            dste: 0,
            dsts: 0,
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

    // Read data from intermediate output buffer holding compressed output.
    // It's unknown if the output from igzip will fit into buffer passed during read
    // so we hold it here and empty as read calls pass.
    // thanks to: https://github.com/BurntSushi/rust-snappy/blob/f9eb8d49c713adc48732fb95682a201a7b74d39a/src/read.rs#L327
    #[inline(always)]
    fn read_from_out_buf(&mut self, buf: &mut [u8]) -> usize {
        let available_bytes = self.dste - self.dsts;
        let count = std::cmp::min(available_bytes, buf.len());
        buf[..count].copy_from_slice(&self.out_buf[self.dsts..self.dsts + count]);
        self.dsts += count;
        count
    }
}

impl<R: io::Read> io::Read for Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Check if there is data left in out_buf, otherwise refill; if end state, return 0
        let count = self.read_from_out_buf(buf);
        if count > 0 {
            Ok(count)
        } else {
            // Read out next buf len worth to compress; filling intermediate out_buf
            self.zst.0.avail_in = self.inner.read(&mut self.in_buf)? as _;
            self.zst.0.next_in = self.in_buf.as_mut_ptr();

            let mut n_bytes = 0;
            while self.zst.0.avail_in != 0 {
                if self.zst.block_state() == isal::isal_block_state_ISAL_BLOCK_NEW_HDR {
                    // Read this member's gzip header
                    let mut gz_hdr: MaybeUninit<isal::isal_gzip_header> = MaybeUninit::uninit();
                    unsafe { isal::isal_gzip_header_init(gz_hdr.as_mut_ptr()) };
                    let mut gz_hdr = unsafe { gz_hdr.assume_init() };
                    read_gzip_header(&mut self.zst.0, &mut gz_hdr)?;
                }

                // decompress member
                loop {
                    self.out_buf.resize(n_bytes + BUF_SIZE, 0);

                    self.zst.0.next_out = self.out_buf[n_bytes..n_bytes + BUF_SIZE].as_mut_ptr();
                    self.zst.0.avail_out = BUF_SIZE as _;

                    self.zst.step_inflate()?;

                    n_bytes += BUF_SIZE - self.zst.0.avail_out as usize;

                    let state = self.zst.block_state();
                    if state == isal::isal_block_state_ISAL_BLOCK_CODED
                        || state == isal::isal_block_state_ISAL_BLOCK_TYPE0
                        || state == isal::isal_block_state_ISAL_BLOCK_HDR
                        || state == isal::isal_block_state_ISAL_BLOCK_FINISH
                    {
                        break;
                    }
                }
                if self.zst.0.block_state == isal::isal_block_state_ISAL_BLOCK_FINISH {
                    self.zst.reset();
                }
            }
            self.out_buf.truncate(n_bytes);
            self.dste = n_bytes;
            self.dsts = 0;

            Ok(self.read_from_out_buf(buf))
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::io::{self, Cursor};

    fn gen_large_data() -> Vec<u8> {
        (0..1_000_000)
            .map(|_| b"oh what a beautiful morning, oh what a beautiful day!!".to_vec())
            .flat_map(|v| v)
            .collect()
    }

    #[test]
    fn large_roundtrip() {
        let input = gen_large_data();
        let mut encoder = Encoder::new(Cursor::new(&input), CompressionLevel::Three, true);
        let mut output = vec![];

        let n = io::copy(&mut encoder, &mut output).unwrap();
        assert!(n < input.len() as u64);

        let mut decoder = Decoder::new(Cursor::new(output));
        let mut decompressed = vec![];
        let nbytes = io::copy(&mut decoder, &mut decompressed).unwrap();

        assert_eq!(nbytes as usize, input.len());
    }

    #[test]
    fn basic_compress() -> Result<()> {
        let input = b"hello, world!";
        let mut encoder = Encoder::new(Cursor::new(input), CompressionLevel::Three, true);
        let mut output = vec![];

        let n = io::copy(&mut encoder, &mut output)? as usize;
        let decompressed = decompress(&output[..n])?;

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

        let mut encoder = Encoder::new(Cursor::new(&input), CompressionLevel::Three, true);
        let mut output = vec![];

        let n = io::copy(&mut encoder, &mut output)? as usize;
        let decompressed = decompress(&output[..n])?;

        assert_eq!(input, decompressed.as_slice());
        Ok(())
    }

    #[test]
    fn basic_decompress() -> Result<()> {
        let input = b"hello, world!";
        let compressed = compress(Cursor::new(input), CompressionLevel::Three, true)?;

        let mut decoder = Decoder::new(compressed.as_slice());
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

        let compressed = compress(input.as_slice(), CompressionLevel::Three, true)?;

        let mut decoder = Decoder::new(compressed.as_slice());
        let mut decompressed = vec![];

        let n = io::copy(&mut decoder, &mut decompressed)? as usize;
        assert_eq!(n, decompressed.len());
        assert_eq!(input, decompressed.as_slice());

        Ok(())
    }

    #[test]
    fn flate2_gzip_compat_encoder_out() {
        let data = gen_large_data();

        // our encoder
        let mut encoder = Encoder::new(data.as_slice(), CompressionLevel::Three, true);
        let mut compressed = vec![];
        io::copy(&mut encoder, &mut compressed).unwrap();

        // their decoder
        let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
        let mut decompressed = vec![];
        io::copy(&mut decoder, &mut decompressed).unwrap();

        assert_eq!(&data, &decompressed);
    }

    #[test]
    fn flate2_gzip_compat_decoder_out() {
        let data = gen_large_data();

        // their encoder
        let mut encoder =
            flate2::read::GzEncoder::new(data.as_slice(), flate2::Compression::fast());
        let mut compressed = vec![];
        io::copy(&mut encoder, &mut compressed).unwrap();

        // our decoder
        let mut decoder = Decoder::new(compressed.as_slice());
        let mut decompressed = vec![];
        io::copy(&mut decoder, &mut decompressed).unwrap();

        assert_eq!(&data, &decompressed);
    }
}
