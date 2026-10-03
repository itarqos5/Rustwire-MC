//! Ordinary clientbound world/player-control envelopes, without simulation.
//!
//! Layouts are checked against all fourteen hash-pinned protocol schemas. The
//! independent synthetic fixtures are not release-API or live-server evidence.
//! Non-enum signed scalars and floating-point bits remain raw: counts here are
//! gameplay scalars, not allocation lengths. Registry IDs are not resolved.
use crate::{
    codec::{BlockPosition, Reader, Writer},
    frame::RawPacket,
    nbt::{Nbt, RootFormat, Tag},
    version::{Direction, State},
    Error, Limits, Result, Version,
};

trait Body: Sized {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()>;
}
macro_rules! packets {
    ($($variant:ident($ty:ident) => $name:literal),* $(,)?) => {
        $(impl $ty {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                version.packet_id(State::Play, Direction::Clientbound, $name)?;
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("world-control packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            /// No partial body is returned on failure.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                version.packet_id(State::Play, Direction::Clientbound, $name)?;
                let mut w = Writer::new();
                self.write(&mut w, version, limits)?;
                if w.as_slice().len() > limits.max_packet {
                    return Err(Error::Limit("world-control packet bytes"));
                }
                Ok(w.into_inner())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::Clientbound, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        })*
        #[derive(Debug, Clone, PartialEq)]
        pub enum WorldControlPacket { $($variant($ty)),* }
        impl WorldControlPacket {
            pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                Ok(match name {
                    $($name => Self::$variant($ty::decode(bytes, version, limits)?),)*
                    _ => return Err(Error::Unsupported("typed world-control packet")),
                })
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                match self { $(Self::$variant(value) => value.encode(version, limits)),* }
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                match self { $(Self::$variant(value) => value.packet(version, limits)),* }
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockBreakAnimation {
    pub entity_id: i32,
    pub position: BlockPosition,
    /// Raw signed byte; values outside 0..=9 are retained.
    pub stage: i8,
}
impl Body for BlockBreakAnimation {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            position: BlockPosition::unpack(r.i64()?),
            stage: r.u8()? as i8,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.entity_id);
        w.i64(self.position.pack()?);
        w.u8(self.stage as u8);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockAction {
    pub position: BlockPosition,
    pub action_id: u8,
    pub action_parameter: u8,
    /// Raw block registry reference, not a block-state ID.
    pub block_id: i32,
}
impl Body for BlockAction {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            action_id: r.u8()?,
            action_parameter: r.u8()?,
            block_id: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.i64(self.position.pack()?);
        w.u8(self.action_id);
        w.u8(self.action_parameter);
        w.var_i32(self.block_id);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenSignEditor {
    pub position: BlockPosition,
    pub is_front_text: bool,
}
impl Body for OpenSignEditor {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            is_front_text: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.i64(self.position.pack()?);
        w.bool(self.is_front_text);
        Ok(())
    }
}

/// A queried optional compound. TAG_End represents an absent response.
/// Named roots are used at 763, anonymous roots at 764 and later.
/// Encoding follows the shared [`Nbt`] framing contract: a missing 763 root
/// name becomes empty, and a supplied name is omitted from anonymous 764+ roots.
/// This adapts framing only; the compound payload remains unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct NbtQueryResponse {
    pub transaction_id: i32,
    pub data: Option<Nbt>,
}
impl Body for NbtQueryResponse {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let transaction_id = r.var_i32()?;
        let data = Nbt::read(r, RootFormat::for_version(version))?;
        r.finish()?;
        if data
            .as_ref()
            .is_some_and(|nbt| !matches!(nbt.root, Tag::Compound(_)))
        {
            return Err(Error::Invalid("query-response compound NBT"));
        }
        Ok(Self {
            transaction_id,
            data,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.var_i32(self.transaction_id);
        match &self.data {
            Some(
                nbt @ Nbt {
                    root: Tag::Compound(_),
                    ..
                },
            ) => {
                nbt.write(
                    w,
                    RootFormat::for_version(version),
                    Limits {
                        max_packet: limits.max_packet.saturating_sub(w.as_slice().len()),
                        ..limits
                    },
                )?;
            }
            Some(_) => return Err(Error::Invalid("query-response compound NBT")),
            None => w.u8(0),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectItem {
    pub collected_entity_id: i32,
    pub collector_entity_id: i32,
    /// Signed scalar, not an allocation count.
    pub count: i32,
}
impl Body for CollectItem {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            collected_entity_id: r.var_i32()?,
            collector_entity_id: r.var_i32()?,
            count: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.collected_entity_id);
        w.var_i32(self.collector_entity_id);
        w.var_i32(self.count);
        Ok(())
    }
}

/// Clientbound vehicle position/rotation. This is not the serverbound packet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleMove {
    pub position: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}
impl Body for VehicleMove {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: [r.f64()?, r.f64()?, r.f64()?],
            yaw: r.f32()?,
            pitch: r.f32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        for value in self.position {
            w.f64(value);
        }
        w.f32(self.yaw);
        w.f32(self.pitch);
        Ok(())
    }
}

/// Known anchor ordinals. Unknown values return `Unsupported` after the
/// complete packet body is checked, allowing typed dispatch to retain raw bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum EntityAnchor {
    Feet = 0,
    Eyes = 1,
}
impl EntityAnchor {
    pub fn from_id(id: i32) -> Result<Self> {
        match id {
            0 => Ok(Self::Feet),
            1 => Ok(Self::Eyes),
            _ => Err(Error::Unsupported("face-player anchor")),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FacePlayerEntity {
    pub entity_id: i32,
    pub anchor: EntityAnchor,
}
/// Look-at control. The position remains present even with an entity target.
/// Both anchors use VarInts in every family. The pinned 763–764 schema string
/// field is corrected using version-pinned independent client implementations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacePlayer {
    pub source_anchor: EntityAnchor,
    pub position: [f64; 3],
    pub entity: Option<FacePlayerEntity>,
}
impl Body for FacePlayer {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let raw_source_anchor = r.var_i32()?;
        let position = [r.f64()?, r.f64()?, r.f64()?];
        let entity = if r.bool()? {
            Some((r.var_i32()?, r.var_i32()?))
        } else {
            None
        };
        r.finish()?;
        Ok(Self {
            source_anchor: EntityAnchor::from_id(raw_source_anchor)?,
            position,
            entity: entity
                .map(|(entity_id, anchor)| -> Result<FacePlayerEntity> {
                    Ok(FacePlayerEntity {
                        entity_id,
                        anchor: EntityAnchor::from_id(anchor)?,
                    })
                })
                .transpose()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.source_anchor as i32);
        for value in self.position {
            w.f64(value);
        }
        w.bool(self.entity.is_some());
        if let Some(entity) = self.entity {
            w.var_i32(entity.entity_id);
            w.var_i32(entity.anchor as i32);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationRelativity {
    pub yaw: bool,
    pub pitch: bool,
}
/// Available from 768; relative flags are present exactly from 773.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerRotation {
    pub yaw: f32,
    pub pitch: f32,
    pub relative: Option<RotationRelativity>,
}
impl Body for PlayerRotation {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let yaw = r.f32()?;
        let relative_yaw = if version.protocol() >= 773 {
            Some(r.bool()?)
        } else {
            None
        };
        let pitch = r.f32()?;
        let relative = match relative_yaw {
            Some(yaw) => Some(RotationRelativity {
                yaw,
                pitch: r.bool()?,
            }),
            None => None,
        };
        Ok(Self {
            yaw,
            pitch,
            relative,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, _: Limits) -> Result<()> {
        if (version.protocol() >= 773) != self.relative.is_some() {
            return Err(Error::Invalid("player-rotation version fields"));
        }
        w.f32(self.yaw);
        if let Some(relative) = self.relative {
            w.bool(relative.yaw);
        }
        w.f32(self.pitch);
        if let Some(relative) = self.relative {
            w.bool(relative.pitch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectilePower {
    /// Protocol 766 only.
    Vector([f64; 3]),
    /// Protocol 767 and later.
    Scalar(f64),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetProjectilePower {
    pub entity_id: i32,
    pub power: ProjectilePower,
}
impl Body for SetProjectilePower {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            power: if version.protocol() == 766 {
                ProjectilePower::Vector([r.f64()?, r.f64()?, r.f64()?])
            } else {
                ProjectilePower::Scalar(r.f64()?)
            },
        })
    }
    fn write(&self, w: &mut Writer, version: Version, _: Limits) -> Result<()> {
        w.var_i32(self.entity_id);
        match self.power {
            ProjectilePower::Vector(values) if version.protocol() == 766 => {
                for value in values {
                    w.f64(value);
                }
            }
            ProjectilePower::Scalar(value) if version.protocol() >= 767 => w.f64(value),
            _ => return Err(Error::Invalid("projectile-power version fields")),
        }
        Ok(())
    }
}

/// Available from 765. No tick-rate clamping or clock is implemented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetTickingState {
    pub tick_rate: f32,
    pub frozen: bool,
}
impl Body for SetTickingState {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            tick_rate: r.f32()?,
            frozen: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.f32(self.tick_rate);
        w.bool(self.frozen);
        Ok(())
    }
}
/// Available from 765. Retains the signed tick-step scalar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepTick {
    pub ticks: i32,
}
impl Body for StepTick {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            ticks: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.ticks);
        Ok(())
    }
}

packets! {
    BlockBreak(BlockBreakAnimation) => "block_break_animation",
    BlockAction(BlockAction) => "block_action",
    OpenSign(OpenSignEditor) => "open_sign_entity",
    NbtQuery(NbtQueryResponse) => "nbt_query_response",
    Collect(CollectItem) => "collect",
    Vehicle(VehicleMove) => "vehicle_move",
    FacePlayer(FacePlayer) => "face_player",
    Rotation(PlayerRotation) => "player_rotation",
    Projectile(SetProjectilePower) => "set_projectile_power",
    Ticking(SetTickingState) => "set_ticking_state",
    Step(StepTick) => "step_tick",
}
