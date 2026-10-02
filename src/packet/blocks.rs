//! Block updates, chunk unloads and respawn metadata for protocols 763–776.
//!
//! Layouts follow the pinned protocol schemas. Section records are unsigned
//! packed values on the wire's VarLong, with 12 position bits below the state ID
//! (also verified against MCProtocolLib's ClientboundSectionBlocksUpdatePacket).
use super::{DeathLocation, DimensionRef, SpawnInfo};
use crate::{
    codec::{BlockPosition, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

fn registry_id(n: i32) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid("negative block state ID"))
    } else {
        Ok(n)
    }
}
trait Body: Sized {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()>;
}
macro_rules! codec {
    ($($t:ty),* $(,)?) => {$ (
        impl $t {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet { return Err(Error::Limit("world packet bytes")); }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            /// Encoding is transactional: no partial body is returned on failure.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                self.write(&mut w, version, limits)?;
                if w.as_slice().len() > limits.max_packet { return Err(Error::Limit("world packet bytes")); }
                Ok(w.into_inner())
            }
        }
    )*};
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockUpdate {
    pub position: BlockPosition,
    pub state_id: i32,
}
impl Body for BlockUpdate {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            state_id: registry_id(r.var_i32()?)?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.i64(self.position.pack()?);
        w.var_i32(registry_id(self.state_id)?);
        Ok(())
    }
}
/// Section coordinates occupy signed 22-bit X/Z and signed 20-bit Y fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl SectionPosition {
    pub fn pack(self) -> Result<i64> {
        if !(-2_097_152..2_097_152).contains(&self.x)
            || !(-2_097_152..2_097_152).contains(&self.z)
            || !(-524_288..524_288).contains(&self.y)
        {
            return Err(Error::Invalid("section position range"));
        }
        Ok(((self.x as i64 & 0x3fffff) << 42)
            | ((self.z as i64 & 0x3fffff) << 20)
            | (self.y as i64 & 0xfffff))
    }
    pub fn unpack(packed: i64) -> Self {
        Self {
            x: (packed >> 42) as i32,
            z: ((packed << 22) >> 42) as i32,
            y: ((packed << 44) >> 44) as i32,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionBlockUpdate {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub state_id: i32,
}
impl SectionBlockUpdate {
    fn pack(self) -> Result<i64> {
        if self.x > 15 || self.y > 15 || self.z > 15 {
            return Err(Error::Invalid("section local position"));
        }
        Ok(((registry_id(self.state_id)? as i64) << 12)
            | ((self.x as i64) << 8)
            | ((self.z as i64) << 4)
            | self.y as i64)
    }
    fn unpack(packed: i64) -> Result<Self> {
        if packed < 0 || packed >> 12 > i32::MAX as i64 {
            return Err(Error::Invalid("section block state ID"));
        }
        Ok(Self {
            x: ((packed >> 8) & 15) as u8,
            y: (packed & 15) as u8,
            z: ((packed >> 4) & 15) as u8,
            state_id: (packed >> 12) as i32,
        })
    }
    pub fn world_position(self, section: SectionPosition) -> Result<BlockPosition> {
        self.pack()?;
        section.pack()?;
        // The full section-Y range exceeds packed BlockPosition's 12-bit Y range.
        // This returns coordinates, which callers may use without re-packing them.
        Ok(BlockPosition {
            x: section.x * 16 + self.x as i32,
            y: section.y * 16 + self.y as i32,
            z: section.z * 16 + self.z as i32,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionBlockUpdates {
    pub section: SectionPosition,
    pub records: Vec<SectionBlockUpdate>,
}
impl Body for SectionBlockUpdates {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let section = SectionPosition::unpack(r.i64()?);
        let count = r.count(r.limits.max_collection.min(4096))?;
        if count > r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut records = Vec::with_capacity(count);
        for _ in 0..count {
            records.push(SectionBlockUpdate::unpack(r.var_i64()?)?);
        }
        Ok(Self { section, records })
    }
    fn write(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        if self.records.len() > limits.max_collection.min(4096).min(limits.max_packet) {
            return Err(Error::Limit("section block update count"));
        }
        w.i64(self.section.pack()?);
        w.var_i32(self.records.len() as i32);
        for record in &self.records {
            w.var_i64(record.pack()?);
            if w.as_slice().len() > limits.max_packet {
                return Err(Error::Limit("world packet bytes"));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnloadChunk {
    pub x: i32,
    pub z: i32,
}
impl Body for UnloadChunk {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        let first = r.i32()?;
        let second = r.i32()?;
        Ok(if v.protocol() == 763 {
            Self {
                x: first,
                z: second,
            }
        } else {
            Self {
                x: second,
                z: first,
            }
        })
    }
    fn write(&self, w: &mut Writer, v: Version, _: Limits) -> Result<()> {
        if v.protocol() == 763 {
            w.i32(self.x);
            w.i32(self.z);
        } else {
            w.i32(self.z);
            w.i32(self.x);
        }
        Ok(())
    }
}
/// Retained player-data bitmask, including protocols 763–765.
/// The older upstream schema labels this byte a boolean; the 1.20.1 server
/// codec was inspected to verify that all four values are meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RespawnData {
    pub attributes: bool,
    pub entity_data: bool,
}
impl RespawnData {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let flags = r.u8()?;
        if flags & !3 != 0 {
            return Err(Error::Invalid("respawn keep-data flags"));
        }
        Ok(Self {
            attributes: flags & 1 != 0,
            entity_data: flags & 2 != 0,
        })
    }
    fn flags(self) -> u8 {
        u8::from(self.attributes) | (u8::from(self.entity_data) << 1)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Respawn {
    pub spawn: SpawnInfo,
    pub data: RespawnData,
}
impl Body for Respawn {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        if v.protocol() >= 766 {
            let spawn = SpawnInfo::read(r, v)?;
            if spawn.portal_cooldown < 0 {
                return Err(Error::Invalid("negative portal cooldown"));
            }
            let data = RespawnData::read(r)?;
            return Ok(Self { spawn, data });
        }
        let dimension_type = DimensionRef::Name(r.string(32767)?.into());
        let dimension_name = r.string(32767)?.into();
        let hashed_seed = r.i64()?;
        let game_mode = r.u8()?;
        let previous_game_mode = r.u8()? as i8;
        if game_mode > 3 || !(-1..=3).contains(&previous_game_mode) {
            return Err(Error::Invalid("respawn game mode"));
        }
        let is_debug = r.bool()?;
        let is_flat = r.bool()?;
        let early_copy = if v.protocol() == 763 {
            Some(RespawnData::read(r)?)
        } else {
            None
        };
        let death = if r.bool()? {
            Some(DeathLocation {
                dimension: r.string(32767)?.into(),
                position: BlockPosition::unpack(r.i64()?),
            })
        } else {
            None
        };
        let portal_cooldown = r.var_i32()?;
        if portal_cooldown < 0 {
            return Err(Error::Invalid("negative portal cooldown"));
        }
        let copy_metadata = match early_copy {
            Some(value) => value,
            None => RespawnData::read(r)?,
        };
        Ok(Self {
            spawn: SpawnInfo {
                dimension_type,
                dimension_name,
                hashed_seed,
                game_mode,
                previous_game_mode,
                is_debug,
                is_flat,
                death,
                portal_cooldown,
                sea_level: None,
            },
            data: copy_metadata,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, limits: Limits) -> Result<()> {
        let s = &self.spawn;
        if s.game_mode > 3 || !(-1..=3).contains(&s.previous_game_mode) || s.portal_cooldown < 0 {
            return Err(Error::Invalid("respawn world settings"));
        }
        match (&s.dimension_type, v.protocol() >= 766) {
            (DimensionRef::Name(name), false) => {
                w.string(name, limits.max_string_chars.min(32767))?
            }
            (DimensionRef::Id(id), true) if *id <= i32::MAX as u32 => w.var_i32(*id as i32),
            _ => {
                return Err(Error::Invalid(
                    "respawn dimension representation for protocol",
                ))
            }
        }
        w.string(&s.dimension_name, limits.max_string_chars.min(32767))?;
        w.i64(s.hashed_seed);
        w.u8(s.game_mode);
        w.u8(s.previous_game_mode as u8);
        w.bool(s.is_debug);
        w.bool(s.is_flat);
        let flags = self.data.flags();
        if v.protocol() == 763 {
            w.u8(flags);
        }
        w.bool(s.death.is_some());
        if let Some(death) = &s.death {
            w.string(&death.dimension, limits.max_string_chars.min(32767))?;
            w.i64(death.position.pack()?);
        }
        w.var_i32(s.portal_cooldown);
        match (s.sea_level, v.protocol() >= 768) {
            (Some(level), true) => w.var_i32(level),
            (None, false) => {}
            _ => return Err(Error::Invalid("respawn sea level for protocol")),
        }
        if v.protocol() != 763 {
            w.u8(flags);
        }
        Ok(())
    }
}
codec!(BlockUpdate, SectionBlockUpdates, UnloadChunk, Respawn);
#[derive(Debug, Clone, PartialEq)]
pub enum WorldPacket {
    Block(BlockUpdate),
    SectionBlocks(SectionBlockUpdates),
    Unload(UnloadChunk),
    Respawn(Respawn),
}
impl WorldPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "block_change" => Self::Block(BlockUpdate::decode(bytes, version, limits)?),
            "multi_block_change" => {
                Self::SectionBlocks(SectionBlockUpdates::decode(bytes, version, limits)?)
            }
            "unload_chunk" => Self::Unload(UnloadChunk::decode(bytes, version, limits)?),
            "respawn" => Self::Respawn(Respawn::decode(bytes, version, limits)?),
            _ => return Err(Error::Unsupported("typed world update packet")),
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        let (name, body) = match self {
            Self::Block(p) => ("block_change", p.encode(version, limits)?),
            Self::SectionBlocks(p) => ("multi_block_change", p.encode(version, limits)?),
            Self::Unload(p) => ("unload_chunk", p.encode(version, limits)?),
            Self::Respawn(p) => ("respawn", p.encode(version, limits)?),
        };
        Ok(RawPacket::new(
            version.packet_id(State::Play, Direction::Clientbound, name)?,
            body,
        ))
    }
}
