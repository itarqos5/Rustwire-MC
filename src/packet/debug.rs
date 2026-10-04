//! Bounded debug-sample and subscription envelopes.
//!
//! Samples and registry references are data, not permission or instructions to
//! subscribe. No scheduling, telemetry collection, rendering or replies occur.
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DebugSampleKind {
    TickTime = 0,
}
impl DebugSampleKind {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::TickTime),
            _ => Err(Error::Invalid("debug sample kind")),
        }
    }
}
/// A variable-length array of raw signed samples. Units/positions are not
/// interpreted; the codec does not require a particular sample count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugSample {
    pub samples: Vec<i64>,
    pub kind: DebugSampleKind,
}
/// Legacy request, present in protocols 766–772 only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebugSampleSubscription {
    pub kind: DebugSampleKind,
}
/// Modern request from protocol 773, carrying unresolved debug registry IDs.
/// Wire order and duplicates are retained; vanilla resolves these into a set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebugSubscriptionRequest {
    pub subscriptions: Vec<u32>,
}

fn prefix_len(mut n: usize) -> usize {
    let mut bytes = 1;
    while n > 127 {
        bytes += 1;
        n >>= 7;
    }
    bytes
}
fn count(len: usize, maximum: usize) -> Result<()> {
    if len > maximum.min(i32::MAX as usize) {
        Err(Error::Limit("debug collection length"))
    } else {
        Ok(())
    }
}
trait Body: Sized {
    fn read_body(r: &mut Reader<'_>) -> Result<Self>;
    fn size(&self, limits: Limits) -> Result<usize>;
    fn write_body(&self, w: &mut Writer);
}
macro_rules! body_codec {
    ($ty:ty, $name:literal, $direction:ident) => {
        impl $ty {
            /// Read one body, leaving the reader unchanged on any error.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                version.packet_id(State::Play, Direction::$direction, $name)?;
                let mut body = Reader::new(
                    &r.remaining()[..r.remaining().len().min(r.limits.max_packet)],
                    r.limits,
                );
                let value = Self::read_body(&mut body)?;
                r.take(body.position())?;
                Ok(value)
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("debug packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            /// Check all sizes and fields before reserving a single output buffer.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                version.packet_id(State::Play, Direction::$direction, $name)?;
                let size = self.size(limits)?;
                if size > limits.max_packet {
                    return Err(Error::Limit("debug packet bytes"));
                }
                let mut w = Writer::with_capacity(size);
                self.write_body(&mut w);
                Ok(w.into_inner())
            }
            /// Append one body atomically, with a per-body byte budget.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?);
                Ok(())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::$direction, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    };
}
impl Body for DebugSample {
    fn read_body(r: &mut Reader<'_>) -> Result<Self> {
        let n = r.count(r.limits.max_collection)?;
        let bytes = n
            .checked_mul(8)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::Limit("debug sample bytes"))?;
        if bytes > r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            samples.push(r.i64()?);
        }
        Ok(Self {
            samples,
            kind: DebugSampleKind::read(r)?,
        })
    }
    fn size(&self, limits: Limits) -> Result<usize> {
        count(self.samples.len(), limits.max_collection)?;
        self.samples
            .len()
            .checked_mul(8)
            .and_then(|n| n.checked_add(prefix_len(self.samples.len()) + 1))
            .ok_or(Error::Limit("debug sample bytes"))
    }
    fn write_body(&self, w: &mut Writer) {
        w.var_i32(self.samples.len() as i32);
        for sample in &self.samples {
            w.i64(*sample);
        }
        w.var_i32(self.kind as i32);
    }
}
impl Body for DebugSampleSubscription {
    fn read_body(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            kind: DebugSampleKind::read(r)?,
        })
    }
    fn size(&self, _limits: Limits) -> Result<usize> {
        Ok(1)
    }
    fn write_body(&self, w: &mut Writer) {
        w.var_i32(self.kind as i32);
    }
}
impl DebugSubscriptionRequest {
    /// Conservative limit matching the inspected official 26.2 collection codec.
    pub const MAX_SUBSCRIPTIONS: usize = 32;
}
impl Body for DebugSubscriptionRequest {
    fn read_body(r: &mut Reader<'_>) -> Result<Self> {
        let n = r.count(r.limits.max_collection.min(Self::MAX_SUBSCRIPTIONS))?;
        if n > r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut subscriptions = Vec::with_capacity(n);
        for _ in 0..n {
            let id = r.var_i32()?;
            if id < 0 {
                return Err(Error::Invalid("negative debug subscription registry ID"));
            }
            subscriptions.push(id as u32);
        }
        Ok(Self { subscriptions })
    }
    fn size(&self, limits: Limits) -> Result<usize> {
        count(
            self.subscriptions.len(),
            limits.max_collection.min(Self::MAX_SUBSCRIPTIONS),
        )?;
        let mut size = prefix_len(self.subscriptions.len());
        for id in &self.subscriptions {
            if *id > i32::MAX as u32 {
                return Err(Error::Invalid("debug subscription registry ID"));
            }
            size += prefix_len(*id as usize);
        }
        Ok(size)
    }
    fn write_body(&self, w: &mut Writer) {
        w.var_i32(self.subscriptions.len() as i32);
        for id in &self.subscriptions {
            w.var_i32(*id as i32);
        }
    }
}
body_codec!(DebugSample, "debug_sample", Clientbound);
body_codec!(
    DebugSampleSubscription,
    "debug_sample_subscription",
    Serverbound
);
body_codec!(
    DebugSubscriptionRequest,
    "debug_subscription_request",
    Serverbound
);
