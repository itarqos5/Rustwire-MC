//! Semantic Java chunk-with-light codec (packet body, excluding packet ID).
//!
//! Numeric block/biome IDs are scoped to the negotiated version/registry. This
//! module does not ship a block-name database or infer missing dimension data.
//! Section count must come from the dimension registry (`height / 16`).
//!
//! Format boundaries: protocol 770 removes paletted-array length prefixes and
//! replaces NBT heightmaps with a typed array; protocol 775 adds fluid counts.
//! Checked against PrismarineJS minecraft-data protocol schemas and
//! prismarine-chunk's `src/pc/{1.18/ChunkColumn,common/PaletteContainer,
//! common/PaletteChunkSection}.js` (retrieved 2026-10-02).
use crate::{
    codec::{Reader, Writer},
    nbt::{Nbt, RootFormat},
    Error, Limits, Result, Version,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    Blocks,
    Biomes,
}
impl ContainerKind {
    pub fn entries(self) -> usize {
        match self {
            Self::Blocks => 4096,
            Self::Biomes => 64,
        }
    }
    fn indirect_min(self) -> u8 {
        match self {
            Self::Blocks => 4,
            Self::Biomes => 1,
        }
    }
    fn indirect_max(self) -> u8 {
        match self {
            Self::Blocks => 8,
            Self::Biomes => 3,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Palette {
    Single(u32),
    Indirect(Vec<u32>),
    Direct,
}
/// Packed no-span storage: values never straddle 64-bit words. Coordinates are
/// X fastest, then Z, then Y; blocks use 16³ entries and biomes use 4³ entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteContainer {
    pub kind: ContainerKind,
    pub bits_per_entry: u8,
    pub palette: Palette,
    pub data: Vec<u64>,
}
impl PaletteContainer {
    pub fn single(kind: ContainerKind, value: u32) -> Self {
        Self {
            kind,
            bits_per_entry: 0,
            palette: Palette::Single(value),
            data: Vec::new(),
        }
    }
    pub fn get(&self, index: usize) -> Option<u32> {
        if index >= self.kind.entries() {
            return None;
        }
        match &self.palette {
            Palette::Single(v) => Some(*v),
            p => {
                let bits = self.bits_per_entry as usize;
                if bits == 0 || bits > 31 {
                    return None;
                }
                let per_word = 64 / bits;
                let word = *self.data.get(index / per_word)?;
                let value = ((word >> ((index % per_word) * bits)) & ((1u64 << bits) - 1)) as u32;
                match p {
                    Palette::Indirect(ids) => ids.get(value as usize).copied(),
                    _ => Some(value),
                }
            }
        }
    }
    pub fn get_xyz(&self, x: usize, y: usize, z: usize) -> Option<u32> {
        let side = match self.kind {
            ContainerKind::Blocks => 16,
            ContainerKind::Biomes => 4,
        };
        if x >= side || y >= side || z >= side {
            return None;
        }
        self.get((y * side + z) * side + x)
    }
    pub fn values(&self) -> impl Iterator<Item = Option<u32>> + '_ {
        (0..self.kind.entries()).map(|i| self.get(i))
    }
    /// `direct_bits` must reflect the complete negotiated registry cardinality,
    /// not just IDs present here. It is only used when an indirect palette overflows.
    pub fn from_values(kind: ContainerKind, values: &[u32], direct_bits: u8) -> Result<Self> {
        if values.len() != kind.entries() {
            return Err(Error::Invalid("palette entry count"));
        }
        if values.iter().any(|&v| v > i32::MAX as u32) {
            return Err(Error::Invalid("negative registry ID"));
        }
        let mut palette = Vec::new();
        let mut indices = Vec::with_capacity(values.len());
        let max_palette = 1usize << kind.indirect_max();
        for &value in values {
            let index = if let Some(n) = palette.iter().position(|&v| v == value) {
                n
            } else {
                palette.push(value);
                palette.len() - 1
            };
            indices.push(index as u32);
            if palette.len() > max_palette {
                break;
            }
        }
        if palette.len() == 1 {
            return Ok(Self::single(kind, palette[0]));
        }
        let (bits, palette, packed) = if palette.len() > max_palette {
            if direct_bits <= kind.indirect_max() || direct_bits > 31 {
                return Err(Error::Invalid("direct palette bits"));
            }
            (direct_bits, Palette::Direct, values)
        } else {
            let bits = (usize::BITS - (palette.len() - 1).leading_zeros()) as u8;
            (
                bits.max(kind.indirect_min()),
                Palette::Indirect(palette),
                indices.as_slice(),
            )
        };
        let per_word = 64 / usize::from(bits);
        let mut data = vec![0; kind.entries().div_ceil(per_word)];
        for (i, &value) in packed.iter().enumerate() {
            if u64::from(value) >= (1u64 << bits) {
                return Err(Error::Invalid("registry ID does not fit palette bits"));
            }
            data[i / per_word] |= u64::from(value) << ((i % per_word) * usize::from(bits));
        }
        Ok(Self {
            kind,
            bits_per_entry: bits,
            palette,
            data,
        })
    }
    pub fn read(r: &mut Reader<'_>, version: Version, kind: ContainerKind) -> Result<Self> {
        let bits = r.u8()?;
        let palette = if bits == 0 {
            Palette::Single(read_id(r)?)
        } else if (kind.indirect_min()..=kind.indirect_max()).contains(&bits) {
            let n = r.count((1usize << bits).min(r.limits.max_collection))?;
            if n == 0 {
                return Err(Error::Invalid("empty indirect palette"));
            }
            let mut ids = Vec::with_capacity(n);
            for _ in 0..n {
                ids.push(read_id(r)?);
            }
            Palette::Indirect(ids)
        } else if bits > kind.indirect_max() && bits <= 31 {
            Palette::Direct
        } else {
            return Err(Error::Invalid("palette bits per entry"));
        };
        let expected = if bits == 0 {
            0
        } else {
            kind.entries().div_ceil(64 / usize::from(bits))
        };
        let n = if version.protocol() < 770 {
            r.count(r.limits.max_collection)?
        } else {
            expected
        };
        if n != expected {
            return Err(Error::Invalid("palette data-array length"));
        }
        if n > r.limits.max_collection {
            return Err(Error::Limit("palette data-array length"));
        }
        // Check all bytes before allocating.
        let raw = r.take(n * 8)?;
        let data = raw
            .as_chunks::<8>()
            .0
            .iter()
            .map(|v| u64::from_be_bytes(*v))
            .collect();
        let result = Self {
            kind,
            bits_per_entry: bits,
            palette,
            data,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        self.validate()?;
        if self.data.len() > limits.max_collection {
            return Err(Error::Limit("palette data-array length"));
        }
        w.u8(self.bits_per_entry);
        match &self.palette {
            Palette::Single(v) => w.var_i32(*v as i32),
            Palette::Indirect(ids) => {
                write_count(w, ids.len(), limits)?;
                for v in ids {
                    w.var_i32(*v as i32);
                }
            }
            Palette::Direct => {}
        }
        if version.protocol() < 770 {
            write_count(w, self.data.len(), limits)?;
        }
        for &word in &self.data {
            w.i64(word as i64);
        }
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        let bits = self.bits_per_entry;
        match &self.palette {
            Palette::Single(v) => {
                if bits != 0 || !self.data.is_empty() || *v > i32::MAX as u32 {
                    return Err(Error::Invalid("single palette"));
                }
                return Ok(());
            }
            Palette::Indirect(ids) => {
                if !(self.kind.indirect_min()..=self.kind.indirect_max()).contains(&bits)
                    || ids.is_empty()
                    || ids.len() > (1usize << bits)
                    || ids.iter().any(|&v| v > i32::MAX as u32)
                {
                    return Err(Error::Invalid("indirect palette"));
                }
            }
            Palette::Direct => {
                if bits <= self.kind.indirect_max() || bits > 31 {
                    return Err(Error::Invalid("direct palette bits"));
                }
            }
        }
        let expected = self.kind.entries().div_ceil(64 / usize::from(bits));
        if self.data.len() != expected {
            return Err(Error::Invalid("palette data-array length"));
        }
        if self.values().any(|v| v.is_none()) {
            return Err(Error::Invalid("palette index"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkSection {
    pub non_air_count: u16,
    /// Present beginning with 26.1 (protocol 775).
    pub fluid_count: Option<u16>,
    pub blocks: PaletteContainer,
    pub biomes: PaletteContainer,
}
impl ChunkSection {
    pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let non_air_count = r.u16()?;
        let fluid_count = if version.protocol() >= 775 {
            Some(r.u16()?)
        } else {
            None
        };
        if non_air_count > 4096 || fluid_count.is_some_and(|n| n > 4096) {
            return Err(Error::Invalid("section block/fluid count"));
        }
        let blocks = PaletteContainer::read(r, version, ContainerKind::Blocks)?;
        let biomes = PaletteContainer::read(r, version, ContainerKind::Biomes)?;
        Ok(Self {
            non_air_count,
            fluid_count,
            blocks,
            biomes,
        })
    }
    pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        if self.non_air_count > 4096 || self.fluid_count.is_some_and(|n| n > 4096) {
            return Err(Error::Invalid("section block/fluid count"));
        }
        if self.blocks.kind != ContainerKind::Blocks || self.biomes.kind != ContainerKind::Biomes {
            return Err(Error::Invalid("section container kind"));
        }
        if (version.protocol() >= 775) != self.fluid_count.is_some() {
            return Err(Error::Invalid("fluid count version"));
        }
        w.u16(self.non_air_count);
        if let Some(n) = self.fluid_count {
            w.u16(n);
        }
        self.blocks.write(w, version, limits)?;
        self.biomes.write(w, version, limits)?;
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Heightmaps {
    Nbt(Nbt),
    /// The numeric heightmap kind is preserved, including unknown future kinds.
    Typed(Vec<Heightmap>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heightmap {
    pub kind: i32,
    pub data: Vec<i64>,
}
impl Heightmap {
    /// Decode 256 relative heights, indexed by `z * 16 + x`. A value of zero
    /// means no surface; other values are one above the top block, relative to
    /// dimension min-Y. Supply the dimension's actual registry height.
    pub fn heights(&self, dimension_height: u32) -> Result<[u32; 256]> {
        if dimension_height == 0 || dimension_height > i32::MAX as u32 {
            return Err(Error::Invalid("heightmap dimension height"));
        }
        let bits = u32::BITS - dimension_height.leading_zeros();
        let per_word = 64 / bits as usize;
        if self.data.len() != 256usize.div_ceil(per_word) {
            return Err(Error::Invalid("heightmap data-array length"));
        }
        let mask = (1u64 << bits) - 1;
        let mut result = [0; 256];
        for (index, value) in result.iter_mut().enumerate() {
            *value = ((self.data[index / per_word] as u64 >> ((index % per_word) * bits as usize))
                & mask) as u32;
            if *value > dimension_height {
                return Err(Error::Invalid("heightmap value exceeds dimension"));
            }
        }
        Ok(result)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct BlockEntity {
    pub local_x: u8,
    pub local_z: u8,
    pub y: i16,
    pub kind: u32,
    pub data: Option<Nbt>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LightData {
    pub sky_mask: Vec<i64>,
    pub block_mask: Vec<i64>,
    pub empty_sky_mask: Vec<i64>,
    pub empty_block_mask: Vec<i64>,
    pub sky_arrays: Vec<Vec<u8>>,
    pub block_arrays: Vec<Vec<u8>>,
}
impl LightData {
    pub fn read(r: &mut Reader<'_>) -> Result<Self> {
        let result = Self {
            sky_mask: read_longs(r)?,
            block_mask: read_longs(r)?,
            empty_sky_mask: read_longs(r)?,
            empty_block_mask: read_longs(r)?,
            sky_arrays: read_light_arrays(r)?,
            block_arrays: read_light_arrays(r)?,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn write(&self, w: &mut Writer, limits: Limits) -> Result<()> {
        self.validate()?;
        // Preflight all lengths before modifying the caller's buffer.
        fn prefix(n: usize, limits: Limits) -> Result<usize> {
            if n > limits.max_collection || n > i32::MAX as usize {
                return Err(Error::Limit("light collection length"));
            }
            let mut bytes = 1;
            let mut n = n;
            while n >= 128 {
                bytes += 1;
                n >>= 7;
            }
            Ok(bytes)
        }
        let mut projected = w.as_slice().len();
        for longs in [
            &self.sky_mask,
            &self.block_mask,
            &self.empty_sky_mask,
            &self.empty_block_mask,
        ] {
            let bytes = longs
                .len()
                .checked_mul(8)
                .and_then(|n| n.checked_add(prefix(longs.len(), limits).ok()?))
                .ok_or(Error::Limit("light encoded length"))?;
            projected = projected
                .checked_add(bytes)
                .ok_or(Error::Limit("light encoded length"))?;
        }
        for arrays in [&self.sky_arrays, &self.block_arrays] {
            projected = projected
                .checked_add(prefix(arrays.len(), limits)?)
                .ok_or(Error::Limit("light encoded length"))?;
            for array in arrays {
                let bytes = prefix(array.len(), limits)?
                    .checked_add(array.len())
                    .ok_or(Error::Limit("light encoded length"))?;
                projected = projected
                    .checked_add(bytes)
                    .ok_or(Error::Limit("light encoded length"))?;
            }
        }
        if projected > limits.max_packet {
            return Err(Error::Limit("light encoded byte budget"));
        }

        for longs in [
            &self.sky_mask,
            &self.block_mask,
            &self.empty_sky_mask,
            &self.empty_block_mask,
        ] {
            write_longs(w, longs, limits)?;
        }
        for arrays in [&self.sky_arrays, &self.block_arrays] {
            write_count(w, arrays.len(), limits)?;
            for a in arrays {
                w.bytes(a)?;
            }
        }
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        fn popcount(mask: &[i64]) -> usize {
            mask.iter().map(|x| x.count_ones() as usize).sum()
        }
        if popcount(&self.sky_mask) != self.sky_arrays.len()
            || popcount(&self.block_mask) != self.block_arrays.len()
        {
            return Err(Error::Invalid("light mask/array count"));
        }
        if self
            .sky_mask
            .iter()
            .zip(&self.empty_sky_mask)
            .any(|(a, b)| a & b != 0)
            || self
                .block_mask
                .iter()
                .zip(&self.empty_block_mask)
                .any(|(a, b)| a & b != 0)
        {
            return Err(Error::Invalid("overlapping light masks"));
        }
        if self
            .sky_arrays
            .iter()
            .chain(&self.block_arrays)
            .any(|a| a.len() != 2048)
        {
            return Err(Error::Invalid("light array must contain 2048 bytes"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkData {
    pub x: i32,
    pub z: i32,
    pub heightmaps: Heightmaps,
    pub sections: Vec<ChunkSection>,
    pub block_entities: Vec<BlockEntity>,
    pub light: LightData,
}
impl ChunkData {
    pub fn decode(
        bytes: &[u8],
        version: Version,
        section_count: usize,
        limits: Limits,
    ) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("chunk packet bytes"));
        }
        if section_count == 0 || section_count > limits.max_collection {
            return Err(Error::Limit("chunk section count"));
        }
        let mut r = Reader::new(bytes, limits);
        let x = r.i32()?;
        let z = r.i32()?;
        let heightmaps = if version.protocol() < 770 {
            Heightmaps::Nbt(
                Nbt::read(&mut r, RootFormat::for_version(version))?
                    .ok_or(Error::Invalid("missing heightmap NBT"))?,
            )
        } else {
            let count = r.count(limits.max_collection)?;
            let mut maps = Vec::new();
            for _ in 0..count {
                let kind = r.var_i32()?;
                if kind < 0 {
                    return Err(Error::Invalid("heightmap kind"));
                }
                maps.push(Heightmap {
                    kind,
                    data: read_longs(&mut r)?,
                });
            }
            Heightmaps::Typed(maps)
        };
        let section_bytes = r.bytes(limits.max_packet)?;
        let mut section_reader = Reader::new(section_bytes, limits);
        let mut sections = Vec::new();
        for _ in 0..section_count {
            sections.push(ChunkSection::read(&mut section_reader, version)?);
        }
        let padding = section_reader.remaining();
        // Paper 1.20.1 (protocol763) can over-allocate one byte for each
        // singleton block palette when computing its section buffer size.
        // Accept exactly that verified zero-padding pattern, never arbitrary
        // trailing data or padding on newer protocol families.
        let singleton_sections = sections
            .iter()
            .filter(|s| matches!(s.blocks.palette, Palette::Single(_)))
            .count();
        let legacy_padding = version.protocol() == 763
            && padding.len() == singleton_sections
            && padding.iter().all(|&b| b == 0);
        if !padding.is_empty() && !legacy_padding {
            return Err(Error::TrailingBytes {
                context: "chunk sections",
                count: padding.len(),
            });
        }
        let count = r.count(limits.max_collection)?;
        let mut block_entities = Vec::new();
        for _ in 0..count {
            let packed = r.u8()?;
            let y = r.i16()?;
            let kind = read_id(&mut r)?;
            let data = Nbt::read(&mut r, RootFormat::for_version(version))?;
            block_entities.push(BlockEntity {
                local_x: packed >> 4,
                local_z: packed & 15,
                y,
                kind,
                data,
            });
        }
        let light = LightData::read(&mut r)?;
        r.finish()
            .map_err(|_| Error::Invalid("trailing chunk packet bytes"))?;
        Ok(Self {
            x,
            z,
            heightmaps,
            sections,
            block_entities,
            light,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if self.sections.is_empty() || self.sections.len() > limits.max_collection {
            return Err(Error::Limit("chunk section count"));
        }
        let mut w = Writer::new();
        w.i32(self.x);
        w.i32(self.z);
        match &self.heightmaps {
            Heightmaps::Nbt(n) if version.protocol() < 770 => {
                n.write(&mut w, RootFormat::for_version(version), limits)?
            }
            Heightmaps::Typed(maps) if version.protocol() >= 770 => {
                write_count(&mut w, maps.len(), limits)?;
                for map in maps {
                    if map.kind < 0 {
                        return Err(Error::Invalid("heightmap kind"));
                    }
                    w.var_i32(map.kind);
                    write_longs(&mut w, &map.data, limits)?;
                }
            }
            _ => return Err(Error::Invalid("heightmap version")),
        }
        let mut section_writer = Writer::new();
        for section in &self.sections {
            section.write(&mut section_writer, version, limits)?;
            check_size(&section_writer, limits)?;
        }
        w.bytes(section_writer.as_slice())?;
        write_count(&mut w, self.block_entities.len(), limits)?;
        for entity in &self.block_entities {
            if entity.local_x > 15 || entity.local_z > 15 || entity.kind > i32::MAX as u32 {
                return Err(Error::Invalid("block entity fields"));
            }
            w.u8((entity.local_x << 4) | entity.local_z);
            w.i16(entity.y);
            w.var_i32(entity.kind as i32);
            if let Some(n) = &entity.data {
                n.write(&mut w, RootFormat::for_version(version), limits)?;
            } else {
                w.u8(0);
            }
            check_size(&w, limits)?;
        }
        self.light.write(&mut w, limits)?;
        check_size(&w, limits)?;
        Ok(w.into_inner())
    }
}
fn read_id(r: &mut Reader<'_>) -> Result<u32> {
    let id = r.var_i32()?;
    if id < 0 {
        return Err(Error::Invalid("negative registry ID"));
    }
    Ok(id as u32)
}
fn read_longs(r: &mut Reader<'_>) -> Result<Vec<i64>> {
    let n = r.count(r.limits.max_collection)?;
    let raw = r.take(n.checked_mul(8).ok_or(Error::Limit("long array bytes"))?)?;
    Ok(raw
        .as_chunks::<8>()
        .0
        .iter()
        .map(|v| i64::from_be_bytes(*v))
        .collect())
}
fn write_longs(w: &mut Writer, data: &[i64], limits: Limits) -> Result<()> {
    write_count(w, data.len(), limits)?;
    for &n in data {
        w.i64(n);
    }
    check_size(w, limits)
}
fn write_count(w: &mut Writer, count: usize, limits: Limits) -> Result<()> {
    if count > limits.max_collection || count > i32::MAX as usize {
        return Err(Error::Limit("collection length"));
    }
    w.var_i32(count as i32);
    Ok(())
}
fn read_light_arrays(r: &mut Reader<'_>) -> Result<Vec<Vec<u8>>> {
    let count = r.count(r.limits.max_collection)?;
    let mut result = Vec::new();
    for _ in 0..count {
        let bytes = r.bytes(2048)?;
        if bytes.len() != 2048 {
            return Err(Error::Invalid("light array must contain 2048 bytes"));
        }
        result.push(bytes.to_vec());
    }
    Ok(result)
}
fn check_size(w: &Writer, limits: Limits) -> Result<()> {
    if w.as_slice().len() > limits.max_packet {
        Err(Error::Limit("chunk packet bytes"))
    } else {
        Ok(())
    }
}
