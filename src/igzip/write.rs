//! Encoder and Decoder implementing `std::io::Write`
use crate::igzip::*;
use std::io;
use std::io::Write;

/// Streaming compression for input streams implementing `std::io::Write`.
///
/// Notes
/// -----
/// One should consider using `crate::igzip::compress` or `crate::igzip::compress_into` if possible.
/// In that context, we do not need to hold and maintain intermediate buffers for reading and writing.
///
/// Example
/// -------
/// ```
/// use std::{io, io::Write};
/// use isal::igzip::{write::Encoder, CompressionLevel, decompress, Codec};
///
/// let data = b"Hello, World!".to_vec();
/// let mut compressed = vec![];
///
/// let mut encoder = Encoder::new(&mut compressed, CompressionLevel::Three, Codec::Gzip);
///
/// // Numbeer of compressed bytes written to `output`
/// io::copy(&mut io::Cursor::new(&data), &mut encoder).unwrap();
///
/// // call .flush to finish the stream
/// encoder.flush().unwrap();
///
/// let decompressed = decompress(io::Cursor::new(&compressed)).unwrap();
/// assert_eq!(decompressed.as_slice(), data);
///
/// ```
pub struct Encoder<W: io::Write> {
    inner: W,
    stream: ZStream,
    out_buf: Vec<u8>,
    dsts: usize,
    dste: usize,
    total_in: usize,
    total_out: usize,
}

impl<W: io::Write> Encoder<W> {
    /// Create a new `Encoder` which implements the `std::io::Read` trait.
    pub fn new(writer: W, level: CompressionLevel, codec: Codec) -> Encoder<W> {
        let out_buf = Vec::with_capacity(BUF_SIZE);

        let mut zstream = ZStream::new(level, ZStreamKind::Stateful);

        zstream.stream.end_of_stream = 0;
        zstream.stream.flush = FlushFlags::NoFlush as _;
        zstream.stream.gzip_flag = codec as _;

        Self {
            inner: writer,
            stream: zstream,
            out_buf,
            dste: 0,
            dsts: 0,
            total_in: 0,
            total_out: 0,
        }
    }

    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    #[inline(always)]
    fn write_from_out_buf(&mut self) -> io::Result<usize> {
        let count = self.dste - self.dsts;
        self.inner
            .write_all(&mut self.out_buf[self.dsts..self.dste])?;
        self.out_buf.truncate(0);
        self.dsts = 0;
        self.dste = 0;
        Ok(count)
    }

    /// Call flush and return the inner writer
    pub fn finish(mut self) -> io::Result<W> {
        self.flush()?;
        Ok(self.inner)
    }

    /// total bytes written to the writer, inclusive of all streams if `flush` has been called before
    pub fn total_out(&self) -> usize {
        self.stream.stream.total_out as usize + self.total_out
    }

    /// total bytes processed, inclusive of all streams if `flush` has been called before
    pub fn total_in(&self) -> usize {
        self.stream.stream.total_in as usize + self.total_in
    }
}

impl<W: io::Write> io::Write for Encoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.stream.stream.avail_in = buf.len() as _;
        self.stream.stream.next_in = buf.as_ptr() as *mut _;

        while self.stream.stream.avail_in > 0 {
            self.out_buf.resize(self.dste + BUF_SIZE, 0);

            self.stream.stream.avail_out = BUF_SIZE as _;
            self.stream.stream.next_out =
                self.out_buf[self.dste..self.dste + BUF_SIZE].as_mut_ptr();

            self.stream.deflate()?;

            self.dste += BUF_SIZE - self.stream.stream.avail_out as usize;
        }

        self.write_from_out_buf()?;

        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        // Write footer and flush to inner
        self.stream.stream.end_of_stream = 1;
        self.stream.stream.flush = FlushFlags::FullFlush as _;
        while self.stream.stream.internal_state.state != isal::isal_zstate_state_ZSTATE_END {
            self.out_buf.resize(self.dste + BUF_SIZE, 0);
            self.stream.stream.avail_out = BUF_SIZE as _;
            self.stream.stream.next_out =
                self.out_buf[self.dste..self.dste + BUF_SIZE].as_mut_ptr();
            self.stream.deflate()?;
            self.dste += BUF_SIZE - self.stream.stream.avail_out as usize;
        }
        self.write_from_out_buf()?;
        self.inner.flush()?;

        // Prep for next stream should user call 'write' again after flush.
        // needs to store total_in/out separately as checksum is calculated
        // from these values per stream
        self.total_in += self.stream.stream.total_in as usize;
        self.total_out += self.stream.stream.total_out as usize;
        unsafe { isal::isal_deflate_reset(&mut self.stream.stream) };

        // TODO: when doing something other than gzip, or flush flags on init
        //       store these and re-initialize
        self.stream.stream.flush = FlushFlags::NoFlush as _;
        self.stream.stream.end_of_stream = 0;
        self.stream.stream.gzip_flag = 1;
        Ok(())
    }
}

/// Streaming compression for input streams implementing `std::io::Write`.
///
/// Notes
/// -----
/// One should consider using `crate::igzip::decompress` or `crate::igzip::decompress_into` if possible.
/// In that context, we do not need to hold and maintain intermediate buffers for reading and writing.
///
/// Example
/// -------
/// ```
/// use std::{io, io::Write};
/// use isal::igzip::{write::Decoder, CompressionLevel, compress, Codec};
/// let data = b"Hello, World!".to_vec();
///
/// let compressed = compress(io::Cursor::new(data.as_slice()), CompressionLevel::Three, Codec::Gzip).unwrap();
///
/// let mut decompressed = vec![];
/// let mut decoder = Decoder::new(&mut decompressed);
///
/// // Numbeer of compressed bytes written to `output`
/// let n = io::copy(&mut io::Cursor::new(&compressed), &mut decoder).unwrap();
/// assert_eq!(n as usize, compressed.len());
/// assert_eq!(decompressed.as_slice(), data);
/// ```
pub struct Decoder<W: io::Write> {
    inner: W,
    zst: InflateState,
    out_buf: Vec<u8>,
    dsts: usize,
    dste: usize,
}

impl<W: io::Write> Decoder<W> {
    pub fn new(writer: W) -> Decoder<W> {
        let mut zst = InflateState::new();
        zst.0.crc_flag = isal::IGZIP_GZIP;

        Self {
            inner: writer,
            zst,
            out_buf: Vec::with_capacity(BUF_SIZE),
            dste: 0,
            dsts: 0,
        }
    }

    /// Mutable reference to underlying reader, not advisable to modify during reading.
    pub fn get_ref_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    // Reference to underlying reader
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    #[inline(always)]
    fn write_from_out_buf(&mut self) -> io::Result<usize> {
        let count = self.dste - self.dsts;
        self.inner
            .write_all(&mut self.out_buf[self.dsts..self.dste])?;
        self.out_buf.truncate(0);
        self.dsts = 0;
        self.dste = 0;
        Ok(count)
    }
}

impl<W: io::Write> io::Write for Decoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Check if there is data left in out_buf, otherwise refill; if end state, return 0
        // Read out next buf len worth to compress; filling intermediate out_buf
        self.zst.0.avail_in = buf.len() as _;
        self.zst.0.next_in = buf.as_ptr() as *mut _;

        let mut n_bytes = 0;
        while self.zst.0.avail_in > 0 {
            if self.zst.block_state() == isal::isal_block_state_ISAL_BLOCK_NEW_HDR {
                // Read this member's gzip header
                let mut gz_hdr: mem::MaybeUninit<isal::isal_gzip_header> =
                    mem::MaybeUninit::uninit();
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
        self.write_from_out_buf()?;

        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use io::Cursor;

    use super::*;
    use crate::igzip::tests::{gen_large_data, same_same};
    use std::io::Write;

    #[test]
    fn test_encoder_basic() {
        let data = gen_large_data();

        let mut compressed = vec![];
        let mut encoder = Encoder::new(&mut compressed, CompressionLevel::Three, Codec::Gzip);
        let nbytes = io::copy(&mut io::Cursor::new(&data), &mut encoder).unwrap();

        // Footer isn't written until .flush is called
        let before_flush_bytes_out = encoder.total_out();
        encoder.flush().unwrap();
        let after_flush_bytes_out = encoder.total_out();
        assert!(before_flush_bytes_out < after_flush_bytes_out);

        // nbytes read equals data lenth; compressed is between 0 and data length
        assert_eq!(nbytes, data.len() as _);
        assert!(compressed.len() > 0);
        assert!(compressed.len() < data.len());

        // total out after flush should equal compressed length
        assert_eq!(after_flush_bytes_out, compressed.len());

        // and can be decompressed
        let decompressed = crate::igzip::decompress(io::Cursor::new(&compressed)).unwrap();
        assert!(same_same(&decompressed, &data));
    }

    #[test]
    fn test_encoder_multi_stream() {
        let first = b"foo";
        let second = b"bar";

        let mut compressed = vec![];
        let mut encoder = Encoder::new(&mut compressed, CompressionLevel::Three, Codec::Gzip);

        encoder.write_all(first).unwrap();
        encoder.flush().unwrap();
        assert_eq!(encoder.total_in(), first.len());

        encoder.write_all(second).unwrap();
        encoder.flush().unwrap();
        assert_eq!(encoder.total_in(), first.len() + second.len());

        let decompressed = crate::igzip::decompress(io::Cursor::new(&compressed)).unwrap();
        assert_eq!(&decompressed, b"foobar");
    }

    #[test]
    fn test_decoder_basic() {
        let data = gen_large_data();

        let compressed =
            crate::igzip::compress(io::Cursor::new(&data), CompressionLevel::Three, Codec::Gzip)
                .unwrap();

        let mut decompressed = vec![];
        let mut decoder = Decoder::new(&mut decompressed);
        let nbytes = io::copy(&mut io::Cursor::new(&compressed), &mut decoder).unwrap();
        assert_eq!(nbytes, compressed.len() as u64);
        assert!(same_same(&decompressed, &data));
    }

    #[test]
    fn test_decoder_multi_stream() {
        let first = b"foo";
        let second = b"bar";

        let mut compressed = crate::igzip::compress(
            io::Cursor::new(&first),
            CompressionLevel::Three,
            Codec::Gzip,
        )
        .unwrap();
        compressed.extend(
            crate::igzip::compress(
                io::Cursor::new(&second),
                CompressionLevel::Three,
                Codec::Gzip,
            )
            .unwrap(),
        );

        let mut decompressed = vec![];
        let mut decoder = Decoder::new(&mut decompressed);

        let nbytes = io::copy(&mut io::Cursor::new(&compressed), &mut decoder).unwrap();
        assert_eq!(nbytes, compressed.len() as _);
        assert_eq!(&decompressed, b"foobar");
    }

    #[test]
    fn flate2_gzip_compat_encoder_out() {
        let data = gen_large_data();

        // our encoder
        let mut compressed = vec![];
        {
            let mut encoder = Encoder::new(&mut compressed, CompressionLevel::Three, Codec::Gzip);
            io::copy(&mut Cursor::new(&data), &mut encoder).unwrap();
            encoder.flush().unwrap();
        }

        // their decoder
        let mut decompressed = vec![];
        {
            let mut decoder = flate2::write::GzDecoder::new(&mut decompressed);
            io::copy(&mut Cursor::new(&compressed), &mut decoder).unwrap();
            decoder.flush().unwrap();
        }

        assert!(same_same(&data, &decompressed));
    }

    #[test]
    fn flate2_gzip_compat_decoder_out() {
        let data = gen_large_data();

        // their encoder
        let mut compressed = vec![];
        {
            let mut encoder =
                flate2::write::GzEncoder::new(&mut compressed, flate2::Compression::fast());
            io::copy(&mut Cursor::new(&data), &mut encoder).unwrap();
            encoder.flush().unwrap();
        }

        // our decoder
        let mut decompressed = vec![];
        {
            let mut decoder = Decoder::new(&mut decompressed);
            io::copy(&mut Cursor::new(&compressed), &mut decoder).unwrap();
            decoder.flush().unwrap();
        }

        assert!(same_same(&data, &decompressed));
    }
}
