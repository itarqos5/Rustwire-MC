//! Player-list update envelopes for release protocols 763–776.
//!
//! Profiles, public keys and signatures are unverified wire data. Parsing them
//! does not authenticate an identity, verify a signature, or establish a secure
//! chat session. Applications own those decisions and the resulting roster.
//!
//! Layouts are checked against the pinned schemas and release server codecs.
//! In particular, official action ordinals are list order = 6 and hat = 7;
//! some pinned schemas incorrectly swap those two bit labels from protocol 769.
use super::{
    chat::ChatComponent,
    inventory::{self, Budget},
    ProfileProperty,
};
use crate::{
    codec::{Reader, Writer},
    nbt::RootFormat,
    Error, Limits, Result, Version,
};
use std::collections::BTreeSet;

/// A packet-wide action set. Every entry must contain exactly these updates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerInfoActions {
    pub add_player: bool,
    pub initialize_chat: bool,
    pub update_game_mode: bool,
    pub update_listed: bool,
    pub update_latency: bool,
    pub update_display_name: bool,
    /// Introduced in protocol 768, always bit 6.
    pub update_list_order: bool,
    /// Introduced in protocol 769, always bit 7.
    pub update_hat: bool,
}
impl PlayerInfoActions {
    pub fn from_mask(mask: u8, version: Version) -> Result<Self> {
        let allowed = match version.protocol() {
            763..=767 => 0x3f,
            768 => 0x7f,
            _ => 0xff,
        };
        if mask & !allowed != 0 {
            return Err(Error::Invalid("player info action mask for protocol"));
        }
        Ok(Self {
            add_player: mask & 1 != 0,
            initialize_chat: mask & 2 != 0,
            update_game_mode: mask & 4 != 0,
            update_listed: mask & 8 != 0,
            update_latency: mask & 16 != 0,
            update_display_name: mask & 32 != 0,
            update_list_order: mask & 64 != 0,
            update_hat: mask & 128 != 0,
        })
    }
    pub fn mask(self, version: Version) -> Result<u8> {
        let mask = u8::from(self.add_player)
            | (u8::from(self.initialize_chat) << 1)
            | (u8::from(self.update_game_mode) << 2)
            | (u8::from(self.update_listed) << 3)
            | (u8::from(self.update_latency) << 4)
            | (u8::from(self.update_display_name) << 5)
            | (u8::from(self.update_list_order) << 6)
            | (u8::from(self.update_hat) << 7);
        Self::from_mask(mask, version)?;
        Ok(mask)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}
impl GameMode {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::Survival),
            1 => Ok(Self::Creative),
            2 => Ok(Self::Adventure),
            3 => Ok(Self::Spectator),
            _ => Err(Error::Invalid("player game mode")),
        }
    }
}
/// Profile property names may repeat: the official representation is a multimap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerProfile {
    pub name: String,
    pub properties: Vec<ProfileProperty>,
}
/// An unverified remote chat-session envelope, not an authenticated session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSession {
    pub session_id: [u8; 16],
    /// Unix epoch milliseconds. Expiration is not checked by this codec.
    pub expires_at: i64,
    pub public_key: Vec<u8>,
    pub key_signature: Vec<u8>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerInfoEntry {
    pub uuid: [u8; 16],
    pub profile: Option<PlayerProfile>,
    /// Outer None means unchanged; Some(None) explicitly clears the session.
    pub chat_session: Option<Option<ChatSession>>,
    pub game_mode: Option<GameMode>,
    pub listed: Option<bool>,
    /// Signed wire latency, retained without estimating or clamping.
    pub latency: Option<i32>,
    /// Outer None means unchanged; Some(None) removes the display-name override.
    pub display_name: Option<Option<ChatComponent>>,
    pub list_order: Option<i32>,
    pub show_hat: Option<bool>,
}
impl PlayerInfoEntry {
    fn actions(&self) -> PlayerInfoActions {
        PlayerInfoActions {
            add_player: self.profile.is_some(),
            initialize_chat: self.chat_session.is_some(),
            update_game_mode: self.game_mode.is_some(),
            update_listed: self.listed.is_some(),
            update_latency: self.latency.is_some(),
            update_display_name: self.display_name.is_some(),
            update_list_order: self.list_order.is_some(),
            update_hat: self.show_hat.is_some(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerInfo {
    pub actions: PlayerInfoActions,
    pub entries: Vec<PlayerInfoEntry>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerRemove {
    pub players: Vec<[u8; 16]>,
}

fn reader(bytes: &[u8], limits: Limits) -> Result<Reader<'_>> {
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("player packet bytes"));
    }
    Ok(Reader::new(bytes, limits))
}
fn string(w: &mut Writer, s: &str, max: usize, b: &Budget) -> Result<()> {
    if s.len() > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("player packet string bytes"));
    }
    w.string(s, max.min(b.limits.max_string_chars))?;
    b.check_bytes(w)
}
fn read_session_bytes(r: &mut Reader<'_>, max: usize, b: &mut Budget) -> Result<Vec<u8>> {
    let count = r.count(max.min(b.limits.max_collection))?;
    b.charge(count)?;
    Ok(r.take(count)?.to_vec())
}
fn read_component(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<ChatComponent> {
    if version.protocol() < 765 {
        ChatComponent::read(r, version)
    } else {
        Ok(ChatComponent::Nbt(
            inventory::read_nbt(r, RootFormat::Anonymous, b)?
                .ok_or(Error::Invalid("absent player display component"))?,
        ))
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
        _ => Err(Error::Invalid("player display component for protocol")),
    }
}
impl PlayerInfo {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let actions = PlayerInfoActions::from_mask(r.u8()?, version)?;
        let mut b = Budget::new(limits);
        let count = b.count(&mut r)?;
        let mut entries = Vec::with_capacity(count.min(r.remaining().len() / 16));
        let mut seen = BTreeSet::new();
        for _ in 0..count {
            let uuid = r.uuid()?;
            if !seen.insert(uuid) {
                return Err(Error::Invalid("duplicate player info UUID"));
            }
            let mut entry = PlayerInfoEntry {
                uuid,
                ..PlayerInfoEntry::default()
            };
            if actions.add_player {
                let name = r.string(16)?.to_owned();
                let count = b.count(&mut r)?;
                if version.protocol() >= 766 && count > 16 {
                    return Err(Error::Limit("player profile property count"));
                }
                let mut properties = Vec::with_capacity(count.min(r.remaining().len() / 3));
                for _ in 0..count {
                    properties.push(ProfileProperty {
                        name: r
                            .string(if version.protocol() >= 766 { 64 } else { 32767 })?
                            .to_owned(),
                        value: r.string(32767)?.to_owned(),
                        signature: if r.bool()? {
                            Some(
                                r.string(if version.protocol() >= 766 {
                                    1024
                                } else {
                                    32767
                                })?
                                .to_owned(),
                            )
                        } else {
                            None
                        },
                    });
                }
                entry.profile = Some(PlayerProfile { name, properties });
            }
            if actions.initialize_chat {
                entry.chat_session = Some(if r.bool()? {
                    Some(ChatSession {
                        session_id: r.uuid()?,
                        expires_at: r.i64()?,
                        public_key: read_session_bytes(&mut r, 512, &mut b)?,
                        key_signature: read_session_bytes(&mut r, 4096, &mut b)?,
                    })
                } else {
                    None
                });
            }
            if actions.update_game_mode {
                entry.game_mode = Some(GameMode::read(&mut r)?);
            }
            if actions.update_listed {
                entry.listed = Some(r.bool()?);
            }
            if actions.update_latency {
                entry.latency = Some(r.var_i32()?);
            }
            if actions.update_display_name {
                entry.display_name = Some(if r.bool()? {
                    Some(read_component(&mut r, version, &mut b)?)
                } else {
                    None
                });
            }
            if actions.update_list_order {
                entry.list_order = Some(r.var_i32()?);
            }
            if actions.update_hat {
                entry.show_hat = Some(r.bool()?);
            }
            entries.push(entry);
        }
        r.finish()?;
        Ok(Self { actions, entries })
    }
    /// All updates must match the packet action set. No partially encoded body
    /// is returned on error, and collection/NBT budgets span the whole packet.
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        w.u8(self.actions.mask(version)?);
        let mut b = Budget::new(limits);
        b.write_count(self.entries.len(), &mut w)?;
        let mut seen = BTreeSet::new();
        for entry in &self.entries {
            if entry.actions() != self.actions {
                return Err(Error::Invalid("player entry fields do not match actions"));
            }
            if !seen.insert(entry.uuid) {
                return Err(Error::Invalid("duplicate player info UUID"));
            }
            w.raw(&entry.uuid);
            if let Some(profile) = &entry.profile {
                string(&mut w, &profile.name, 16, &b)?;
                if version.protocol() >= 766 && profile.properties.len() > 16 {
                    return Err(Error::Limit("player profile property count"));
                }
                b.write_count(profile.properties.len(), &mut w)?;
                for property in &profile.properties {
                    string(
                        &mut w,
                        &property.name,
                        if version.protocol() >= 766 { 64 } else { 32767 },
                        &b,
                    )?;
                    string(&mut w, &property.value, 32767, &b)?;
                    w.bool(property.signature.is_some());
                    if let Some(signature) = &property.signature {
                        string(
                            &mut w,
                            signature,
                            if version.protocol() >= 766 {
                                1024
                            } else {
                                32767
                            },
                            &b,
                        )?;
                    }
                }
            }
            if let Some(session) = &entry.chat_session {
                w.bool(session.is_some());
                if let Some(session) = session {
                    if session.public_key.len() > 512.min(limits.max_collection)
                        || session.key_signature.len() > 4096.min(limits.max_collection)
                    {
                        return Err(Error::Limit("player chat session bytes"));
                    }
                    b.charge(session.public_key.len())?;
                    b.charge(session.key_signature.len())?;
                    w.raw(&session.session_id);
                    w.i64(session.expires_at);
                    w.var_i32(session.public_key.len() as i32);
                    w.raw(&session.public_key);
                    w.var_i32(session.key_signature.len() as i32);
                    w.raw(&session.key_signature);
                    b.check_bytes(&w)?;
                }
            }
            if let Some(mode) = entry.game_mode {
                w.var_i32(mode as i32);
            }
            if let Some(listed) = entry.listed {
                w.bool(listed);
            }
            if let Some(latency) = entry.latency {
                w.var_i32(latency);
            }
            if let Some(display) = &entry.display_name {
                w.bool(display.is_some());
                if let Some(display) = display {
                    write_component(display, &mut w, version, &mut b)?;
                }
            }
            if let Some(order) = entry.list_order {
                w.var_i32(order);
            }
            if let Some(hat) = entry.show_hat {
                w.bool(hat);
            }
            b.check_bytes(&w)?;
        }
        b.check_bytes(&w)?;
        Ok(w.into_inner())
    }
}
impl PlayerRemove {
    pub fn decode(bytes: &[u8], _version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let count = r.count(limits.max_collection)?;
        let mut players = Vec::with_capacity(count.min(r.remaining().len() / 16));
        let mut seen = BTreeSet::new();
        for _ in 0..count {
            let uuid = r.uuid()?;
            if !seen.insert(uuid) {
                return Err(Error::Invalid("duplicate removed player UUID"));
            }
            players.push(uuid);
        }
        r.finish()?;
        Ok(Self { players })
    }
    pub fn encode(&self, _version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        b.write_count(self.players.len(), &mut w)?;
        let mut seen = BTreeSet::new();
        for uuid in &self.players {
            if !seen.insert(uuid) {
                return Err(Error::Invalid("duplicate removed player UUID"));
            }
            w.raw(uuid);
            b.check_bytes(&w)?;
        }
        Ok(w.into_inner())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerPacket {
    Info(PlayerInfo),
    Remove(PlayerRemove),
}
impl PlayerPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        match name {
            "player_info" => Ok(Self::Info(PlayerInfo::decode(bytes, version, limits)?)),
            "player_remove" => Ok(Self::Remove(PlayerRemove::decode(bytes, version, limits)?)),
            _ => Err(Error::Unsupported("typed player packet")),
        }
    }
}
