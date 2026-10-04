//! Bounded plugin/mod channel envelopes, with optional zero-copy decoding.
//!
//! A resource identifier is followed by the rest of the packet body, without an
//! inner length prefix. Channel-specific bytes remain opaque; decoding does not
//! register channels, execute handlers or send a response.
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

/// Owned custom-payload envelope for typed connection events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomPayload {
    pub channel: String,
    pub data: Vec<u8>,
}
/// A zero-copy view of a complete, already delimited packet body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CustomPayloadRef<'a> {
    pub channel: &'a str,
    pub data: &'a [u8],
}

fn supported(version: Version, state: State) -> Result<()> {
    if !matches!(state, State::Configuration | State::Play) {
        return Err(Error::State("custom payload state"));
    }
    if state == State::Configuration && !version.has_configuration() {
        return Err(Error::Unsupported(
            "configuration custom payload before 1.20.2",
        ));
    }
    Ok(())
}
fn maximum(direction: Direction) -> usize {
    match direction {
        Direction::Clientbound => CustomPayload::MAX_CLIENTBOUND_BYTES,
        Direction::Serverbound => CustomPayload::MAX_SERVERBOUND_BYTES,
    }
}
fn prefix_len(mut len: usize) -> usize {
    let mut n = 1;
    while len > 127 {
        n += 1;
        len >>= 7;
    }
    n
}
impl<'a> CustomPayloadRef<'a> {
    /// Decode one whole packet body without allocating or interpreting its data.
    /// The payload extends to the body boundary; shorter payloads are valid and
    /// cannot be diagnosed as truncated without a channel-specific codec.
    pub fn decode(
        bytes: &'a [u8],
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<Self> {
        supported(version, state)?;
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("custom payload packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let channel = r.string(32767)?;
        identifier::validate(channel, version)?;
        let data = r.remaining();
        if data.len() > maximum(direction) {
            return Err(Error::Limit("custom payload data bytes"));
        }
        Ok(Self { channel, data })
    }
    /// Validate the complete budget before reserving one output buffer.
    pub fn encode(
        &self,
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<Vec<u8>> {
        supported(version, state)?;
        identifier::validate(self.channel, version)?;
        if self.channel.len() > 32767.min(limits.max_string_chars) {
            // Identifier validation guarantees ASCII, so bytes equal UTF-16 units.
            return Err(Error::Limit("custom payload channel length"));
        }
        if self.data.len() > maximum(direction) {
            return Err(Error::Limit("custom payload data bytes"));
        }
        let size = prefix_len(self.channel.len())
            .checked_add(self.channel.len())
            .and_then(|n| n.checked_add(self.data.len()))
            .ok_or(Error::Limit("custom payload packet bytes"))?;
        if size > limits.max_packet {
            return Err(Error::Limit("custom payload packet bytes"));
        }
        let mut w = Writer::with_capacity(size);
        w.string(self.channel, 32767.min(limits.max_string_chars))?;
        w.raw(self.data);
        Ok(w.into_inner())
    }
    /// Append a complete body; the writer is unchanged if validation fails.
    pub fn write(
        &self,
        w: &mut Writer,
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<()> {
        w.raw(&self.encode(version, state, direction, limits)?);
        Ok(())
    }
    pub fn packet(
        &self,
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(state, direction, "custom_payload")?,
            self.encode(version, state, direction, limits)?,
        ))
    }
    pub fn to_owned(self) -> CustomPayload {
        CustomPayload {
            channel: self.channel.into(),
            data: self.data.into(),
        }
    }
}
impl CustomPayload {
    /// Conservative envelope cap, matching the vanilla unknown-channel fallback.
    pub const MAX_CLIENTBOUND_BYTES: usize = 1_048_576;
    pub const MAX_SERVERBOUND_BYTES: usize = 32_767;
    pub fn as_ref(&self) -> CustomPayloadRef<'_> {
        CustomPayloadRef {
            channel: &self.channel,
            data: &self.data,
        }
    }
    pub fn decode(
        bytes: &[u8],
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<Self> {
        Ok(CustomPayloadRef::decode(bytes, version, state, direction, limits)?.to_owned())
    }
    pub fn encode(
        &self,
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<Vec<u8>> {
        self.as_ref().encode(version, state, direction, limits)
    }
    pub fn write(
        &self,
        w: &mut Writer,
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<()> {
        self.as_ref().write(w, version, state, direction, limits)
    }
    pub fn packet(
        &self,
        version: Version,
        state: State,
        direction: Direction,
        limits: Limits,
    ) -> Result<RawPacket> {
        self.as_ref().packet(version, state, direction, limits)
    }
}
