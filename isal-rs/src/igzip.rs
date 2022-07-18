#![allow(dead_code)] // TODO
use std::io::{self, Read, Write};
use std::mem;
use std::os::raw::c_int;

use isal_sys as isal;

/// Buffer size
pub const BUF_SIZE: usize = 16 * 1024;

/// Result type
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub mod read {

    use super::*;

    pub struct Encoder<R: io::Read> {
        inner: R,
        stream: isal::isal_zstream,
        level: CompressionLevel,
        in_buf: [u8; BUF_SIZE],
        out_buf: [u8; BUF_SIZE],
        dsts: usize,
        dste: usize,
        level_buf: Vec<u8>,
        is_gzip: bool, // gzip or deflate
    }

    impl<R: io::Read> Encoder<R> {
        pub fn new(reader: R, level: CompressionLevel, is_gzip: bool) -> Encoder<R> {
            let mut zstream = new_zstream(isal::isal_deflate_init);

            let in_buf = [0_u8; BUF_SIZE as _];
            let out_buf = [0_u8; BUF_SIZE as _];

            zstream.end_of_stream = 0;
            zstream.flush = isal::SYNC_FLUSH as _;

            zstream.level = 3; //level as _;
            zstream.gzip_flag = is_gzip as _;

            let level_buf_size = isal::ISAL_DEF_LVL3_LARGE; // TODO: set level buf sizes
            let mut level_buf = vec![0_u8; level_buf_size as _];
            zstream.level_buf = level_buf.as_mut_ptr();
            zstream.level_buf_size = level_buf.len() as _;

            Self {
                inner: reader,
                stream: zstream,
                level_buf,
                level,
                in_buf,
                out_buf,
                is_gzip,
                dste: 0,
                dsts: 0,
            }
        }

        pub fn get_ref(&mut self) -> &mut R {
            &mut self.inner
        }

        pub fn read_from_out_buf(&mut self, buf: &mut [u8]) -> usize {
            let available_bytes = self.dste - self.dsts;
            let count = std::cmp::min(available_bytes, buf.len());
            buf[..count].copy_from_slice(&self.out_buf[self.dsts..self.dsts + count]);
            self.dsts += count;
            count
        }
    }

    impl<R: io::Read> io::Read for Encoder<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let count = self.read_from_out_buf(buf);
            if count > 0 {
                Ok(count)
            } else if self.stream.internal_state.state != isal::isal_zstate_state_ZSTATE_END {
                // read input into buffer
                self.stream.avail_in = self.inner.read(&mut self.in_buf).map(|v| v as u32)?;
                self.stream.end_of_stream =
                    (self.stream.avail_in < self.in_buf.len() as u32) as u16;

                if self.stream.end_of_stream == 1 {
                    self.stream.flush = isal::FULL_FLUSH as _;
                }
                self.stream.next_in = self.in_buf.as_mut_ptr();

                // compress this block
                self.stream.avail_out = self.out_buf.len() as _;
                self.stream.next_out = self.out_buf.as_mut_ptr();

                debug_assert_eq!(unsafe { isal::isal_deflate(&mut self.stream) }, 0);

                self.dsts = 0;
                self.dste = self.stream.avail_out as usize;

                Ok(self.read_from_out_buf(buf))
            } else {
                Ok(0)
            }

            // TODO: impl level one condition: https://github.com/intel/isa-l/blob/62519d97ec8242dce393a1f81593f4f67da3ac92/igzip/igzip_example.c#L70
        }
    }

    #[cfg(test)]
    mod tests {}
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CompressionLevel {
    Zero,
    One,
    Three,
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
#[inline]
pub fn compress_into(input: &[u8], output: &mut [u8], level: u8, is_gzip: bool) -> Result<usize> {
    let mut zstream = new_zstream(isal::isal_deflate_stateless_init);

    zstream.flush = isal::NO_FLUSH as _;
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

    isal_deflate_core(&mut zstream, isal::isal_deflate_stateless)
}

/// Compress `input`
#[inline(always)]
pub fn compress(input: &[u8], level: CompressionLevel, is_gzip: bool) -> Result<Vec<u8>> {
    let mut buf = vec![];

    let mut n_bytes = 0;

    let mut zstream = new_zstream(isal::isal_deflate_init);

    zstream.end_of_stream = 1;
    zstream.flush = isal::NO_FLUSH as _;

    zstream.level = 3; //level as _;
    zstream.gzip_flag = is_gzip as _;

    let level_buf_size = isal::ISAL_DEF_LVL3_DEFAULT; // TODO: set level buf sizes
    let mut level_buf = vec![0_u8; level_buf_size as _];
    zstream.level_buf = level_buf.as_mut_ptr();
    zstream.level_buf_size = level_buf.len() as _;

    // TODO: impl level one condition: https://github.com/intel/isa-l/blob/62519d97ec8242dce393a1f81593f4f67da3ac92/igzip/igzip_example.c#L70
    // read input into buffer
    zstream.avail_in = input.len() as _;
    zstream.next_in = input.as_ptr() as *mut _;

    // compress this block
    //dbg!(zstream.avail_in, zstream.internal_state.state, zstream.end_of_stream);
    //std::thread::sleep(std::time::Duration::from_millis(50));
    while zstream.internal_state.state != isal::isal_zstate_state_ZSTATE_END {
        buf.resize(buf.len() + BUF_SIZE, 0);
        zstream.avail_out = BUF_SIZE as _;
        zstream.next_out = buf[n_bytes..n_bytes + BUF_SIZE].as_mut_ptr();

        unsafe { isal::isal_deflate(&mut zstream as *mut _) };

        let n = BUF_SIZE - zstream.avail_out as usize;
        n_bytes += n;
    }
    buf.truncate(n_bytes);
    Ok(buf)
}

/// Combine error handling for both isal_deflate/_stateless functions
#[inline(always)]
fn isal_deflate_core(
    zstream: &mut isal::isal_zstream,
    op: unsafe extern "C" fn(*mut isal::isal_zstream) -> c_int,
) -> Result<usize> {
    let ret = unsafe { op(zstream as *mut _) };
    debug_assert!(zstream.avail_in == 0);

    // TODO? Awkward, COMP_OK is u32, and other variants are i32
    if ret as u32 == isal::COMP_OK {
        Ok(zstream.total_out as _)
    } else {
        match ret {
            isal::INVALID_FLUSH => todo!(),
            isal::ISAL_INVALID_LEVEL => todo!(),
            isal::ISAL_INVALID_LEVEL_BUF => todo!(),
            isal::STATELESS_OVERFLOW => todo!(),
            _ => unreachable!("Unaccounted for error from isal_deflate"),
        }
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
enum MemLevel {
    Default,
    Min,
    Small,
    Medium,
    Large,
    ExtraLarge,
}

#[cfg(test)]
mod tests {

    use std::{fs, io::Cursor};

    use super::*;

    fn get_data() -> Vec<u8> {
        fs::read(format!(
            "{}/../../pyrus-cramjam/benchmarks/data/html_x_4",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn basic_compress_into() {
        let data = get_data();
        let mut output = vec![0_u8; data.len()]; // assume compression isn't worse than input len.
        let n_bytes = compress_into(data.as_slice(), &mut output, 3, true).unwrap();
        println!(
            "n_bytes: {} - {:?}",
            n_bytes,
            &output[..std::cmp::min(output.len() - 1, 100)]
        );
    }

    #[test]
    fn basic_compress() {
        let data = get_data();
        let output = compress(&data, CompressionLevel::Three, true).unwrap();
        dbg!(output.len());
        println!(
            "n_bytes: {:?}",
            &output[..std::cmp::min(output.len() - 1, 100)]
        );
    }
}
