//! Ordinary clientbound entity-control bodies for protocols 763–776.
//!
//! These seven layouts are identical in all fourteen hash-pinned schemas; packet
//! IDs still come from the exact version catalog. Entity references, damage
//! registry references, animation codes and floating-point values retain their
//! raw wire values. This module does not resolve registries, validate entity
//! existence or simulate riding, leashing, camera or damage behavior.
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    packet::entity::Angle,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

trait Body: Sized {
    fn read(r: &mut Reader<'_>) -> Result<Self>;
    fn write(&self, w: &mut Writer, limits: Limits) -> Result<()>;
}
macro_rules! codec {
    ($($ty:ty => $name:literal),* $(,)?) => {$ (
        impl $ty {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                version.packet_id(State::Play, Direction::Clientbound, $name)?;
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("entity-control packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r)?;
                r.finish()?;
                Ok(value)
            }
            /// Transactional encoding: no partial body is returned on failure.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                version.packet_id(State::Play, Direction::Clientbound, $name)?;
                let mut w = Writer::new();
                self.write(&mut w, limits)?;
                if w.as_slice().len() > limits.max_packet {
                    return Err(Error::Limit("entity-control packet bytes"));
                }
                Ok(w.into_inner())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::Clientbound, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    )*};
}

fn varint_len(value: i32) -> usize {
    let bits = u32::BITS - (value as u32).leading_zeros();
    bits.max(1).div_ceil(7) as usize
}

/// Replaces a vehicle's ordered passenger list. Empty lists detach all riders.
/// Duplicates and signed entity references are retained as supplied on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPassengers {
    pub entity_id: i32,
    pub passengers: Vec<i32>,
}
impl Body for SetPassengers {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let entity_id = r.var_i32()?;
        let count = r.count(r.limits.max_collection)?;
        // Every passenger consumes at least one byte. Reject impossible counts
        // before allocating, even if the caller supplies very large limits.
        if count > r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut passengers = Vec::with_capacity(count);
        for _ in 0..count {
            passengers.push(r.var_i32()?);
        }
        Ok(Self {
            entity_id,
            passengers,
        })
    }
    fn write(&self, w: &mut Writer, limits: Limits) -> Result<()> {
        let count = self.passengers.len();
        if count > limits.max_collection.min(i32::MAX as usize) {
            return Err(Error::Limit("passenger count"));
        }
        // Preflight exact size, including the VarInt count and signed IDs,
        // before growing the output buffer for an attacker-controlled list.
        let mut size = varint_len(self.entity_id) + varint_len(count as i32);
        if size > limits.max_packet || count > limits.max_packet - size {
            return Err(Error::Limit("entity-control packet bytes"));
        }
        for &id in &self.passengers {
            size = size
                .checked_add(varint_len(id))
                .ok_or(Error::Limit("entity-control packet bytes"))?;
            if size > limits.max_packet {
                return Err(Error::Limit("entity-control packet bytes"));
            }
        }
        w.var_i32(self.entity_id);
        w.var_i32(count as i32);
        for &id in &self.passengers {
            w.var_i32(id);
        }
        Ok(())
    }
}

/// The `attach_entity`/entity-link packet. Both fields are fixed-width i32s,
/// unlike the VarInt references in the passenger packet. The holder reference
/// is retained verbatim, including the unlink sentinel; no riding state is inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachEntity {
    pub entity_id: i32,
    pub holder_id: i32,
}
impl Body for AttachEntity {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            entity_id: r.i32()?,
            holder_id: r.i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Limits) -> Result<()> {
        w.i32(self.entity_id);
        w.i32(self.holder_id);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityHeadRotation {
    pub entity_id: i32,
    /// Lossless byte angle, interpreted modulo one full turn.
    pub head_yaw: Angle,
}
impl Body for EntityHeadRotation {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            head_yaw: Angle(r.u8()?),
        })
    }
    fn write(&self, w: &mut Writer, _: Limits) -> Result<()> {
        w.var_i32(self.entity_id);
        w.u8(self.head_yaw.0);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetCamera {
    pub camera_id: i32,
}
impl Body for SetCamera {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            camera_id: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Limits) -> Result<()> {
        w.var_i32(self.camera_id);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityAnimation {
    pub entity_id: i32,
    /// Raw unsigned byte, not a VarInt or a guessed closed action enumeration.
    pub animation: u8,
}
impl Body for EntityAnimation {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            animation: r.u8()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Limits) -> Result<()> {
        w.var_i32(self.entity_id);
        w.u8(self.animation);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageEvent {
    pub entity_id: i32,
    /// Raw unresolved damage-type registry reference.
    pub source_type_id: i32,
    /// Raw wire value, including the optional source entity's offset encoding.
    /// This is not an already-resolved entity ID.
    pub source_cause_id: i32,
    /// Raw wire value, including the optional direct entity's offset encoding.
    /// This is not an already-resolved entity ID.
    pub source_direct_id: i32,
    /// Raw f64 coordinates. NaNs, infinities and signed zero are preserved.
    pub source_position: Option<[f64; 3]>,
}
impl Body for DamageEvent {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            source_type_id: r.var_i32()?,
            source_cause_id: r.var_i32()?,
            source_direct_id: r.var_i32()?,
            source_position: if r.bool()? {
                Some([r.f64()?, r.f64()?, r.f64()?])
            } else {
                None
            },
        })
    }
    fn write(&self, w: &mut Writer, _: Limits) -> Result<()> {
        w.var_i32(self.entity_id);
        w.var_i32(self.source_type_id);
        w.var_i32(self.source_cause_id);
        w.var_i32(self.source_direct_id);
        w.bool(self.source_position.is_some());
        if let Some(position) = self.source_position {
            for value in position {
                w.f64(value);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HurtAnimation {
    pub entity_id: i32,
    /// Raw f32 angle; non-finite values and signed zero are not normalized.
    pub yaw: f32,
}
impl Body for HurtAnimation {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            yaw: r.f32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Limits) -> Result<()> {
        w.var_i32(self.entity_id);
        w.f32(self.yaw);
        Ok(())
    }
}

codec!(
    SetPassengers => "set_passengers",
    AttachEntity => "attach_entity",
    EntityHeadRotation => "entity_head_rotation",
    SetCamera => "camera",
    EntityAnimation => "animation",
    DamageEvent => "damage_event",
    HurtAnimation => "hurt_animation",
);

#[derive(Debug, Clone, PartialEq)]
pub enum EntityControlPacket {
    Passengers(SetPassengers),
    Attach(AttachEntity),
    HeadRotation(EntityHeadRotation),
    Camera(SetCamera),
    Animation(EntityAnimation),
    Damage(DamageEvent),
    HurtAnimation(HurtAnimation),
}
impl EntityControlPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "set_passengers" => Self::Passengers(SetPassengers::decode(bytes, version, limits)?),
            "attach_entity" => Self::Attach(AttachEntity::decode(bytes, version, limits)?),
            "entity_head_rotation" => {
                Self::HeadRotation(EntityHeadRotation::decode(bytes, version, limits)?)
            }
            "camera" => Self::Camera(SetCamera::decode(bytes, version, limits)?),
            "animation" => Self::Animation(EntityAnimation::decode(bytes, version, limits)?),
            "damage_event" => Self::Damage(DamageEvent::decode(bytes, version, limits)?),
            "hurt_animation" => Self::HurtAnimation(HurtAnimation::decode(bytes, version, limits)?),
            _ => return Err(Error::Unsupported("typed entity-control packet")),
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        match self {
            Self::Passengers(value) => value.encode(version, limits),
            Self::Attach(value) => value.encode(version, limits),
            Self::HeadRotation(value) => value.encode(version, limits),
            Self::Camera(value) => value.encode(version, limits),
            Self::Animation(value) => value.encode(version, limits),
            Self::Damage(value) => value.encode(version, limits),
            Self::HurtAnimation(value) => value.encode(version, limits),
        }
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        match self {
            Self::Passengers(value) => value.packet(version, limits),
            Self::Attach(value) => value.packet(version, limits),
            Self::HeadRotation(value) => value.packet(version, limits),
            Self::Camera(value) => value.packet(version, limits),
            Self::Animation(value) => value.packet(version, limits),
            Self::Damage(value) => value.packet(version, limits),
            Self::HurtAnimation(value) => value.packet(version, limits),
        }
    }
}
