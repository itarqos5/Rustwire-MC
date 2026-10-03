//! Bounded clientbound HUD/player-feedback envelopes for protocols 763–776.
//!
//! These packets describe UI notifications, not rendered components or gameplay
//! state. Components retain JSON through 764 and anonymous NBT from 765 without
//! validating their meaning. Integer scalars and every experience-bar float bit
//! pattern are retained; there is no clamping or registry lookup. An unrecognized
//! book-hand ID remains a typed raw scalar, not an unsupported nested layout.
//!
//! Evidence is the hash-pinned schemas and independently authored wire fixtures.
//! No release-API or live-server validation is claimed for this module; see
//! `docs/hud-wire-audit.md` for exact boundaries.
use super::{
    chat::ChatComponent,
    interact::Hand,
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
            .ok_or(Error::Invalid("absent HUD component NBT"))
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
            let mut prefix = 1usize;
            let mut len = json.len();
            while len > 127 {
                prefix += 1;
                len >>= 7;
            }
            if json.len().saturating_add(prefix)
                > budget.limits.max_packet.saturating_sub(w.as_slice().len())
            {
                return Err(Error::Limit("HUD component bytes"));
            }
            w.string(json, 262_144.min(budget.limits.max_string_chars))?;
        }
        (ChatComponent::Nbt(nbt), false) => {
            inventory::write_nbt(Some(nbt), w, RootFormat::Anonymous, budget)?;
        }
        _ => return Err(Error::Invalid("HUD component representation for version")),
    }
    budget.check_bytes(w)
}

macro_rules! packet_codec {
    ($ty:ty, $name:literal) => {
        impl $ty {
            /// Reads one bounded body, leaving the reader unchanged on failure.
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
                    return Err(Error::Limit("HUD packet bytes"));
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
pub struct ClearTitles {
    pub reset: bool,
}
impl ClearTitles {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self { reset: r.bool()? })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.bool(self.reset);
        Ok(())
    }
}
packet_codec!(ClearTitles, "clear_titles");

macro_rules! text_packet {
    ($ty:ident, $name:literal) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct $ty {
            pub text: ChatComponent,
        }
        impl $ty {
            fn read_body(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
                Ok(Self {
                    text: component(r, version, b)?,
                })
            }
            fn write_body(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
                write_component(&self.text, w, version, b)
            }
        }
        packet_codec!($ty, $name);
    };
}
text_packet!(ActionBar, "action_bar");
text_packet!(SetTitleText, "set_title_text");
text_packet!(SetTitleSubtitle, "set_title_subtitle");

/// Signed big-endian tick scalars. Negative values are retained without policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetTitleTime {
    pub fade_in: i32,
    pub stay: i32,
    pub fade_out: i32,
}
impl SetTitleTime {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            fade_in: r.i32()?,
            stay: r.i32()?,
            fade_out: r.i32()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.i32(self.fade_in);
        w.i32(self.stay);
        w.i32(self.fade_out);
        Ok(())
    }
}
packet_codec!(SetTitleTime, "set_title_time");

/// Only protocol 775's pinned schema annotates the two known hand names.
/// All releases carry a VarInt; unknown values are preserved without claiming
/// that the receiving game accepts them. `known_hand` provides an optional view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenBook {
    pub hand_id: i32,
}
impl OpenBook {
    pub const fn for_hand(hand: Hand) -> Self {
        Self {
            hand_id: hand as i32,
        }
    }
    pub const fn known_hand(self) -> Option<Hand> {
        match self.hand_id {
            0 => Some(Hand::Main),
            1 => Some(Hand::Off),
            _ => None,
        }
    }
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            hand_id: r.var_i32()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.var_i32(self.hand_id);
        Ok(())
    }
}
packet_codec!(OpenBook, "open_book");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Experience {
    /// Unclamped wire float; NaN payloads, infinities and signed zero survive.
    pub bar: f32,
    pub level: i32,
    pub total: i32,
}
impl Experience {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            bar: r.f32()?,
            level: r.var_i32()?,
            total: r.var_i32()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.f32(self.bar);
        w.var_i32(self.level);
        w.var_i32(self.total);
        Ok(())
    }
}
packet_codec!(Experience, "experience");

/// Bodyless clientbound notification. No combat state machine is maintained.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnterCombat;
impl EnterCombat {
    fn read_body(_: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self)
    }
    fn write_body(&self, _: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        Ok(())
    }
}
packet_codec!(EnterCombat, "enter_combat_event");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EndCombat {
    /// Signed duration scalar, not an allocation count or elapsed-time estimate.
    pub duration: i32,
}
impl EndCombat {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            duration: r.var_i32()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.var_i32(self.duration);
        Ok(())
    }
}
packet_codec!(EndCombat, "end_combat_event");

#[derive(Clone, Debug, PartialEq)]
pub struct DeathCombat {
    /// Unresolved wire entity ID, without an entity-registry lookup.
    pub player_id: i32,
    pub message: ChatComponent,
}
impl DeathCombat {
    fn read_body(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        Ok(Self {
            player_id: r.var_i32()?,
            message: component(r, version, b)?,
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        w.var_i32(self.player_id);
        write_component(&self.message, w, version, b)
    }
}
packet_codec!(DeathCombat, "death_combat_event");

macro_rules! family {
    ($($variant:ident($ty:ty) => $name:literal),* $(,)?) => {
        #[derive(Clone, Debug, PartialEq)]
        pub enum HudPacket { $($variant($ty)),* }
        impl HudPacket {
            /// Unknown packet names are unsupported. Malformed known bodies
            /// return their decoding error; no opaque body is guessed or skipped.
            pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                Ok(match name {
                    $($name => Self::$variant(<$ty>::decode(bytes, version, limits)?),)*
                    _ => return Err(Error::Unsupported("typed HUD packet")),
                })
            }
            pub const fn name(&self) -> &'static str {
                match self { $(Self::$variant(_) => $name),* }
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
family! {
    ClearTitles(ClearTitles) => "clear_titles",
    ActionBar(ActionBar) => "action_bar",
    Title(SetTitleText) => "set_title_text",
    Subtitle(SetTitleSubtitle) => "set_title_subtitle",
    TitleTime(SetTitleTime) => "set_title_time",
    OpenBook(OpenBook) => "open_book",
    Experience(Experience) => "experience",
    EnterCombat(EnterCombat) => "enter_combat_event",
    EndCombat(EndCombat) => "end_combat_event",
    DeathCombat(DeathCombat) => "death_combat_event",
}
