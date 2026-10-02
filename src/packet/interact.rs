//! Typed serverbound interaction/control packets. These codecs serialize intent;
//! they do not maintain game state, grant abilities, or validate server permissions.
use crate::{
    codec::{BlockPosition, Writer},
    frame::RawPacket,
    version::State,
    Error, Result, Version,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Hand {
    Main = 0,
    Off = 1,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BlockFace {
    Bottom = 0,
    Top = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}
fn packet(version: Version, name: &str, w: Writer) -> Result<RawPacket> {
    super::named(version, State::Play, name, w.into_inner())
}
fn sequence(w: &mut Writer, sequence: u32) -> Result<()> {
    if sequence > i32::MAX as u32 {
        return Err(Error::Invalid("interaction sequence"));
    }
    w.var_i32(sequence as i32);
    Ok(())
}
fn entity_id(w: &mut Writer, id: i32) -> Result<()> {
    if id < 0 {
        return Err(Error::Invalid("negative entity ID"));
    }
    w.var_i32(id);
    Ok(())
}

pub fn swing_arm(version: Version, hand: Hand) -> Result<RawPacket> {
    let mut w = Writer::new();
    w.var_i32(hand as i32);
    packet(version, "arm_animation", w)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ClientCommand {
    Respawn = 0,
    RequestStatistics = 1,
}
impl ClientCommand {
    pub fn encode(self, version: Version) -> Result<RawPacket> {
        let mut w = Writer::new();
        w.var_i32(self as i32);
        packet(version, "client_command", w)
    }
}
/// Only the flying bit is client-controlled. This does not authorize flight.
pub fn player_abilities(version: Version, flying: bool) -> Result<RawPacket> {
    let mut w = Writer::new();
    w.u8(if flying { 0x02 } else { 0 });
    packet(version, "abilities", w)
}
/// End-of-tick marker, introduced in protocol 768 (1.21.2).
pub fn tick_end(version: Version) -> Result<RawPacket> {
    if version.protocol() < 768 {
        return Err(Error::Unsupported("tick end before 1.21.2"));
    }
    packet(version, "tick_end", Writer::new())
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputKeys {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}
impl InputKeys {
    pub fn bits(self) -> u8 {
        u8::from(self.forward)
            | u8::from(self.backward) << 1
            | u8::from(self.left) << 2
            | u8::from(self.right) << 3
            | u8::from(self.jump) << 4
            | u8::from(self.shift) << 5
            | u8::from(self.sprint) << 6
    }
}
/// The analogue vehicle-input packet was replaced by digital player-input keys
/// in protocol 768. A mismatched variant is rejected rather than approximated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerInput {
    Legacy {
        sideways: f32,
        forward: f32,
        jump: bool,
        dismount: bool,
    },
    Keys(InputKeys),
}
impl PlayerInput {
    pub fn encode(self, version: Version) -> Result<RawPacket> {
        let mut w = Writer::new();
        match self {
            Self::Legacy {
                sideways,
                forward,
                jump,
                dismount,
            } if version.protocol() < 768 => {
                if ![sideways, forward]
                    .iter()
                    .all(|v| v.is_finite() && (-1.0..=1.0).contains(v))
                {
                    return Err(Error::Invalid("analogue player input"));
                }
                w.f32(sideways);
                w.f32(forward);
                w.u8(u8::from(jump) | u8::from(dismount) << 1);
                packet(version, "steer_vehicle", w)
            }
            Self::Keys(keys) if version.protocol() >= 768 => {
                w.u8(keys.bits());
                packet(version, "player_input", w)
            }
            _ => Err(Error::Unsupported(
                "player input representation for version",
            )),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DiggingAction {
    Start = 0,
    Cancel = 1,
    Finish = 2,
    DropStack = 3,
    DropItem = 4,
    ReleaseUse = 5,
    SwapHands = 6,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digging {
    pub action: DiggingAction,
    pub position: BlockPosition,
    pub face: BlockFace,
    /// Caller-managed block-change sequence, acknowledged by the server.
    pub sequence: u32,
}
impl Digging {
    pub fn encode(self, version: Version) -> Result<RawPacket> {
        let mut w = Writer::new();
        w.var_i32(self.action as i32);
        w.i64(self.position.pack()?);
        w.u8(self.face as u8);
        sequence(&mut w, self.sequence)?;
        packet(version, "block_dig", w)
    }
}
/// Right-click a block, including the hit location within that block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseBlock {
    pub hand: Hand,
    pub position: BlockPosition,
    pub face: BlockFace,
    pub cursor: [f32; 3],
    pub inside_block: bool,
    /// Available from protocol 768; true is rejected on older versions.
    pub world_border_hit: bool,
    pub sequence: u32,
}
impl UseBlock {
    pub fn encode(self, version: Version) -> Result<RawPacket> {
        if !self
            .cursor
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            return Err(Error::Invalid("block hit cursor"));
        }
        if self.world_border_hit && version.protocol() < 768 {
            return Err(Error::Unsupported("world border hit before 1.21.2"));
        }
        let mut w = Writer::new();
        w.var_i32(self.hand as i32);
        w.i64(self.position.pack()?);
        w.var_i32(self.face as i32);
        for coordinate in self.cursor {
            w.f32(coordinate);
        }
        w.bool(self.inside_block);
        if version.protocol() >= 768 {
            w.bool(self.world_border_hit);
        }
        sequence(&mut w, self.sequence)?;
        packet(version, "block_place", w)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseItem {
    pub hand: Hand,
    pub sequence: u32,
    /// [yaw, pitch] in degrees, required from protocol 767 and absent before it.
    pub rotation: Option<[f32; 2]>,
}
impl UseItem {
    pub fn encode(self, version: Version) -> Result<RawPacket> {
        if self.rotation.is_some() != (version.protocol() >= 767) {
            return Err(Error::Invalid("use-item rotation for version"));
        }
        if self
            .rotation
            .is_some_and(|rotation| !rotation.iter().all(|v| v.is_finite()))
        {
            return Err(Error::Invalid("non-finite use-item rotation"));
        }
        let mut w = Writer::new();
        w.var_i32(self.hand as i32);
        sequence(&mut w, self.sequence)?;
        if let Some(rotation) = self.rotation {
            for angle in rotation {
                w.f32(angle);
            }
        }
        packet(version, "use_item", w)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityAction {
    /// Removed in 1.21.6; use the shift key in PlayerInput on newer versions.
    StartSneaking,
    StopSneaking,
    LeaveBed,
    StartSprinting,
    StopSprinting,
    /// Jump charge in the inclusive range 0..=100.
    StartHorseJump(u8),
    StopHorseJump,
    OpenVehicleInventory,
    StartElytraFlying,
}
impl EntityAction {
    pub fn encode(self, version: Version, player_entity_id: i32) -> Result<RawPacket> {
        let (old_id, boost) = match self {
            Self::StartSneaking => (0, 0),
            Self::StopSneaking => (1, 0),
            Self::LeaveBed => (2, 0),
            Self::StartSprinting => (3, 0),
            Self::StopSprinting => (4, 0),
            Self::StartHorseJump(boost) if boost <= 100 => (5, boost),
            Self::StartHorseJump(_) => return Err(Error::Invalid("horse jump charge")),
            Self::StopHorseJump => (6, 0),
            Self::OpenVehicleInventory => (7, 0),
            Self::StartElytraFlying => (8, 0),
        };
        let id = if version.protocol() >= 771 {
            if old_id < 2 {
                return Err(Error::Unsupported("sneaking entity action after 1.21.5"));
            }
            old_id - 2
        } else {
            old_id
        };
        let mut w = Writer::new();
        entity_id(&mut w, player_entity_id)?;
        w.var_i32(id);
        w.var_i32(boost as i32);
        packet(version, "entity_action", w)
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EntityInteraction {
    /// No hit position; supported only through protocol 774. Newer protocols
    /// require InteractAt with a location and never receive invented coordinates.
    Interact {
        hand: Hand,
    },
    Attack,
    /// Location relative to the target entity. From protocol 775 these are
    /// quantized using the protocol low-precision vector representation.
    InteractAt {
        hand: Hand,
        location: [f32; 3],
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseEntity {
    pub target: i32,
    pub interaction: EntityInteraction,
    pub sneaking: bool,
}
impl UseEntity {
    pub fn encode(self, version: Version) -> Result<RawPacket> {
        let mut w = Writer::new();
        entity_id(&mut w, self.target)?;
        if version.protocol() >= 775 {
            return match self.interaction {
                EntityInteraction::Attack if !self.sneaking => packet(version, "attack", w),
                EntityInteraction::Attack => Err(Error::Unsupported(
                    "sneaking flag on dedicated attack packet",
                )),
                EntityInteraction::InteractAt { hand, location } => {
                    w.var_i32(hand as i32);
                    super::entity::LowPrecisionVector::quantize(location.map(f64::from))?
                        .write(&mut w)?;
                    w.bool(self.sneaking);
                    packet(version, "use_entity", w)
                }
                EntityInteraction::Interact { .. } => Err(Error::Unsupported(
                    "entity interaction without a hit location from 26.1",
                )),
            };
        }
        match self.interaction {
            EntityInteraction::Interact { hand } => {
                w.var_i32(0);
                w.var_i32(hand as i32);
            }
            EntityInteraction::Attack => w.var_i32(1),
            EntityInteraction::InteractAt { hand, location } => {
                if !location.iter().all(|v| v.is_finite()) {
                    return Err(Error::Invalid("non-finite entity interaction location"));
                }
                w.var_i32(2);
                for coordinate in location {
                    w.f32(coordinate);
                }
                w.var_i32(hand as i32);
            }
        }
        w.bool(self.sneaking);
        packet(version, "use_entity", w)
    }
}
