//! State-aware dialog and custom-click wire envelopes for protocols 771–776.
//!
//! Dialog NBT remains opaque: these codecs never render it, interpret actions,
//! navigate URLs or execute clicks. Configuration dialogs carry direct anonymous
//! NBT; play dialogs carry registry holders. Custom-click payloads are separately
//! length-framed anonymous NBT, with TAG_End for absence, not a boolean option.
//! See `docs/dialog-wire-audit.md` for schema corrections and evidence limits.
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    nbt::{Nbt, RootFormat},
    packet::entity_metadata::holders::RegistryHolder,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

fn supported(version: Version, state: State) -> Result<()> {
    if !matches!(state, State::Configuration | State::Play) {
        return Err(Error::State("dialog packet state"));
    }
    if version.protocol() < 771 {
        return Err(Error::Unsupported("dialog packets before 1.21.6"));
    }
    Ok(())
}
fn available(w: &Writer, additional: usize, limits: Limits) -> Result<()> {
    if additional > limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("dialog packet bytes"));
    }
    Ok(())
}
fn prefix_len(mut value: usize) -> usize {
    let mut len = 1;
    while value > 127 {
        value >>= 7;
        len += 1;
    }
    len
}
fn required_nbt(r: &mut Reader<'_>) -> Result<Nbt> {
    Nbt::read(r, RootFormat::Anonymous)?.ok_or(Error::Invalid("absent dialog NBT"))
}
fn write_nbt(nbt: &Nbt, w: &mut Writer, limits: Limits) -> Result<()> {
    nbt.write(
        w,
        RootFormat::Anonymous,
        Limits {
            max_packet: limits.max_packet.saturating_sub(w.as_slice().len()),
            ..limits
        },
    )
}

macro_rules! packet_codec {
    ($ty:ty, $name:literal, $direction:ident) => {
        impl $ty {
            /// Reads one state-specific bounded body, without advancing on failure.
            pub fn read(r: &mut Reader<'_>, version: Version, state: State) -> Result<Self> {
                supported(version, state)?;
                let limits = r.limits;
                let input = &r.remaining()[..r.remaining().len().min(limits.max_packet)];
                let mut bounded = Reader::new(input, limits);
                let value = Self::read_body(&mut bounded, version, state)?;
                r.take(bounded.position())?;
                Ok(value)
            }
            pub fn decode(
                bytes: &[u8],
                version: Version,
                state: State,
                limits: Limits,
            ) -> Result<Self> {
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("dialog packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version, state)?;
                r.finish()?;
                Ok(value)
            }
            pub fn encode(
                &self,
                version: Version,
                state: State,
                limits: Limits,
            ) -> Result<Vec<u8>> {
                supported(version, state)?;
                let mut w = Writer::new();
                self.write_body(&mut w, version, state, limits)?;
                if w.as_slice().len() > limits.max_packet {
                    return Err(Error::Limit("dialog packet bytes"));
                }
                Ok(w.into_inner())
            }
            /// Appends one complete body; failure leaves the destination unchanged.
            pub fn write(
                &self,
                w: &mut Writer,
                version: Version,
                state: State,
                limits: Limits,
            ) -> Result<()> {
                w.raw(&self.encode(version, state, limits)?);
                Ok(())
            }
            /// Constructs a packet only. It does not send it or perform an action.
            pub fn packet(
                &self,
                version: Version,
                state: State,
                limits: Limits,
            ) -> Result<RawPacket> {
                supported(version, state)?;
                Ok(RawPacket::new(
                    version.packet_id(state, Direction::$direction, $name)?,
                    self.encode(version, state, limits)?,
                ))
            }
        }
    };
}

/// Empty clientbound dialog-clear notification; no UI state is modified here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClearDialog;
impl ClearDialog {
    fn read_body(_r: &mut Reader<'_>, _version: Version, _state: State) -> Result<Self> {
        Ok(Self)
    }
    fn write_body(
        &self,
        _w: &mut Writer,
        _version: Version,
        _state: State,
        _limits: Limits,
    ) -> Result<()> {
        Ok(())
    }
}
packet_codec!(ClearDialog, "clear_dialog", Clientbound);

/// Opaque dialog representation. Configuration requires Inline and transmits it
/// directly; play transmits zero + inline NBT or a positive registry-ID-plus-one.
/// Registry references remain unresolved, and NBT dialog semantics are not checked.
#[derive(Clone, Debug, PartialEq)]
pub struct ShowDialog {
    pub dialog: RegistryHolder<Nbt>,
}
impl ShowDialog {
    fn read_body(r: &mut Reader<'_>, _version: Version, state: State) -> Result<Self> {
        let dialog = if state == State::Configuration {
            RegistryHolder::Inline(required_nbt(r)?)
        } else {
            match r.var_i32()? {
                0 => RegistryHolder::Inline(required_nbt(r)?),
                marker @ 1.. => RegistryHolder::RegistryId(marker - 1),
                _ => return Err(Error::Invalid("negative dialog holder marker")),
            }
        };
        Ok(Self { dialog })
    }
    fn write_body(
        &self,
        w: &mut Writer,
        _version: Version,
        state: State,
        limits: Limits,
    ) -> Result<()> {
        match (&self.dialog, state) {
            (RegistryHolder::Inline(nbt), State::Configuration) => write_nbt(nbt, w, limits),
            (RegistryHolder::Inline(nbt), State::Play) => {
                available(w, 1, limits)?;
                w.u8(0);
                write_nbt(nbt, w, limits)
            }
            (RegistryHolder::RegistryId(id), State::Play) => {
                let marker = id
                    .checked_add(1)
                    .filter(|n| *id >= 0 && *n > 0)
                    .ok_or(Error::Invalid("dialog holder registry ID"))?;
                available(w, prefix_len(marker as usize), limits)?;
                w.var_i32(marker);
                Ok(())
            }
            _ => Err(Error::Invalid("configuration dialog requires inline NBT")),
        }
    }
}
packet_codec!(ShowDialog, "show_dialog", Clientbound);

/// Explicit serverbound custom action envelope, not an action executor.
/// The optional payload uses a TAG_End sentinel inside its length-framed buffer.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomClickAction {
    pub id: String,
    pub payload: Option<Nbt>,
}
impl CustomClickAction {
    /// Maximum length of the NBT sub-buffer, including its root/sentinel byte.
    pub const MAX_PAYLOAD_BYTES: usize = 65_536;
    /// Conservative structural depth cap in Rustwire's root-at-zero NBT model.
    /// This does not reproduce a receiving JVM's allocation-accounting quota.
    pub const MAX_PAYLOAD_DEPTH: usize = 15;

    fn read_body(r: &mut Reader<'_>, version: Version, _state: State) -> Result<Self> {
        let id = r.string(32767)?.to_owned();
        identifier::validate(&id, version)?;
        let length = r.count(Self::MAX_PAYLOAD_BYTES.min(r.limits.max_packet))?;
        let limits = Limits {
            max_packet: length,
            max_nbt_depth: r.limits.max_nbt_depth.min(Self::MAX_PAYLOAD_DEPTH),
            ..r.limits
        };
        let mut payload_reader = Reader::new(r.take(length)?, limits);
        let payload = Nbt::read(&mut payload_reader, RootFormat::Anonymous)?;
        payload_reader.finish()?;
        Ok(Self { id, payload })
    }
    fn write_body(
        &self,
        w: &mut Writer,
        version: Version,
        _state: State,
        limits: Limits,
    ) -> Result<()> {
        identifier::validate(&self.id, version)?;
        available(
            w,
            self.id.len().saturating_add(prefix_len(self.id.len())),
            limits,
        )?;
        w.string(&self.id, 32767.min(limits.max_string_chars))?;
        let payload_limits = Limits {
            max_packet: Self::MAX_PAYLOAD_BYTES
                .min(limits.max_packet.saturating_sub(w.as_slice().len())),
            max_nbt_depth: limits.max_nbt_depth.min(Self::MAX_PAYLOAD_DEPTH),
            ..limits
        };
        let mut payload = Writer::new();
        if let Some(nbt) = &self.payload {
            nbt.write(&mut payload, RootFormat::Anonymous, payload_limits)?;
        } else {
            available(&payload, 1, payload_limits)?;
            payload.u8(0);
        }
        let len = payload.as_slice().len();
        available(w, prefix_len(len).saturating_add(len), limits)?;
        w.var_i32(len as i32);
        w.raw(payload.as_slice());
        Ok(())
    }
}
packet_codec!(CustomClickAction, "custom_click_action", Serverbound);

#[derive(Clone, Debug, PartialEq)]
pub enum DialogPacket {
    Clear(ClearDialog),
    Show(ShowDialog),
}
impl DialogPacket {
    /// Clientbound-only dispatch with an explicit configuration/play state.
    /// Unknown names remain unhandled; custom_click_action is serverbound-only.
    pub fn decode(
        state: State,
        name: &str,
        bytes: &[u8],
        version: Version,
        limits: Limits,
    ) -> Result<Option<Self>> {
        Ok(Some(match name {
            "clear_dialog" => Self::Clear(ClearDialog::decode(bytes, version, state, limits)?),
            "show_dialog" => Self::Show(ShowDialog::decode(bytes, version, state, limits)?),
            _ => return Ok(None),
        }))
    }
}
