//! Bounds-checked cursor over a byte slice with switchable endianness.

use anyhow::{bail, Result};

pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
    pub big_endian: bool,
}

macro_rules! read_num {
    ($name:ident, $ty:ty) => {
        pub fn $name(&mut self) -> Result<$ty> {
            const N: usize = std::mem::size_of::<$ty>();
            let bytes: [u8; N] = self.bytes(N)?.try_into().unwrap();
            Ok(if self.big_endian {
                <$ty>::from_be_bytes(bytes)
            } else {
                <$ty>::from_le_bytes(bytes)
            })
        }
    };
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0, big_endian: false }
    }

    pub fn big_endian(data: &'a [u8]) -> Self {
        Self { data, pos: 0, big_endian: true }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.remaining() {
            bail!("read of {n} bytes at {} runs past end ({})", self.pos, self.data.len());
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }

    pub fn align(&mut self, n: usize) {
        self.pos = (self.pos + n - 1) / n * n;
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn i8(&mut self) -> Result<i8> {
        Ok(self.bytes(1)?[0] as i8)
    }

    read_num!(u16, u16);
    read_num!(i16, i16);
    read_num!(u32, u32);
    read_num!(i32, i32);
    read_num!(u64, u64);
    read_num!(i64, i64);
    read_num!(f32, f32);
    read_num!(f64, f64);

    /// NUL-terminated string.
    pub fn cstr(&mut self) -> Result<String> {
        let rest = &self.data[self.pos.min(self.data.len())..];
        let Some(len) = rest.iter().position(|&b| b == 0) else {
            bail!("unterminated string at {}", self.pos);
        };
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.pos += len + 1;
        Ok(s)
    }
}
