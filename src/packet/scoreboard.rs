//! Scoreboard objectives, display slots, scores and teams for protocols 763–776.
//!
//! These types preserve wire values, not game rules or a client-side scoreboard.
//! Components and styles are untrusted, unrendered data. Number formats require
//! protocol 765; team parameters change at 770 and again at 776. See the release
//! serializer audit in `docs/scoreboard-wire-audit.md` for boundary provenance.
mod objective;
mod score;
mod team;
pub use objective::*;
pub use score::*;
pub use team::*;

use super::{
    chat::ChatComponent,
    inventory::{self, Budget},
};
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    nbt::{Nbt, RootFormat},
    version::{Direction, State},
    Error, Limits, Result, Version,
};

trait Body: Sized {
    fn read(r: &mut Reader<'_>, version: Version, budget: &mut Budget) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, budget: &mut Budget) -> Result<()>;
}
macro_rules! codec {
    ($($t:ty => $name:literal),* $(,)?) => {$ (
        impl $t {
            /// Reads one body transactionally, leaving subsequent bytes unread.
            /// The reader's limits bound this body and aggregate NBT nodes.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                let limits = r.limits;
                let input = &r.remaining()[..r.remaining().len().min(limits.max_packet)];
                let mut bounded = Reader::new(input, limits);
                let value = <Self as Body>::read(&mut bounded, version, &mut Budget::new(limits))?;
                r.take(bounded.position())?;
                Ok(value)
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet { return Err(Error::Limit("scoreboard packet bytes")); }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            /// Encoding is transactional: an error never returns a partial body.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                let mut b = Budget::new(limits);
                <Self as Body>::write(self, &mut w, version, &mut b)?;
                b.check_bytes(&w)?;
                Ok(w.into_inner())
            }
            /// Appends one body; the destination is unchanged on failure.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?);
                Ok(())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(version.packet_id(State::Play, Direction::Clientbound, $name)?, self.encode(version, limits)?))
            }
        }
    )*};
}
codec! {
    ScoreboardObjective => "scoreboard_objective",
    DisplayObjective => "scoreboard_display_objective",
    ScoreboardScore => "scoreboard_score",
    ResetScore => "reset_score",
    Teams => "teams",
}
fn string(w: &mut Writer, value: &str, maximum: usize, b: &Budget) -> Result<()> {
    // Check before allocating an arbitrarily large output string.
    if value.len() > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("scoreboard string bytes"));
    }
    w.string(value, maximum.min(b.limits.max_string_chars))?;
    b.check_bytes(w)
}
fn read_nbt(r: &mut Reader<'_>, b: &mut Budget) -> Result<Nbt> {
    inventory::read_nbt(r, RootFormat::Anonymous, b)?.ok_or(Error::Invalid("absent scoreboard NBT"))
}
fn read_component(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<ChatComponent> {
    if version.protocol() < 765 {
        ChatComponent::read(r, version)
    } else {
        Ok(ChatComponent::Nbt(read_nbt(r, b)?))
    }
}
fn write_component(
    value: &ChatComponent,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    match (value, version.protocol() >= 765) {
        (ChatComponent::Json(s), false) => string(w, s, 262_144, b),
        (ChatComponent::Nbt(nbt), true) => {
            inventory::write_nbt(Some(nbt), w, RootFormat::Anonymous, b)
        }
        _ => Err(Error::Invalid("scoreboard component for protocol")),
    }
}

/// Typed clientbound scoreboard packet family. State/direction dispatch belongs
/// to the enclosing connection or typed-packet dispatcher.
#[derive(Clone, Debug, PartialEq)]
pub enum ScoreboardPacket {
    Objective(ScoreboardObjective),
    Display(DisplayObjective),
    Score(ScoreboardScore),
    Reset(ResetScore),
    Teams(Teams),
}
impl ScoreboardPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "scoreboard_objective" => {
                Self::Objective(ScoreboardObjective::decode(bytes, version, limits)?)
            }
            "scoreboard_display_objective" => {
                Self::Display(DisplayObjective::decode(bytes, version, limits)?)
            }
            "scoreboard_score" => Self::Score(ScoreboardScore::decode(bytes, version, limits)?),
            "reset_score" => Self::Reset(ResetScore::decode(bytes, version, limits)?),
            "teams" => Self::Teams(Teams::decode(bytes, version, limits)?),
            _ => return Err(Error::Unsupported("typed scoreboard packet")),
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        match self {
            Self::Objective(p) => p.encode(version, limits),
            Self::Display(p) => p.encode(version, limits),
            Self::Score(p) => p.encode(version, limits),
            Self::Reset(p) => p.encode(version, limits),
            Self::Teams(p) => p.encode(version, limits),
        }
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        match self {
            Self::Objective(p) => p.packet(version, limits),
            Self::Display(p) => p.packet(version, limits),
            Self::Score(p) => p.packet(version, limits),
            Self::Reset(p) => p.packet(version, limits),
            Self::Teams(p) => p.packet(version, limits),
        }
    }
}
