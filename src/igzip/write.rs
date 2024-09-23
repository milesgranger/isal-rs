use crate::igzip::*;
use std::io;
use std::io::Write;

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
/// use std::{io, io::Write};
/// use isal::igzip::{write::Encoder, CompressionLevel, decompress};
///
/// let data = b"Hello, World!".to_vec();
/// let mut compressed = vec![];
///
/// let mut encoder = Encoder::new(&mut compressed, CompressionLevel::Three, true);
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
    pub fn new(writer: W, level: CompressionLevel, is_gzip: bool) -> Encoder<W> {
        let out_buf = Vec::with_capacity(BUF_SIZE);

        let mut zstream = ZStream::new(level, ZStreamKind::Stateful);

        zstream.stream.end_of_stream = 0;
        zstream.stream.flush = FlushFlags::NoFlush as _;
        zstream.stream.gzip_flag = is_gzip as _;

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

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;

    fn gen_large_data() -> Vec<u8> {
        (0..1_000_000)
            .map(|_| b"oh what a beautiful morning, oh what a beautiful day!!".to_vec())
            .flat_map(|v| v)
            .collect()
    }

    #[test]
    fn test_encoder_basic() {
        let data = gen_large_data();

        let mut compressed = vec![];
        let mut encoder = Encoder::new(&mut compressed, CompressionLevel::Three, true);
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
        assert_eq!(decompressed.len(), data.len());
    }

    #[test]
    fn test_encoder_multi_stream() {
        let first = b"foo";
        let second = b"bar";

        let mut compressed = vec![];
        let mut encoder = Encoder::new(&mut compressed, CompressionLevel::Three, true);

        encoder.write_all(first).unwrap();
        encoder.flush().unwrap();
        assert_eq!(encoder.total_in(), first.len());

        encoder.write_all(second).unwrap();
        encoder.flush().unwrap();
        assert_eq!(encoder.total_in(), first.len() + second.len());

        let decompressed = crate::igzip::decompress(io::Cursor::new(&compressed)).unwrap();
        assert_eq!(&decompressed, b"foobar");
    }
}
