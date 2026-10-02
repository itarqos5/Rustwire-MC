//! Registry tag membership updates, protocols 763–776.
//!
//! The body is unchanged across these releases: a registry map, then a tag map,
//! then VarInt membership IDs. The packet is clientbound in play for every
//! release, and also configuration from 764 onward. The body codecs preserve
//! order, identifier spellings, duplicate keys and repeated membership IDs.
//! Lookup helpers use the last matching key, including implicit `minecraft:`
//! namespaces, matching the official map reader. No sorting or deduplication is
//! performed. IDs are nonnegative registry IDs, not block-state IDs.
//!
//! `max_collection` is one aggregate budget for registries + tags + member IDs.
//! Unknown registries, tags and IDs remain data; neither static registry names
//! nor gameplay behavior are inferred. Re-exported at `packet::common` for
//! compatibility with the original decoder.
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    registry::{Registry, RegistryEntry},
    version::{Direction, State},
    Error, Limits, Result, Version,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryTag {
    pub name: String,
    /// Membership IDs in wire order. Duplicate IDs are retained.
    pub entries: Vec<u32>,
}
impl RegistryTag {
    pub fn contains(&self, id: u32) -> bool {
        self.entries.contains(&id)
    }
    /// Resolve only against the supplied registry. Unknown IDs remain `None`;
    /// an existing entry may itself have data omitted by known packs.
    pub fn resolve<'a>(
        &'a self,
        registry: &'a Registry,
    ) -> impl Iterator<Item = (u32, Option<&'a RegistryEntry>)> + 'a {
        self.entries.iter().map(|&id| (id, registry.by_id(id)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedRegistry {
    pub registry: String,
    pub tags: Vec<RegistryTag>,
}
impl TaggedRegistry {
    /// Last matching tag, as in the official map decoder. Matching recognizes
    /// the default namespace; the returned spelling remains unchanged.
    pub fn tag(&self, name: &str) -> Option<&RegistryTag> {
        let name = identifier::parts(name);
        self.tags
            .iter()
            .rev()
            .find(|tag| identifier::parts(&tag.name) == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateTags {
    pub registries: Vec<TaggedRegistry>,
}
impl UpdateTags {
    /// Decode a packet body. Use `decode_in_state` to validate its wire state.
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("tag-update packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let mut budget = limits.max_collection;
        let count = read_count(&mut r, &mut budget, 2)?;
        let mut registries = Vec::with_capacity(count);
        for _ in 0..count {
            let registry = read_identifier(&mut r, version)?;
            let count = read_count(&mut r, &mut budget, 2)?;
            let mut tags = Vec::with_capacity(count);
            for _ in 0..count {
                let name = read_identifier(&mut r, version)?;
                let count = read_count(&mut r, &mut budget, 1)?;
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    let id = r.var_i32()?;
                    if id < 0 {
                        return Err(Error::Invalid("negative tag registry ID"));
                    }
                    entries.push(id as u32);
                }
                tags.push(RegistryTag { name, entries });
            }
            registries.push(TaggedRegistry { registry, tags });
        }
        r.finish()?;
        Ok(Self { registries })
    }
    pub fn decode_in_state(
        bytes: &[u8],
        version: Version,
        state: State,
        limits: Limits,
    ) -> Result<Self> {
        version.packet_id(state, Direction::Clientbound, "tags")?;
        Self::decode(bytes, version, limits)
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        let mut budget = limits.max_collection;
        write_count(&mut w, self.registries.len(), &mut budget, limits)?;
        for registry in &self.registries {
            write_identifier(&mut w, &registry.registry, version, limits)?;
            write_count(&mut w, registry.tags.len(), &mut budget, limits)?;
            for tag in &registry.tags {
                write_identifier(&mut w, &tag.name, version, limits)?;
                write_count(&mut w, tag.entries.len(), &mut budget, limits)?;
                for &id in &tag.entries {
                    if id > i32::MAX as u32 {
                        return Err(Error::Invalid("tag registry ID exceeds VarInt range"));
                    }
                    reserve_bytes(&w, varint_size(id as usize), limits)?;
                    w.var_i32(id as i32);
                }
            }
        }
        Ok(w.into_inner())
    }
    pub fn packet(&self, version: Version, state: State, limits: Limits) -> Result<RawPacket> {
        let id = version.packet_id(state, Direction::Clientbound, "tags")?;
        Ok(RawPacket::new(id, self.encode(version, limits)?))
    }
    /// Last matching registry. A later duplicate registry replaces its entire
    /// tag map for lookup; tags are not merged with an earlier occurrence.
    pub fn registry(&self, name: &str) -> Option<&TaggedRegistry> {
        let name = identifier::parts(name);
        self.registries
            .iter()
            .rev()
            .find(|registry| identifier::parts(&registry.registry) == name)
    }
}

fn read_count(r: &mut Reader<'_>, budget: &mut usize, minimum_bytes: usize) -> Result<usize> {
    let count = r.count(*budget)?;
    *budget -= count;
    if count > r.remaining().len() / minimum_bytes {
        return Err(Error::Eof);
    }
    Ok(count)
}
fn write_count(w: &mut Writer, count: usize, budget: &mut usize, limits: Limits) -> Result<()> {
    if count > *budget || count > i32::MAX as usize {
        return Err(Error::Limit("tag-update aggregate entries"));
    }
    *budget -= count;
    reserve_bytes(w, varint_size(count), limits)?;
    w.var_i32(count as i32);
    Ok(())
}
fn read_identifier(r: &mut Reader<'_>, version: Version) -> Result<String> {
    let value = r.string(32767)?;
    identifier::validate(value, version)?;
    Ok(value.into())
}
fn write_identifier(w: &mut Writer, value: &str, version: Version, limits: Limits) -> Result<()> {
    identifier::validate(value, version)?;
    reserve_bytes(
        w,
        value.len().saturating_add(varint_size(value.len())),
        limits,
    )?;
    w.string(value, limits.max_string_chars.min(32767))
}
fn reserve_bytes(w: &Writer, count: usize, limits: Limits) -> Result<()> {
    if count > limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("tag-update packet bytes"));
    }
    Ok(())
}
fn varint_size(mut value: usize) -> usize {
    let mut size = 1;
    while value > 127 {
        value >>= 7;
        size += 1;
    }
    size
}
