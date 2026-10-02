//! Clientbound boss bars and player-list header/footer overlays, protocols 763–776.
//!
//! These are wire envelopes, not UI state or rendered text. Chat components use
//! JSON through 764 and anonymous NBT from 765; header and footer share one NBT
//! node budget. Boss progress retains every IEEE-754 bit pattern and flags retain
//! reserved bits. The official wire serializers do not constrain progress to
//! the nominal 0–1 display range. Unknown operation, color and division IDs are
//! rejected because those fields are closed wire enums.
//!
//! Layouts follow the pinned `packet_boss_bar` and `packet_playerlist_header`
//! schemas and were checked against all fourteen cached release serializers.
use super::{
    chat::ChatComponent,
    inventory::{self, Budget},
};
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    nbt::RootFormat,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

fn component(r: &mut Reader<'_>, version: Version, budget: &mut Budget) -> Result<ChatComponent> {
    if version.protocol() < 765 {
        Ok(ChatComponent::Json(r.string(262_144)?.to_owned()))
    } else {
        inventory::read_nbt(r, RootFormat::Anonymous, budget)?
            .map(ChatComponent::Nbt)
            .ok_or(Error::Invalid("absent overlay component NBT"))
    }
}
fn write_component(
    value: &ChatComponent,
    w: &mut Writer,
    version: Version,
    budget: &mut Budget,
) -> Result<()> {
    match (value, version.protocol() < 765) {
        (ChatComponent::Json(json), true) => {
            // Reject oversized input before copying it into an output allocation.
            let prefix = (32 - (json.len().min(u32::MAX as usize) as u32).leading_zeros())
                .max(1)
                .div_ceil(7) as usize;
            if json.len().saturating_add(prefix)
                > budget.limits.max_packet.saturating_sub(w.as_slice().len())
            {
                return Err(Error::Limit("overlay component bytes"));
            }
            w.string(json, 262_144.min(budget.limits.max_string_chars))?;
        }
        (ChatComponent::Nbt(nbt), false) => {
            inventory::write_nbt(Some(nbt), w, RootFormat::Anonymous, budget)?;
        }
        _ => {
            return Err(Error::Invalid(
                "overlay component representation for version",
            ))
        }
    }
    budget.check_bytes(w)
}

macro_rules! packet_codec {
    ($ty:ty, $name:literal) => {
        impl $ty {
            /// Reads one body from the reader, leaving any subsequent bytes unread.
            /// The reader's limits bound this body, including aggregate NBT nodes.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                let limits = r.limits;
                let input = &r.remaining()[..r.remaining().len().min(limits.max_packet)];
                let mut bounded = Reader::new(input, limits);
                let value = Self::read_body(&mut bounded, version, &mut Budget::new(limits))?;
                r.take(bounded.position())?;
                Ok(value)
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("overlay packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                let mut budget = Budget::new(limits);
                self.write_body(&mut w, version, &mut budget)?;
                budget.check_bytes(&w)?;
                Ok(w.into_inner())
            }
            /// Appends one body. The destination is unchanged if encoding fails.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?);
                Ok(())
            }
            /// Wraps this body with its clientbound play packet ID for the release.
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::Clientbound, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BossBarColor {
    Pink = 0,
    Blue = 1,
    Red = 2,
    Green = 3,
    Yellow = 4,
    Purple = 5,
    White = 6,
}
impl BossBarColor {
    pub fn from_id(id: i32) -> Result<Self> {
        match id {
            0 => Ok(Self::Pink),
            1 => Ok(Self::Blue),
            2 => Ok(Self::Red),
            3 => Ok(Self::Green),
            4 => Ok(Self::Yellow),
            5 => Ok(Self::Purple),
            6 => Ok(Self::White),
            _ => Err(Error::Invalid("boss bar color")),
        }
    }
    pub const fn id(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BossBarDivision {
    None = 0,
    Six = 1,
    Ten = 2,
    Twelve = 3,
    Twenty = 4,
}
impl BossBarDivision {
    pub fn from_id(id: i32) -> Result<Self> {
        match id {
            0 => Ok(Self::None),
            1 => Ok(Self::Six),
            2 => Ok(Self::Ten),
            3 => Ok(Self::Twelve),
            4 => Ok(Self::Twenty),
            _ => Err(Error::Invalid("boss bar division")),
        }
    }
    pub const fn id(self) -> i32 {
        self as i32
    }
}

/// The entire wire byte is retained, including reserved bits 3–7.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BossBarFlags(pub u8);
impl BossBarFlags {
    pub const DARKEN_SKY: u8 = 0x01;
    pub const PLAY_BOSS_MUSIC: u8 = 0x02;
    pub const CREATE_FOG: u8 = 0x04;
    pub const fn darken_sky(self) -> bool {
        self.0 & Self::DARKEN_SKY != 0
    }
    pub const fn play_boss_music(self) -> bool {
        self.0 & Self::PLAY_BOSS_MUSIC != 0
    }
    pub const fn create_fog(self) -> bool {
        self.0 & Self::CREATE_FOG != 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BossBarAction {
    Add {
        title: ChatComponent,
        /// Raw wire float, normally 0–1; no gameplay or finiteness restriction.
        health: f32,
        color: BossBarColor,
        division: BossBarDivision,
        flags: BossBarFlags,
    },
    Remove,
    /// Raw wire float, normally 0–1; no gameplay or finiteness restriction.
    UpdateHealth(f32),
    UpdateTitle(ChatComponent),
    UpdateStyle {
        color: BossBarColor,
        division: BossBarDivision,
    },
    UpdateFlags(BossBarFlags),
}
impl BossBarAction {
    pub const fn id(&self) -> i32 {
        match self {
            Self::Add { .. } => 0,
            Self::Remove => 1,
            Self::UpdateHealth(_) => 2,
            Self::UpdateTitle(_) => 3,
            Self::UpdateStyle { .. } => 4,
            Self::UpdateFlags(_) => 5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BossBar {
    /// Boss-bar identity; this UUID need not identify an entity.
    pub uuid: [u8; 16],
    pub action: BossBarAction,
}
impl BossBar {
    fn read_body(r: &mut Reader<'_>, version: Version, budget: &mut Budget) -> Result<Self> {
        let uuid = r.uuid()?;
        let action = match r.var_i32()? {
            0 => BossBarAction::Add {
                title: component(r, version, budget)?,
                health: r.f32()?,
                color: BossBarColor::from_id(r.var_i32()?)?,
                division: BossBarDivision::from_id(r.var_i32()?)?,
                flags: BossBarFlags(r.u8()?),
            },
            1 => BossBarAction::Remove,
            2 => BossBarAction::UpdateHealth(r.f32()?),
            3 => BossBarAction::UpdateTitle(component(r, version, budget)?),
            4 => BossBarAction::UpdateStyle {
                color: BossBarColor::from_id(r.var_i32()?)?,
                division: BossBarDivision::from_id(r.var_i32()?)?,
            },
            5 => BossBarAction::UpdateFlags(BossBarFlags(r.u8()?)),
            _ => return Err(Error::Invalid("boss bar action")),
        };
        Ok(Self { uuid, action })
    }
    fn write_body(&self, w: &mut Writer, version: Version, budget: &mut Budget) -> Result<()> {
        w.raw(&self.uuid);
        w.var_i32(self.action.id());
        match &self.action {
            BossBarAction::Add {
                title,
                health,
                color,
                division,
                flags,
            } => {
                write_component(title, w, version, budget)?;
                w.f32(*health);
                w.var_i32(color.id());
                w.var_i32(division.id());
                w.u8(flags.0);
            }
            BossBarAction::Remove => {}
            BossBarAction::UpdateHealth(health) => w.f32(*health),
            BossBarAction::UpdateTitle(title) => write_component(title, w, version, budget)?,
            BossBarAction::UpdateStyle { color, division } => {
                w.var_i32(color.id());
                w.var_i32(division.id());
            }
            BossBarAction::UpdateFlags(flags) => w.u8(flags.0),
        }
        budget.check_bytes(w)
    }
}
packet_codec!(BossBar, "boss_bar");

/// Text shown above and below the player list. An empty component clears a field.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerListHeaderFooter {
    pub header: ChatComponent,
    pub footer: ChatComponent,
}
impl PlayerListHeaderFooter {
    fn read_body(r: &mut Reader<'_>, version: Version, budget: &mut Budget) -> Result<Self> {
        Ok(Self {
            header: component(r, version, budget)?,
            footer: component(r, version, budget)?,
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, budget: &mut Budget) -> Result<()> {
        write_component(&self.header, w, version, budget)?;
        write_component(&self.footer, w, version, budget)
    }
}
packet_codec!(PlayerListHeaderFooter, "playerlist_header");
