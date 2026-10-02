//! Streamed chunk lighting, block entities, biome replacements and view controls.
//!
//! Packet and stream codecs were checked in all fourteen cached release
//! artifacts, alongside the pinned `research/protocols` schemas. All six packet
//! names exist in protocols 763–776. Light packets have no trust-edges flag.
//! Biome buffers contain one semantic 64-entry palette per dimension section;
//! there are no block/fluid counts. Their long-array lengths disappear at 770.
//! Unlike the stale optional-NBT schema, block-entity NBT must be a non-null
//! compound from 766 onward. Earlier releases permit the TAG_End sentinel.
//!
//! Registry IDs remain unresolved numbers. Callers must supply the active
//! dimension's section count (for example from `RegistryStore`); neither packet
//! contains enough information to infer dimension height or minimum Y. Lighting
//! includes two extra boundary sections. No world is mutated by these codecs.
use crate::{
    chunk::{charge_elements, varint_size, ContainerKind, LightData, Palette, PaletteContainer},
    codec::{BlockPosition, Reader, Writer},
    frame::RawPacket,
    nbt::{Nbt, RootFormat, Tag},
    version::{Direction, State},
    Error, Limits, Result, Version,
};

fn reader(bytes: &[u8], limits: Limits) -> Result<Reader<'_>> {
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("chunk-update packet bytes"));
    }
    Ok(Reader::new(bytes, limits))
}
fn finish(w: Writer, limits: Limits) -> Result<Vec<u8>> {
    if w.as_slice().len() > limits.max_packet {
        return Err(Error::Limit("chunk-update packet bytes"));
    }
    Ok(w.into_inner())
}
fn sections(count: usize, limits: Limits) -> Result<()> {
    if count == 0 {
        return Err(Error::Invalid("empty dimension"));
    }
    if count > limits.max_collection || count > i32::MAX as usize {
        return Err(Error::Limit("dimension section count"));
    }
    Ok(())
}
fn packet(version: Version, name: &str, data: Vec<u8>) -> Result<RawPacket> {
    Ok(RawPacket::new(
        version.packet_id(State::Play, Direction::Clientbound, name)?,
        data,
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateLight {
    pub chunk_x: i32,
    pub chunk_z: i32,
    /// Arrays are 2048 packed bytes, each holding two four-bit light levels;
    /// X is fastest, then Z, then Y. Unset bits in both masks mean unchanged.
    pub light: LightData,
}
impl UpdateLight {
    pub fn decode(
        bytes: &[u8],
        _version: Version,
        section_count: usize,
        limits: Limits,
    ) -> Result<Self> {
        sections(section_count, limits)?;
        let mut r = reader(bytes, limits)?;
        let result = Self {
            chunk_x: r.var_i32()?,
            chunk_z: r.var_i32()?,
            light: LightData::read_for_sections(&mut r, section_count)?,
        };
        r.finish()?;
        Ok(result)
    }
    pub fn encode(
        &self,
        _version: Version,
        section_count: usize,
        limits: Limits,
    ) -> Result<Vec<u8>> {
        sections(section_count, limits)?;
        let mut w = Writer::new();
        w.var_i32(self.chunk_x);
        w.var_i32(self.chunk_z);
        self.light
            .write_for_sections(&mut w, section_count, limits)?;
        finish(w, limits)
    }
    pub fn packet(
        &self,
        version: Version,
        section_count: usize,
        limits: Limits,
    ) -> Result<RawPacket> {
        packet(
            version,
            "update_light",
            self.encode(version, section_count, limits)?,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TileEntityData {
    pub position: BlockPosition,
    /// Negotiated block-entity registry ID, not a block state or action enum.
    pub kind: u32,
    /// A compound root. `None` (TAG_End) is valid only in protocols 763–765.
    pub data: Option<Nbt>,
}
impl TileEntityData {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let position = BlockPosition::unpack(r.i64()?);
        let kind = r.var_i32()?;
        if kind < 0 {
            return Err(Error::Invalid("negative block-entity registry ID"));
        }
        // Reject incompatible root types before parsing their payloads.
        match r.remaining().first() {
            Some(10) => {}
            Some(0) if version.protocol() <= 765 => {}
            Some(_) => return Err(Error::Invalid("block-entity compound NBT")),
            None => return Err(Error::Eof),
        }
        let data = Nbt::read(&mut r, RootFormat::for_version(version))?;
        r.finish()?;
        Ok(Self {
            position,
            kind: kind as u32,
            data,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if self.kind > i32::MAX as u32 {
            return Err(Error::Invalid("block-entity registry ID"));
        }
        match &self.data {
            Some(Nbt {
                root: Tag::Compound(_),
                ..
            }) => {}
            None if version.protocol() <= 765 => {}
            _ => return Err(Error::Invalid("block-entity compound NBT")),
        }
        let mut w = Writer::new();
        w.i64(self.position.pack()?);
        w.var_i32(self.kind as i32);
        if let Some(nbt) = &self.data {
            let nbt_limits = Limits {
                max_packet: limits.max_packet.saturating_sub(w.as_slice().len()),
                ..limits
            };
            nbt.write(&mut w, RootFormat::for_version(version), nbt_limits)?;
        } else {
            w.u8(0);
        }
        finish(w, limits)
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        packet(version, "tile_entity_data", self.encode(version, limits)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkBiomeData {
    pub chunk_x: i32,
    pub chunk_z: i32,
    /// Exactly one biome container for every section, from bottom to top.
    pub sections: Vec<PaletteContainer>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChunkBiomes {
    pub chunks: Vec<ChunkBiomeData>,
}
// The release reader uses this inclusive byte-array bound in all 14 families.
const MAX_BIOME_BYTES: usize = 2_097_152;
impl ChunkBiomes {
    pub fn decode(
        bytes: &[u8],
        version: Version,
        section_count: usize,
        limits: Limits,
    ) -> Result<Self> {
        sections(section_count, limits)?;
        let mut r = reader(bytes, limits)?;
        let count = r.count(limits.max_collection)?;
        let mut remaining = limits.max_collection;
        charge_elements(&mut remaining, count)?;
        charge_elements(
            &mut remaining,
            count
                .checked_mul(section_count)
                .ok_or(Error::Limit("biome section count"))?,
        )?;
        let mut chunks = Vec::new();
        for _ in 0..count {
            // Packed chunk position has X in the low 32 bits, unlike BlockPos.
            let packed = r.i64()? as u64;
            let chunk_x = packed as u32 as i32;
            let chunk_z = (packed >> 32) as u32 as i32;
            let bytes = r.bytes(MAX_BIOME_BYTES.min(limits.max_packet))?;
            let mut palette_reader = Reader::new(bytes, limits);
            let mut sections = Vec::new();
            for _ in 0..section_count {
                sections.push(PaletteContainer::read_bounded(
                    &mut palette_reader,
                    version,
                    ContainerKind::Biomes,
                    &mut remaining,
                )?);
            }
            let padding = palette_reader.remaining();
            // Paper 1.21.5 build 114 allocates the removed array-length prefix
            // for each biome section, leaving this exact zero tail. Other
            // trailing bytes and padding in all other releases are errors.
            let obsolete_prefixes: usize = sections.iter().map(|s| varint_size(s.data.len())).sum();
            if !padding.is_empty()
                && !(version.protocol() == 770
                    && padding.len() == obsolete_prefixes
                    && padding.iter().all(|&b| b == 0))
            {
                return Err(Error::TrailingBytes {
                    context: "chunk biome sections",
                    count: padding.len(),
                });
            }
            chunks.push(ChunkBiomeData {
                chunk_x,
                chunk_z,
                sections,
            });
        }
        r.finish()?;
        Ok(Self { chunks })
    }
    /// All dimensions, palettes, aggregate allocations and encoded byte counts
    /// are checked before constructing a packet body. Accepted Paper 770
    /// padding is not re-emitted; the result uses canonical compact palettes.
    pub fn encode(
        &self,
        version: Version,
        section_count: usize,
        limits: Limits,
    ) -> Result<Vec<u8>> {
        sections(section_count, limits)?;
        if self.chunks.len() > i32::MAX as usize {
            return Err(Error::Limit("biome chunk count"));
        }
        let mut remaining = limits.max_collection;
        charge_elements(&mut remaining, self.chunks.len())?;
        let mut total = varint_size(self.chunks.len());
        for chunk in &self.chunks {
            if chunk.sections.len() != section_count {
                return Err(Error::Invalid("biome dimension section count"));
            }
            charge_elements(&mut remaining, chunk.sections.len())?;
            let mut length = 0usize;
            for section in &chunk.sections {
                if section.kind != ContainerKind::Biomes {
                    return Err(Error::Invalid("biome container kind"));
                }
                if let Palette::Indirect(ids) = &section.palette {
                    charge_elements(&mut remaining, ids.len())?;
                }
                charge_elements(&mut remaining, section.data.len())?;
                length = length
                    .checked_add(section.encoded_len(version, limits)?)
                    .ok_or(Error::Limit("biome section bytes"))?;
            }
            if length > MAX_BIOME_BYTES {
                return Err(Error::Limit("biome section bytes"));
            }
            total = total
                .checked_add(8 + varint_size(length) + length)
                .ok_or(Error::Limit("chunk-update packet bytes"))?;
            if total > limits.max_packet {
                return Err(Error::Limit("chunk-update packet bytes"));
            }
        }
        if total > limits.max_packet {
            return Err(Error::Limit("chunk-update packet bytes"));
        }
        let mut w = Writer::new();
        w.var_i32(self.chunks.len() as i32);
        for chunk in &self.chunks {
            w.i64(
                ((u64::from(chunk.chunk_z as u32) << 32) | u64::from(chunk.chunk_x as u32)) as i64,
            );
            let length = chunk.sections.iter().try_fold(0usize, |n, section| {
                Ok::<_, Error>(n + section.encoded_len(version, limits)?)
            })?;
            w.var_i32(length as i32);
            for section in &chunk.sections {
                section.write(&mut w, version, limits)?;
            }
        }
        finish(w, limits)
    }
    pub fn packet(
        &self,
        version: Version,
        section_count: usize,
        limits: Limits,
    ) -> Result<RawPacket> {
        packet(
            version,
            "chunk_biomes",
            self.encode(version, section_count, limits)?,
        )
    }
}

/// The chunk coordinates at the center of the client's view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateViewPosition {
    pub chunk_x: i32,
    pub chunk_z: i32,
}
impl UpdateViewPosition {
    pub fn decode(bytes: &[u8], _version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let result = Self {
            chunk_x: r.var_i32()?,
            chunk_z: r.var_i32()?,
        };
        r.finish()?;
        Ok(result)
    }
    pub fn encode(&self, _version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        w.var_i32(self.chunk_x);
        w.var_i32(self.chunk_z);
        finish(w, limits)
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        packet(
            version,
            "update_view_position",
            self.encode(version, limits)?,
        )
    }
}
macro_rules! distance {
    ($name:ident, $packet:literal) => {
        /// Exact signed VarInt scalar. Release readers impose no range clamp;
        /// deciding acceptable gameplay/settings ranges belongs to the caller.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name {
            pub distance: i32,
        }
        impl $name {
            pub fn decode(bytes: &[u8], _version: Version, limits: Limits) -> Result<Self> {
                let mut r = reader(bytes, limits)?;
                let value = Self {
                    distance: r.var_i32()?,
                };
                r.finish()?;
                Ok(value)
            }
            pub fn encode(&self, _version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                w.var_i32(self.distance);
                finish(w, limits)
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                packet(version, $packet, self.encode(version, limits)?)
            }
        }
    };
}
distance!(UpdateViewDistance, "update_view_distance");
distance!(SimulationDistance, "simulation_distance");
