//! Bounded inventory and item-stack codecs for release protocols 763–776.
//!
//! Classic slots (763–765) retain complete NBT. Component slots (766+) support
//! the layouts reported by [`component_registry`]; unsupported layouts return
//! [`Error::Unsupported`] immediately. Component payloads are not length-framed,
//! so they can never safely be skipped. Retain the original `RawPacket` when a
//! typed decode fails. Registry IDs are version-specific; component names are
//! semantic names without the `minecraft:` prefix.
//!
//! Source: pinned `research/protocols/*.json`, `Slot`, `SlotComponent`,
//! `SlotComponentType`, `HashedSlot`, and inventory packet definitions from
//! <https://github.com/PrismarineJS/minecraft-data/tree/f5d7d74604d8c6153fd086bfe035e0630a5207cc/data/pc>.
//! Protocol 776 provenance is recorded separately in `research/26.2-commit.json`.
//! These are wire codecs, not an inventory simulator: callers manage state IDs
//! and click predictions. The [`super::item_hash`] adapters derive verified
//! component hashes for supported protocol-770+ payloads.
use super::entity_metadata::holders::RegistryHolderSet;
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    nbt::{Nbt, RootFormat, Tag},
    version::State,
    Error, Limits, Result, Version,
};
use std::collections::BTreeSet;
#[path = "inventory/components.rs"]
mod components;
#[path = "item_components.rs"]
mod registry;
pub use components::*;

/// Wire layout supported for a particular component in a particular release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentWire {
    Unit,
    Bool,
    VarInt,
    Int,
    Float,
    String,
    Nbt,
    /// A VarInt enum with values 0 through the inclusive maximum.
    Enum(i32),
    LoreOptional,
    Lore,
    EnchantmentsTooltip,
    Enchantments,
    DyedColorTooltip,
    CustomModelData,
    TooltipDisplay,
    BlockState,
    /// Required, nonempty slot (through protocol 774).
    Item,
    /// List of required, nonempty slots (through protocol 774).
    Items,
    /// List allowing empty slot sentinels, with an inclusive maximum length.
    OptionalItems(usize),
    /// Item ID, count, then patch, including all fields when count is zero.
    ItemTemplate,
    /// Template list with the release's inclusive maximum length.
    ItemTemplates(usize),
    /// Boolean-prefixed optional templates with an inclusive maximum length.
    OptionalItemTemplates(usize),
    IntList,
    TypedNbt,
    FoodLegacy,
    Food,
    PotionContents,
    StewEffects,
    WritableBook,
    WrittenBook,
    AttributeModifiers,
    LodestoneTracker,
    FireworkExplosion,
    Fireworks,
    Bees,
    Tool,
    Repairable,
    Unsupported,
}
/// Ordered by on-wire ID. An empty registry denotes classic-NBT releases.
/// Removed component IDs and hashed components are supported even when their
/// added payload layout is marked `Unsupported`.
pub fn component_registry(version: Version) -> &'static [(&'static str, ComponentWire)] {
    registry::registry(version.protocol())
}
pub fn component_id(version: Version, name: &str) -> Result<i32> {
    component_registry(version)
        .iter()
        .position(|(n, _)| *n == name)
        .map(|id| id as i32)
        .ok_or(Error::Unsupported("item component in selected release"))
}
pub(crate) fn component_type(version: Version, id: i32) -> Result<(&'static str, ComponentWire)> {
    usize::try_from(id)
        .ok()
        .and_then(|i| component_registry(version).get(i))
        .copied()
        .ok_or(Error::Unsupported("unknown item component ID"))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enchantment {
    pub id: i32,
    pub level: i32,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomModelData {
    pub floats: Vec<f32>,
    pub flags: Vec<bool>,
    pub strings: Vec<String>,
    pub colors: Vec<i32>,
}
/// Strongly typed payload shapes. Use [`component_registry`] to select the
/// shape for a semantic component name and target release. For example damage
/// uses `VarInt`, custom_name uses `Nbt`, and unbreakable changes `Bool` → `Unit`
/// at 770. Encoding rejects a mismatched shape rather than guessing.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentValue {
    Unit,
    Bool(bool),
    VarInt(i32),
    Int(i32),
    Float(f32),
    String(String),
    Nbt(Nbt),
    /// Absent entries are allowed only for protocols 766–769.
    Lore(Vec<Option<Nbt>>),
    Enchantments {
        entries: Vec<Enchantment>,
        show_tooltip: Option<bool>,
    },
    /// 766–769 only; 770+ dyed_color is `Int`.
    DyedColor {
        color: i32,
        show_tooltip: bool,
    },
    CustomModelData(CustomModelData),
    TooltipDisplay {
        hide_tooltip: bool,
        hidden_components: Vec<&'static str>,
    },
    BlockState(Vec<(String, String)>),
    /// Required nonempty stack in the selected release.
    Item(Box<Slot>),
    /// `container` permits empty slots; bundle/projectile lists require items.
    Items(Vec<Slot>),
    /// Protocol 775+ item template; requires component data and a nonnegative
    /// count. Unlike `Slot::Empty`, count zero still carries item ID and patch.
    ItemTemplate(Box<ItemStack>),
    ItemTemplates(Vec<ItemStack>),
    OptionalItemTemplates(Vec<Option<ItemStack>>),
    IntList(Vec<i32>),
    Food(Food),
    PotionContents(PotionContents),
    StewEffects(Vec<StewEffect>),
    WritableBook(Vec<Filterable<String>>),
    WrittenBook(WrittenBook),
    AttributeModifiers(AttributeModifiers),
    LodestoneTracker(LodestoneTracker),
    FireworkExplosion(FireworkExplosion),
    Fireworks(Fireworks),
    Bees(Vec<BeeOccupant>),
    Tool(Tool),
    Repairable(RegistryHolderSet),
    /// Registry type ID plus anonymous NBT, used by entity_data and
    /// block_entity_data from protocol 773.
    TypedNbt {
        type_id: i32,
        data: Nbt,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub name: &'static str,
    pub value: ComponentValue,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComponentPatch {
    pub added: Vec<Component>,
    pub removed: Vec<&'static str>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ItemData {
    Legacy(Option<Nbt>),
    Components(ComponentPatch),
}
#[derive(Clone, Debug, PartialEq)]
pub struct ItemStack {
    pub item_id: i32,
    pub count: i32,
    pub data: ItemData,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Slot {
    #[default]
    Empty,
    Item(ItemStack),
}
impl Slot {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let value = Self::read(&mut r, version)?;
        r.finish()?;
        Ok(value)
    }
    /// Read one slot atomically from an enclosing packet. For multiple slots
    /// prefer the packet codecs, which share aggregate collection and NBT node
    /// budgets. The input view is byte-bounded before allocating any payload.
    pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let bytes = r.remaining();
        let mut bounded = Reader::new(&bytes[..bytes.len().min(r.limits.max_packet)], r.limits);
        let mut b = Budget::new(r.limits);
        let slot = read_slot(&mut bounded, version, &mut b, 0)?;
        r.take(bounded.position())?;
        Ok(slot)
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        write_slot(self, &mut w, version, &mut Budget::new(limits), 0)?;
        finish(w, limits)
    }
    /// Atomic with respect to the destination writer on failure.
    pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.raw(&self.encode(version, limits)?);
        Ok(())
    }
}

// Inventory arrays, component patches and nested item lists share a collection
// budget. All NBT roots in a packet additionally share max_nbt_nodes. Nested
// items use max_nbt_depth (hard-capped at 64) to bound the Rust call stack.
pub(crate) struct Budget {
    pub(crate) limits: Limits,
    remaining: usize,
    nbt_remaining: usize,
}
impl Budget {
    pub(crate) fn new(limits: Limits) -> Self {
        Self {
            limits,
            remaining: limits.max_collection,
            nbt_remaining: limits.max_nbt_nodes,
        }
    }
    pub(crate) fn charge(&mut self, n: usize) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(n)
            .ok_or(Error::Limit("inventory aggregate elements"))?;
        Ok(())
    }
    fn depth(&self, depth: usize) -> Result<()> {
        if depth > self.limits.max_nbt_depth.min(64) {
            Err(Error::Limit("nested item depth"))
        } else {
            Ok(())
        }
    }
    pub(crate) fn count(&mut self, r: &mut Reader<'_>) -> Result<usize> {
        let n = r.count(self.remaining)?;
        self.charge(n)?;
        Ok(n)
    }
    pub(crate) fn write_count(&mut self, n: usize, w: &mut Writer) -> Result<()> {
        if n > i32::MAX as usize {
            return Err(Error::Limit("inventory collection"));
        }
        self.charge(n)?;
        w.var_i32(n as i32);
        self.check_bytes(w)
    }
    pub(crate) fn check_bytes(&self, w: &Writer) -> Result<()> {
        if w.as_slice().len() > self.limits.max_packet {
            Err(Error::Limit("inventory packet bytes"))
        } else {
            Ok(())
        }
    }
}
fn reader(bytes: &[u8], limits: Limits) -> Result<Reader<'_>> {
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("inventory packet bytes"));
    }
    Ok(Reader::new(bytes, limits))
}
fn finish(w: Writer, limits: Limits) -> Result<Vec<u8>> {
    if w.as_slice().len() > limits.max_packet {
        Err(Error::Limit("inventory packet bytes"))
    } else {
        Ok(w.into_inner())
    }
}
fn nonnegative(n: i32, context: &'static str) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid(context))
    } else {
        Ok(n)
    }
}
fn positive(n: i32, context: &'static str) -> Result<i32> {
    if n <= 0 {
        Err(Error::Invalid(context))
    } else {
        Ok(n)
    }
}
fn string(w: &mut Writer, value: &str, b: &Budget) -> Result<()> {
    let mut prefix = 1usize;
    let mut len = value.len();
    while len > 127 {
        prefix += 1;
        len >>= 7;
    }
    let needed = value
        .len()
        .checked_add(prefix)
        .ok_or(Error::Limit("inventory string bytes"))?;
    if needed > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("inventory string bytes"));
    }
    w.string(value, b.limits.max_string_chars.min(32767))?;
    b.check_bytes(w)
}
fn nbt_nodes(tag: &Tag) -> usize {
    1 + match tag {
        Tag::ByteArray(a) => a.len(),
        Tag::IntArray(a) => a.len(),
        Tag::LongArray(a) => a.len(),
        Tag::List { elements, .. } => elements.iter().map(nbt_nodes).sum(),
        Tag::Compound(a) => a.iter().map(|(_, t)| nbt_nodes(t)).sum(),
        _ => 0,
    }
}
pub(crate) fn read_nbt(
    r: &mut Reader<'_>,
    format: RootFormat,
    b: &mut Budget,
) -> Result<Option<Nbt>> {
    let original = r.limits.max_nbt_nodes;
    r.limits.max_nbt_nodes = original.min(b.nbt_remaining);
    let result = Nbt::read(r, format);
    r.limits.max_nbt_nodes = original;
    let value = result?;
    if let Some(n) = &value {
        b.nbt_remaining -= nbt_nodes(&n.root);
    }
    Ok(value)
}
fn required_nbt(r: &mut Reader<'_>, b: &mut Budget) -> Result<Nbt> {
    read_nbt(r, RootFormat::Anonymous, b)?.ok_or(Error::Invalid("missing component NBT"))
}
pub(crate) fn write_nbt(
    value: Option<&Nbt>,
    w: &mut Writer,
    format: RootFormat,
    b: &mut Budget,
) -> Result<()> {
    if let Some(value) = value {
        let limits = Limits {
            max_nbt_nodes: b.nbt_remaining,
            max_packet: b.limits.max_packet.saturating_sub(w.as_slice().len()),
            ..b.limits
        };
        value.write(w, format, limits)?;
        b.nbt_remaining -= nbt_nodes(&value.root);
    } else {
        w.u8(0);
    }
    b.check_bytes(w)
}
pub(crate) fn read_slot(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<Slot> {
    b.depth(depth)?;
    b.charge(1)?;
    if version.protocol() <= 765 {
        if !r.bool()? {
            return Ok(Slot::Empty);
        }
        let item_id = nonnegative(r.var_i32()?, "item ID")?;
        let count = positive(r.u8()? as i8 as i32, "item count")?;
        let nbt = read_nbt(r, RootFormat::for_version(version), b)?;
        return Ok(Slot::Item(ItemStack {
            item_id,
            count,
            data: ItemData::Legacy(nbt),
        }));
    }
    let count = if version.protocol() == 766 {
        r.u8()? as i8 as i32
    } else {
        r.var_i32()?
    };
    if count == 0 {
        return Ok(Slot::Empty);
    }
    positive(count, "item count")?;
    let item_id = nonnegative(r.var_i32()?, "item ID")?;
    let patch = read_patch(r, version, b, depth)?;
    Ok(Slot::Item(ItemStack {
        item_id,
        count,
        data: ItemData::Components(patch),
    }))
}
pub(crate) fn write_slot(
    slot: &Slot,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    b.depth(depth)?;
    b.charge(1)?;
    let item = match slot {
        Slot::Empty => {
            w.u8(0);
            return b.check_bytes(w);
        }
        Slot::Item(item) => item,
    };
    nonnegative(item.item_id, "item ID")?;
    positive(item.count, "item count")?;
    if version.protocol() <= 766 && item.count > 127 {
        return Err(Error::Invalid("signed-byte item count"));
    }
    match (&item.data, version.protocol()) {
        (ItemData::Legacy(nbt), 763..=765) => {
            w.bool(true);
            w.var_i32(item.item_id);
            w.u8(item.count as u8);
            write_nbt(nbt.as_ref(), w, RootFormat::for_version(version), b)?;
        }
        (ItemData::Components(patch), 766..=776) => {
            if version.protocol() == 766 {
                w.u8(item.count as u8);
            } else {
                w.var_i32(item.count);
            }
            w.var_i32(item.item_id);
            write_patch(patch, w, version, b, depth)?;
        }
        _ => return Err(Error::Unsupported("item data format for selected release")),
    }
    b.check_bytes(w)
}
/// The 775+ template layout is ID before count, with no empty-stack sentinel.
/// Keep this shared by inventory components and item particles so recursive
/// templates consume the same collection, NBT, byte, and call-stack budgets.
pub(crate) fn read_template(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<ItemStack> {
    if version.protocol() < 775 {
        return Err(Error::Unsupported("item template before protocol 775"));
    }
    b.depth(depth)?;
    b.charge(1)?;
    let item_id = nonnegative(r.var_i32()?, "template item ID")?;
    let count = nonnegative(r.var_i32()?, "template item count")?;
    Ok(ItemStack {
        item_id,
        count,
        data: ItemData::Components(read_patch(r, version, b, depth)?),
    })
}
pub(crate) fn write_template(
    item: &ItemStack,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    if version.protocol() < 775 {
        return Err(Error::Unsupported("item template before protocol 775"));
    }
    b.depth(depth)?;
    b.charge(1)?;
    w.var_i32(nonnegative(item.item_id, "template item ID")?);
    w.var_i32(nonnegative(item.count, "template item count")?);
    let ItemData::Components(patch) = &item.data else {
        return Err(Error::Invalid("item template requires components"));
    };
    write_patch(patch, w, version, b, depth)
}
pub(crate) fn read_patch(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<ComponentPatch> {
    let added = b.count(r)?;
    let removed = b.count(r)?;
    let mut patch = ComponentPatch::default();
    let mut seen = BTreeSet::new();
    for _ in 0..added {
        let id = r.var_i32()?;
        let (name, wire) = component_type(version, id)?;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate item component"));
        }
        let value = read_component(r, wire, version, b, depth)?;
        patch.added.push(Component { name, value });
    }
    for _ in 0..removed {
        let id = r.var_i32()?;
        let (name, _) = component_type(version, id)?;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate or conflicting component removal"));
        }
        patch.removed.push(name);
    }
    Ok(patch)
}
pub(crate) fn write_patch(
    patch: &ComponentPatch,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    b.write_count(patch.added.len(), w)?;
    b.write_count(patch.removed.len(), w)?;
    let mut seen = BTreeSet::new();
    for component in &patch.added {
        let id = component_id(version, component.name)?;
        let (_, wire) = component_type(version, id)?;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate item component"));
        }
        w.var_i32(id);
        write_component(&component.value, wire, w, version, b, depth)?;
    }
    for name in &patch.removed {
        let id = component_id(version, name)?;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate or conflicting component removal"));
        }
        w.var_i32(id);
    }
    b.check_bytes(w)
}
fn read_component(
    r: &mut Reader<'_>,
    wire: ComponentWire,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<ComponentValue> {
    use ComponentValue as V;
    use ComponentWire as W;
    Ok(match wire {
        W::ItemTemplate
        | W::ItemTemplates(_)
        | W::OptionalItemTemplates(_)
        | W::FoodLegacy
        | W::Food
        | W::PotionContents
        | W::StewEffects
        | W::WritableBook
        | W::WrittenBook
        | W::AttributeModifiers
        | W::LodestoneTracker
        | W::FireworkExplosion
        | W::Fireworks
        | W::Bees
        | W::Tool
        | W::Repairable => components::read(r, wire, version, b, depth)?,
        W::Unsupported => return Err(Error::Unsupported("item component payload layout")),
        W::Unit => V::Unit,
        W::Bool => V::Bool(r.bool()?),
        W::VarInt => V::VarInt(r.var_i32()?),
        W::Enum(max) => {
            let n = r.var_i32()?;
            if !(0..=max).contains(&n) {
                return Err(Error::Invalid("item component enum"));
            }
            V::VarInt(n)
        }
        W::Int => V::Int(r.i32()?),
        W::Float => V::Float(r.f32()?),
        W::String => V::String(r.string(32767)?.into()),
        W::Nbt => V::Nbt(required_nbt(r, b)?),
        W::TypedNbt => V::TypedNbt {
            type_id: nonnegative(r.var_i32()?, "component registry ID")?,
            data: required_nbt(r, b)?,
        },
        W::Lore | W::LoreOptional => {
            let n = b.count(r)?;
            let mut lines = Vec::new();
            for _ in 0..n {
                let line = read_nbt(r, RootFormat::Anonymous, b)?;
                if line.is_none() && wire == W::Lore {
                    return Err(Error::Invalid("missing lore NBT"));
                }
                lines.push(line);
            }
            V::Lore(lines)
        }
        W::Enchantments | W::EnchantmentsTooltip => {
            let n = b.count(r)?;
            let mut entries = Vec::new();
            for _ in 0..n {
                entries.push(Enchantment {
                    id: nonnegative(r.var_i32()?, "enchantment ID")?,
                    level: nonnegative(r.var_i32()?, "enchantment level")?,
                });
            }
            let show_tooltip = if wire == W::EnchantmentsTooltip {
                Some(r.bool()?)
            } else {
                None
            };
            V::Enchantments {
                entries,
                show_tooltip,
            }
        }
        W::DyedColorTooltip => V::DyedColor {
            color: r.i32()?,
            show_tooltip: r.bool()?,
        },
        W::CustomModelData => {
            let mut data = CustomModelData::default();
            for _ in 0..b.count(r)? {
                data.floats.push(r.f32()?);
            }
            for _ in 0..b.count(r)? {
                data.flags.push(r.bool()?);
            }
            for _ in 0..b.count(r)? {
                data.strings.push(r.string(32767)?.into());
            }
            for _ in 0..b.count(r)? {
                data.colors.push(r.i32()?);
            }
            V::CustomModelData(data)
        }
        W::TooltipDisplay => {
            let hide_tooltip = r.bool()?;
            let n = b.count(r)?;
            let mut hidden_components = Vec::new();
            for _ in 0..n {
                hidden_components.push(component_type(version, r.var_i32()?)?.0);
            }
            V::TooltipDisplay {
                hide_tooltip,
                hidden_components,
            }
        }
        W::BlockState => {
            let n = b.count(r)?;
            let mut values = Vec::new();
            for _ in 0..n {
                values.push((r.string(32767)?.into(), r.string(32767)?.into()));
            }
            V::BlockState(values)
        }
        W::Item => {
            let item = read_slot(r, version, b, depth + 1)?;
            if item == Slot::Empty {
                return Err(Error::Invalid("empty required component item"));
            }
            V::Item(Box::new(item))
        }
        W::Items | W::OptionalItems(_) => {
            let n = b.count(r)?;
            if let W::OptionalItems(maximum) = wire {
                if n > maximum {
                    return Err(Error::Limit("component item list"));
                }
            }
            let mut items = Vec::new();
            for _ in 0..n {
                let item = read_slot(r, version, b, depth + 1)?;
                if wire == W::Items && item == Slot::Empty {
                    return Err(Error::Invalid("empty required component item"));
                }
                items.push(item);
            }
            V::Items(items)
        }
        W::IntList => {
            let n = b.count(r)?;
            let mut items = Vec::new();
            for _ in 0..n {
                items.push(r.var_i32()?);
            }
            V::IntList(items)
        }
    })
}
pub(crate) fn write_component(
    value: &ComponentValue,
    wire: ComponentWire,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    use ComponentValue as V;
    use ComponentWire as W;
    match (wire, value) {
        (
            W::ItemTemplate
            | W::ItemTemplates(_)
            | W::OptionalItemTemplates(_)
            | W::FoodLegacy
            | W::Food
            | W::PotionContents
            | W::StewEffects
            | W::WritableBook
            | W::WrittenBook
            | W::AttributeModifiers
            | W::LodestoneTracker
            | W::FireworkExplosion
            | W::Fireworks
            | W::Bees
            | W::Tool
            | W::Repairable,
            value,
        ) => components::write(value, wire, w, version, b, depth)?,
        (W::Unsupported, _) => return Err(Error::Unsupported("item component payload layout")),
        (W::Unit, V::Unit) => {}
        (W::Bool, V::Bool(v)) => w.bool(*v),
        (W::VarInt, V::VarInt(v)) => w.var_i32(*v),
        (W::Enum(max), V::VarInt(v)) => {
            if !(0..=max).contains(v) {
                return Err(Error::Invalid("item component enum"));
            }
            w.var_i32(*v);
        }
        (W::Int, V::Int(v)) => w.i32(*v),
        (W::Float, V::Float(v)) => w.f32(*v),
        (W::String, V::String(v)) => string(w, v, b)?,
        (W::Nbt, V::Nbt(v)) => write_nbt(Some(v), w, RootFormat::Anonymous, b)?,
        (W::TypedNbt, V::TypedNbt { type_id, data }) => {
            w.var_i32(nonnegative(*type_id, "component registry ID")?);
            write_nbt(Some(data), w, RootFormat::Anonymous, b)?;
        }
        (W::Lore | W::LoreOptional, V::Lore(lines)) => {
            b.write_count(lines.len(), w)?;
            for line in lines {
                if line.is_none() && wire == W::Lore {
                    return Err(Error::Invalid("missing lore NBT"));
                }
                write_nbt(line.as_ref(), w, RootFormat::Anonymous, b)?;
            }
        }
        (
            W::Enchantments | W::EnchantmentsTooltip,
            V::Enchantments {
                entries,
                show_tooltip,
            },
        ) => {
            if show_tooltip.is_some() != (wire == W::EnchantmentsTooltip) {
                return Err(Error::Invalid("enchantment tooltip version"));
            }
            b.write_count(entries.len(), w)?;
            for entry in entries {
                w.var_i32(nonnegative(entry.id, "enchantment ID")?);
                w.var_i32(nonnegative(entry.level, "enchantment level")?);
                b.check_bytes(w)?;
            }
            if let Some(show) = show_tooltip {
                w.bool(*show);
            }
        }
        (
            W::DyedColorTooltip,
            V::DyedColor {
                color,
                show_tooltip,
            },
        ) => {
            w.i32(*color);
            w.bool(*show_tooltip);
        }
        (W::CustomModelData, V::CustomModelData(v)) => {
            b.write_count(v.floats.len(), w)?;
            for x in &v.floats {
                w.f32(*x);
                b.check_bytes(w)?;
            }
            b.write_count(v.flags.len(), w)?;
            for x in &v.flags {
                w.bool(*x);
                b.check_bytes(w)?;
            }
            b.write_count(v.strings.len(), w)?;
            for x in &v.strings {
                string(w, x, b)?;
            }
            b.write_count(v.colors.len(), w)?;
            for x in &v.colors {
                w.i32(*x);
                b.check_bytes(w)?;
            }
        }
        (
            W::TooltipDisplay,
            V::TooltipDisplay {
                hide_tooltip,
                hidden_components,
            },
        ) => {
            w.bool(*hide_tooltip);
            b.write_count(hidden_components.len(), w)?;
            for name in hidden_components {
                w.var_i32(component_id(version, name)?);
                b.check_bytes(w)?;
            }
        }
        (W::BlockState, V::BlockState(v)) => {
            b.write_count(v.len(), w)?;
            for (name, value) in v {
                string(w, name, b)?;
                string(w, value, b)?;
            }
        }
        (W::Item, V::Item(v)) => {
            b.depth(depth + 1)?;
            if **v == Slot::Empty {
                return Err(Error::Invalid("empty required component item"));
            }
            write_slot(v, w, version, b, depth + 1)?;
        }
        (W::Items | W::OptionalItems(_), V::Items(v)) => {
            if let W::OptionalItems(maximum) = wire {
                if v.len() > maximum {
                    return Err(Error::Limit("component item list"));
                }
            }
            b.write_count(v.len(), w)?;
            for x in v {
                b.depth(depth + 1)?;
                if wire == W::Items && *x == Slot::Empty {
                    return Err(Error::Invalid("empty required component item"));
                }
                write_slot(x, w, version, b, depth + 1)?;
            }
        }
        (W::IntList, V::IntList(v)) => {
            b.write_count(v.len(), w)?;
            for x in v {
                w.var_i32(*x);
                b.check_bytes(w)?;
            }
        }
        _ => {
            return Err(Error::Invalid(
                "component value does not match release layout",
            ))
        }
    }
    b.check_bytes(w)
}

fn read_container_id(r: &mut Reader<'_>, version: Version) -> Result<i32> {
    if version.protocol() < 768 {
        Ok(i32::from(r.u8()?))
    } else {
        nonnegative(r.var_i32()?, "container ID")
    }
}
fn write_container_id(id: i32, w: &mut Writer, version: Version) -> Result<()> {
    nonnegative(id, "container ID")?;
    if version.protocol() < 768 {
        w.u8(u8::try_from(id).map_err(|_| Error::Invalid("byte container ID"))?);
    } else {
        w.var_i32(id);
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerContent {
    pub window_id: i32,
    pub state_id: i32,
    pub items: Vec<Slot>,
    pub carried_item: Slot,
}
impl ContainerContent {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let mut b = Budget::new(limits);
        let window_id = read_container_id(&mut r, version)?;
        let state_id = nonnegative(r.var_i32()?, "container state ID")?;
        let n = b.count(&mut r)?;
        let mut items = Vec::new();
        for _ in 0..n {
            items.push(read_slot(&mut r, version, &mut b, 0)?);
        }
        let carried_item = read_slot(&mut r, version, &mut b, 0)?;
        r.finish()?;
        Ok(Self {
            window_id,
            state_id,
            items,
            carried_item,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        write_container_id(self.window_id, &mut w, version)?;
        w.var_i32(nonnegative(self.state_id, "container state ID")?);
        b.write_count(self.items.len(), &mut w)?;
        for item in &self.items {
            write_slot(item, &mut w, version, &mut b, 0)?;
        }
        write_slot(&self.carried_item, &mut w, version, &mut b, 0)?;
        finish(w, limits)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct SetContainerSlot {
    /// Protocols 763–767 use a signed byte (including special IDs -1 and -2).
    /// Protocol 768+ uses a nonnegative VarInt and separate cursor/player packets.
    pub window_id: i32,
    pub state_id: i32,
    pub slot: i16,
    pub item: Slot,
}
impl SetContainerSlot {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let window_id = if version.protocol() <= 767 {
            r.u8()? as i8 as i32
        } else {
            read_container_id(&mut r, version)?
        };
        let state_id = nonnegative(r.var_i32()?, "container state ID")?;
        let slot = r.i16()?;
        let item = Slot::read(&mut r, version)?;
        r.finish()?;
        Ok(Self {
            window_id,
            state_id,
            slot,
            item,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        if version.protocol() <= 767 {
            w.u8(i8::try_from(self.window_id)
                .map_err(|_| Error::Invalid("signed-byte container ID"))? as u8);
        } else {
            write_container_id(self.window_id, &mut w, version)?;
        }
        w.var_i32(nonnegative(self.state_id, "container state ID")?);
        w.i16(self.slot);
        write_slot(&self.item, &mut w, version, &mut Budget::new(limits), 0)?;
        finish(w, limits)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum ScreenTitle {
    /// Protocols 763–764 retain the JSON string without reformatting it.
    Json(String),
    /// Protocol 765 onward uses anonymous network NBT text components.
    Nbt(Nbt),
}
#[derive(Clone, Debug, PartialEq)]
pub struct OpenScreen {
    pub window_id: i32,
    pub menu_type: i32,
    pub title: ScreenTitle,
}
impl OpenScreen {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let window_id = nonnegative(r.var_i32()?, "container ID")?;
        let menu_type = nonnegative(r.var_i32()?, "menu type ID")?;
        let title = if version.protocol() < 765 {
            ScreenTitle::Json(r.string(32767)?.into())
        } else {
            ScreenTitle::Nbt(required_nbt(&mut r, &mut Budget::new(limits))?)
        };
        r.finish()?;
        Ok(Self {
            window_id,
            menu_type,
            title,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        w.var_i32(nonnegative(self.window_id, "container ID")?);
        w.var_i32(nonnegative(self.menu_type, "menu type ID")?);
        match (&self.title, version.protocol()) {
            (ScreenTitle::Json(title), 763..=764) => string(&mut w, title, &b)?,
            (ScreenTitle::Nbt(title), 765..=776) => {
                write_nbt(Some(title), &mut w, RootFormat::Anonymous, &mut b)?
            }
            _ => return Err(Error::Invalid("screen title format for selected release")),
        }
        finish(w, limits)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseContainer {
    pub window_id: i32,
}
impl CloseContainer {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let window_id = read_container_id(&mut r, version)?;
        r.finish()?;
        Ok(Self { window_id })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        write_container_id(self.window_id, &mut w, version)?;
        finish(w, limits)
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::named(
            version,
            State::Play,
            "close_window",
            self.encode(version, limits)?,
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetSelectedSlot {
    pub slot: u8,
}
impl SetSelectedSlot {
    /// Clientbound field is i8 before 769 and VarInt from 769 onward.
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let slot = if version.protocol() < 769 {
            r.u8()? as i8 as i32
        } else {
            r.var_i32()?
        };
        if !(0..=8).contains(&slot) {
            return Err(Error::Invalid("hotbar slot"));
        }
        r.finish()?;
        Ok(Self { slot: slot as u8 })
    }
    /// Encode the clientbound packet body.
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if self.slot > 8 {
            return Err(Error::Invalid("hotbar slot"));
        }
        let mut w = Writer::new();
        if version.protocol() < 769 {
            w.u8(self.slot);
        } else {
            w.var_i32(self.slot as i32);
        }
        finish(w, limits)
    }
    /// The serverbound selected-slot field remains an i16 in all releases.
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        if self.slot > 8 {
            return Err(Error::Invalid("hotbar slot"));
        }
        let mut w = Writer::new();
        w.i16(self.slot as i16);
        super::named(version, State::Play, "held_item_slot", finish(w, limits)?)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct SetPlayerInventory {
    pub slot: i32,
    pub item: Slot,
}
impl SetPlayerInventory {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if version.protocol() < 768 {
            return Err(Error::Unsupported("set player inventory before 768"));
        }
        let mut r = reader(bytes, limits)?;
        let slot = nonnegative(r.var_i32()?, "player inventory slot")?;
        let item = Slot::read(&mut r, version)?;
        r.finish()?;
        Ok(Self { slot, item })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if version.protocol() < 768 {
            return Err(Error::Unsupported("set player inventory before 768"));
        }
        let mut w = Writer::new();
        w.var_i32(nonnegative(self.slot, "player inventory slot")?);
        write_slot(&self.item, &mut w, version, &mut Budget::new(limits), 0)?;
        finish(w, limits)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct SetCursorItem {
    pub item: Slot,
}
impl SetCursorItem {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if version.protocol() < 768 {
            return Err(Error::Unsupported("set cursor item before 768"));
        }
        Ok(Self {
            item: Slot::decode(bytes, version, limits)?,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if version.protocol() < 768 {
            return Err(Error::Unsupported("set cursor item before 768"));
        }
        self.item.encode(version, limits)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ClickMode {
    Pickup = 0,
    QuickMove = 1,
    Swap = 2,
    Clone = 3,
    Throw = 4,
    QuickCraft = 5,
    PickupAll = 6,
}
impl TryFrom<i32> for ClickMode {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self> {
        Ok(match value {
            0 => Self::Pickup,
            1 => Self::QuickMove,
            2 => Self::Swap,
            3 => Self::Clone,
            4 => Self::Throw,
            5 => Self::QuickCraft,
            6 => Self::PickupAll,
            _ => return Err(Error::Invalid("container click mode")),
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClickHeader {
    pub window_id: i32,
    pub state_id: i32,
    /// Includes the protocol's outside-container sentinel -999.
    pub slot: i16,
    pub button: i8,
    pub mode: ClickMode,
}
impl ClickHeader {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            window_id: read_container_id(r, version)?,
            state_id: nonnegative(r.var_i32()?, "container state ID")?,
            slot: r.i16()?,
            button: r.u8()? as i8,
            mode: ClickMode::try_from(r.var_i32()?)?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version) -> Result<()> {
        write_container_id(self.window_id, w, version)?;
        w.var_i32(nonnegative(self.state_id, "container state ID")?);
        w.i16(self.slot);
        w.u8(self.button as u8);
        w.var_i32(self.mode as i32);
        Ok(())
    }
}
/// 763–769 click prediction carries full stacks. State IDs are supplied by the
/// caller from the latest container update; no optimistic state is invented.
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerClick {
    pub header: ClickHeader,
    pub changed_slots: Vec<(i16, Slot)>,
    pub carried_item: Slot,
}
impl ContainerClick {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if version.protocol() >= 770 {
            return Err(Error::Unsupported("full-stack clicks from protocol 770"));
        }
        let mut r = reader(bytes, limits)?;
        let mut b = Budget::new(limits);
        let header = ClickHeader::read(&mut r, version)?;
        let n = b.count(&mut r)?;
        let mut changed_slots = Vec::new();
        let mut seen = BTreeSet::new();
        for _ in 0..n {
            let slot = r.i16()?;
            if !seen.insert(slot) {
                return Err(Error::Invalid("duplicate changed slot"));
            }
            changed_slots.push((slot, read_slot(&mut r, version, &mut b, 0)?));
        }
        let carried_item = read_slot(&mut r, version, &mut b, 0)?;
        r.finish()?;
        Ok(Self {
            header,
            changed_slots,
            carried_item,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if version.protocol() >= 770 {
            return Err(Error::Unsupported("full-stack clicks from protocol 770"));
        }
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        self.header.write(&mut w, version)?;
        b.write_count(self.changed_slots.len(), &mut w)?;
        let mut seen = BTreeSet::new();
        for (slot, item) in &self.changed_slots {
            if !seen.insert(*slot) {
                return Err(Error::Invalid("duplicate changed slot"));
            }
            w.i16(*slot);
            write_slot(item, &mut w, version, &mut b, 0)?;
        }
        write_slot(&self.carried_item, &mut w, version, &mut b, 0)?;
        finish(w, limits)
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::named(
            version,
            State::Play,
            "window_click",
            self.encode(version, limits)?,
        )
    }
}
/// Explicit hashed-stack representation for protocol-770+ click predictions.
/// [`Self::from_slot`] derives supported component hashes; callers may supply
/// additional hashes verified against the selected release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashedItemStack {
    pub item_id: i32,
    pub count: i32,
    pub components: Vec<(&'static str, i32)>,
    pub removed_components: Vec<&'static str>,
}
fn read_hashed(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<Option<HashedItemStack>> {
    b.charge(1)?;
    if !r.bool()? {
        return Ok(None);
    }
    let item_id = nonnegative(r.var_i32()?, "item ID")?;
    let count = positive(r.var_i32()?, "item count")?;
    let n = b.count(r)?;
    let mut components = Vec::new();
    let mut seen = BTreeSet::new();
    for _ in 0..n {
        let id = r.var_i32()?;
        let name = component_type(version, id)?.0;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate hashed component"));
        }
        components.push((name, r.i32()?));
    }
    let n = b.count(r)?;
    let mut removed_components = Vec::new();
    for _ in 0..n {
        let id = r.var_i32()?;
        let name = component_type(version, id)?.0;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate or conflicting component removal"));
        }
        removed_components.push(name);
    }
    Ok(Some(HashedItemStack {
        item_id,
        count,
        components,
        removed_components,
    }))
}
fn write_hashed(
    item: Option<&HashedItemStack>,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    b.charge(1)?;
    w.bool(item.is_some());
    let Some(item) = item else {
        return b.check_bytes(w);
    };
    w.var_i32(nonnegative(item.item_id, "item ID")?);
    w.var_i32(positive(item.count, "item count")?);
    b.write_count(item.components.len(), w)?;
    let mut seen = BTreeSet::new();
    for (name, hash) in &item.components {
        let id = component_id(version, name)?;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate hashed component"));
        }
        w.var_i32(id);
        w.i32(*hash);
    }
    b.write_count(item.removed_components.len(), w)?;
    for name in &item.removed_components {
        let id = component_id(version, name)?;
        if !seen.insert(id) {
            return Err(Error::Invalid("duplicate or conflicting component removal"));
        }
        w.var_i32(id);
    }
    b.check_bytes(w)
}
/// Protocol 770+ click prediction. `None` is an empty stack, encoded as a
/// false option flag. Explicit inputs can be constructed with
/// [`HashedItemStack::from_slot`]; full stack encoding is never silently
/// substituted for this format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashedContainerClick {
    pub header: ClickHeader,
    pub changed_slots: Vec<(i16, Option<HashedItemStack>)>,
    pub carried_item: Option<HashedItemStack>,
}
impl HashedContainerClick {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if version.protocol() < 770 {
            return Err(Error::Unsupported("hashed clicks before protocol 770"));
        }
        let mut r = reader(bytes, limits)?;
        let mut b = Budget::new(limits);
        let header = ClickHeader::read(&mut r, version)?;
        let n = b.count(&mut r)?;
        let mut changed_slots = Vec::new();
        let mut seen = BTreeSet::new();
        for _ in 0..n {
            let slot = r.i16()?;
            if !seen.insert(slot) {
                return Err(Error::Invalid("duplicate changed slot"));
            }
            changed_slots.push((slot, read_hashed(&mut r, version, &mut b)?));
        }
        let carried_item = read_hashed(&mut r, version, &mut b)?;
        r.finish()?;
        Ok(Self {
            header,
            changed_slots,
            carried_item,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if version.protocol() < 770 {
            return Err(Error::Unsupported("hashed clicks before protocol 770"));
        }
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        self.header.write(&mut w, version)?;
        b.write_count(self.changed_slots.len(), &mut w)?;
        let mut seen = BTreeSet::new();
        for (slot, item) in &self.changed_slots {
            if !seen.insert(*slot) {
                return Err(Error::Invalid("duplicate changed slot"));
            }
            w.i16(*slot);
            write_hashed(item.as_ref(), &mut w, version, &mut b)?;
        }
        write_hashed(self.carried_item.as_ref(), &mut w, version, &mut b)?;
        finish(w, limits)
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::named(
            version,
            State::Play,
            "window_click",
            self.encode(version, limits)?,
        )
    }
}
