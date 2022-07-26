use std::io;
use std::mem;
use std::os::raw::c_int;

pub(crate) use crate::error::{Error, Result};
use isal_sys as isal;

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
pub enum CompressionReturnValues {
    CompOk = isal::COMP_OK as _,
    InvalidFlush = isal::INVALID_FLUSH as _,
    InvalidParam = isal::INVALID_PARAM as _,
    StatelessOverflow = isal::STATELESS_OVERFLOW as _,
    InvalidOperation = isal::ISAL_INVALID_OPERATION as _,
    InvalidState = isal::ISAL_INVALID_STATE as _,
    InvalidLevel = isal::ISAL_INVALID_LEVEL as _,
    InvalidLevelBuf = isal::ISAL_INVALID_LEVEL_BUF as _,
}

impl TryFrom<isize> for CompressionReturnValues {
    type Error = Error;

    #[inline]
    fn try_from(value: isize) -> Result<Self> {
        match value {
            v if v == CompressionReturnValues::CompOk as _ => Ok(CompressionReturnValues::CompOk),
            v if v == CompressionReturnValues::InvalidFlush as _ => {
                Ok(CompressionReturnValues::InvalidFlush)
            }
            v if v == CompressionReturnValues::InvalidParam as _ => {
                Ok(CompressionReturnValues::InvalidParam)
            }
            v if v == CompressionReturnValues::StatelessOverflow as _ => {
                Ok(CompressionReturnValues::StatelessOverflow)
            }
            v if v == CompressionReturnValues::InvalidOperation as _ => {
                Ok(CompressionReturnValues::InvalidOperation)
            }
            v if v == CompressionReturnValues::InvalidState as _ => {
                Ok(CompressionReturnValues::InvalidState)
            }
            v if v == CompressionReturnValues::InvalidLevel as _ => {
                Ok(CompressionReturnValues::InvalidLevel)
            }
            v if v == CompressionReturnValues::InvalidLevelBuf as _ => {
                Ok(CompressionReturnValues::InvalidLevelBuf)
            }
            _ => Err(Error::Other((
                Some(value),
                "Unknown exit code from compression".to_string(),
            ))),
        }
    }
}

/// Decompression return values
#[derive(Copy, Clone, Debug)]
#[repr(i8)]
pub enum DecompressionReturnValues {
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

impl TryFrom<isize> for DecompressionReturnValues {
    type Error = Error;

    #[inline]
    fn try_from(value: isize) -> Result<Self> {
        match value {
            v if v == DecompressionReturnValues::DecompOk as _ => {
                Ok(DecompressionReturnValues::DecompOk)
            }
            v if v == DecompressionReturnValues::EndInput as _ => {
                Ok(DecompressionReturnValues::EndInput)
            }
            v if v == DecompressionReturnValues::OutOverflow as _ => {
                Ok(DecompressionReturnValues::OutOverflow)
            }
            v if v == DecompressionReturnValues::NameOverflow as _ => {
                Ok(DecompressionReturnValues::NameOverflow)
            }
            v if v == DecompressionReturnValues::CommentOverflow as _ => {
                Ok(DecompressionReturnValues::CommentOverflow)
            }
            v if v == DecompressionReturnValues::ExtraOverflow as _ => {
                Ok(DecompressionReturnValues::ExtraOverflow)
            }
            v if v == DecompressionReturnValues::NeedDict as _ => {
                Ok(DecompressionReturnValues::NeedDict)
            }
            v if v == DecompressionReturnValues::InvalidBlock as _ => {
                Ok(DecompressionReturnValues::InvalidBlock)
            }
            v if v == DecompressionReturnValues::InvalidSymbol as _ => {
                Ok(DecompressionReturnValues::InvalidSymbol)
            }
            v if v == DecompressionReturnValues::InvalidLoopBack as _ => {
                Ok(DecompressionReturnValues::InvalidLoopBack)
            }
            v if v == DecompressionReturnValues::InvalidWrapper as _ => {
                Ok(DecompressionReturnValues::InvalidWrapper)
            }
            v if v == DecompressionReturnValues::UnsupportedMethod as _ => {
                Ok(DecompressionReturnValues::UnsupportedMethod)
            }
            v if v == DecompressionReturnValues::IncorrectChecksum as _ => {
                Ok(DecompressionReturnValues::IncorrectChecksum)
            }
            _ => Err(Error::Other((
                Some(value),
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

pub mod read {

    use super::*;

    /// Streaming compression for input streams implementing `std::io::Read`.
    ///
    /// Notes
    /// -----
    /// One should consider using `crate::igzip::compress` or `crate::igzip::compress_into` if possible.
    /// In that context, we do not need to hold and maintain intermediate buffers for reading and writing.
    pub struct Encoder<R: io::Read> {
        inner: R,
        stream: isal::isal_zstream,
        in_buf: [u8; BUF_SIZE],
        out_buf: Vec<u8>,
        dsts: usize,
        dste: usize,

        #[allow(dead_code)] // held for releasing memory, buffer is only used by zstream
        level_buf: Vec<u8>,
    }

    impl<R: io::Read> Encoder<R> {
        /// Create a new `Encoder` which implements the `std::io::Read` trait.
        ///
        /// Example
        /// -------
        /// ```
        /// use std::{io, io::Read};
        /// use isal_rs::igzip::{read::Encoder, CompressionLevel, decompress};
        /// let data = b"Hello, World!".to_vec();
        ///
        /// let mut encoder = Encoder::new(data.as_slice(), CompressionLevel::Three, true);
        /// let mut output = vec![];
        ///
        /// // Numbeer of compressed bytes written to `output`
        /// let n = io::copy(&mut encoder, &mut output).unwrap();
        /// assert_eq!(n as usize, output.len());
        ///
        /// let decompressed = decompress(&output).unwrap();
        /// assert_eq!(decompressed.as_slice(), data);
        /// ```
        pub fn new(reader: R, level: CompressionLevel, is_gzip: bool) -> Encoder<R> {
            let in_buf = [0_u8; BUF_SIZE];
            let out_buf = Vec::with_capacity(BUF_SIZE);

            let mut zstream = new_zstream(isal::isal_deflate_init);

            zstream.end_of_stream = 0;
            zstream.flush = FlushFlags::SyncFlush as _;
            zstream.level = level as _;
            zstream.gzip_flag = is_gzip as _;

            // TODO: set level buf sizes
            let mut level_buf = vec![0_u8; isal::ISAL_DEF_LVL3_DEFAULT as _];
            zstream.level_buf = level_buf.as_mut_ptr();
            zstream.level_buf_size = level_buf.len() as _;

            Self {
                inner: reader,
                stream: zstream,
                level_buf,
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
            } else if self.stream.internal_state.state != isal::isal_zstate_state_ZSTATE_END {
                // Read out next buf len worth to compress; filling intermediate out_buf
                self.stream.avail_in = self.inner.read(&mut self.in_buf)? as _;
                if self.stream.avail_in < self.in_buf.len() as _ {
                    self.stream.end_of_stream = 1;
                }
                self.stream.next_in = self.in_buf.as_mut_ptr();

                let mut n_bytes = 0;
                self.out_buf.truncate(0);

                // compress this chunk into out_buf
                while self.stream.avail_in > 0 {
                    self.out_buf.resize(self.out_buf.len() + BUF_SIZE, 0);

                    self.stream.avail_out = BUF_SIZE as _;
                    self.stream.next_out = self.out_buf[n_bytes..n_bytes + BUF_SIZE].as_mut_ptr();

                    isal_deflate_core(&mut self.stream, isal::isal_deflate)?;

                    n_bytes += BUF_SIZE - self.stream.avail_out as usize;
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

    #[cfg(test)]
    mod tests {

        use super::*;
        use std::io::{self, Cursor};

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
    }
}

/// Create a new zstream, calling the `init` operation from underlying isal lib.
#[inline(always)]
pub(crate) fn new_zstream(
    init: unsafe extern "C" fn(*mut isal::isal_zstream),
) -> isal::isal_zstream {
    let mut zstream_uninit: mem::MaybeUninit<isal::isal_zstream> = mem::MaybeUninit::uninit();
    unsafe { init(zstream_uninit.as_mut_ptr()) };
    unsafe { zstream_uninit.assume_init() }
}

/// Compress `input` directly into `output`. This is the fastest possible compression available.
#[inline(always)]
pub fn compress_into(
    input: &[u8],
    output: &mut [u8],
    level: CompressionLevel,
    is_gzip: bool,
) -> Result<usize> {
    let mut zstream = new_zstream(isal::isal_deflate_stateless_init);

    zstream.flush = FlushFlags::NoFlush as _;
    zstream.level = level as _;
    zstream.gzip_flag = is_gzip as _;
    zstream.end_of_stream = 1;

    let level_buf_size = isal::ISAL_DEF_LVL3_LARGE; //mem_level_to_bufsize(level, mem_level);
    let mut level_buf = vec![0_u8; level_buf_size as _];
    zstream.level_buf = level_buf.as_mut_ptr();
    zstream.level_buf_size = level_buf.len() as _;

    // read input into buffer
    zstream.avail_in = input.len() as _;
    zstream.next_in = input.as_ptr() as *mut _;

    // compress this block in its entirety
    zstream.avail_out = output.len() as _;
    zstream.next_out = output.as_mut_ptr();

    isal_deflate_core(&mut zstream, isal::isal_deflate_stateless)?;
    Ok(zstream.total_out as _)
}

/// Compress `input`
#[inline(always)]
pub fn compress(input: &[u8], level: CompressionLevel, is_gzip: bool) -> Result<Vec<u8>> {
    let mut zstream = new_zstream(isal::isal_deflate_init);

    zstream.end_of_stream = 1;
    zstream.flush = FlushFlags::NoFlush as _;

    zstream.level = level as _;
    zstream.gzip_flag = is_gzip as _;

    let level_buf_size = isal::ISAL_DEF_LVL3_DEFAULT; // TODO: set level buf sizes
    let mut level_buf = vec![0_u8; level_buf_size as _];
    zstream.level_buf = level_buf.as_mut_ptr();
    zstream.level_buf_size = level_buf.len() as _;

    // TODO: impl level one condition: https://github.com/intel/isa-l/blob/62519d97ec8242dce393a1f81593f4f67da3ac92/igzip/igzip_example.c#L70
    // read input into buffer
    zstream.avail_in = input.len() as _;
    zstream.next_in = input.as_ptr() as *mut _;

    // compress input
    let mut buf = Vec::with_capacity(BUF_SIZE);
    let mut n_bytes = 0;
    while zstream.internal_state.state != isal::isal_zstate_state_ZSTATE_END {
        buf.resize(buf.len() + BUF_SIZE, 0);

        zstream.avail_out = BUF_SIZE as _;
        zstream.next_out = buf[n_bytes..n_bytes + BUF_SIZE].as_mut_ptr();

        isal_deflate_core(&mut zstream, isal::isal_deflate)?;

        n_bytes += BUF_SIZE - zstream.avail_out as usize;
    }
    buf.truncate(n_bytes);
    Ok(buf)
}

#[inline(always)]
pub(crate) fn new_inflate_state(
    init: unsafe extern "C" fn(*mut isal::inflate_state),
) -> isal::inflate_state {
    let mut uninit: mem::MaybeUninit<isal::inflate_state> = mem::MaybeUninit::uninit();
    unsafe { init(uninit.as_mut_ptr()) };
    unsafe { uninit.assume_init() }
}

#[inline(always)]
pub fn decompress(input: &[u8]) -> Result<Vec<u8>> {
    let mut zst = new_inflate_state(isal::isal_inflate_init);
    zst.avail_in = input.len() as _;
    zst.next_in = input.as_ptr() as *mut _;
    zst.crc_flag = 1;

    let mut gz_hdr = isal::isal_gzip_header::default();
    unsafe { isal::isal_gzip_header_init(&mut gz_hdr as *mut _) };

    let mut buf = Vec::with_capacity(BUF_SIZE);
    let mut n_bytes = 0;

    while zst.avail_in != 0 {
        // Ensure reset for next member (if exists; not on first iteration)
        if n_bytes > 0 {
            unsafe { isal::isal_inflate_reset(&mut zst as *mut _) };
        }

        // Read this member's gzip header
        read_gzip_header(&mut zst, &mut gz_hdr)?;

        // decompress member
        while zst.block_state != isal::isal_block_state_ISAL_BLOCK_FINISH {
            buf.resize(buf.len() + BUF_SIZE, 0);
            zst.next_out = buf[n_bytes..n_bytes + BUF_SIZE].as_mut_ptr();
            zst.avail_out = BUF_SIZE as _;

            isal_inflate_core(&mut zst, isal::isal_inflate)?;

            n_bytes += BUF_SIZE - zst.avail_out as usize;
        }
    }

    buf.truncate(n_bytes);

    Ok(buf)
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
    let ret = unsafe { isal::isal_read_gzip_header(zst as *mut _, gz_hdr as *mut _) } as isize;
    match DecompressionReturnValues::try_from(ret)? {
        DecompressionReturnValues::DecompOk => Ok(()),
        r => Err(Error::DecompressionError(r)),
    }
}

#[inline(always)]
pub fn decompress_into(input: &[u8], output: &mut [u8]) -> Result<usize> {
    let mut zst = new_inflate_state(isal::isal_inflate_init);
    zst.avail_in = input.len() as _;
    zst.next_in = input.as_ptr() as *mut _;
    zst.crc_flag = 1;

    zst.avail_out = output.len() as _;
    zst.next_out = output.as_mut_ptr();

    isal_inflate_core(&mut zst, isal::isal_inflate_stateless)?;

    Ok(zst.total_out as _)
}

/// Combine error handling for both isal_deflate/_stateless functions
#[inline(always)]
fn isal_inflate_core(
    zst: &mut isal::inflate_state,
    op: unsafe extern "C" fn(*mut isal::inflate_state) -> c_int,
) -> Result<()> {
    let ret = unsafe { op(zst as *mut _) } as isize;
    match DecompressionReturnValues::try_from(ret)? {
        DecompressionReturnValues::DecompOk => Ok(()),
        r => Err(Error::DecompressionError(r)),
    }
}

/// Combine error handling for both isal_deflate/_stateless functions
#[inline(always)]
fn isal_deflate_core(
    zstream: &mut isal::isal_zstream,
    op: unsafe extern "C" fn(*mut isal::isal_zstream) -> c_int,
) -> Result<()> {
    let ret = unsafe { op(zstream as *mut _) } as isize;
    match CompressionReturnValues::try_from(ret)? {
        CompressionReturnValues::CompOk => Ok(()),
        r => Err(Error::CompressionError(r)),
    }
}

#[cfg(test)]
mod tests {

    use md5;
    use std::fs;

    use super::*;

    fn same_same(a: &[u8], b: &[u8]) -> bool {
        md5::compute(a) == md5::compute(b)
    }

    fn get_data() -> std::result::Result<Vec<u8>, std::io::Error> {
        fs::read(format!(
            "{}/../../pyrus-cramjam/benchmarks/data/fireworks.jpeg",
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
        let output = compress(&data, CompressionLevel::Three, true)?;
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
        let decompressed = decompress(&compressed)?;
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
        let compressed = compress(&data, CompressionLevel::Three, true)?;
        let decompressed = decompress(&compressed)?;
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

        let decompressed = decompress(&compressed)?;
        assert_eq!(decompressed, b"hello, world!hello, world!");
        Ok(())
    }

    #[test]
    fn basic_round_trip() -> Result<()> {
        let data = b"hello, world!";
        let compressed = compress(data, CompressionLevel::Three, true)?;
        let decompressed = decompress(&compressed)?;
        assert_eq!(decompressed, data);
        Ok(())
    }

    #[test]
    fn basic_round_trip_into() -> Result<()> {
        let data = b"hello, world!".to_vec();

        let compressed_len = compress(&data, CompressionLevel::Three, true)?.len();
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
}
