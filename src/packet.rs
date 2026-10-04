//! Typed codecs for connection control and common play operations.
pub mod advancements;
pub mod blocks;
pub mod chat;
pub mod chunk_updates;
pub mod commands;
pub mod common;
pub mod entity;
pub mod entity_control;
pub mod entity_metadata;
pub mod entity_state;
pub mod hud;
pub mod interact;
pub mod inventory;
pub mod item_hash;
pub mod map;
pub mod movement;
pub mod overlay;
pub mod player;
pub mod scoreboard;
pub mod server_metadata;
pub mod statistics;
pub mod tags;
pub mod typed;
mod world;
pub mod world_control;
pub mod world_effects;
pub mod world_state;
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};
pub use world::{DeathLocation, DimensionRef, JoinGame, SpawnInfo};
pub fn named(version: Version, state: State, name: &str, data: Vec<u8>) -> Result<RawPacket> {
    Ok(RawPacket::new(
        version.packet_id(state, Direction::Serverbound, name)?,
        data,
    ))
}
pub fn handshake(version: Version, host: &str, port: u16, next: State) -> Result<RawPacket> {
    let next = match next {
        State::Status => 1,
        State::Login => 2,
        _ => return Err(Error::State("handshake next state")),
    };
    let mut w = Writer::new();
    w.var_i32(version.protocol());
    w.string(host, 255)?;
    w.u16(port);
    w.var_i32(next);
    Ok(RawPacket::new(0, w.into_inner()))
}
pub fn login_start(version: Version, username: &str, uuid: [u8; 16]) -> Result<RawPacket> {
    if username.is_empty()
        || !username
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(Error::Invalid("Minecraft username"));
    }
    let mut w = Writer::new();
    w.string(username, 16)?;
    if version.protocol() == 763 {
        w.bool(true);
    }
    w.raw(&uuid);
    named(version, State::Login, "login_start", w.into_inner())
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileProperty {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSuccess {
    pub uuid: [u8; 16],
    pub username: String,
    pub properties: Vec<ProfileProperty>,
    pub strict_error_handling: Option<bool>,
    pub session_id: Option<[u8; 16]>,
}
impl LoginSuccess {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let uuid = r.uuid()?;
        let username = r.string(16)?.to_owned();
        let n = r.count(1024.min(limits.max_collection))?;
        let mut properties = Vec::with_capacity(n);
        for _ in 0..n {
            properties.push(ProfileProperty {
                name: r.string(32767)?.to_owned(),
                value: r.string(32767)?.to_owned(),
                signature: if r.bool()? {
                    Some(r.string(32767)?.to_owned())
                } else {
                    None
                },
            });
        }
        let strict_error_handling = if (766..=767).contains(&version.protocol()) {
            Some(r.bool()?)
        } else {
            None
        };
        let session_id = if version.protocol() >= 776 {
            Some(r.uuid()?)
        } else {
            None
        };
        r.finish()?;
        Ok(Self {
            uuid,
            username,
            properties,
            strict_error_handling,
            session_id,
        })
    }
}
#[derive(Debug, Clone)]
pub struct EncryptionRequest {
    pub server_id: String,
    pub public_key: Vec<u8>,
    pub verify_token: Vec<u8>,
    pub should_authenticate: bool,
}
impl EncryptionRequest {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let server_id = r.string(20)?.to_owned();
        let public_key = r.bytes(8192)?.to_vec();
        let verify_token = r.bytes(1024)?.to_vec();
        let should_authenticate = if version.protocol() >= 766 {
            r.bool()?
        } else {
            true
        };
        r.finish()?;
        Ok(Self {
            server_id,
            public_key,
            verify_token,
            should_authenticate,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}
pub fn known_packs(bytes: &[u8], limits: Limits) -> Result<Vec<KnownPack>> {
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("packet bytes"));
    }
    let mut r = Reader::new(bytes, limits);
    let count = r.count(1024.min(limits.max_collection))?;
    let mut packs = Vec::with_capacity(count);
    for _ in 0..count {
        packs.push(KnownPack {
            namespace: r.string(32767)?.into(),
            id: r.string(32767)?.into(),
            version: r.string(32767)?.into(),
        });
    }
    r.finish()?;
    Ok(packs)
}
/// Client configuration. Resource packs and server transfers remain application decisions.
#[derive(Debug, Clone)]
pub struct ClientSettings {
    pub locale: String,
    pub view_distance: u8,
    pub chat_mode: i32,
    pub chat_colors: bool,
    pub skin_parts: u8,
    pub main_hand: i32,
    pub text_filtering: bool,
    pub server_listing: bool,
    pub particle_status: i32,
}
impl Default for ClientSettings {
    fn default() -> Self {
        Self {
            locale: "en_us".into(),
            view_distance: 8,
            chat_mode: 0,
            chat_colors: true,
            skin_parts: 0x7f,
            main_hand: 1,
            text_filtering: false,
            server_listing: false,
            particle_status: 0,
        }
    }
}
impl ClientSettings {
    pub fn encode(&self, version: Version, state: State) -> Result<RawPacket> {
        if !(2..=32).contains(&self.view_distance)
            || !(0..=2).contains(&self.chat_mode)
            || !(0..=1).contains(&self.main_hand)
            || !(0..=2).contains(&self.particle_status)
        {
            return Err(Error::Invalid("client settings"));
        }
        let mut w = Writer::new();
        w.string(&self.locale, 16)?;
        w.u8(self.view_distance);
        w.var_i32(self.chat_mode);
        w.bool(self.chat_colors);
        w.u8(self.skin_parts);
        w.var_i32(self.main_hand);
        w.bool(self.text_filtering);
        w.bool(self.server_listing);
        if version.protocol() >= 768 {
            w.var_i32(self.particle_status);
        }
        named(version, state, "settings", w.into_inner())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct PositionSync {
    pub teleport_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub velocity: Option<[f64; 3]>,
    pub yaw: f32,
    pub pitch: f32,
    pub relative_flags: u32,
}
impl PositionSync {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let modern = version.protocol() >= 768;
        let first_id = if modern { r.var_i32()? } else { 0 };
        let (x, y, z) = (r.f64()?, r.f64()?, r.f64()?);
        let velocity = if modern {
            Some([r.f64()?, r.f64()?, r.f64()?])
        } else {
            None
        };
        let (yaw, pitch) = (r.f32()?, r.f32()?);
        let relative_flags = if modern {
            r.i32()? as u32
        } else {
            r.u8()? as u32
        };
        let teleport_id = if modern { first_id } else { r.var_i32()? };
        r.finish()?;
        if ![x, y, z].iter().all(|n| n.is_finite())
            || !yaw.is_finite()
            || !pitch.is_finite()
            || velocity.is_some_and(|v| !v.iter().all(|n| n.is_finite()))
        {
            return Err(Error::Invalid("non-finite position"));
        }
        Ok(Self {
            teleport_id,
            x,
            y,
            z,
            velocity,
            yaw,
            pitch,
            relative_flags,
        })
    }
    pub fn acknowledgement(&self, version: Version) -> Result<RawPacket> {
        let mut w = Writer::new();
        w.var_i32(self.teleport_id);
        named(version, State::Play, "teleport_confirm", w.into_inner())
    }
}
/// Absolute player movement. Position flags changed to a bitfield in 1.21.2.
pub fn player_position(
    version: Version,
    x: f64,
    y: f64,
    z: f64,
    on_ground: bool,
    horizontal_collision: bool,
) -> Result<RawPacket> {
    movement::PlayerMovement {
        position: Some([x, y, z]),
        rotation: None,
        on_ground,
        horizontal_collision,
    }
    .packet(version, Limits::default())
}
pub fn custom_payload(
    version: Version,
    state: State,
    channel: &str,
    payload: &[u8],
) -> Result<RawPacket> {
    if payload.len() > 32767 {
        return Err(Error::Limit("serverbound custom payload"));
    }
    let mut w = Writer::new();
    w.string(channel, 32767)?;
    w.raw(payload);
    named(version, state, "custom_payload", w.into_inner())
}
