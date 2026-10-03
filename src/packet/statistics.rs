//! Bounded clientbound statistics wire envelopes for protocols 763–776.
//!
//! Category and statistic IDs stay unresolved signed VarInts; this module does
//! not guess registry membership, ID limits, or cross-version identity. Values
//! retain the full signed VarInt domain. Entries retain wire order and duplicates
//! rather than applying the receiving client's map/state semantics.
//!
//! Layout evidence is the hash-pinned schemas, not a release-API or live-server
//! oracle. See `docs/map-statistics-wire-audit.md` for the evidence boundary.
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Statistic {
    pub category_id: i32,
    /// ID within the registry selected by category_id.
    pub statistic_id: i32,
    pub value: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    pub entries: Vec<Statistic>,
}

impl Statistics {
    /// Reads one bounded body; the reader is unchanged on failure.
    pub fn read(r: &mut Reader<'_>, _version: Version) -> Result<Self> {
        let limits = r.limits;
        let input = &r.remaining()[..r.remaining().len().min(limits.max_packet)];
        let mut bounded = Reader::new(input, limits);
        let count = bounded.count(limits.max_collection)?;
        // Three VarInts require at least three bytes per entry. Check before
        // reserving memory, even when the caller permits a very large count.
        if count > bounded.remaining().len() / 3 {
            return Err(Error::Eof);
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(Statistic {
                category_id: bounded.var_i32()?,
                statistic_id: bounded.var_i32()?,
                value: bounded.var_i32()?,
            });
        }
        r.take(bounded.position())?;
        Ok(Self { entries })
    }
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("statistics packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let value = Self::read(&mut r, version)?;
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, _version: Version, limits: Limits) -> Result<Vec<u8>> {
        if self.entries.len() > limits.max_collection || self.entries.len() > i32::MAX as usize {
            return Err(Error::Limit("statistics entries"));
        }
        let mut w = Writer::new();
        w.var_i32(self.entries.len() as i32);
        if w.as_slice().len() > limits.max_packet {
            return Err(Error::Limit("statistics packet bytes"));
        }
        for entry in &self.entries {
            w.var_i32(entry.category_id);
            w.var_i32(entry.statistic_id);
            w.var_i32(entry.value);
            if w.as_slice().len() > limits.max_packet {
                return Err(Error::Limit("statistics packet bytes"));
            }
        }
        Ok(w.into_inner())
    }
    /// Appends one body. The destination is unchanged if encoding fails.
    pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.raw(&self.encode(version, limits)?);
        Ok(())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(State::Play, Direction::Clientbound, "statistics")?,
            self.encode(version, limits)?,
        ))
    }
}
