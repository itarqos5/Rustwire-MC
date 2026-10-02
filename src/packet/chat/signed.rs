//! Signed-chat wire envelopes and canonical signing input, without private keys.
//!
//! Applications obtain a legitimate Minecraft chat certificate, verify its trust
//! and expiry, maintain their session/message indices, and choose an appropriate
//! RSA-2048 SHA-256 PKCS#1 v1.5 signing/verification provider. This module neither
//! generates credentials nor performs a private RSA operation. An encoded or
//! parsed signature is not necessarily valid, and does not establish identity.
use super::{state::MessageSignature, LastSeenUpdate};
use crate::{
    codec::Writer, frame::RawPacket, packet::player::ChatSession, version::State, Error, Limits,
    Result, Version,
};
use std::collections::BTreeSet;

/// Inputs authenticated by a player-message or signable-command-argument
/// signature. This is distinct from the packet's byte serialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SigningData<'a> {
    pub sender: [u8; 16],
    pub session_id: [u8; 16],
    pub index: u32,
    pub content: &'a str,
    /// Unix milliseconds on the packet wire. The signing input uses floor-divided
    /// Unix seconds, matching Java Instant even for negative timestamps.
    pub timestamp_ms: i64,
    pub salt: i64,
    /// Resolved signatures in acknowledgement order, oldest to newest.
    pub last_seen: &'a [MessageSignature],
}
impl SigningData<'_> {
    /// Canonical bytes for the application's RSA-2048/SHA-256 signature provider.
    /// No hashing or cryptographic operation is performed here.
    pub fn encode(&self, limits: Limits) -> Result<Vec<u8>> {
        if self.index > i32::MAX as u32 {
            return Err(Error::Invalid("chat chain index"));
        }
        if self.last_seen.len() > 20.min(limits.max_collection) {
            return Err(Error::Limit("signed chat history"));
        }
        if self.content.encode_utf16().count() > 32767.min(limits.max_string_chars) {
            return Err(Error::Limit("signed chat content"));
        }
        let bytes = 64usize
            .checked_add(self.content.len())
            .and_then(|n| n.checked_add(self.last_seen.len() * 256))
            .ok_or(Error::Limit("signed chat payload bytes"))?;
        if bytes > limits.max_packet {
            return Err(Error::Limit("signed chat payload bytes"));
        }
        let mut w = Writer::with_capacity(bytes);
        w.i32(1); // signature domain/version, not a VarInt
        w.raw(&self.sender);
        w.raw(&self.session_id);
        w.i32(self.index as i32);
        w.i64(self.salt);
        w.i64(self.timestamp_ms.div_euclid(1000));
        w.i32(self.content.len() as i32);
        w.raw(self.content.as_bytes());
        w.i32(self.last_seen.len() as i32);
        for signature in self.last_seen {
            w.raw(signature);
        }
        Ok(w.into_inner())
    }
    /// Delegate signing to a caller-owned provider. The callback must implement
    /// RSA-2048 with SHA-256 and PKCS#1 v1.5 over these complete canonical bytes.
    /// Returning a 256-byte value is a wire-size check, not signature verification.
    /// Session indices/acknowledgements are not advanced by this method.
    pub fn sign_with<F>(&self, limits: Limits, mut provider: F) -> Result<MessageSignature>
    where
        F: FnMut(&[u8]) -> Result<MessageSignature>,
    {
        let bytes = self.encode(limits)?;
        provider(&bytes)
    }
}

fn bounded(w: Writer, version: Version, name: &str, limits: Limits) -> Result<RawPacket> {
    if w.as_slice().len() > limits.max_packet {
        return Err(Error::Limit("signed chat packet bytes"));
    }
    super::super::named(version, State::Play, name, w.into_inner())
}
fn varint_size(mut value: usize) -> usize {
    let mut bytes = 1;
    while value > 127 {
        value >>= 7;
        bytes += 1;
    }
    bytes
}
fn text_size(value: &str, max: usize, limits: Limits) -> Result<usize> {
    if value.len() > limits.max_packet
        || value.encode_utf16().count() > max.min(limits.max_string_chars)
    {
        return Err(Error::Limit("signed chat string bytes/characters"));
    }
    value
        .len()
        .checked_add(varint_size(value.len()))
        .ok_or(Error::Limit("signed chat string bytes"))
}
fn packet_size(parts: &[usize], limits: Limits) -> Result<usize> {
    let size = parts
        .iter()
        .try_fold(0usize, |sum, n| sum.checked_add(*n))
        .ok_or(Error::Limit("signed chat packet bytes"))?;
    if size > limits.max_packet {
        return Err(Error::Limit("signed chat packet bytes"));
    }
    Ok(size)
}
fn acknowledgement(update: LastSeenUpdate, version: Version) -> Result<Vec<u8>> {
    let mut w = Writer::with_capacity(9);
    update.write(&mut w, version)?;
    Ok(w.into_inner())
}

/// A signed-message envelope using a signature supplied by the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedChatMessage {
    pub message: String,
    pub timestamp_ms: i64,
    pub salt: i64,
    pub signature: Box<MessageSignature>,
    pub last_seen: LastSeenUpdate,
}
impl SignedChatMessage {
    pub fn encode(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        let ack = acknowledgement(self.last_seen, version)?;
        let size = packet_size(
            &[text_size(&self.message, 256, limits)?, 273, ack.len()],
            limits,
        )?;
        let mut w = Writer::with_capacity(size);
        w.string(&self.message, 256.min(limits.max_string_chars))?;
        w.i64(self.timestamp_ms);
        w.i64(self.salt);
        w.bool(true);
        w.raw(self.signature.as_ref());
        w.raw(&ack);
        bounded(w, version, "chat_message", limits)
    }
}
/// The argument name is its Brigadier parser-node name, not the value being signed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgumentSignature {
    pub name: String,
    pub signature: Box<MessageSignature>,
}
/// A command's signed arguments, in the application's signable-argument order.
/// Parsing a command and deciding which arguments require signatures is outside
/// this envelope. Each signed argument uses its own session index, same timestamp,
/// salt and last-seen history; signing the whole command is not equivalent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedChatCommand {
    /// Excludes the leading slash.
    pub command: String,
    pub timestamp_ms: i64,
    pub salt: i64,
    pub arguments: Vec<ArgumentSignature>,
    pub last_seen: LastSeenUpdate,
}
impl SignedChatCommand {
    pub fn encode(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        if self.command.is_empty() || self.command.starts_with('/') {
            return Err(Error::Invalid("command must omit slash and be nonempty"));
        }
        if self.arguments.len() > 8.min(limits.max_collection) {
            return Err(Error::Limit("signed command arguments"));
        }
        let mut names = BTreeSet::new();
        for argument in &self.arguments {
            if argument.name.is_empty() || !names.insert(&argument.name) {
                return Err(Error::Invalid("signed command argument name"));
            }
        }
        let ack = acknowledgement(self.last_seen, version)?;
        let mut size = packet_size(
            &[text_size(&self.command, 32767, limits)?, 17, ack.len()],
            limits,
        )?;
        for argument in &self.arguments {
            size = packet_size(&[size, text_size(&argument.name, 16, limits)?, 256], limits)?;
        }
        let mut w = Writer::with_capacity(size);
        w.string(&self.command, 32767.min(limits.max_string_chars))?;
        w.i64(self.timestamp_ms);
        w.i64(self.salt);
        w.var_i32(self.arguments.len() as i32);
        for argument in &self.arguments {
            w.string(&argument.name, 16.min(limits.max_string_chars))?;
            w.raw(argument.signature.as_ref());
            if w.as_slice().len() > limits.max_packet {
                return Err(Error::Limit("signed command packet bytes"));
            }
        }
        w.raw(&ack);
        bounded(
            w,
            version,
            if version.protocol() < 766 {
                "chat_command"
            } else {
                "chat_command_signed"
            },
            limits,
        )
    }
}
/// Advertise an application-owned, legitimately issued chat-session certificate.
/// This only encodes the public envelope. It does not fetch/validate a certificate,
/// authenticate an account, check expiry, or prove possession of the private key.
pub fn session_update(
    version: Version,
    session: &ChatSession,
    limits: Limits,
) -> Result<RawPacket> {
    if session.public_key.is_empty()
        || session.public_key.len() > 512
        || session.key_signature.len() > 4096
    {
        return Err(Error::Invalid("chat session key envelope size"));
    }
    let size = 24
        + session.public_key.len()
        + session.key_signature.len()
        + varint_size(session.public_key.len())
        + varint_size(session.key_signature.len());
    if size > limits.max_packet {
        return Err(Error::Limit("chat session envelope bytes"));
    }
    let mut w = Writer::with_capacity(size);
    w.raw(&session.session_id);
    w.i64(session.expires_at);
    w.bytes(&session.public_key)?;
    w.bytes(&session.key_signature)?;
    bounded(w, version, "chat_session_update", limits)
}
