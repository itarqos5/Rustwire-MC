//! Borrowing, checked binary codecs. Minecraft VarInts are two's-complement, not zigzag.
use crate::{Error, Limits, Result};
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    input: &'a [u8],
    offset: usize,
    pub limits: Limits,
}
impl<'a> Reader<'a> {
    pub fn new(input: &'a [u8], limits: Limits) -> Self {
        Self {
            input,
            offset: 0,
            limits,
        }
    }
    pub fn position(&self) -> usize {
        self.offset
    }
    pub fn remaining(&self) -> &'a [u8] {
        &self.input[self.offset..]
    }
    pub fn finish(&self) -> Result<()> {
        if self.remaining().is_empty() {
            Ok(())
        } else {
            Err(Error::Invalid("trailing bytes"))
        }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(Error::Limit("offset overflow"))?;
        let bytes = self.input.get(self.offset..end).ok_or(Error::Eof)?;
        self.offset = end;
        Ok(bytes)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Invalid("boolean")),
        }
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    pub fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.i32()? as u32))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.i64()? as u64))
    }
    pub fn var_i32(&mut self) -> Result<i32> {
        let mut value = 0u32;
        for i in 0..5 {
            let byte = self.u8()?;
            if i == 4 && byte & 0xf0 != 0 {
                return Err(Error::Invalid("VarInt overflow"));
            }
            value |= ((byte & 0x7f) as u32) << (i * 7);
            if byte & 0x80 == 0 {
                return Ok(value as i32);
            }
        }
        Err(Error::Invalid("VarInt too long"))
    }
    pub fn var_i64(&mut self) -> Result<i64> {
        let mut value = 0u64;
        for i in 0..10 {
            let byte = self.u8()?;
            if i == 9 && byte & 0xfe != 0 {
                return Err(Error::Invalid("VarLong overflow"));
            }
            value |= ((byte & 0x7f) as u64) << (i * 7);
            if byte & 0x80 == 0 {
                return Ok(value as i64);
            }
        }
        Err(Error::Invalid("VarLong too long"))
    }
    pub fn count(&mut self, maximum: usize) -> Result<usize> {
        let n = self.var_i32()?;
        if n < 0 {
            return Err(Error::Invalid("negative length"));
        }
        let n = n as usize;
        if n > maximum {
            return Err(Error::Limit("collection length"));
        }
        Ok(n)
    }
    pub fn string(&mut self, max_chars: usize) -> Result<&'a str> {
        let max_chars = max_chars.min(self.limits.max_string_chars);
        let len = self.count(
            max_chars
                .checked_mul(3)
                .ok_or(Error::Limit("string length"))?,
        )?;
        let s = std::str::from_utf8(self.take(len)?).map_err(|_| Error::Invalid("UTF-8 string"))?;
        if s.encode_utf16().count() > max_chars {
            return Err(Error::Limit("string UTF-16 length"));
        }
        Ok(s)
    }
    pub fn bytes(&mut self, max: usize) -> Result<&'a [u8]> {
        let n = self.count(max)?;
        self.take(n)
    }
    pub fn uuid(&mut self) -> Result<[u8; 16]> {
        Ok(self.take(16)?.try_into().unwrap())
    }
}
#[derive(Clone, Debug, Default)]
pub struct Writer {
    output: Vec<u8>,
}
impl Writer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            output: Vec::with_capacity(capacity),
        }
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.output
    }
    pub fn into_inner(self) -> Vec<u8> {
        self.output
    }
    pub fn raw(&mut self, data: &[u8]) {
        self.output.extend_from_slice(data);
    }
    pub fn u8(&mut self, n: u8) {
        self.output.push(n);
    }
    pub fn bool(&mut self, b: bool) {
        self.u8(u8::from(b));
    }
    pub fn u16(&mut self, n: u16) {
        self.raw(&n.to_be_bytes());
    }
    pub fn i16(&mut self, n: i16) {
        self.raw(&n.to_be_bytes());
    }
    pub fn i32(&mut self, n: i32) {
        self.raw(&n.to_be_bytes());
    }
    pub fn i64(&mut self, n: i64) {
        self.raw(&n.to_be_bytes());
    }
    pub fn f32(&mut self, n: f32) {
        self.raw(&n.to_be_bytes());
    }
    pub fn f64(&mut self, n: f64) {
        self.raw(&n.to_be_bytes());
    }
    pub fn var_i32(&mut self, n: i32) {
        let mut n = n as u32;
        loop {
            if n & !0x7f == 0 {
                self.u8(n as u8);
                break;
            }
            self.u8((n as u8 & 0x7f) | 0x80);
            n >>= 7;
        }
    }
    pub fn var_i64(&mut self, n: i64) {
        let mut n = n as u64;
        loop {
            if n & !0x7f == 0 {
                self.u8(n as u8);
                break;
            }
            self.u8((n as u8 & 0x7f) | 0x80);
            n >>= 7;
        }
    }
    pub fn string(&mut self, s: &str, max_chars: usize) -> Result<()> {
        if s.encode_utf16().count() > max_chars || s.len() > i32::MAX as usize {
            return Err(Error::Limit("string length"));
        }
        self.var_i32(s.len() as i32);
        self.raw(s.as_bytes());
        Ok(())
    }
    pub fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.len() > i32::MAX as usize {
            return Err(Error::Limit("byte array"));
        }
        self.var_i32(bytes.len() as i32);
        self.raw(bytes);
        Ok(())
    }
}
/// Packed block position used in protocol packets (26-bit X/Z, 12-bit Y).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl BlockPosition {
    pub fn pack(self) -> Result<i64> {
        if !(-33_554_432..33_554_432).contains(&self.x)
            || !(-33_554_432..33_554_432).contains(&self.z)
            || !(-2048..2048).contains(&self.y)
        {
            return Err(Error::Invalid("position range"));
        }
        Ok((((self.x as i64) & 0x3ffffff) << 38)
            | (((self.z as i64) & 0x3ffffff) << 12)
            | ((self.y as i64) & 0xfff))
    }
    pub fn unpack(p: i64) -> Self {
        Self {
            x: (p >> 38) as i32,
            y: ((p << 52) >> 52) as i32,
            z: ((p << 26) >> 38) as i32,
        }
    }
}
