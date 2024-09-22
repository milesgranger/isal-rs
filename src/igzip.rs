use std::io;
use std::mem;

pub(crate) use crate::error::{Error, Result};
use isal_sys::igzip_lib as isal;

/// Buffer size
pub const BUF_SIZE: usize = 16 * 1024;

/// Flush Flags
#[derive(Copy, Clone)]
#[repr(i8)]
pub enum FlushFlags {
    NoFlush = isal::NO_FLUSH as _,
    SyncFlush = isal::SYNC_FLUSH as _,
    FullFlush = isal::FULL_FLUSH as _,
}

/// Compression return values
#[derive(Copy, Clone, Debug)]
#[repr(i8)]
pub enum CompCode {
    CompOk = isal::COMP_OK as _,
    InvalidFlush = isal::INVALID_FLUSH as _,
    InvalidParam = isal::INVALID_PARAM as _,
    StatelessOverflow = isal::STATELESS_OVERFLOW as _,
    InvalidOperation = isal::ISAL_INVALID_OPERATION as _,
    InvalidState = isal::ISAL_INVALID_STATE as _,
    InvalidLevel = isal::ISAL_INVALID_LEVEL as _,
    InvalidLevelBuf = isal::ISAL_INVALID_LEVEL_BUF as _,
}

impl TryFrom<i32> for CompCode {
    type Error = Error;

    #[inline]
    fn try_from(value: i32) -> Result<Self> {
        match value {
            v if v == Self::CompOk as i32 => Ok(Self::CompOk),
            v if v == Self::InvalidFlush as i32 => Ok(Self::InvalidFlush),
            v if v == Self::InvalidParam as i32 => Ok(Self::InvalidParam),
            v if v == Self::StatelessOverflow as i32 => Ok(Self::StatelessOverflow),
            v if v == Self::InvalidOperation as i32 => Ok(Self::InvalidOperation),
            v if v == Self::InvalidState as i32 => Ok(Self::InvalidState),
            v if v == Self::InvalidLevel as i32 => Ok(Self::InvalidLevel),
            v if v == Self::InvalidLevelBuf as i32 => Ok(Self::InvalidLevelBuf),
            _ => Err(Error::Other((
                Some(value as isize),
                "Unknown exit code from compression".to_string(),
            ))),
        }
    }
}

/// Decompression return values
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(i8)]
pub enum DecompCode {
    DecompOk = isal::ISAL_DECOMP_OK as _, /* No errors encountered while decompressing */
    EndInput = isal::ISAL_END_INPUT as _, /* End of input reached */
    OutOverflow = isal::ISAL_OUT_OVERFLOW as _, /* End of output reached */
    NameOverflow = isal::ISAL_NAME_OVERFLOW as _, /* End of gzip name buffer reached */
    CommentOverflow = isal::ISAL_COMMENT_OVERFLOW as _, /* End of gzip name buffer reached */
    ExtraOverflow = isal::ISAL_EXTRA_OVERFLOW as _, /* End of extra buffer reached */
    NeedDict = isal::ISAL_NEED_DICT as _, /* Stream needs a dictionary to continue */
    InvalidBlock = isal::ISAL_INVALID_BLOCK as _, /* Invalid deflate block found */
    InvalidSymbol = isal::ISAL_INVALID_SYMBOL as _, /* Invalid deflate symbol found */
    InvalidLoopBack = isal::ISAL_INVALID_LOOKBACK as _, /* Invalid lookback distance found */
    InvalidWrapper = isal::ISAL_INVALID_WRAPPER as _, /* Invalid gzip/zlib wrapper found */
    UnsupportedMethod = isal::ISAL_UNSUPPORTED_METHOD as _, /* Gzip/zlib wrapper specifies unsupported compress method */
    IncorrectChecksum = isal::ISAL_INCORRECT_CHECKSUM as _, /* Incorrect checksum found */
}

impl TryFrom<i32> for DecompCode {
    type Error = Error;

    #[inline]
    fn try_from(value: i32) -> Result<Self> {
        match value {
            v if v == Self::DecompOk as i32 => Ok(Self::DecompOk),
            v if v == Self::EndInput as i32 => Ok(Self::EndInput),
            v if v == Self::OutOverflow as i32 => Ok(Self::OutOverflow),
            v if v == Self::NameOverflow as i32 => Ok(Self::NameOverflow),
            v if v == Self::CommentOverflow as i32 => Ok(Self::CommentOverflow),
            v if v == Self::ExtraOverflow as i32 => Ok(Self::ExtraOverflow),
            v if v == Self::NeedDict as i32 => Ok(Self::NeedDict),
            v if v == Self::InvalidBlock as i32 => Ok(Self::InvalidBlock),
            v if v == Self::InvalidSymbol as i32 => Ok(Self::InvalidSymbol),
            v if v == Self::InvalidLoopBack as i32 => Ok(Self::InvalidLoopBack),
            v if v == Self::InvalidWrapper as i32 => Ok(Self::InvalidWrapper),
            v if v == Self::UnsupportedMethod as i32 => Ok(Self::UnsupportedMethod),
            v if v == Self::IncorrectChecksum as i32 => Ok(Self::IncorrectChecksum),
            _ => Err(Error::Other((
                Some(value as isize),
                "Unknown exit code from decompression".to_string(),
            ))),
        }
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CompressionLevel {
    Zero = 0,
    One = 1,
    Three = 3,
}

impl TryFrom<isize> for CompressionLevel {
    type Error = crate::error::Error;
    fn try_from(value: isize) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Zero),
            1 => Ok(Self::One),
            3 => Ok(Self::Three),
            _ => Err(Self::Error::Other((
                None,
                format!(
                    "Compression level {} not supported, must be one of [0, 1, 3]",
                    value
                ),
            ))),
        }
    }
}

pub mod read {

    use mem::MaybeUninit;

    use super::*;

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

            let mut zstream = ZStream::new_stateful(level);

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
            } else if self.stream.stream.internal_state.state != isal::isal_zstate_state_ZSTATE_END
            {
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

                    self.stream.deflate_stateful()?;

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

                        self.zst.0.next_out =
                            self.out_buf[n_bytes..n_bytes + BUF_SIZE].as_mut_ptr();
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
    }
}

pub struct ZStream {
    stream: isal::isal_zstream,
    #[allow(dead_code)] // Pointer used by stream, kept here to release when dropped
    level_buf: Vec<u8>,
}

impl ZStream {
    pub fn new_stateful(level: CompressionLevel) -> Self {
        let mut zstream_uninit: mem::MaybeUninit<isal::isal_zstream> = mem::MaybeUninit::uninit();
        unsafe { isal::isal_deflate_init(zstream_uninit.as_mut_ptr()) };
        let mut zstream = unsafe { zstream_uninit.assume_init() };
        let buf_size = match level {
            CompressionLevel::Zero => isal::ISAL_DEF_LVL0_DEFAULT,
            CompressionLevel::One => isal::ISAL_DEF_LVL1_DEFAULT,
            CompressionLevel::Three => isal::ISAL_DEF_LVL3_DEFAULT,
        };
        let mut buf = vec![0u8; buf_size as usize];

        zstream.level = level as _;
        zstream.level_buf = buf.as_mut_ptr();
        zstream.level_buf_size = buf.len() as _;

        Self {
            stream: zstream,
            level_buf: buf,
        }
    }
    pub fn new_stateless(level: CompressionLevel) -> Self {
        let mut zstream_uninit: mem::MaybeUninit<isal::isal_zstream> = mem::MaybeUninit::uninit();
        unsafe { isal::isal_deflate_stateless_init(zstream_uninit.as_mut_ptr()) };
        let mut zstream = unsafe { zstream_uninit.assume_init() };
        let buf_size = match level {
            CompressionLevel::Zero => isal::ISAL_DEF_LVL0_DEFAULT,
            CompressionLevel::One => isal::ISAL_DEF_LVL1_DEFAULT,
            CompressionLevel::Three => isal::ISAL_DEF_LVL3_DEFAULT,
        };
        let mut buf = vec![0u8; buf_size as usize];

        zstream.level = level as _;
        zstream.level_buf = buf.as_mut_ptr();
        zstream.level_buf_size = buf.len() as _;

        Self {
            stream: zstream,
            level_buf: buf,
        }
    }

    pub fn deflate_stateful(&mut self) -> Result<()> {
        let ret = unsafe { isal::isal_deflate(&mut self.stream) };
        match CompCode::try_from(ret)? {
            CompCode::CompOk => Ok(()),
            r => Err(Error::CompressionError(r)),
        }
    }

    pub fn deflate_stateless(&mut self) -> Result<()> {
        let ret = unsafe { isal::isal_deflate_stateless(&mut self.stream) };
        match CompCode::try_from(ret)? {
            CompCode::CompOk => Ok(()),
            r => Err(Error::CompressionError(r)),
        }
    }
}

/// Compress `input` directly into `output`. This is the fastest possible compression available.
#[inline(always)]
pub fn compress_into(
    input: &[u8],
    output: &mut [u8],
    level: CompressionLevel,
    is_gzip: bool,
) -> Result<usize> {
    let mut zstream = ZStream::new_stateless(level);

    zstream.stream.flush = FlushFlags::NoFlush as _;
    zstream.stream.gzip_flag = is_gzip as _;
    zstream.stream.end_of_stream = 1;

    // read input into buffer
    zstream.stream.avail_in = input.len() as _;
    zstream.stream.next_in = input.as_ptr() as *mut _;

    // compress this block in its entirety
    zstream.stream.avail_out = output.len() as _;
    zstream.stream.next_out = output.as_mut_ptr();

    zstream.deflate_stateless()?;
    Ok(zstream.stream.total_out as _)
}

/// Compress `input`
#[inline(always)]
pub fn compress<R: std::io::Read>(
    input: R,
    level: CompressionLevel,
    is_gzip: bool,
) -> Result<Vec<u8>> {
    let mut out = vec![];
    let mut encoder = read::Encoder::new(input, level, is_gzip);
    io::copy(&mut encoder, &mut out)?;
    Ok(out)
}

pub struct InflateState(isal::inflate_state);

impl InflateState {
    pub fn new() -> Self {
        let mut uninit: mem::MaybeUninit<isal::inflate_state> = mem::MaybeUninit::uninit();
        unsafe { isal::isal_inflate_init(uninit.as_mut_ptr()) };
        let state = unsafe { uninit.assume_init() };
        Self(state)
    }

    pub fn block_state(&self) -> u32 {
        self.0.block_state
    }

    pub fn reset(&mut self) {
        unsafe { isal::isal_inflate_reset(&mut self.0) }
    }

    pub fn step_inflate(&mut self) -> Result<()> {
        let ret = unsafe { isal::isal_inflate(&mut self.0) };
        match DecompCode::try_from(ret)? {
            DecompCode::DecompOk => Ok(()),
            r => Err(Error::DecompressionError(r)),
        }
    }

    pub fn inflate_stateless(&mut self) -> Result<()> {
        let ret = unsafe { isal::isal_inflate_stateless(&mut self.0) };
        match DecompCode::try_from(ret)? {
            DecompCode::DecompOk => Ok(()),
            r => Err(Error::DecompressionError(r)),
        }
    }
}

/// Read and return gzip header information
///
/// On entry state must be initialized and next_in pointing to a gzip compressed
/// buffer. The buffers gz_hdr->extra, gz_hdr->name, gz_hdr->comments and the
/// buffer lengths must be set to record the corresponding field, or set to NULL
/// to disregard that gzip header information. If one of these buffers overflows,
/// the user can reallocate a larger buffer and call this function again to
/// continue reading the header information.
#[inline(always)]
pub fn read_gzip_header(
    zst: &mut isal::inflate_state,
    gz_hdr: &mut isal::isal_gzip_header,
) -> Result<()> {
    let ret = unsafe { isal::isal_read_gzip_header(zst as *mut _, gz_hdr as *mut _) };
    match DecompCode::try_from(ret)? {
        DecompCode::DecompOk => Ok(()),
        r => Err(Error::DecompressionError(r)),
    }
}

#[inline(always)]
pub fn decompress<R: std::io::Read>(input: R) -> Result<Vec<u8>> {
    let mut out = vec![];
    let mut decoder = read::Decoder::new(input);
    io::copy(&mut decoder, &mut out)?;
    Ok(out)
}

#[inline(always)]
pub fn decompress_into(input: &[u8], output: &mut [u8]) -> Result<usize> {
    let mut zst = InflateState::new();
    zst.0.avail_in = input.len() as _;
    zst.0.next_in = input.as_ptr() as *mut _;
    zst.0.crc_flag = 1;

    zst.0.avail_out = output.len() as _;
    zst.0.next_out = output.as_mut_ptr();

    zst.inflate_stateless()?;

    Ok(zst.0.total_out as _)
}

#[cfg(test)]
mod tests {

    use io::Cursor;
    use md5;
    use std::fs;

    use super::*;

    // Default testing data
    fn gen_large_data() -> Vec<u8> {
        (0..1_000_000)
            .map(|_| b"oh what a beautiful morning, oh what a beautiful day!!".to_vec())
            .flat_map(|v| v)
            .collect()
    }

    fn same_same(a: &[u8], b: &[u8]) -> bool {
        md5::compute(a) == md5::compute(b)
    }

    fn get_data() -> std::result::Result<Vec<u8>, std::io::Error> {
        fs::read(format!(
            "{}/test-data/fireworks.jpeg",
            env!("CARGO_MANIFEST_DIR")
        ))
    }

    #[test]
    fn basic_compress_into() -> Result<()> {
        let data = get_data()?;
        let mut output = vec![0_u8; data.len()]; // assume compression isn't worse than input len.
        let n_bytes = compress_into(data.as_slice(), &mut output, CompressionLevel::Three, true)?;
        println!(
            "n_bytes: {} - {:?}",
            n_bytes,
            &output[..std::cmp::min(output.len() - 1, 100)]
        );
        Ok(())
    }

    #[test]
    fn basic_compress() -> Result<()> {
        let data = get_data()?;
        let output = compress(Cursor::new(data), CompressionLevel::Three, true)?;
        println!(
            "n_bytes: {:?}",
            &output[..std::cmp::min(output.len() - 1, 100)]
        );
        Ok(())
    }

    #[test]
    fn basic_decompress() -> Result<()> {
        // compressed b"hello, world!"
        let compressed = vec![
            31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 203, 72, 205, 201, 201, 215, 81, 40, 207, 47, 202,
            73, 81, 4, 0, 19, 141, 152, 88, 13, 0, 0, 0,
        ];
        let decompressed = decompress(Cursor::new(compressed))?;
        assert_eq!(decompressed, b"hello, world!");
        Ok(())
    }

    #[test]
    fn basic_decompress_into() -> Result<()> {
        // compressed b"hello, world!"
        let compressed = vec![
            31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 203, 72, 205, 201, 201, 215, 81, 40, 207, 47, 202,
            73, 81, 4, 0, 19, 141, 152, 88, 13, 0, 0, 0,
        ];
        let mut decompressed = b"world!, hello".to_vec(); // same len, wrong data
        let n_bytes = decompress_into(&compressed, &mut decompressed)?;
        assert_eq!(n_bytes, decompressed.len());
        assert_eq!(&decompressed, b"hello, world!");
        Ok(())
    }

    #[test]
    fn larger_decompress() -> Result<()> {
        /* Decompress data which is larger than BUF_SIZE */
        let data = get_data()?;
        let compressed = compress(Cursor::new(&data), CompressionLevel::Three, true)?;
        let decompressed = decompress(Cursor::new(compressed))?;
        assert!(same_same(&data, &decompressed));
        Ok(())
    }

    #[test]
    fn basic_decompress_multi_stream() -> Result<()> {
        // compressed b"hello, world!" * 2
        let mut compressed = vec![
            31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 203, 72, 205, 201, 201, 215, 81, 40, 207, 47, 202,
            73, 81, 4, 0, 19, 141, 152, 88, 13, 0, 0, 0,
        ];
        compressed.extend(compressed.clone());

        let decompressed = decompress(Cursor::new(compressed))?;
        assert_eq!(decompressed, b"hello, world!hello, world!");
        Ok(())
    }

    #[test]
    fn basic_round_trip() -> Result<()> {
        let data = b"hello, world!";
        let compressed = compress(Cursor::new(&data), CompressionLevel::Three, true)?;
        let decompressed = decompress(Cursor::new(compressed))?;
        assert_eq!(decompressed, data);
        Ok(())
    }

    #[test]
    fn basic_round_trip_into() -> Result<()> {
        let data = b"hello, world!".to_vec();

        let compressed_len = compress(Cursor::new(&data), CompressionLevel::Three, true)?.len();
        let decompressed_len = data.len();

        let mut compressed = vec![0; compressed_len];
        let mut decompressed = vec![0; decompressed_len];

        // compress_into
        let n_bytes = compress_into(&data, &mut compressed, CompressionLevel::Three, true)?;
        assert_eq!(n_bytes, compressed_len);

        // decompress_into
        let n_bytes = decompress_into(&compressed, &mut decompressed)?;
        assert_eq!(n_bytes, decompressed_len);

        // round trip output matches original input
        assert!(same_same(&data, &decompressed));
        Ok(())
    }

    #[test]
    fn large_round_trip_into() -> Result<()> {
        let data = gen_large_data();

        let compressed_len = compress(Cursor::new(&data), CompressionLevel::Three, true)?.len();
        let decompressed_len = data.len();

        let mut compressed = vec![0; compressed_len];
        let mut decompressed = vec![0; decompressed_len];

        // compress_into
        let n_bytes = compress_into(&data, &mut compressed, CompressionLevel::Three, true)?;
        assert!(n_bytes < data.len());

        // decompress_into
        let n_bytes = decompress_into(&compressed, &mut decompressed)?;
        assert_eq!(n_bytes, decompressed_len);

        // round trip output matches original input
        assert!(same_same(&data, &decompressed));
        Ok(())
    }
}
