//! Bounded clientbound map-data wire envelopes for protocols 763–776.
//!
//! Labels are JSON through 764 and anonymous NBT from 765. Decoration kinds are
//! unresolved numeric wire IDs; no release-specific registry table is guessed.
//! Scale, rotation, coordinates and color bytes retain their full wire domains.
//! A patch is not applied to a canvas: dimensions, offsets and color-array length
//! are preserved without asserting that they describe a valid 128×128 update.
//!
//! Layout evidence is the hash-pinned protocol schemas, not a release-API or
//! live-server oracle. See `docs/map-statistics-wire-audit.md` for the boundary.
use super::{
    chat::ChatComponent,
    inventory::{self, Budget},
};
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    nbt::RootFormat,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

#[derive(Clone, Debug, PartialEq)]
pub struct MapDecoration {
    /// Unresolved signed VarInt. Interpretation depends on the selected release.
    pub kind_id: i32,
    pub x: i8,
    pub z: i8,
    /// Full wire byte, without a renderer-specific rotation mask.
    pub rotation: u8,
    pub label: Option<ChatComponent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapPatch {
    /// Must be nonzero: zero is the wire discriminant for an absent patch.
    pub columns: u8,
    pub rows: u8,
    pub x: u8,
    pub z: u8,
    /// Palette indices, not expanded RGB values. No palette is assumed.
    pub colors: Vec<u8>,
}

impl MapPatch {
    /// Checks whether this patch can be safely applied to a 128×128 map canvas.
    ///
    /// Wire decoding intentionally does not impose these gameplay constraints.
    /// This method checks nonzero dimensions, canvas bounds and exact color count;
    /// it does not validate palette indices or apply the update.
    pub fn validate_canvas(&self) -> Result<()> {
        if self.columns == 0 || self.rows == 0 {
            return Err(Error::Invalid("empty map patch dimensions"));
        }
        if usize::from(self.x) + usize::from(self.columns) > 128
            || usize::from(self.z) + usize::from(self.rows) > 128
        {
            return Err(Error::Invalid("map patch outside canvas"));
        }
        if self.colors.len() != usize::from(self.columns) * usize::from(self.rows) {
            return Err(Error::Invalid("map patch color count"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapData {
    pub map_id: i32,
    pub scale: i8,
    pub locked: bool,
    /// None leaves decorations unchanged; Some(empty) clears the decorations.
    pub decorations: Option<Vec<MapDecoration>>,
    /// None leaves pixels unchanged.
    pub patch: Option<MapPatch>,
}

fn read_label(r: &mut Reader<'_>, version: Version, budget: &mut Budget) -> Result<ChatComponent> {
    if version.protocol() < 765 {
        Ok(ChatComponent::Json(r.string(262_144)?.to_owned()))
    } else {
        inventory::read_nbt(r, RootFormat::Anonymous, budget)?
            .map(ChatComponent::Nbt)
            .ok_or(Error::Invalid("absent map label NBT"))
    }
}
fn available(w: &Writer, additional: usize, limits: Limits) -> Result<()> {
    if additional > limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("map packet bytes"));
    }
    Ok(())
}
fn write_label(
    label: &ChatComponent,
    w: &mut Writer,
    version: Version,
    budget: &mut Budget,
) -> Result<()> {
    match (label, version.protocol() < 765) {
        (ChatComponent::Json(json), true) => {
            let mut prefix = 1;
            let mut len = json.len();
            while len > 127 {
                prefix += 1;
                len >>= 7;
            }
            available(w, json.len().saturating_add(prefix), budget.limits)?;
            w.string(json, 262_144.min(budget.limits.max_string_chars))?;
        }
        (ChatComponent::Nbt(nbt), false) => {
            inventory::write_nbt(Some(nbt), w, RootFormat::Anonymous, budget)?;
        }
        _ => return Err(Error::Invalid("map label representation for version")),
    }
    budget.check_bytes(w)
}

impl MapData {
    fn read_body(r: &mut Reader<'_>, version: Version, budget: &mut Budget) -> Result<Self> {
        let map_id = r.var_i32()?;
        let scale = r.u8()? as i8;
        let locked = r.bool()?;
        let decorations = if r.bool()? {
            let count = budget.count(r)?;
            // Each entry needs at least a one-byte ID, x, z, rotation and flag.
            // Reject impossible lengths before reserving the collection.
            if count > r.remaining().len() / 5 {
                return Err(Error::Eof);
            }
            let mut decorations = Vec::with_capacity(count);
            for _ in 0..count {
                decorations.push(MapDecoration {
                    kind_id: r.var_i32()?,
                    x: r.u8()? as i8,
                    z: r.u8()? as i8,
                    rotation: r.u8()?,
                    label: if r.bool()? {
                        Some(read_label(r, version, budget)?)
                    } else {
                        None
                    },
                });
            }
            Some(decorations)
        } else {
            None
        };
        let columns = r.u8()?;
        let patch = if columns == 0 {
            None
        } else {
            let rows = r.u8()?;
            let x = r.u8()?;
            let z = r.u8()?;
            let len = budget.count(r)?;
            Some(MapPatch {
                columns,
                rows,
                x,
                z,
                colors: r.take(len)?.to_vec(),
            })
        };
        Ok(Self {
            map_id,
            scale,
            locked,
            decorations,
            patch,
        })
    }
    /// Reads one bounded body; the reader is unchanged on failure.
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
            return Err(Error::Limit("map packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let value = Self::read(&mut r, version)?;
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        let mut budget = Budget::new(limits);
        w.var_i32(self.map_id);
        w.u8(self.scale as u8);
        w.bool(self.locked);
        w.bool(self.decorations.is_some());
        budget.check_bytes(&w)?;
        if let Some(decorations) = &self.decorations {
            budget.write_count(decorations.len(), &mut w)?;
            for decoration in decorations {
                w.var_i32(decoration.kind_id);
                w.u8(decoration.x as u8);
                w.u8(decoration.z as u8);
                w.u8(decoration.rotation);
                w.bool(decoration.label.is_some());
                budget.check_bytes(&w)?;
                if let Some(label) = &decoration.label {
                    write_label(label, &mut w, version, &mut budget)?;
                }
            }
        }
        if let Some(patch) = &self.patch {
            if patch.columns == 0 {
                return Err(Error::Invalid("zero-width map patch"));
            }
            w.u8(patch.columns);
            w.u8(patch.rows);
            w.u8(patch.x);
            w.u8(patch.z);
            budget.write_count(patch.colors.len(), &mut w)?;
            available(&w, patch.colors.len(), limits)?;
            w.raw(&patch.colors);
        } else {
            w.u8(0);
        }
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
            version.packet_id(State::Play, Direction::Clientbound, "map")?,
            self.encode(version, limits)?,
        ))
    }
}
