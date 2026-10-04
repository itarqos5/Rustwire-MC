//! Bounded entity movement and player-state codecs for protocols 763–776.
//!
//! Layouts follow the pinned `research/protocols` schemas. The native `lpVec3`
//! representation follows PrismarineJS/node-minecraft-protocol's
//! `src/datatypes/lpVec3.js`; quantized values are retained rather than silently
//! converting them back and forth through floating point. Metadata/equipment
//! with item components are intentionally not decoded by this module.
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

fn nonnegative(n: i32) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid("negative entity/registry ID"))
    } else {
        Ok(n)
    }
}
fn vector(r: &mut Reader<'_>) -> Result<[f64; 3]> {
    let v = [r.f64()?, r.f64()?, r.f64()?];
    finite(&v)?;
    Ok(v)
}
fn finite(v: &[f64]) -> Result<()> {
    if v.iter().all(|n| n.is_finite()) {
        Ok(())
    } else {
        Err(Error::Invalid("non-finite entity coordinate"))
    }
}
fn put_vector(w: &mut Writer, v: [f64; 3]) -> Result<()> {
    finite(&v)?;
    for n in v {
        w.f64(n);
    }
    Ok(())
}
trait Body: Sized {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()>;
}
macro_rules! codec {
    ($($t:ty),* $(,)?) => {$ (
        impl $t {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet { return Err(Error::Limit("entity packet bytes")); }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            /// Builds a new body; an error never exposes a partially encoded packet.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                self.write(&mut w, version, limits)?;
                if w.as_slice().len() > limits.max_packet { return Err(Error::Limit("entity packet bytes")); }
                Ok(w.into_inner())
            }
        }
    )*};
}

/// One unsigned 1/256-turn angle. All 256 wire values are representable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Angle(pub u8);
impl Angle {
    pub fn degrees(self) -> f32 {
        self.0 as f32 * (360.0 / 256.0)
    }
}

/// Lossless fields of the 1.21.9+ low-precision vector.
/// `scale == 0` is the one-byte zero vector and requires `[16383; 3]`.
/// Otherwise each component has 15 bits; 32767 decodes with the same clamp as 32766.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LowPrecisionVector {
    pub scale: u64,
    pub components: [u16; 3],
}
impl LowPrecisionVector {
    pub const ZERO: Self = Self {
        scale: 0,
        components: [16383; 3],
    };
    pub const MAX_SCALE: u64 = 17_179_869_183;
    pub fn quantize(value: [f64; 3]) -> Result<Self> {
        finite(&value)?;
        let max = value.iter().map(|n| n.abs()).fold(0.0, f64::max);
        if max > Self::MAX_SCALE as f64 {
            return Err(Error::Invalid("low-precision vector range"));
        }
        if max < 1.0 / 32766.0 {
            return Ok(Self::ZERO);
        }
        let scale = max.ceil() as u64;
        Ok(Self {
            scale,
            components: value.map(|n| ((n / scale as f64 * 0.5 + 0.5) * 32766.0).round() as u16),
        })
    }
    pub fn value(self) -> [f64; 3] {
        self.components
            .map(|n| (n.min(32766) as f64 * 2.0 / 32766.0 - 1.0) * self.scale as f64)
    }
    pub(crate) fn read(r: &mut Reader<'_>) -> Result<Self> {
        let first = r.u8()?;
        if first == 0 {
            return Ok(Self::ZERO);
        }
        let second = r.u8()?;
        let high = r.i32()? as u32 as u64;
        let packed = (high << 16) | ((second as u64) << 8) | first as u64;
        let mut scale = (first & 3) as u64;
        if first & 4 != 0 {
            scale |= (r.var_i32()? as u32 as u64) << 2;
        }
        if scale == 0 {
            return Err(Error::Invalid("zero low-precision vector scale"));
        }
        Ok(Self {
            scale,
            components: [
                ((packed >> 3) & 32767) as u16,
                ((packed >> 18) & 32767) as u16,
                ((packed >> 33) & 32767) as u16,
            ],
        })
    }
    pub(crate) fn write(self, w: &mut Writer) -> Result<()> {
        if self.scale == 0 {
            if self.components != Self::ZERO.components {
                return Err(Error::Invalid("zero low-precision vector components"));
            }
            w.u8(0);
            return Ok(());
        }
        if self.scale > Self::MAX_SCALE || self.components.iter().any(|n| *n > 32767) {
            return Err(Error::Invalid("low-precision vector range"));
        }
        let extended = self.scale > 3;
        let marker = (self.scale & 3) | if extended { 4 } else { 0 };
        let packed = marker
            | ((self.components[0] as u64) << 3)
            | ((self.components[1] as u64) << 18)
            | ((self.components[2] as u64) << 33);
        w.u8(packed as u8);
        w.u8((packed >> 8) as u8);
        w.i32((packed >> 16) as u32 as i32);
        if extended {
            w.var_i32((self.scale >> 2) as u32 as i32);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Velocity {
    /// Signed units of 1/8000 block per tick, protocols 763–772.
    Fixed([i16; 3]),
    /// Protocols 773–776.
    LowPrecision(LowPrecisionVector),
}
impl Velocity {
    pub fn blocks_per_tick(self) -> [f64; 3] {
        match self {
            Self::Fixed(v) => v.map(|n| n as f64 / 8000.0),
            Self::LowPrecision(v) => v.value(),
        }
    }
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        if version.protocol() >= 773 {
            Ok(Self::LowPrecision(LowPrecisionVector::read(r)?))
        } else {
            Ok(Self::Fixed([r.i16()?, r.i16()?, r.i16()?]))
        }
    }
    fn write(self, w: &mut Writer, version: Version) -> Result<()> {
        match self {
            Self::Fixed(v) if version.protocol() < 773 => {
                for n in v {
                    w.i16(n);
                }
                Ok(())
            }
            Self::LowPrecision(v) if version.protocol() >= 773 => v.write(w),
            _ => Err(Error::Invalid("velocity representation for protocol")),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnEntity {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub entity_type: i32,
    pub position: [f64; 3],
    pub pitch: Angle,
    pub yaw: Angle,
    pub head_yaw: Angle,
    pub data: i32,
    pub velocity: Velocity,
}
impl Body for SpawnEntity {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        let entity_id = nonnegative(r.var_i32()?)?;
        let uuid = r.uuid()?;
        let entity_type = nonnegative(r.var_i32()?)?;
        let position = vector(r)?;
        let early_velocity = if v.protocol() >= 773 {
            Some(Velocity::read(r, v)?)
        } else {
            None
        };
        let pitch = Angle(r.u8()?);
        let yaw = Angle(r.u8()?);
        let head_yaw = Angle(r.u8()?);
        let data = r.var_i32()?;
        let velocity = match early_velocity {
            Some(velocity) => velocity,
            None => Velocity::read(r, v)?,
        };
        Ok(Self {
            entity_id,
            uuid,
            entity_type,
            position,
            pitch,
            yaw,
            head_yaw,
            data,
            velocity,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, _: Limits) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        w.raw(&self.uuid);
        w.var_i32(nonnegative(self.entity_type)?);
        put_vector(w, self.position)?;
        if v.protocol() >= 773 {
            self.velocity.write(w, v)?;
        }
        w.u8(self.pitch.0);
        w.u8(self.yaw.0);
        w.u8(self.head_yaw.0);
        w.var_i32(self.data);
        if v.protocol() < 773 {
            self.velocity.write(w, v)?;
        }
        Ok(())
    }
}
/// Legacy experience-orb spawn, present only in protocols 763–769.
/// The signed entity reference, signed short value and all coordinate bits are
/// retained as wire data; this codec does not enforce gameplay-valid values.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnExperienceOrb {
    pub entity_id: i32,
    pub position: [f64; 3],
    /// The schema's `count` field is the orb's experience value, not a list length.
    pub value: i16,
}
impl Body for SpawnExperienceOrb {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        v.packet_id(
            State::Play,
            Direction::Clientbound,
            "spawn_entity_experience_orb",
        )?;
        Ok(Self {
            entity_id: r.var_i32()?,
            position: [r.f64()?, r.f64()?, r.f64()?],
            value: r.i16()?,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, _: Limits) -> Result<()> {
        v.packet_id(
            State::Play,
            Direction::Clientbound,
            "spawn_entity_experience_orb",
        )?;
        w.var_i32(self.entity_id);
        for value in self.position {
            w.f64(value);
        }
        w.i16(self.value);
        Ok(())
    }
}

/// Legacy `named_entity_spawn`, present only in protocol 763 (1.20/1.20.1).
/// UUID bytes, signed entity references, coordinate bits and byte angles are
/// retained without registry lookup, entity simulation or gameplay validation.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPlayer {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub position: [f64; 3],
    pub yaw: Angle,
    pub pitch: Angle,
}
impl Body for SpawnPlayer {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        v.packet_id(State::Play, Direction::Clientbound, "named_entity_spawn")?;
        Ok(Self {
            entity_id: r.var_i32()?,
            uuid: r.uuid()?,
            position: [r.f64()?, r.f64()?, r.f64()?],
            yaw: Angle(r.u8()?),
            pitch: Angle(r.u8()?),
        })
    }
    fn write(&self, w: &mut Writer, v: Version, _: Limits) -> Result<()> {
        v.packet_id(State::Play, Direction::Clientbound, "named_entity_spawn")?;
        w.var_i32(self.entity_id);
        w.raw(&self.uuid);
        for value in self.position {
            w.f64(value);
        }
        w.u8(self.yaw.0);
        w.u8(self.pitch.0);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeMove {
    pub entity_id: i32,
    pub delta: [i16; 3],
    pub on_ground: bool,
}
impl RelativeMove {
    pub fn delta_blocks(&self) -> [f64; 3] {
        self.delta.map(|n| n as f64 / 4096.0)
    }
}
impl Body for RelativeMove {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: nonnegative(r.var_i32()?)?,
            delta: [r.i16()?, r.i16()?, r.i16()?],
            on_ground: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        for n in self.delta {
            w.i16(n);
        }
        w.bool(self.on_ground);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityLook {
    pub entity_id: i32,
    pub yaw: Angle,
    pub pitch: Angle,
    pub on_ground: bool,
}
impl Body for EntityLook {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: nonnegative(r.var_i32()?)?,
            yaw: Angle(r.u8()?),
            pitch: Angle(r.u8()?),
            on_ground: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        w.u8(self.yaw.0);
        w.u8(self.pitch.0);
        w.bool(self.on_ground);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveAndLook {
    pub entity_id: i32,
    pub delta: [i16; 3],
    pub yaw: Angle,
    pub pitch: Angle,
    pub on_ground: bool,
}
impl Body for MoveAndLook {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: nonnegative(r.var_i32()?)?,
            delta: [r.i16()?, r.i16()?, r.i16()?],
            yaw: Angle(r.u8()?),
            pitch: Angle(r.u8()?),
            on_ground: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        for n in self.delta {
            w.i16(n);
        }
        w.u8(self.yaw.0);
        w.u8(self.pitch.0);
        w.bool(self.on_ground);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityVelocity {
    pub entity_id: i32,
    pub velocity: Velocity,
}
impl Body for EntityVelocity {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        Ok(Self {
            entity_id: nonnegative(r.var_i32()?)?,
            velocity: Velocity::read(r, v)?,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, _: Limits) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        self.velocity.write(w, v)
    }
}
/// Byte angles in protocols 763–767 and 776, float angles and deltas in 768–775.
#[derive(Debug, Clone, PartialEq)]
pub enum TeleportTransform {
    Absolute {
        position: [f64; 3],
        yaw: Angle,
        pitch: Angle,
    },
    Relative {
        position: [f64; 3],
        delta: [f64; 3],
        yaw: f32,
        pitch: f32,
        flags: u32,
    },
}
#[derive(Debug, Clone, PartialEq)]
pub struct EntityTeleport {
    pub entity_id: i32,
    pub transform: TeleportTransform,
    pub on_ground: bool,
}
impl Body for EntityTeleport {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        let entity_id = nonnegative(r.var_i32()?)?;
        let position = vector(r)?;
        let transform = if (768..=775).contains(&v.protocol()) {
            let delta = vector(r)?;
            let yaw = r.f32()?;
            let pitch = r.f32()?;
            let flags = r.i32()? as u32;
            finite(&[yaw as f64, pitch as f64])?;
            if flags & !0x1ff != 0 {
                return Err(Error::Invalid("entity teleport relative flags"));
            }
            TeleportTransform::Relative {
                position,
                delta,
                yaw,
                pitch,
                flags,
            }
        } else {
            TeleportTransform::Absolute {
                position,
                yaw: Angle(r.u8()?),
                pitch: Angle(r.u8()?),
            }
        };
        Ok(Self {
            entity_id,
            transform,
            on_ground: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, _: Limits) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        match self.transform {
            TeleportTransform::Absolute {
                position,
                yaw,
                pitch,
            } if !(768..=775).contains(&v.protocol()) => {
                put_vector(w, position)?;
                w.u8(yaw.0);
                w.u8(pitch.0);
            }
            TeleportTransform::Relative {
                position,
                delta,
                yaw,
                pitch,
                flags,
            } if (768..=775).contains(&v.protocol()) => {
                finite(&[yaw as f64, pitch as f64])?;
                if flags & !0x1ff != 0 {
                    return Err(Error::Invalid("entity teleport relative flags"));
                }
                put_vector(w, position)?;
                put_vector(w, delta)?;
                w.f32(yaw);
                w.f32(pitch);
                w.i32(flags as i32);
            }
            _ => {
                return Err(Error::Invalid(
                    "entity teleport representation for protocol",
                ))
            }
        }
        w.bool(self.on_ground);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SyncEntityPosition {
    pub entity_id: i32,
    pub position: [f64; 3],
    pub delta: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}
impl Body for SyncEntityPosition {
    fn read(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        if v.protocol() < 768 {
            return Err(Error::Unsupported(
                "entity position sync before protocol 768",
            ));
        }
        let entity_id = nonnegative(r.var_i32()?)?;
        let position = vector(r)?;
        let delta = vector(r)?;
        let yaw = r.f32()?;
        let pitch = r.f32()?;
        finite(&[yaw as f64, pitch as f64])?;
        Ok(Self {
            entity_id,
            position,
            delta,
            yaw,
            pitch,
            on_ground: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, _: Limits) -> Result<()> {
        if v.protocol() < 768 {
            return Err(Error::Unsupported(
                "entity position sync before protocol 768",
            ));
        }
        finite(&[self.yaw as f64, self.pitch as f64])?;
        w.var_i32(nonnegative(self.entity_id)?);
        put_vector(w, self.position)?;
        put_vector(w, self.delta)?;
        w.f32(self.yaw);
        w.f32(self.pitch);
        w.bool(self.on_ground);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestroyEntities {
    pub entity_ids: Vec<i32>,
}
impl Body for DestroyEntities {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let count = r.count(r.limits.max_collection)?;
        if count > r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut entity_ids = Vec::with_capacity(count);
        for _ in 0..count {
            entity_ids.push(nonnegative(r.var_i32()?)?);
        }
        Ok(Self { entity_ids })
    }
    fn write(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        if self.entity_ids.len()
            > limits
                .max_collection
                .min(i32::MAX as usize)
                .min(limits.max_packet)
        {
            return Err(Error::Limit("entity ID count"));
        }
        w.var_i32(self.entity_ids.len() as i32);
        for &id in &self.entity_ids {
            w.var_i32(nonnegative(id)?);
            if w.as_slice().len() > limits.max_packet {
                return Err(Error::Limit("entity packet bytes"));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityStatus {
    pub entity_id: i32,
    pub status: i8,
}
impl Body for EntityStatus {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: nonnegative(r.i32()?)?,
            status: r.u8()? as i8,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.i32(nonnegative(self.entity_id)?);
        w.u8(self.status as u8);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Health {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
}
impl Health {
    fn validate(&self) -> Result<()> {
        if !self.health.is_finite()
            || !self.saturation.is_finite()
            || self.health < 0.0
            || self.food < 0
            || self.saturation < 0.0
        {
            Err(Error::Invalid("health, food or saturation"))
        } else {
            Ok(())
        }
    }
}
impl Body for Health {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let value = Self {
            health: r.f32()?,
            food: r.var_i32()?,
            saturation: r.f32()?,
        };
        value.validate()?;
        Ok(value)
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        self.validate()?;
        w.f32(self.health);
        w.var_i32(self.food);
        w.f32(self.saturation);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAbilities {
    pub flags: u8,
    pub flying_speed: f32,
    pub walking_speed: f32,
}
impl PlayerAbilities {
    fn validate(&self) -> Result<()> {
        if self.flags & !15 != 0
            || !self.flying_speed.is_finite()
            || !self.walking_speed.is_finite()
            || self.flying_speed < 0.0
            || self.walking_speed < 0.0
        {
            Err(Error::Invalid("player abilities"))
        } else {
            Ok(())
        }
    }
}
impl Body for PlayerAbilities {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let value = Self {
            flags: r.u8()?,
            flying_speed: r.f32()?,
            walking_speed: r.f32()?,
        };
        value.validate()?;
        Ok(value)
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        self.validate()?;
        w.u8(self.flags);
        w.f32(self.flying_speed);
        w.f32(self.walking_speed);
        Ok(())
    }
}
codec!(
    SpawnEntity,
    SpawnExperienceOrb,
    SpawnPlayer,
    RelativeMove,
    EntityLook,
    MoveAndLook,
    EntityVelocity,
    EntityTeleport,
    SyncEntityPosition,
    DestroyEntities,
    EntityStatus,
    Health,
    PlayerAbilities
);

/// Typed clientbound subset. Unknown packets fail explicitly instead of opaque success.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityPacket {
    Spawn(SpawnEntity),
    ExperienceOrb(SpawnExperienceOrb),
    PlayerSpawn(SpawnPlayer),
    Move(RelativeMove),
    Look(EntityLook),
    MoveLook(MoveAndLook),
    Velocity(EntityVelocity),
    Teleport(EntityTeleport),
    SyncPosition(SyncEntityPosition),
    Destroy(DestroyEntities),
    Status(EntityStatus),
    Health(Health),
    Abilities(PlayerAbilities),
}
impl EntityPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "spawn_entity" => Self::Spawn(SpawnEntity::decode(bytes, version, limits)?),
            "spawn_entity_experience_orb" => {
                Self::ExperienceOrb(SpawnExperienceOrb::decode(bytes, version, limits)?)
            }
            "named_entity_spawn" => Self::PlayerSpawn(SpawnPlayer::decode(bytes, version, limits)?),
            "rel_entity_move" => Self::Move(RelativeMove::decode(bytes, version, limits)?),
            "entity_look" => Self::Look(EntityLook::decode(bytes, version, limits)?),
            "entity_move_look" => Self::MoveLook(MoveAndLook::decode(bytes, version, limits)?),
            "entity_velocity" => Self::Velocity(EntityVelocity::decode(bytes, version, limits)?),
            "entity_teleport" => Self::Teleport(EntityTeleport::decode(bytes, version, limits)?),
            "sync_entity_position" => {
                Self::SyncPosition(SyncEntityPosition::decode(bytes, version, limits)?)
            }
            "entity_destroy" => Self::Destroy(DestroyEntities::decode(bytes, version, limits)?),
            "entity_status" => Self::Status(EntityStatus::decode(bytes, version, limits)?),
            "update_health" => Self::Health(Health::decode(bytes, version, limits)?),
            "abilities" => Self::Abilities(PlayerAbilities::decode(bytes, version, limits)?),
            "entity_metadata" | "entity_equipment" => {
                return Err(Error::Unsupported(
                    "entity metadata/equipment item components",
                ))
            }
            _ => return Err(Error::Unsupported("typed entity packet")),
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        let (name, body) = match self {
            Self::Spawn(p) => ("spawn_entity", p.encode(version, limits)?),
            Self::ExperienceOrb(p) => ("spawn_entity_experience_orb", p.encode(version, limits)?),
            Self::PlayerSpawn(p) => ("named_entity_spawn", p.encode(version, limits)?),
            Self::Move(p) => ("rel_entity_move", p.encode(version, limits)?),
            Self::Look(p) => ("entity_look", p.encode(version, limits)?),
            Self::MoveLook(p) => ("entity_move_look", p.encode(version, limits)?),
            Self::Velocity(p) => ("entity_velocity", p.encode(version, limits)?),
            Self::Teleport(p) => ("entity_teleport", p.encode(version, limits)?),
            Self::SyncPosition(p) => ("sync_entity_position", p.encode(version, limits)?),
            Self::Destroy(p) => ("entity_destroy", p.encode(version, limits)?),
            Self::Status(p) => ("entity_status", p.encode(version, limits)?),
            Self::Health(p) => ("update_health", p.encode(version, limits)?),
            Self::Abilities(p) => ("abilities", p.encode(version, limits)?),
        };
        Ok(RawPacket::new(
            version.packet_id(State::Play, Direction::Clientbound, name)?,
            body,
        ))
    }
}
