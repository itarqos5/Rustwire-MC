//! Adventure-mode block predicates and their component matchers.
//!
//! The release stream codecs, rather than historical schema backports, specify
//! three important details: block NBT has a boolean presence flag; partial
//! component matchers contain an NBT-serialized codec value; and protocol 774
//! adds a boolean selector for concrete versus any-value partial matchers.
//! Verified against AdventureModePredicate, BlockPredicate, NbtPredicate,
//! StatePropertiesPredicate, DataComponentMatchers, DataComponentPredicate,
//! DataComponentExactPredicate and ByteBufCodecs in the corresponding release
//! server artifacts. Only original protocol models and codecs are included.
//! The checked release families are 1.20.6, 1.21.1, 1.21.3, 1.21.4, 1.21.5,
//! 1.21.6, 1.21.8, 1.21.10, 1.21.11, 26.1.2 and 26.2. Partial matcher registry
//! IDs 0–13 are stable from protocol 770; villager/variant is ID 14 from 775.
use super::{
    component_id, component_type, read_component, required_nbt, string, write_component, write_nbt,
    Budget, Component,
};
use crate::{
    codec::{Reader, Writer},
    nbt::{Nbt, RootFormat, Tag},
    packet::entity_metadata::holders::{read_holder_set, write_holder_set, RegistryHolderSet},
    Error, Result, Version,
};
use std::collections::BTreeSet;

/// Payload shared by `can_place_on` and `can_break`.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockPredicates {
    pub predicates: Vec<ItemBlockPredicate>,
    /// Required for protocols 766–769; absent from protocol 770 onward.
    pub show_tooltip: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemBlockPredicate {
    pub blocks: Option<RegistryHolderSet>,
    /// `None` and an explicitly present empty list have distinct wire forms.
    pub properties: Option<Vec<BlockPropertyMatcher>>,
    /// Boolean-prefixed, required anonymous compound when present.
    pub nbt: Option<Nbt>,
    /// Required from protocol 770; absent in protocols 766–769.
    pub components: Option<DataComponentMatchers>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockPropertyMatcher {
    pub name: String,
    pub value: BlockPropertyValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockPropertyValue {
    Exact(String),
    Range {
        minimum: Option<String>,
        maximum: Option<String>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DataComponentMatchers {
    /// Ordered typed component values, using the selected release's ordinary
    /// component payloads, with no removed-component count.
    pub exact: Vec<Component>,
    /// At most 64 entries. A predicate kind may occur only once.
    pub partial: Vec<PartialComponentMatcher>,
}

/// The actual network format is a predicate-kind discriminator followed by a
/// required anonymous NBT tree, produced by the game's registry-aware data
/// codec. Preserve that tree without translating identifiers to numeric IDs.
/// This library validates the framing and budgets, not the data codec's game
/// rules or registry-dependent semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct PartialComponentMatcher {
    pub kind: ComponentPredicateKind,
    pub data: Nbt,
}

/// Semantic registry entries for the partial-match discriminator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentPredicateKind {
    Damage,
    Enchantments,
    StoredEnchantments,
    PotionContents,
    CustomData,
    Container,
    BundleContents,
    FireworkExplosion,
    Fireworks,
    WritableBookContent,
    WrittenBookContent,
    AttributeModifiers,
    Trim,
    JukeboxPlayable,
    /// Available from protocol 775.
    VillagerVariant,
    /// Available from protocol 774. The name is an ordinary component's
    /// semantic name. Its NBT payload is ordinarily an empty compound, but is
    /// still present on the wire and retained by `PartialComponentMatcher`.
    AnyValue(&'static str),
}

impl ComponentPredicateKind {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        if version.protocol() >= 774 && !r.bool()? {
            return Ok(Self::AnyValue(component_type(version, r.var_i32()?)?.0));
        }
        Ok(match r.var_i32()? {
            0 => Self::Damage,
            1 => Self::Enchantments,
            2 => Self::StoredEnchantments,
            3 => Self::PotionContents,
            4 => Self::CustomData,
            5 => Self::Container,
            6 => Self::BundleContents,
            7 => Self::FireworkExplosion,
            8 => Self::Fireworks,
            9 => Self::WritableBookContent,
            10 => Self::WrittenBookContent,
            11 => Self::AttributeModifiers,
            12 => Self::Trim,
            13 => Self::JukeboxPlayable,
            14 if version.protocol() >= 775 => Self::VillagerVariant,
            _ => return Err(Error::Unsupported("data-component predicate kind")),
        })
    }

    fn write(self, w: &mut Writer, version: Version) -> Result<()> {
        if let Self::AnyValue(name) = self {
            if version.protocol() < 774 {
                return Err(Error::Unsupported(
                    "any-value predicate before protocol 774",
                ));
            }
            w.bool(false);
            w.var_i32(component_id(version, name)?);
            return Ok(());
        }
        if version.protocol() >= 774 {
            w.bool(true);
        }
        w.var_i32(match self {
            Self::Damage => 0,
            Self::Enchantments => 1,
            Self::StoredEnchantments => 2,
            Self::PotionContents => 3,
            Self::CustomData => 4,
            Self::Container => 5,
            Self::BundleContents => 6,
            Self::FireworkExplosion => 7,
            Self::Fireworks => 8,
            Self::WritableBookContent => 9,
            Self::WrittenBookContent => 10,
            Self::AttributeModifiers => 11,
            Self::Trim => 12,
            Self::JukeboxPlayable => 13,
            Self::VillagerVariant if version.protocol() >= 775 => 14,
            Self::VillagerVariant => {
                return Err(Error::Unsupported("villager predicate before protocol 775"));
            }
            Self::AnyValue(_) => unreachable!(),
        });
        Ok(())
    }
}

fn read_optional_string(r: &mut Reader<'_>) -> Result<Option<String>> {
    Ok(if r.bool()? {
        Some(r.string(32767)?.into())
    } else {
        None
    })
}

fn write_optional_string(value: &Option<String>, w: &mut Writer, b: &Budget) -> Result<()> {
    w.bool(value.is_some());
    if let Some(value) = value {
        string(w, value, b)?;
    }
    b.check_bytes(w)
}

fn compound(value: &Nbt) -> Result<()> {
    if !matches!(value.root, Tag::Compound(_)) {
        return Err(Error::Invalid("block predicate NBT must be a compound"));
    }
    Ok(())
}

fn read_matchers(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<DataComponentMatchers> {
    let mut result = DataComponentMatchers::default();
    let n = b.count(r)?;
    for _ in 0..n {
        // Exact component values can themselves contain a block predicate,
        // item stack, or template. All paths consume the same depth budget.
        b.depth(depth + 1)?;
        let (name, wire) = component_type(version, r.var_i32()?)?;
        let value = read_component(r, wire, version, b, depth + 1)?;
        result.exact.push(Component { name, value });
    }
    let n = b.count(r)?;
    if n > 64 {
        return Err(Error::Limit("partial component matchers"));
    }
    let mut seen = BTreeSet::new();
    for _ in 0..n {
        let kind = ComponentPredicateKind::read(r, version)?;
        if !seen.insert(kind) {
            return Err(Error::Invalid("duplicate partial component matcher"));
        }
        result.partial.push(PartialComponentMatcher {
            kind,
            data: required_nbt(r, b)?,
        });
    }
    Ok(result)
}

fn write_matchers(
    value: &DataComponentMatchers,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    b.write_count(value.exact.len(), w)?;
    for component in &value.exact {
        b.depth(depth + 1)?;
        let id = component_id(version, component.name)?;
        let (_, wire) = component_type(version, id)?;
        w.var_i32(id);
        write_component(&component.value, wire, w, version, b, depth + 1)?;
    }
    if value.partial.len() > 64 {
        return Err(Error::Limit("partial component matchers"));
    }
    b.write_count(value.partial.len(), w)?;
    let mut seen = BTreeSet::new();
    for matcher in &value.partial {
        if !seen.insert(matcher.kind) {
            return Err(Error::Invalid("duplicate partial component matcher"));
        }
        matcher.kind.write(w, version)?;
        write_nbt(Some(&matcher.data), w, RootFormat::Anonymous, b)?;
    }
    b.check_bytes(w)
}

pub(super) fn read(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<BlockPredicates> {
    b.depth(depth)?;
    if version.protocol() < 766 {
        return Err(Error::Unsupported(
            "block predicate components before protocol 766",
        ));
    }
    let n = b.count(r)?;
    let mut predicates = Vec::new();
    for _ in 0..n {
        let blocks = if r.bool()? {
            Some(read_holder_set(r, b)?)
        } else {
            None
        };
        let properties = if r.bool()? {
            let n = b.count(r)?;
            let mut properties = Vec::new();
            for _ in 0..n {
                let name = r.string(32767)?.into();
                let value = if r.bool()? {
                    BlockPropertyValue::Exact(r.string(32767)?.into())
                } else {
                    BlockPropertyValue::Range {
                        minimum: read_optional_string(r)?,
                        maximum: read_optional_string(r)?,
                    }
                };
                properties.push(BlockPropertyMatcher { name, value });
            }
            Some(properties)
        } else {
            None
        };
        let nbt = if r.bool()? {
            let nbt = required_nbt(r, b)?;
            compound(&nbt)?;
            Some(nbt)
        } else {
            None
        };
        let components = if version.protocol() >= 770 {
            Some(read_matchers(r, version, b, depth)?)
        } else {
            None
        };
        predicates.push(ItemBlockPredicate {
            blocks,
            properties,
            nbt,
            components,
        });
    }
    let show_tooltip = if version.protocol() < 770 {
        Some(r.bool()?)
    } else {
        None
    };
    Ok(BlockPredicates {
        predicates,
        show_tooltip,
    })
}

pub(super) fn write(
    value: &BlockPredicates,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    b.depth(depth)?;
    if version.protocol() < 766 {
        return Err(Error::Unsupported(
            "block predicate components before protocol 766",
        ));
    }
    if value.show_tooltip.is_some() != (version.protocol() < 770) {
        return Err(Error::Invalid(
            "block predicate tooltip for selected release",
        ));
    }
    b.write_count(value.predicates.len(), w)?;
    for predicate in &value.predicates {
        w.bool(predicate.blocks.is_some());
        if let Some(blocks) = &predicate.blocks {
            write_holder_set(blocks, w, b)?;
        }
        w.bool(predicate.properties.is_some());
        if let Some(properties) = &predicate.properties {
            b.write_count(properties.len(), w)?;
            for property in properties {
                string(w, &property.name, b)?;
                match &property.value {
                    BlockPropertyValue::Exact(value) => {
                        w.bool(true);
                        string(w, value, b)?;
                    }
                    BlockPropertyValue::Range { minimum, maximum } => {
                        w.bool(false);
                        write_optional_string(minimum, w, b)?;
                        write_optional_string(maximum, w, b)?;
                    }
                }
            }
        }
        w.bool(predicate.nbt.is_some());
        if let Some(nbt) = &predicate.nbt {
            compound(nbt)?;
            write_nbt(Some(nbt), w, RootFormat::Anonymous, b)?;
        }
        match (&predicate.components, version.protocol() >= 770) {
            (Some(components), true) => write_matchers(components, w, version, b, depth)?,
            (None, false) => {}
            _ => {
                return Err(Error::Invalid(
                    "block component matchers for selected release",
                ))
            }
        }
        b.check_bytes(w)?;
    }
    if let Some(show_tooltip) = value.show_tooltip {
        w.bool(show_tooltip);
    }
    b.check_bytes(w)
}
