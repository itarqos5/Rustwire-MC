//! Packet framing and negotiated zlib compression with allocation limits.
use crate::{
    codec::{Reader, Writer},
    Error, Limits, Result,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawPacket {
    pub id: i32,
    pub data: Vec<u8>,
}
impl RawPacket {
    pub fn new(id: i32, data: impl Into<Vec<u8>>) -> Self {
        Self {
            id,
            data: data.into(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct FrameCodec {
    pub limits: Limits,
    compression_threshold: Option<usize>,
}
impl Default for FrameCodec {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}
impl FrameCodec {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            compression_threshold: None,
        }
    }
    pub fn compression_threshold(&self) -> Option<usize> {
        self.compression_threshold
    }
    pub fn set_compression(&mut self, threshold: Option<usize>) -> Result<()> {
        #[cfg(not(feature = "compression"))]
        if threshold.is_some() {
            return Err(Error::Unsupported("enable compression feature"));
        }
        if threshold.is_some_and(|n| n > self.limits.max_packet) {
            return Err(Error::Limit("compression threshold"));
        }
        self.compression_threshold = threshold;
        Ok(())
    }
    pub fn encode(&self, packet: &RawPacket) -> Result<Vec<u8>> {
        if packet.id < 0 {
            return Err(Error::Invalid("negative packet id"));
        }
        if packet.data.len() > self.limits.max_packet {
            return Err(Error::Limit("packet"));
        }
        let mut inner = Writer::with_capacity(packet.data.len() + 5);
        inner.var_i32(packet.id);
        inner.raw(&packet.data);
        if inner.as_slice().len() > self.limits.max_packet {
            return Err(Error::Limit("packet"));
        }
        let inner = inner.into_inner();
        let body = if let Some(threshold) = self.compression_threshold {
            let mut body = Writer::new();
            if inner.len() >= threshold {
                body.var_i32(inner.len() as i32);
                #[cfg(feature = "compression")]
                {
                    use std::io::Write;
                    let mut z =
                        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
                    z.write_all(&inner)?;
                    body.raw(&z.finish()?);
                }
                #[cfg(not(feature = "compression"))]
                return Err(Error::Unsupported("compression"));
            } else {
                body.var_i32(0);
                body.raw(&inner);
            }
            body.into_inner()
        } else {
            inner
        };
        if body.is_empty() || body.len() > self.limits.max_frame.min(2_097_151) {
            return Err(Error::Limit("frame"));
        }
        let mut out = Writer::with_capacity(body.len() + 3);
        out.var_i32(body.len() as i32);
        out.raw(&body);
        Ok(out.into_inner())
    }
    /// Decodes the frame body (without its outer length).
    pub fn decode_body(&self, body: &[u8]) -> Result<RawPacket> {
        if body.is_empty() || body.len() > self.limits.max_frame.min(2_097_151) {
            return Err(Error::Limit("frame"));
        }
        let mut r = Reader::new(body, self.limits);
        #[cfg(feature = "compression")]
        let decompressed;
        let data = if let Some(threshold) = self.compression_threshold {
            let expected = r.count(self.limits.max_packet)?;
            if expected == 0 {
                if r.remaining().len() >= threshold {
                    return Err(Error::Invalid(
                        "uncompressed packet at compression threshold",
                    ));
                }
                r.remaining()
            } else {
                if expected < threshold {
                    return Err(Error::Invalid("compressed packet below threshold"));
                }
                #[cfg(feature = "compression")]
                {
                    use flate2::{Decompress, FlushDecompress, Status};
                    // One spare byte detects streams that lie about decompressed size.
                    let mut output = vec![
                        0;
                        expected
                            .checked_add(1)
                            .ok_or(Error::Limit("decompressed size"))?
                    ];
                    let mut z = Decompress::new(true);
                    let status = z
                        .decompress(r.remaining(), &mut output, FlushDecompress::Finish)
                        .map_err(|_| Error::Invalid("zlib stream"))?;
                    if status != Status::StreamEnd
                        || z.total_out() != expected as u64
                        || z.total_in() != r.remaining().len() as u64
                    {
                        return Err(Error::Invalid("zlib size or trailing data"));
                    }
                    output.truncate(expected);
                    decompressed = output;
                    &decompressed
                }
                #[cfg(not(feature = "compression"))]
                {
                    return Err(Error::Unsupported("compression"));
                }
            }
        } else {
            body
        };
        if data.len() > self.limits.max_packet {
            return Err(Error::Limit("packet"));
        }
        let mut r = Reader::new(data, self.limits);
        let id = r.var_i32()?;
        if id < 0 {
            return Err(Error::Invalid("negative packet id"));
        }
        Ok(RawPacket::new(id, r.remaining()))
    }
    /// Transactional partial-frame decode. On incomplete input, consumes nothing.
    pub fn decode(&self, input: &mut &[u8]) -> Result<Option<RawPacket>> {
        let mut n = 0usize;
        for i in 0..3 {
            let Some(&b) = input.get(i) else {
                return Ok(None);
            };
            n |= ((b & 0x7f) as usize) << (7 * i);
            if b & 0x80 == 0 {
                if n == 0 || n > self.limits.max_frame {
                    return Err(Error::Limit("frame"));
                }
                let end = i + 1 + n;
                if input.len() < end {
                    return Ok(None);
                }
                let packet = self.decode_body(&input[i + 1..end])?;
                *input = &input[end..];
                return Ok(Some(packet));
            }
        }
        Err(Error::Invalid("frame length exceeds three bytes"))
    }
}
