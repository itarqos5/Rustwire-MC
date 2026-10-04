//! Version-aware chat envelopes. These codecs do not authenticate messages, verify
//! signatures or implement the secure-chat signing chain. The `state` module
//! separately provides bounded acknowledgement and packed-signature tracking.
//! Applications must make trust and server-policy decisions themselves.
pub mod signed;
pub mod state;

use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    nbt::{Nbt, RootFormat},
    version::State,
    Error, Limits, Result, Version,
};

/// Lossless wire representation, not a rendered or trusted chat message.
/// JSON is retained verbatim; its syntax and component semantics are not validated.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatComponent {
    Json(String),
    Nbt(Nbt),
}
impl ChatComponent {
    /// Reads a play/configuration component (login disconnects always use JSON).
    pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        if version.protocol() < 765 {
            Self::read_json(r)
        } else {
            Ok(Self::Nbt(read_nbt(r)?))
        }
    }
    fn read_json(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self::Json(r.string(262_144)?.to_owned()))
    }
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = packet_reader(bytes, limits)?;
        let component = Self::read(&mut r, version)?;
        r.finish()?;
        Ok(component)
    }
    /// Does not translate JSON to NBT or vice versa. A mismatched variant is an error.
    pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        match (self, version.protocol() >= 765) {
            (Self::Json(json), false) => w.string(json, 262_144.min(limits.max_string_chars)),
            (Self::Nbt(nbt), true) => nbt.write(w, RootFormat::Anonymous, limits),
            _ => Err(Error::Invalid("chat component representation for version")),
        }
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        self.write(&mut w, version, limits)?;
        if w.as_slice().len() > limits.max_packet {
            return Err(Error::Limit("chat component bytes"));
        }
        Ok(w.into_inner())
    }
}
fn packet_reader(bytes: &[u8], limits: Limits) -> Result<Reader<'_>> {
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("chat packet bytes"));
    }
    Ok(Reader::new(bytes, limits))
}
fn nonnegative(r: &mut Reader<'_>) -> Result<u32> {
    let n = r.var_i32()?;
    if n < 0 {
        Err(Error::Invalid("negative chat index"))
    } else {
        Ok(n as u32)
    }
}
fn read_nbt(r: &mut Reader<'_>) -> Result<Nbt> {
    Nbt::read(r, RootFormat::Anonymous)?.ok_or(Error::Invalid("absent chat NBT"))
}
fn optional_component(r: &mut Reader<'_>, version: Version) -> Result<Option<ChatComponent>> {
    if r.bool()? {
        Ok(Some(ChatComponent::read(r, version)?))
    } else {
        Ok(None)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemChat {
    pub content: ChatComponent,
    /// True displays the message above the hotbar rather than in chat.
    pub overlay: bool,
}
impl SystemChat {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = packet_reader(bytes, limits)?;
        let value = Self {
            content: ChatComponent::read(&mut r, version)?,
            overlay: r.bool()?,
        };
        r.finish()?;
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatParameter {
    Content,
    Sender,
    Target,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChatDecoration {
    pub translation_key: String,
    pub parameters: Vec<ChatParameter>,
    /// The inline holder's style is an anonymous NBT value, not a whole component.
    pub style: Nbt,
}
impl ChatDecoration {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let translation_key = r.string(32767)?.to_owned();
        let count = r.count(r.limits.max_collection)?;
        let mut parameters = Vec::with_capacity(count.min(r.remaining().len()));
        for _ in 0..count {
            parameters.push(match r.var_i32()? {
                0 => ChatParameter::Content,
                1 => ChatParameter::Sender,
                2 => ChatParameter::Target,
                _ => return Err(Error::Invalid("chat decoration parameter")),
            });
        }
        Ok(Self {
            translation_key,
            parameters,
            style: read_nbt(r)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum ChatType {
    /// Zero-based index in the chat-type registry.
    Registry(u32),
    /// Added in protocol 767. Wire holder zero introduces inline data.
    Inline {
        chat: Box<ChatDecoration>,
        narration: Box<ChatDecoration>,
    },
}
impl ChatType {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let id = nonnegative(r)?;
        if version.protocol() < 767 {
            Ok(Self::Registry(id))
        } else if id == 0 {
            Ok(Self::Inline {
                chat: Box::new(ChatDecoration::read(r)?),
                narration: Box::new(ChatDecoration::read(r)?),
            })
        } else {
            Ok(Self::Registry(id - 1))
        }
    }
}
/// The `profileless_chat` packet, also known as disguised chat. No signature.
#[derive(Clone, Debug, PartialEq)]
pub struct DisguisedChat {
    pub message: ChatComponent,
    pub chat_type: ChatType,
    pub name: ChatComponent,
    pub target: Option<ChatComponent>,
}
impl DisguisedChat {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = packet_reader(bytes, limits)?;
        let value = Self {
            message: ChatComponent::read(&mut r, version)?,
            chat_type: ChatType::read(&mut r, version)?,
            name: ChatComponent::read(&mut r, version)?,
            target: optional_component(&mut r, version)?,
        };
        r.finish()?;
        Ok(value)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviousMessage {
    Signature(Box<[u8; 256]>),
    /// Zero-based entry in the application's signature cache, not a message index.
    Cached(u32),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatFilter {
    PassThrough,
    FullyFiltered,
    /// Bitset words, retaining the transmitted mask without applying it to text.
    PartiallyFiltered(Vec<u64>),
}
/// A parsed envelope only. A present signature has NOT been verified. Cached
/// signatures must be resolved by the application before cryptographic validation.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerChat {
    /// Present from protocol 770 (1.21.5).
    pub global_index: Option<u32>,
    pub sender: [u8; 16],
    pub index: u32,
    pub signature: Option<Box<[u8; 256]>>,
    pub message: String,
    pub timestamp: i64,
    pub salt: i64,
    pub previous_messages: Vec<PreviousMessage>,
    pub unsigned_content: Option<ChatComponent>,
    pub filter: ChatFilter,
    pub chat_type: ChatType,
    pub name: ChatComponent,
    pub target: Option<ChatComponent>,
}
impl PlayerChat {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = packet_reader(bytes, limits)?;
        let global_index = if version.protocol() >= 770 {
            Some(nonnegative(&mut r)?)
        } else {
            None
        };
        let sender = r.uuid()?;
        let index = nonnegative(&mut r)?;
        let signature = if r.bool()? {
            Some(Box::new(r.take(256)?.try_into().unwrap()))
        } else {
            None
        };
        let message = r.string(256)?.to_owned();
        let timestamp = r.i64()?;
        let salt = r.i64()?;
        let count = r.count(20.min(limits.max_collection))?;
        let mut previous_messages = Vec::with_capacity(count);
        for _ in 0..count {
            previous_messages.push(match nonnegative(&mut r)? {
                0 => PreviousMessage::Signature(Box::new(r.take(256)?.try_into().unwrap())),
                n => PreviousMessage::Cached(n - 1),
            });
        }
        let unsigned_content = optional_component(&mut r, version)?;
        let filter = match r.var_i32()? {
            0 => ChatFilter::PassThrough,
            1 => ChatFilter::FullyFiltered,
            2 => {
                let count = r.count(limits.max_collection)?;
                if count > r.remaining().len() / 8 {
                    return Err(Error::Eof);
                }
                let mut mask = Vec::with_capacity(count);
                for _ in 0..count {
                    mask.push(r.i64()? as u64);
                }
                ChatFilter::PartiallyFiltered(mask)
            }
            _ => return Err(Error::Invalid("chat filter type")),
        };
        let chat_type = ChatType::read(&mut r, version)?;
        let name = ChatComponent::read(&mut r, version)?;
        let target = optional_component(&mut r, version)?;
        r.finish()?;
        Ok(Self {
            global_index,
            sender,
            index,
            signature,
            message,
            timestamp,
            salt,
            previous_messages,
            unsigned_content,
            filter,
            chat_type,
            name,
            target,
        })
    }
}

/// Login disconnects use JSON on every version; play/configuration follow the
/// component format switch at 765. Status and handshake have no such packet.
#[derive(Clone, Debug, PartialEq)]
pub struct Disconnect {
    pub reason: ChatComponent,
}
impl Disconnect {
    pub fn decode(bytes: &[u8], version: Version, state: State, limits: Limits) -> Result<Self> {
        let mut r = packet_reader(bytes, limits)?;
        let reason = match state {
            State::Login => ChatComponent::read_json(&mut r)?,
            State::Play => ChatComponent::read(&mut r, version)?,
            State::Configuration if version.has_configuration() => {
                ChatComponent::read(&mut r, version)?
            }
            _ => return Err(Error::State("disconnect packet state")),
        };
        r.finish()?;
        Ok(Self { reason })
    }
}

/// Acknowledgement payload, either caller-maintained or generated by
/// [`state::LastSeenTracker`] after explicit display/trust decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastSeenUpdate {
    pub offset: u32,
    /// Twenty acknowledgement bits, little-endian within each byte. High four
    /// bits of byte 2 are reserved and must be zero.
    pub acknowledged: [u8; 3],
    /// Required from protocol 770 and forbidden on older versions.
    pub checksum: Option<u8>,
}
impl LastSeenUpdate {
    fn write(self, w: &mut Writer, version: Version) -> Result<()> {
        if self.offset > i32::MAX as u32 || self.acknowledged[2] & 0xf0 != 0 {
            return Err(Error::Invalid("chat acknowledgement state"));
        }
        if self.checksum.is_some() != (version.protocol() >= 770) {
            return Err(Error::Invalid("chat acknowledgement checksum for version"));
        }
        w.var_i32(self.offset as i32);
        w.raw(&self.acknowledged);
        if let Some(checksum) = self.checksum {
            w.u8(checksum);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsignedChatPolicy {
    /// The caller has established that this server accepts unsigned messages.
    AllowedByServer,
    SigningRequired,
}
/// Encodes an actually unsigned message, with an absent signature. This cannot
/// send on servers requiring signed chat and never manufactures a signature or
/// acknowledgement history. Timestamp, salt and last-seen state belong to the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsignedChatMessage {
    pub message: String,
    pub timestamp: i64,
    pub salt: i64,
    pub last_seen: LastSeenUpdate,
}
impl UnsignedChatMessage {
    pub fn encode(&self, version: Version, policy: UnsignedChatPolicy) -> Result<RawPacket> {
        if policy != UnsignedChatPolicy::AllowedByServer {
            return Err(Error::Unsupported("secure-chat signing is not implemented"));
        }
        let mut w = Writer::new();
        w.string(&self.message, 256)?;
        w.i64(self.timestamp);
        w.i64(self.salt);
        w.bool(false);
        self.last_seen.write(&mut w, version)?;
        super::named(version, State::Play, "chat_message", w.into_inner())
    }
}
/// Sends the dedicated unsigned-command packet introduced in 1.20.5. `command`
/// excludes the leading slash. Commands requiring signed arguments are outside
/// this API; the server may reject them. Use [`signed::SignedChatCommand`] for
/// signed-command envelopes, including legacy protocols 763–765. Signing and
/// certificate trust remain application responsibilities.
pub fn unsigned_command(version: Version, command: &str) -> Result<RawPacket> {
    if version.protocol() < 766 {
        return Err(Error::Unsupported(
            "dedicated unsigned command before 1.20.5",
        ));
    }
    if command.is_empty() || command.starts_with('/') {
        return Err(Error::Invalid(
            "command must be nonempty and omit leading slash",
        ));
    }
    let mut w = Writer::new();
    w.string(command, 32767)?;
    super::named(version, State::Play, "chat_command", w.into_inner())
}
/// Acknowledges a caller-maintained count; does not itself advance a chat cache.
pub fn message_acknowledgement(version: Version, count: u32) -> Result<RawPacket> {
    if count > i32::MAX as u32 {
        return Err(Error::Invalid("chat acknowledgement count"));
    }
    let mut w = Writer::new();
    w.var_i32(count as i32);
    super::named(
        version,
        State::Play,
        "message_acknowledgement",
        w.into_inner(),
    )
}
