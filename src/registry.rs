//! Dynamic registry wire data and explicit numeric/name lookup.
//!
//! Protocol 763's login dimension codec and 764–765's configuration registry
//! packet carry one NBT compound. From 766, each packet carries one registry and
//! an ordered list of entries with a boolean plus optional anonymous NBT.
//! Entries omitted via known packs remain unresolved (`None`); this module never
//! invents vanilla or modded values. Static block/item registries are not supplied.
use crate::{
    codec::{Reader, Writer},
    nbt::{Nbt, RootFormat, Tag, TagType},
    Error, Limits, Result, Version,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct RegistryEntry {
    pub key: String,
    pub data: Option<Nbt>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum RegistryData {
    Legacy(Nbt),
    Entries {
        registry: String,
        entries: Vec<RegistryEntry>,
    },
}
impl RegistryData {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("registry packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let result = Self::read(&mut r, version)?;
        r.finish()?;
        Ok(result)
    }
    /// Also usable at the legacy registry codec field within the 1.20 login packet.
    pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let available = r.remaining();
        let mut bounded = Reader::new(
            &available[..available.len().min(r.limits.max_packet)],
            r.limits,
        );
        let result = Self::read_payload(&mut bounded, version)?;
        let consumed = bounded.position();
        r.take(consumed)?;
        Ok(result)
    }
    fn read_payload(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        if version.protocol() < 766 {
            Ok(Self::Legacy(
                Nbt::read(r, RootFormat::for_version(version))?
                    .ok_or(Error::Invalid("missing registry NBT"))?,
            ))
        } else {
            let registry = r.string(32767)?.to_owned();
            let count = r.count(r.limits.max_collection)?;
            let mut entries = Vec::new();
            for _ in 0..count {
                let key = r.string(32767)?.to_owned();
                let data = if r.bool()? {
                    Some(
                        Nbt::read(r, RootFormat::Anonymous)?
                            .ok_or(Error::Invalid("present registry entry has End root"))?,
                    )
                } else {
                    None
                };
                entries.push(RegistryEntry { key, data });
            }
            Ok(Self::Entries { registry, entries })
        }
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        match self {
            Self::Legacy(n) if version.protocol() < 766 => {
                n.write(&mut w, RootFormat::for_version(version), limits)?
            }
            Self::Entries { registry, entries } if version.protocol() >= 766 => {
                w.string(registry, limits.max_string_chars.min(32767))?;
                if entries.len() > limits.max_collection || entries.len() > i32::MAX as usize {
                    return Err(Error::Limit("registry entries"));
                }
                w.var_i32(entries.len() as i32);
                for entry in entries {
                    w.string(&entry.key, limits.max_string_chars.min(32767))?;
                    w.bool(entry.data.is_some());
                    if let Some(n) = &entry.data {
                        n.write(&mut w, RootFormat::Anonymous, limits)?;
                    }
                    if w.as_slice().len() > limits.max_packet {
                        return Err(Error::Limit("registry packet bytes"));
                    }
                }
            }
            _ => return Err(Error::Invalid("registry packet version")),
        }
        if w.as_slice().len() > limits.max_packet {
            return Err(Error::Limit("registry packet bytes"));
        }
        Ok(w.into_inner())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registry {
    pub entries: BTreeMap<u32, RegistryEntry>,
}
impl Registry {
    pub fn by_id(&self, id: u32) -> Option<&RegistryEntry> {
        self.entries.get(&id)
    }
    pub fn by_key(&self, key: &str) -> Option<(u32, &RegistryEntry)> {
        self.entries
            .iter()
            .find(|(_, v)| v.key == key)
            .map(|(&id, v)| (id, v))
    }
}
/// Accumulate one configuration epoch. Call `clear` when beginning a new
/// configuration; a packet replaces its named registry atomically.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegistryStore {
    pub registries: BTreeMap<String, Registry>,
}
impl RegistryStore {
    pub fn clear(&mut self) {
        self.registries.clear();
    }
    pub fn get(&self, name: &str) -> Option<&Registry> {
        self.registries.get(name)
    }
    pub fn apply(&mut self, packet: RegistryData) -> Result<()> {
        let updates = match packet {
            RegistryData::Entries { registry, entries } => {
                let mut result = Registry::default();
                let mut keys = std::collections::BTreeSet::new();
                for (id, entry) in entries.into_iter().enumerate() {
                    if id > i32::MAX as usize {
                        return Err(Error::Limit("registry ID"));
                    }
                    if !keys.insert(entry.key.clone()) {
                        return Err(Error::Invalid("duplicate registry key"));
                    }
                    result.entries.insert(id as u32, entry);
                }
                BTreeMap::from([(registry, result)])
            }
            RegistryData::Legacy(nbt) => parse_legacy(nbt.root)?,
        };
        self.registries.extend(updates);
        Ok(())
    }
    pub fn dimension_by_id(&self, id: u32) -> Result<DimensionInfo> {
        let entry = self
            .get("minecraft:dimension_type")
            .and_then(|r| r.by_id(id))
            .ok_or(Error::State("dimension registry ID unavailable"))?;
        dimension_entry(entry)
    }
    pub fn dimension_by_key(&self, key: &str) -> Result<DimensionInfo> {
        let (_, entry) = self
            .get("minecraft:dimension_type")
            .and_then(|r| r.by_key(key))
            .ok_or(Error::State("dimension registry key unavailable"))?;
        dimension_entry(entry)
    }
}
fn dimension_entry(entry: &RegistryEntry) -> Result<DimensionInfo> {
    DimensionInfo::from_nbt(entry.data.as_ref().ok_or(Error::State(
        "dimension data omitted by known packs; supply matching pack data",
    ))?)
}
fn parse_legacy(root: Tag) -> Result<BTreeMap<String, Registry>> {
    let Tag::Compound(registries) = root else {
        return Err(Error::Invalid("legacy registry root compound"));
    };
    let mut result = BTreeMap::new();
    for (registry_key, registry_tag) in registries {
        let registry_key = registry_key
            .to_string()
            .map_err(|_| Error::Invalid("registry key UTF-16"))?;
        let value = registry_tag
            .get("value")
            .ok_or(Error::Invalid("legacy registry value"))?;
        let Tag::List {
            element_type,
            elements,
        } = value
        else {
            return Err(Error::Invalid("legacy registry entry list"));
        };
        if *element_type != TagType::Compound && !elements.is_empty() {
            return Err(Error::Invalid("legacy registry entry type"));
        }
        let mut registry = Registry::default();
        let mut keys = std::collections::BTreeSet::new();
        for entry in elements {
            let id = entry
                .get("id")
                .and_then(Tag::as_i32)
                .ok_or(Error::Invalid("legacy registry entry ID"))?;
            if id < 0 {
                return Err(Error::Invalid("negative registry ID"));
            }
            let key = entry
                .get("name")
                .and_then(Tag::as_str)
                .ok_or(Error::Invalid("legacy registry entry name"))?
                .to_string()
                .map_err(|_| Error::Invalid("registry key UTF-16"))?;
            let element = entry
                .get("element")
                .ok_or(Error::Invalid("legacy registry entry element"))?
                .clone();
            if !keys.insert(key.clone()) || registry.entries.contains_key(&(id as u32)) {
                return Err(Error::Invalid("duplicate registry entry"));
            }
            registry.entries.insert(
                id as u32,
                RegistryEntry {
                    key,
                    data: Some(Nbt::anonymous(element)),
                },
            );
        }
        if result.insert(registry_key, registry).is_some() {
            return Err(Error::Invalid("duplicate registry"));
        }
    }
    Ok(result)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DimensionInfo {
    pub min_y: i32,
    pub height: i32,
    pub logical_height: Option<i32>,
    pub has_skylight: Option<bool>,
    pub has_ceiling: Option<bool>,
}
impl DimensionInfo {
    pub fn from_nbt(nbt: &Nbt) -> Result<Self> {
        Self::from_tag(&nbt.root)
    }
    pub fn from_tag(tag: &Tag) -> Result<Self> {
        let min_y = tag
            .get("min_y")
            .and_then(Tag::as_i32)
            .ok_or(Error::Invalid("dimension min_y"))?;
        let height = tag
            .get("height")
            .and_then(Tag::as_i32)
            .ok_or(Error::Invalid("dimension height"))?;
        if min_y % 16 != 0 || height <= 0 || height % 16 != 0 || min_y.checked_add(height).is_none()
        {
            return Err(Error::Invalid("dimension section bounds"));
        }
        let logical_height = match tag.get("logical_height") {
            Some(Tag::Int(n)) if *n >= 0 && *n <= height => Some(*n),
            None => None,
            _ => return Err(Error::Invalid("dimension logical height")),
        };
        let has_skylight = optional_bool(tag, "has_skylight")?;
        let has_ceiling = optional_bool(tag, "has_ceiling")?;
        Ok(Self {
            min_y,
            height,
            logical_height,
            has_skylight,
            has_ceiling,
        })
    }
    pub fn section_count(&self) -> usize {
        (self.height.max(0) as usize) / 16
    }
    pub fn min_section_y(&self) -> i32 {
        self.min_y.div_euclid(16)
    }
}
fn optional_bool(tag: &Tag, key: &str) -> Result<Option<bool>> {
    match tag.get(key) {
        Some(Tag::Byte(0)) => Ok(Some(false)),
        Some(Tag::Byte(1)) => Ok(Some(true)),
        None => Ok(None),
        _ => Err(Error::Invalid("dimension boolean")),
    }
}
