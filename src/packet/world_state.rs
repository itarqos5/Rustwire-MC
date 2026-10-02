//! World/session wire state for protocols 763–776, without simulation.
//!
//! Floating-point fields retain their wire values (including infinities, NaNs
//! and signed zero). Borders and sequence acknowledgements retain signed scalar
//! values: they are not allocation counts. Unknown game-event reasons fail with
//! [`Error::Unsupported`] so a higher-level dispatcher can retain the raw packet.
//! Registry clock IDs remain unresolved; no static registry is embedded here.
use crate::{
    codec::{identifier, BlockPosition, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

trait Body: Sized {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()>;
}
macro_rules! codec {
    ($($ty:ty => $name:literal),* $(,)?) => {$ (
        impl $ty {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet { return Err(Error::Limit("world-state packet bytes")); }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            /// Transactional encoding: no partial body is returned on failure.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                self.write(&mut w, version, limits)?;
                if w.as_slice().len() > limits.max_packet { return Err(Error::Limit("world-state packet bytes")); }
                Ok(w.into_inner())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(version.packet_id(State::Play, Direction::Clientbound, $name)?, self.encode(version, limits)?))
            }
        }
    )*};
}
mod border;
mod timing;
pub use border::{
    InitializeWorldBorder, WorldBorderCenter, WorldBorderLerpSize, WorldBorderSize,
    WorldBorderWarningDelay, WorldBorderWarningDistance,
};
pub use timing::{ClockUpdate, TimeData, UpdateTime};

/// Known event IDs, independent of the event-specific floating-point parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GameEventReason {
    NoRespawnBlockAvailable = 0,
    StartRaining = 1,
    StopRaining = 2,
    ChangeGameMode = 3,
    WinGame = 4,
    DemoEvent = 5,
    PlayArrowHitSound = 6,
    RainLevelChange = 7,
    ThunderLevelChange = 8,
    PufferFishSting = 9,
    ElderGuardianEffect = 10,
    ImmediateRespawn = 11,
    /// Protocol 764 and later.
    LimitedCrafting = 12,
    /// Protocol 765 and later.
    LevelChunksLoadStart = 13,
}
impl GameEventReason {
    pub fn from_id(id: u8, version: Version) -> Result<Self> {
        let reason = match id {
            0 => Self::NoRespawnBlockAvailable,
            1 => Self::StartRaining,
            2 => Self::StopRaining,
            3 => Self::ChangeGameMode,
            4 => Self::WinGame,
            5 => Self::DemoEvent,
            6 => Self::PlayArrowHitSound,
            7 => Self::RainLevelChange,
            8 => Self::ThunderLevelChange,
            9 => Self::PufferFishSting,
            10 => Self::ElderGuardianEffect,
            11 => Self::ImmediateRespawn,
            12 if version.protocol() >= 764 => Self::LimitedCrafting,
            13 if version.protocol() >= 765 => Self::LevelChunksLoadStart,
            _ => return Err(Error::Unsupported("game-event reason")),
        };
        Ok(reason)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameStateChange {
    pub reason: GameEventReason,
    /// Raw event-specific scalar; interpreted by the application, not clamped.
    pub value: f32,
}
impl Body for GameStateChange {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let id = r.u8()?;
        let value = r.f32()?;
        // Consume the fixed body before reporting an unsupported reason.
        r.finish()?;
        Ok(Self {
            reason: GameEventReason::from_id(id, version)?,
            value,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, _: Limits) -> Result<()> {
        GameEventReason::from_id(self.reason as u8, version)?;
        w.u8(self.reason as u8);
        w.f32(self.value);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifficultyLevel {
    Peaceful,
    Easy,
    Normal,
    Hard,
}
/// Difficulty retains its raw integer. Release readers wrap it modulo four.
/// The field is an unsigned byte through 770 and a signed VarInt from 771.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difficulty {
    pub raw_difficulty: i32,
    pub locked: bool,
}
impl Difficulty {
    pub fn level(self) -> DifficultyLevel {
        match self.raw_difficulty.rem_euclid(4) {
            0 => DifficultyLevel::Peaceful,
            1 => DifficultyLevel::Easy,
            2 => DifficultyLevel::Normal,
            _ => DifficultyLevel::Hard,
        }
    }
}
impl Body for Difficulty {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            raw_difficulty: if version.protocol() < 771 {
                i32::from(r.u8()?)
            } else {
                r.var_i32()?
            },
            locked: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, _: Limits) -> Result<()> {
        if version.protocol() < 771 {
            w.u8(u8::try_from(self.raw_difficulty)
                .map_err(|_| Error::Invalid("legacy difficulty byte"))?);
        } else {
            w.var_i32(self.raw_difficulty);
        }
        w.bool(self.locked);
        Ok(())
    }
}

/// Spawn position has a dimension identifier and pitch from protocol 773.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnPosition {
    /// Present exactly from protocol 773. No dimension registry lookup is made.
    pub dimension: Option<String>,
    pub position: BlockPosition,
    pub yaw: f32,
    /// Present exactly from protocol 773.
    pub pitch: Option<f32>,
}
impl Body for SpawnPosition {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let dimension = if version.protocol() >= 773 {
            let s = r.string(32767)?;
            identifier::validate(s, version)?;
            Some(s.to_owned())
        } else {
            None
        };
        Ok(Self {
            dimension,
            position: BlockPosition::unpack(r.i64()?),
            yaw: r.f32()?,
            pitch: if version.protocol() >= 773 {
                Some(r.f32()?)
            } else {
                None
            },
        })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        if (version.protocol() >= 773) != self.dimension.is_some()
            || (version.protocol() >= 773) != self.pitch.is_some()
        {
            return Err(Error::Invalid("spawn-position version fields"));
        }
        if let Some(dimension) = &self.dimension {
            identifier::validate(dimension, version)?;
            w.string(dimension, 32767.min(limits.max_string_chars))?;
        }
        w.i64(self.position.pack()?);
        w.f32(self.yaw);
        if let Some(pitch) = self.pitch {
            w.f32(pitch);
        }
        Ok(())
    }
}

/// Clientbound `acknowledge_player_digging`. The signed sequence is preserved;
/// this codec does not enforce gameplay ordering or the sender's counter policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockChangeAck {
    pub sequence: i32,
}
impl Body for BlockChangeAck {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            sequence: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.sequence);
        Ok(())
    }
}
codec!(GameStateChange => "game_state_change", Difficulty => "difficulty", SpawnPosition => "spawn_position", BlockChangeAck => "acknowledge_player_digging");

/// Empty serverbound `player_loaded` acknowledgement, available from 769.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerLoaded;
impl PlayerLoaded {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if version.protocol() < 769 {
            return Err(Error::Unsupported("player-loaded before protocol 769"));
        }
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("world-state packet bytes"));
        }
        Reader::new(bytes, limits).finish()?;
        Ok(Self)
    }
    pub fn encode(&self, version: Version, _: Limits) -> Result<Vec<u8>> {
        if version.protocol() < 769 {
            return Err(Error::Unsupported("player-loaded before protocol 769"));
        }
        Ok(Vec::new())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(State::Play, Direction::Serverbound, "player_loaded")?,
            self.encode(version, limits)?,
        ))
    }
}

/// Clientbound subset implemented by this module. Unknown packet names or game
/// reasons report `Unsupported`; callers may preserve the complete raw packet.
#[derive(Debug, Clone, PartialEq)]
pub enum WorldStatePacket {
    GameState(GameStateChange),
    Time(UpdateTime),
    Spawn(SpawnPosition),
    Difficulty(Difficulty),
    BorderInitialize(InitializeWorldBorder),
    BorderCenter(WorldBorderCenter),
    BorderLerp(WorldBorderLerpSize),
    BorderSize(WorldBorderSize),
    BorderWarningDelay(WorldBorderWarningDelay),
    BorderWarningDistance(WorldBorderWarningDistance),
    BlockChangeAck(BlockChangeAck),
}
impl WorldStatePacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "game_state_change" => {
                Self::GameState(GameStateChange::decode(bytes, version, limits)?)
            }
            "update_time" => Self::Time(UpdateTime::decode(bytes, version, limits)?),
            "spawn_position" => Self::Spawn(SpawnPosition::decode(bytes, version, limits)?),
            "difficulty" => Self::Difficulty(Difficulty::decode(bytes, version, limits)?),
            "initialize_world_border" => {
                Self::BorderInitialize(InitializeWorldBorder::decode(bytes, version, limits)?)
            }
            "world_border_center" => {
                Self::BorderCenter(WorldBorderCenter::decode(bytes, version, limits)?)
            }
            "world_border_lerp_size" => {
                Self::BorderLerp(WorldBorderLerpSize::decode(bytes, version, limits)?)
            }
            "world_border_size" => {
                Self::BorderSize(WorldBorderSize::decode(bytes, version, limits)?)
            }
            "world_border_warning_delay" => {
                Self::BorderWarningDelay(WorldBorderWarningDelay::decode(bytes, version, limits)?)
            }
            "world_border_warning_reach" => Self::BorderWarningDistance(
                WorldBorderWarningDistance::decode(bytes, version, limits)?,
            ),
            "acknowledge_player_digging" => {
                Self::BlockChangeAck(BlockChangeAck::decode(bytes, version, limits)?)
            }
            _ => return Err(Error::Unsupported("typed world-state packet")),
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        match self {
            Self::GameState(p) => p.encode(version, limits),
            Self::Time(p) => p.encode(version, limits),
            Self::Spawn(p) => p.encode(version, limits),
            Self::Difficulty(p) => p.encode(version, limits),
            Self::BorderInitialize(p) => p.encode(version, limits),
            Self::BorderCenter(p) => p.encode(version, limits),
            Self::BorderLerp(p) => p.encode(version, limits),
            Self::BorderSize(p) => p.encode(version, limits),
            Self::BorderWarningDelay(p) => p.encode(version, limits),
            Self::BorderWarningDistance(p) => p.encode(version, limits),
            Self::BlockChangeAck(p) => p.encode(version, limits),
        }
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        match self {
            Self::GameState(p) => p.packet(version, limits),
            Self::Time(p) => p.packet(version, limits),
            Self::Spawn(p) => p.packet(version, limits),
            Self::Difficulty(p) => p.packet(version, limits),
            Self::BorderInitialize(p) => p.packet(version, limits),
            Self::BorderCenter(p) => p.packet(version, limits),
            Self::BorderLerp(p) => p.packet(version, limits),
            Self::BorderSize(p) => p.packet(version, limits),
            Self::BorderWarningDelay(p) => p.packet(version, limits),
            Self::BorderWarningDistance(p) => p.packet(version, limits),
            Self::BlockChangeAck(p) => p.packet(version, limits),
        }
    }
}
