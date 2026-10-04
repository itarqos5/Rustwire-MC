//! Bounded server/chat metadata envelopes for protocols 763–776.
//!
//! Components, icon bytes and report details are untrusted wire data. These
//! codecs do not render images, submit reports, change chat trust, apply message
//! deletions or send replies. Application consent and authentication policies
//! are unchanged. See `docs/server-metadata-wire-audit.md` for evidence limits.
use super::chat::{ChatComponent, PreviousMessage};
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

fn available(w: &Writer, additional: usize, limits: Limits) -> Result<()> {
    if additional > limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("server metadata packet bytes"));
    }
    Ok(())
}
fn prefix_len(mut len: usize) -> usize {
    let mut size = 1;
    while len > 127 {
        len >>= 7;
        size += 1;
    }
    size
}
fn string(w: &mut Writer, value: &str, max: usize, limits: Limits) -> Result<()> {
    available(
        w,
        value.len().saturating_add(prefix_len(value.len())),
        limits,
    )?;
    w.string(value, max.min(limits.max_string_chars))
}
fn count(w: &mut Writer, len: usize, max: usize, limits: Limits) -> Result<()> {
    if len > max.min(limits.max_collection).min(i32::MAX as usize) {
        return Err(Error::Limit("server metadata collection"));
    }
    available(w, prefix_len(len), limits)?;
    w.var_i32(len as i32);
    Ok(())
}
fn component(
    value: &ChatComponent,
    w: &mut Writer,
    version: Version,
    limits: Limits,
) -> Result<()> {
    // ChatComponent::write supplies NBT depth/node/string checks. Check the JSON
    // allocation first and give the component only this body's remaining bytes.
    let remaining = limits.max_packet.saturating_sub(w.as_slice().len());
    if let ChatComponent::Json(json) = value {
        available(w, json.len().saturating_add(prefix_len(json.len())), limits)?;
    }
    value.write(
        w,
        version,
        Limits {
            max_packet: remaining,
            ..limits
        },
    )
}

macro_rules! body_codec {
    ($ty:ty, $minimum:literal) => {
        impl $ty {
            fn supported(version: Version) -> Result<()> {
                if version.protocol() < $minimum {
                    return Err(Error::Unsupported(
                        "server metadata packet in selected release",
                    ));
                }
                Ok(())
            }
            /// Reads one bounded body; failure leaves the reader unchanged.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                Self::supported(version)?;
                let limits = r.limits;
                let input = &r.remaining()[..r.remaining().len().min(limits.max_packet)];
                let mut bounded = Reader::new(input, limits);
                let value = Self::read_body(&mut bounded, version)?;
                r.take(bounded.position())?;
                Ok(value)
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("server metadata packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                Self::supported(version)?;
                let mut w = Writer::new();
                self.write_body(&mut w, version, limits)?;
                if w.as_slice().len() > limits.max_packet {
                    return Err(Error::Limit("server metadata packet bytes"));
                }
                Ok(w.into_inner())
            }
            /// Appends one body. The destination is unchanged on failure.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?);
                Ok(())
            }
        }
    };
}
macro_rules! play_packet {
    ($ty:ty, $name:literal, $direction:ident) => {
        impl $ty {
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::$direction, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    };
}

/// In-session server information, distinct from the JSON status response.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerData {
    pub motd: ChatComponent,
    /// Opaque bytes; no PNG validation, decoding or image allocation is done.
    pub icon: Option<Vec<u8>>,
    /// Required through 765, absent from 766. None is not a trust-policy decision.
    pub enforces_secure_chat: Option<bool>,
}
impl ServerData {
    fn read_body(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            motd: ChatComponent::read(r, version)?,
            icon: if r.bool()? {
                Some(r.bytes(r.limits.max_collection)?.to_vec())
            } else {
                None
            },
            enforces_secure_chat: if version.protocol() < 766 {
                Some(r.bool()?)
            } else {
                None
            },
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        if self.enforces_secure_chat.is_some() != (version.protocol() < 766) {
            return Err(Error::Invalid("server data secure-chat field for version"));
        }
        component(&self.motd, w, version, limits)?;
        available(w, 1, limits)?;
        w.bool(self.icon.is_some());
        if let Some(icon) = &self.icon {
            count(w, icon.len(), limits.max_collection, limits)?;
            available(w, icon.len(), limits)?;
            w.raw(icon);
        }
        if let Some(enforces) = self.enforces_secure_chat {
            available(w, 1, limits)?;
            w.bool(enforces);
        }
        Ok(())
    }
}
body_codec!(ServerData, 763);
play_packet!(ServerData, "server_data", Clientbound);

/// One report-detail pair. Order and duplicate keys are retained from the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportDetail {
    pub key: String,
    pub value: String,
}
/// Clientbound custom report metadata in configuration and play from protocol 767.
/// This merely retains data; it does not generate or transmit a report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomReportDetails {
    pub details: Vec<ReportDetail>,
}
impl CustomReportDetails {
    pub const MAX_DETAILS: usize = 32;
    pub const MAX_KEY_CHARS: usize = 128;
    pub const MAX_VALUE_CHARS: usize = 4096;

    fn read_body(r: &mut Reader<'_>, _version: Version) -> Result<Self> {
        let n = r.count(Self::MAX_DETAILS.min(r.limits.max_collection))?;
        // Even two empty strings require two length bytes per entry.
        if n > r.remaining().len() / 2 {
            return Err(Error::Eof);
        }
        let mut details = Vec::with_capacity(n);
        for _ in 0..n {
            details.push(ReportDetail {
                key: r.string(Self::MAX_KEY_CHARS)?.into(),
                value: r.string(Self::MAX_VALUE_CHARS)?.into(),
            });
        }
        Ok(Self { details })
    }
    fn write_body(&self, w: &mut Writer, _version: Version, limits: Limits) -> Result<()> {
        count(w, self.details.len(), Self::MAX_DETAILS, limits)?;
        for entry in &self.details {
            string(w, &entry.key, Self::MAX_KEY_CHARS, limits)?;
            string(w, &entry.value, Self::MAX_VALUE_CHARS, limits)?;
        }
        Ok(())
    }
    pub fn packet(&self, version: Version, state: State, limits: Limits) -> Result<RawPacket> {
        if !matches!(state, State::Configuration | State::Play) {
            return Err(Error::State("custom report details packet"));
        }
        Ok(RawPacket::new(
            version.packet_id(state, Direction::Clientbound, "custom_report_details")?,
            self.encode(version, limits)?,
        ))
    }
}
body_codec!(CustomReportDetails, 767);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ChatSuggestionsAction {
    Add = 0,
    Remove = 1,
    Set = 2,
}
/// Suggestions for chat completion, not command-tree/tab-complete responses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatSuggestions {
    pub action: ChatSuggestionsAction,
    pub entries: Vec<String>,
}
impl ChatSuggestions {
    fn read_body(r: &mut Reader<'_>, _version: Version) -> Result<Self> {
        let action = match r.var_i32()? {
            0 => ChatSuggestionsAction::Add,
            1 => ChatSuggestionsAction::Remove,
            2 => ChatSuggestionsAction::Set,
            _ => return Err(Error::Invalid("chat suggestions action")),
        };
        let n = r.count(r.limits.max_collection)?;
        // Each string needs at least a length byte, before any allocation.
        if n > r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            entries.push(r.string(32767)?.into());
        }
        Ok(Self { action, entries })
    }
    fn write_body(&self, w: &mut Writer, _version: Version, limits: Limits) -> Result<()> {
        available(w, 1, limits)?;
        w.var_i32(self.action as i32);
        count(w, self.entries.len(), limits.max_collection, limits)?;
        for entry in &self.entries {
            string(w, entry, 32767, limits)?;
        }
        Ok(())
    }
}
body_codec!(ChatSuggestions, 763);
play_packet!(ChatSuggestions, "chat_suggestions", Clientbound);

/// A deletion request with an opaque full signature or zero-based cache index.
/// Parsing never mutates the signature cache, display state or acknowledgements.
/// Use `SignatureCache` to resolve the reference and make an application decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HideMessage {
    pub signature: PreviousMessage,
}
impl HideMessage {
    fn read_body(r: &mut Reader<'_>, _version: Version) -> Result<Self> {
        let id = r.var_i32()?;
        let signature = match id {
            0 => PreviousMessage::Signature(Box::new(r.take(256)?.try_into().unwrap())),
            1.. => PreviousMessage::Cached((id - 1) as u32),
            _ => return Err(Error::Invalid("negative packed chat signature index")),
        };
        Ok(Self { signature })
    }
    fn write_body(&self, w: &mut Writer, _version: Version, limits: Limits) -> Result<()> {
        match &self.signature {
            PreviousMessage::Signature(signature) => {
                available(w, 257, limits)?;
                w.u8(0);
                w.raw(signature.as_ref());
            }
            PreviousMessage::Cached(index) => {
                // Leave actual cache presence/range validation to SignatureCache.
                let id = index
                    .checked_add(1)
                    .filter(|n| *n <= i32::MAX as u32)
                    .ok_or(Error::Invalid("packed chat signature index"))?;
                available(w, prefix_len(id as usize), limits)?;
                w.var_i32(id as i32);
            }
        }
        Ok(())
    }
}
body_codec!(HideMessage, 763);
play_packet!(HideMessage, "hide_message", Clientbound);

/// Clientbound play ping response from 764. The identifier's full i64 domain is
/// retained; no timestamp interpretation or latency measurement is imposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingResponse {
    pub id: i64,
}
/// Serverbound play ping request from 764. Distinct from status ping and the
/// configuration/play i32 ping/pong used by connection control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PingRequest {
    pub id: i64,
}
macro_rules! ping_body {
    ($ty:ty) => {
        impl $ty {
            fn read_body(r: &mut Reader<'_>, _version: Version) -> Result<Self> {
                Ok(Self { id: r.i64()? })
            }
            fn write_body(&self, w: &mut Writer, _version: Version, limits: Limits) -> Result<()> {
                available(w, 8, limits)?;
                w.i64(self.id);
                Ok(())
            }
        }
        body_codec!($ty, 764);
    };
}
ping_body!(PingResponse);
ping_body!(PingRequest);
play_packet!(PingResponse, "ping_response", Clientbound);
play_packet!(PingRequest, "ping_request", Serverbound);

/// Configuration-only request to reset chat state from protocol 766.
/// This is a notification envelope: parsing does not clear application chat,
/// signature caches, acknowledgements, or authentication state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResetChat;
impl ResetChat {
    fn read_body(_r: &mut Reader<'_>, _version: Version) -> Result<Self> {
        Ok(Self)
    }
    fn write_body(&self, _w: &mut Writer, _version: Version, _limits: Limits) -> Result<()> {
        Ok(())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(State::Configuration, Direction::Clientbound, "reset_chat")?,
            self.encode(version, limits)?,
        ))
    }
}
body_codec!(ResetChat, 766);
