use std::io;

use bitstream_io::{BigEndian, BitWrite, BitWriter, Integer};

pub struct BitstreamIoWriter(BitWriter<Vec<u8>, BigEndian>);

impl BitstreamIoWriter {
    pub fn with_capacity(capacity: usize) -> Self {
        let write = Vec::with_capacity(capacity);

        Self(BitWriter::new(write))
    }

    #[inline(always)]
    pub fn write_bit(&mut self, bit: bool) -> io::Result<()> {
        self.0.write_bit(bit)
    }

    #[inline(always)]
    pub fn write<const BITS: u32, I: Integer>(&mut self, value: I) -> io::Result<()> {
        self.0.write::<BITS, I>(value)
    }

    #[inline(always)]
    pub fn write_const<const BITS: u32, const VALUE: u32>(&mut self) -> io::Result<()> {
        self.0.write_const::<BITS, VALUE>()
    }

    #[inline(always)]
    pub fn write_var<I: Integer>(&mut self, bits: u32, value: I) -> io::Result<()> {
        self.0.write_var(bits, value)
    }

    /// Writes an unsigned Exp-Golomb value in `0..=u64::MAX`.
    /// The maximum value uses 64 prefix zeroes and a zero 64-bit suffix.
    #[inline(always)]
    pub fn write_ue(&mut self, v: u64) -> io::Result<()> {
        if let Some(code_num) = v.checked_add(1) {
            let leading_zeroes = 63 - code_num.leading_zeros();
            self.0.write_unary::<1>(leading_zeroes)?;

            let remaining = code_num - (1u64 << leading_zeroes);
            self.write_var(leading_zeroes, remaining)
        } else {
            self.0.write_unary::<1>(64)?;
            self.write_var(64, 0u64)
        }
    }

    /// Writes a signed Exp-Golomb value in `-i64::MAX..=i64::MAX`.
    /// Returns `InvalidInput` for `i64::MIN` without changing the output.
    #[inline(always)]
    pub fn write_se(&mut self, v: i64) -> io::Result<()> {
        let code_num = v
            .unsigned_abs()
            .checked_mul(2)
            .and_then(|value| {
                if v > 0 {
                    value.checked_sub(1)
                } else {
                    Some(value)
                }
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "write_se: value exceeds signed domain",
                )
            })?;

        self.write_ue(code_num)
    }

    #[inline(always)]
    pub fn write_bytes(&mut self, buf: &[u8]) -> io::Result<()> {
        self.0.write_bytes(buf)
    }

    #[inline(always)]
    pub fn byte_aligned(&self) -> bool {
        self.0.byte_aligned()
    }

    #[inline(always)]
    pub fn byte_align(&mut self) -> io::Result<()> {
        self.0.byte_align()
    }

    #[inline(always)]
    pub fn pad(&mut self, bits: u32) -> io::Result<()> {
        self.0.pad(bits)
    }

    /// None if the writer is not byte aligned
    pub fn as_slice(&mut self) -> Option<&[u8]> {
        self.0.writer().map(|e| e.as_slice())
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.0.into_writer()
    }
}
