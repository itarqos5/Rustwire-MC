//! Game-rule string envelopes and low-disk notifications from protocol 775.
//! Values remain text: no registry lookup, value parsing or settings changes occur.
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRuleEntry {
    pub key: String,
    pub value: String,
}
fn preflight(w: &Writer, additional: usize, limits: Limits) -> Result<()> {
    if w.as_slice().len() > limits.max_packet
        || additional > limits.max_packet.saturating_sub(w.as_slice().len())
    {
        return Err(Error::Limit("game-rule packet bytes"));
    }
    Ok(())
}
fn string(w: &mut Writer, value: &str, limits: Limits) -> Result<()> {
    let cap = limits.max_string_chars.min(32767);
    if value.encode_utf16().count() > cap || value.len() > cap.saturating_mul(3) {
        return Err(Error::Limit("game-rule string length"));
    }
    let mut size = value.len();
    let mut prefix = 1;
    while size > 127 {
        prefix += 1;
        size >>= 7;
    }
    preflight(
        w,
        value
            .len()
            .checked_add(prefix)
            .ok_or(Error::Limit("game-rule string bytes"))?,
        limits,
    )?;
    w.string(value, cap)
}
fn read_entries(r: &mut Reader<'_>, version: Version) -> Result<Vec<GameRuleEntry>> {
    let n = r.count(r.limits.max_collection)?;
    if n > r.remaining().len() / 2 {
        return Err(Error::Eof);
    }
    let mut rules = Vec::new();
    for _ in 0..n {
        let key = r.string(32767)?;
        identifier::validate(key, version)?;
        let key = key.to_owned();
        let value = r.string(32767)?.to_owned();
        rules.push(GameRuleEntry { key, value });
    }
    Ok(rules)
}
fn write_entries(
    rules: &[GameRuleEntry],
    w: &mut Writer,
    version: Version,
    limits: Limits,
) -> Result<()> {
    if rules.len() > limits.max_collection.min(i32::MAX as usize) {
        return Err(Error::Limit("game-rule count"));
    }
    w.var_i32(rules.len() as i32);
    preflight(
        w,
        rules
            .len()
            .checked_mul(2)
            .ok_or(Error::Limit("game-rule count bytes"))?,
        limits,
    )?;
    for rule in rules {
        identifier::validate(&rule.key, version)?;
        string(w, &rule.key, limits)?;
        string(w, &rule.value, limits)?;
    }
    Ok(())
}
macro_rules! rule_packet {
    ($ty:ident,$direction:ident,$name:literal) => {
        /// Ordered entries preserve duplicate wire keys; no map replacement is applied.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $ty {
            pub rules: Vec<GameRuleEntry>,
        }
        impl $ty {
            /// Reads one bounded body without advancing the reader on error.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                version.packet_id(State::Play, Direction::$direction, $name)?;
                let mut body = Reader::new(
                    &r.remaining()[..r.remaining().len().min(r.limits.max_packet)],
                    r.limits,
                );
                let rules = read_entries(&mut body, version)?;
                r.take(body.position())?;
                Ok(Self { rules })
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("game-rule packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let result = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(result)
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                version.packet_id(State::Play, Direction::$direction, $name)?;
                let mut w = Writer::new();
                write_entries(&self.rules, &mut w, version, limits)?;
                preflight(&w, 0, limits)?;
                Ok(w.into_inner())
            }
            /// Appends only a complete body; the destination is unchanged on error.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?);
                Ok(())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::$direction, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    };
}
rule_packet!(GameRuleValues, Clientbound, "game_rule_values");
rule_packet!(SetGameRules, Serverbound, "set_game_rule");
/// An empty-body server notification. This is not a local disk-space reading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LowDiskSpaceWarning;
impl LowDiskSpaceWarning {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        version.packet_id(
            State::Play,
            Direction::Clientbound,
            "low_disk_space_warning",
        )?;
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("low-disk packet bytes"));
        }
        if !bytes.is_empty() {
            return Err(Error::Invalid("low-disk warning trailing bytes"));
        }
        Ok(Self)
    }
    pub fn encode(&self, version: Version, _limits: Limits) -> Result<Vec<u8>> {
        version.packet_id(
            State::Play,
            Direction::Clientbound,
            "low_disk_space_warning",
        )?;
        Ok(vec![])
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(
                State::Play,
                Direction::Clientbound,
                "low_disk_space_warning",
            )?,
            self.encode(version, limits)?,
        ))
    }
}
