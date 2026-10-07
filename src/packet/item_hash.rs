//! Component-codec hashes for modern (770+) predicted inventory clicks.
//!
//! These are CRC32C hashes of Minecraft's structured `HashOps` values, **not**
//! hashes of packet bytes. Supported component codecs are deliberately explicit:
//! an unimplemented persistent representation returns [`Error::Unsupported`].
//! Registry references and unsupported text structures fail closed. Literal text,
//! styles and siblings are normalized through the verified persistent codec subset.
//! See `tools/paper/ComponentHashExtendedOracle.java` for the expanded fixtures.
//!
//! The primitive encoding was checked against the official cached Paper 1.21.5
//! and 26.2 `HashOps` implementations. See `tools/paper/HashOracle.java` and
//! `PROVENANCE.md`. This hash is a synchronization checksum, not cryptography.
use super::inventory::{self, Component, ComponentValue as V, HashedItemStack, ItemData, Slot};
use crate::{
    codec::Writer,
    nbt::{Nbt, NbtString, RootFormat, Tag},
    Error, Limits, Result, Version,
};
use std::collections::{BTreeMap, BTreeSet};
mod ordinary;
mod text;

const fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut bit = 0;
        while bit < 8 {
            c = (c >> 1) ^ (0x82f6_3b78 & 0u32.wrapping_sub(c & 1));
            bit += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}
const CRC_TABLE: [u32; 256] = crc_table();
struct Crc(u32);
impl Crc {
    fn new(tag: u8) -> Self {
        let mut h = Self(!0);
        h.byte(tag);
        h
    }
    fn byte(&mut self, b: u8) {
        self.0 = (self.0 >> 8) ^ CRC_TABLE[((self.0 as u8) ^ b) as usize];
    }
    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.byte(b);
        }
    }
    fn finish(self) -> u32 {
        !self.0
    }
}
fn primitive(tag: u8, bytes: &[u8]) -> u32 {
    let mut h = Crc::new(tag);
    h.bytes(bytes);
    h.finish()
}
fn integer(value: i32) -> u32 {
    primitive(8, &value.to_le_bytes())
}
fn float(value: f32) -> u32 {
    primitive(10, &value.to_bits().to_le_bytes())
}
fn boolean(value: bool) -> u32 {
    primitive(13, &[u8::from(value)])
}
fn string_units(units: impl ExactSizeIterator<Item = u16>) -> u32 {
    let mut h = Crc::new(12);
    h.bytes(&(units.len() as i32).to_le_bytes());
    for unit in units {
        h.bytes(&unit.to_le_bytes());
    }
    h.finish()
}
fn string(value: &str) -> u32 {
    // Rust's EncodeUtf16 is not ExactSizeIterator; count without allocating.
    let mut h = Crc::new(12);
    h.bytes(&(value.encode_utf16().count() as i32).to_le_bytes());
    for unit in value.encode_utf16() {
        h.bytes(&unit.to_le_bytes());
    }
    h.finish()
}
fn list(values: impl IntoIterator<Item = u32>) -> u32 {
    let mut h = Crc::new(4);
    for value in values {
        h.bytes(&value.to_le_bytes());
    }
    h.byte(5);
    h.finish()
}
fn map(mut entries: Vec<(u32, u32)>) -> u32 {
    // HashCode.padToLong is unsigned for CRC32C. Ordering signed i32 hashes is wrong.
    entries.sort_unstable();
    let mut h = Crc::new(2);
    for (key, value) in entries {
        h.bytes(&key.to_le_bytes());
        h.bytes(&value.to_le_bytes());
    }
    h.byte(3);
    h.finish()
}
fn fields(entries: Vec<(&str, u32)>) -> u32 {
    map(entries.into_iter().map(|(k, v)| (string(k), v)).collect())
}
fn nbt_hash(tag: &Tag) -> u32 {
    match tag {
        Tag::Byte(v) => primitive(6, &v.to_le_bytes()),
        Tag::Short(v) => primitive(7, &v.to_le_bytes()),
        Tag::Int(v) => integer(*v),
        Tag::Long(v) => primitive(9, &v.to_le_bytes()),
        Tag::Float(v) => float(*v),
        Tag::Double(v) => primitive(11, &v.to_bits().to_le_bytes()),
        Tag::String(v) => string_units(v.0.iter().copied()),
        Tag::ByteArray(values) => {
            let mut h = Crc::new(14);
            for v in values {
                h.byte(*v as u8);
            }
            h.byte(15);
            h.finish()
        }
        Tag::IntArray(values) => {
            let mut h = Crc::new(16);
            for v in values {
                h.bytes(&v.to_le_bytes());
            }
            h.byte(17);
            h.finish()
        }
        Tag::LongArray(values) => {
            let mut h = Crc::new(18);
            for v in values {
                h.bytes(&v.to_le_bytes());
            }
            h.byte(19);
            h.finish()
        }
        Tag::List { elements, .. } => list(elements.iter().map(|item| {
            // Modern ListTag loading unwraps one empty-key compound at each
            // list-entry boundary. Duplicate keys collapse before that check,
            // so a nonempty compound containing only empty keys uses its last
            // value. Empty compounds and compounds with another key stay maps.
            // Do not recursively unwrap here: a real empty-key compound is
            // double-wrapped on the wire and must retain its inner map.
            let logical = match item {
                Tag::Compound(entries) if entries.iter().all(|(key, _)| key.0.is_empty()) => {
                    entries.last().map_or(item, |(_, value)| value)
                }
                _ => item,
            };
            nbt_hash(logical)
        })),
        Tag::Compound(entries) => {
            // NBT decoding into Java CompoundTag retains the last value of duplicate keys.
            let normalized: BTreeMap<&NbtString, &Tag> =
                entries.iter().map(|(k, v)| (k, v)).collect();
            map(normalized
                .into_iter()
                .map(|(k, v)| (string_units(k.0.iter().copied()), nbt_hash(v)))
                .collect())
        }
    }
}
/// Hash an NBT value using the exact primitive widths, UTF-16 strings, arrays,
/// sorted maps and ordered lists consumed by modern `HashOps`. The root name is ignored.
/// Network-NBT mixed-list wrappers are normalized to the logical values seen
/// by modern `NbtOps`; raw NBT decoding and re-encoding remain lossless.
/// This is appropriate for `custom_data`; other component codecs can normalize
/// their values differently and must not automatically use this function.
pub fn hash_nbt(value: &Nbt, limits: Limits) -> Result<i32> {
    value.encode(RootFormat::Anonymous, limits)?;
    Ok(nbt_hash(&value.root) as i32)
}
fn identifier(value: &str) -> Result<u32> {
    let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value));
    if namespace.is_empty()
        || path.is_empty()
        || !namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        || !path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
    {
        return Err(Error::Invalid("component identifier"));
    }
    Ok(string(&format!("{namespace}:{path}")))
}
/// Derive a supported persistent component-codec hash. Wire shape and all input
/// budgets are validated first. Unsupported codecs never produce an estimate.
///
/// In addition to the primitive/custom-data codecs, this supports food, cooldown,
/// weapon/use effects, attack range, swing animation, fireworks, lodestone targets,
/// writable/written books and literal name/lore text. Literal text can include
/// siblings, the five decoration booleans, named/hex colors, integer shadow color,
/// font identifiers and insertion strings. Translation, selector, score, keybind,
/// NBT text, click/hover events and unrecognized style fields remain unsupported.
///
/// Sound components require inline sound events. Consumable/death-protection
/// effects support clearing effects, random teleport and inline sound playback;
/// effect-registry references and numeric sound IDs require caller knowledge and
/// are intentionally unsupported. No registry IDs are guessed from static tables.
/// Defaults, enum names and persistent numeric widths follow the selected release.
pub fn hash_component(component: &Component, version: Version, limits: Limits) -> Result<i32> {
    if version.protocol() < 770 {
        return Err(Error::Unsupported("component hashes before protocol 770"));
    }
    let id = inventory::component_id(version, component.name)?;
    let (_, wire) = inventory::component_type(version, id)?;
    inventory::write_component(
        &component.value,
        wire,
        &mut Writer::new(),
        version,
        &mut inventory::Budget::new(limits),
        0,
    )?;
    Ok(component_hash(component, version)? as i32)
}
fn component_hash(component: &Component, version: Version) -> Result<u32> {
    // Wire primitives are wider than several persistent codecs. Refuse values
    // whose vanilla codec cannot encode, rather than inventing a usable hash.
    let valid = match (component.name, &component.value) {
        ("max_stack_size", V::VarInt(v)) => (1..=99).contains(v),
        ("max_damage" | "enchantable", V::VarInt(v)) => *v > 0,
        ("damage" | "repair_cost", V::VarInt(v)) => *v >= 0,
        ("ominous_bottle_amplifier", V::VarInt(v)) => (0..=4).contains(v),
        ("potion_duration_scale", V::Float(v)) => v.is_finite() && *v >= 0.0,
        ("minimum_attack_charge", V::Float(v)) => (0.0..=1.0).contains(v),
        _ => true,
    };
    if !valid {
        return Err(Error::Invalid("component persistent codec range"));
    }
    Ok(match (component.name, &component.value) {
        (
            "max_stack_size"
            | "max_damage"
            | "damage"
            | "repair_cost"
            | "map_id"
            | "ominous_bottle_amplifier",
            V::VarInt(v),
        ) => integer(*v),
        ("dyed_color" | "map_color", V::Int(v)) => integer(*v),
        ("potion_duration_scale" | "minimum_attack_charge", V::Float(v)) => float(*v),
        ("enchantment_glint_override", V::Bool(v)) => boolean(*v),
        ("unbreakable" | "intangible_projectile" | "glider", V::Unit) => map(vec![]),
        ("rarity", V::VarInt(v)) => string(match v {
            0 => "common",
            1 => "uncommon",
            2 => "rare",
            3 => "epic",
            _ => return Err(Error::Invalid("item rarity")),
        }),
        ("item_model" | "tooltip_style" | "note_block_sound", V::String(v)) => identifier(v)?,
        ("custom_data" | "bucket_entity_data", V::Nbt(v)) => {
            if !matches!(v.root, Tag::Compound(_)) {
                return Err(Error::Invalid("custom data must be a compound"));
            }
            nbt_hash(&v.root)
        }
        ("block_state", V::BlockState(v)) => {
            let normalized: BTreeMap<&String, &String> = v.iter().map(|(k, v)| (k, v)).collect();
            map(normalized
                .into_iter()
                .map(|(k, v)| (string(k), string(v)))
                .collect())
        }
        ("enchantable", V::VarInt(v)) => fields(vec![("value", integer(*v))]),
        ("custom_model_data", V::CustomModelData(v)) => {
            let mut out = Vec::new();
            if !v.floats.is_empty() {
                out.push(("floats", list(v.floats.iter().copied().map(float))));
            }
            if !v.flags.is_empty() {
                out.push(("flags", list(v.flags.iter().copied().map(boolean))));
            }
            if !v.strings.is_empty() {
                out.push(("strings", list(v.strings.iter().map(|v| string(v)))));
            }
            if !v.colors.is_empty() {
                out.push(("colors", list(v.colors.iter().copied().map(integer))));
            }
            fields(out)
        }
        (
            "tooltip_display",
            V::TooltipDisplay {
                hide_tooltip,
                hidden_components,
            },
        ) => {
            let mut out = Vec::new();
            if *hide_tooltip {
                out.push(("hide_tooltip", boolean(true)));
            }
            if !hidden_components.is_empty() {
                let mut seen = BTreeSet::new();
                out.push((
                    "hidden_components",
                    list(
                        hidden_components
                            .iter()
                            .filter(|v| seen.insert(**v))
                            .map(|v| string(&format!("minecraft:{v}"))),
                    ),
                ));
            }
            fields(out)
        }
        _ => return ordinary::hash(component, version),
    })
}
impl HashedItemStack {
    /// Hash the exact component patch of a predicted slot (770+). Empty slots
    /// return `None`. The caller still owns click simulation, state IDs, and
    /// normalization against the target release's default item components.
    pub fn from_slot(slot: &Slot, version: Version, limits: Limits) -> Result<Option<Self>> {
        if version.protocol() < 770 {
            return Err(Error::Unsupported("hashed clicks before protocol 770"));
        }
        // Validate the complete patch once, including aggregate NBT and collection budgets.
        slot.encode(version, limits)?;
        let Slot::Item(item) = slot else {
            return Ok(None);
        };
        let ItemData::Components(patch) = &item.data else {
            return Err(Error::Unsupported("hashed legacy item"));
        };
        let components = patch
            .added
            .iter()
            .map(|c| Ok((c.name, component_hash(c, version)? as i32)))
            .collect::<Result<Vec<_>>>()?;
        Ok(Some(Self {
            item_id: item.item_id,
            count: item.count,
            components,
            removed_components: patch.removed.clone(),
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_hashops_primitive_fixtures() {
        assert_eq!(primitive(6, &(-7i8).to_le_bytes()) as i32, 326275673);
        assert_eq!(primitive(7, &(-1234i16).to_le_bytes()) as i32, -182226219);
        assert_eq!(integer(3) as i32, -499649379);
        assert_eq!(
            primitive(9, &(-123456789012i64).to_le_bytes()) as i32,
            -155379218
        );
        assert_eq!(float(0.2) as i32, 63779642);
        assert_eq!(
            primitive(11, &(-0.125f64).to_bits().to_le_bytes()) as i32,
            -219421683
        );
        assert_eq!(float(f32::from_bits(0x7fc00001)) as i32, 103636112);
        assert_eq!(
            primitive(11, &(-0.0f64).to_bits().to_le_bytes()) as i32,
            -1731220123
        );
        assert_eq!(string("Rustwire 🚀") as i32, 1915252855);
        assert_eq!(string_units([0xd800].into_iter()) as i32, -423001966);
        assert_eq!(boolean(false) as i32, 828198337);
        assert_eq!(boolean(true) as i32, -1019818302);
        assert_eq!(map(vec![]) as i32, -982207288);
        assert_eq!(list([]) as i32, -1978007022);
        assert_eq!(list([integer(3), string("hello")]) as i32, -508761950);
    }
}
