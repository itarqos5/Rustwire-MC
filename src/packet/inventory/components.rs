//! Typed ordinary item-component payloads, shared aggregate budgets and
//! release-specific stream layouts. Re-exported through `packet::inventory`.
use super::{
    nonnegative, read_slot, read_template, required_nbt, string, write_nbt, write_slot,
    write_template, Budget, ComponentValue, ComponentWire, Slot,
};
use crate::packet::entity_metadata::holders::{
    read_holder_set, write_holder_set, RegistryHolderSet,
};
use crate::{
    codec::{BlockPosition, Reader, Writer},
    nbt::{Nbt, RootFormat, Tag},
    Error, Result, Version,
};

/// Food properties. Protocols 766–767 additionally require `legacy_consumption`.
#[derive(Clone, Debug, PartialEq)]
pub struct Food {
    pub nutrition: i32,
    pub saturation_modifier: f32,
    pub can_always_eat: bool,
    pub legacy_consumption: Option<LegacyFoodConsumption>,
}
/// Consumption moved to a separate component in protocol 768.
#[derive(Clone, Debug, PartialEq)]
pub struct LegacyFoodConsumption {
    pub seconds_to_eat: f32,
    /// Only protocol 767 supports this boolean-prefixed, nonempty stack.
    pub using_converts_to: Option<Box<Slot>>,
    pub effects: Vec<FoodEffect>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct FoodEffect {
    pub effect: PotionEffect,
    pub probability: f32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PotionEffect {
    pub id: i32,
    pub details: EffectDetails,
}
/// A bounded recursive hidden-effect chain. Negative durations (including the
/// infinite-duration sentinel) are retained exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectDetails {
    pub amplifier: i32,
    pub duration: i32,
    pub ambient: bool,
    pub show_particles: bool,
    pub show_icon: bool,
    pub hidden_effect: Option<Box<EffectDetails>>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PotionContents {
    pub potion_id: Option<i32>,
    pub custom_color: Option<i32>,
    pub custom_effects: Vec<PotionEffect>,
    /// Added in protocol 768; must be absent in 766–767.
    pub custom_name: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StewEffect {
    pub id: i32,
    pub duration: i32,
}
/// The filtered value is encoded with a boolean presence flag, including when
/// `T` is a text-component NBT root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filterable<T> {
    pub raw: T,
    pub filtered: Option<T>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct WrittenBook {
    pub title: Filterable<String>,
    pub author: String,
    pub generation: i32,
    pub pages: Vec<Filterable<Nbt>>,
    pub resolved: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeModifierId {
    /// Protocol 766 only.
    Legacy { uuid: [u8; 16], name: String },
    /// Resource identifier, protocol 767+.
    Identifier(String),
}
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeDisplay {
    Default,
    Hidden,
    Override(Nbt),
}
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeModifier {
    pub attribute_id: i32,
    pub modifier_id: AttributeModifierId,
    pub amount: f64,
    pub operation: AttributeOperation,
    pub slot: AttributeSlot,
    /// Required from protocol 771; absent in earlier releases.
    pub display: Option<AttributeDisplay>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeModifiers {
    pub entries: Vec<AttributeModifier>,
    /// Required in 766–769; absent from 770 onward.
    pub show_tooltip: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LodestoneTarget {
    pub dimension: String,
    pub position: BlockPosition,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LodestoneTracker {
    pub target: Option<LodestoneTarget>,
    pub tracked: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FireworkExplosion {
    pub shape: FireworkShape,
    pub colors: Vec<i32>,
    pub fade_colors: Vec<i32>,
    pub has_trail: bool,
    pub has_twinkle: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fireworks {
    pub flight_duration: i32,
    pub explosions: Vec<FireworkExplosion>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BeeOccupant {
    /// Required from protocol 773; absent before it.
    pub entity_type: Option<i32>,
    pub entity_data: Nbt,
    pub ticks_in_hive: i32,
    pub minimum_ticks_in_hive: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ToolRule {
    pub blocks: RegistryHolderSet,
    pub speed: Option<f32>,
    pub correct_for_drops: Option<bool>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub rules: Vec<ToolRule>,
    pub default_mining_speed: f32,
    pub damage_per_block: i32,
    /// Required from protocol 770; absent before it.
    pub can_destroy_blocks_in_creative: Option<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum AttributeOperation {
    Add = 0,
    MultiplyBase = 1,
    MultiplyTotal = 2,
}
impl AttributeOperation {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::Add),
            1 => Ok(Self::MultiplyBase),
            2 => Ok(Self::MultiplyTotal),
            _ => Err(Error::Invalid("AttributeOperation")),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum AttributeSlot {
    Any = 0,
    MainHand = 1,
    OffHand = 2,
    Hand = 3,
    Feet = 4,
    Legs = 5,
    Chest = 6,
    Head = 7,
    Armor = 8,
    Body = 9,
    Saddle = 10,
}
impl AttributeSlot {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::Any),
            1 => Ok(Self::MainHand),
            2 => Ok(Self::OffHand),
            3 => Ok(Self::Hand),
            4 => Ok(Self::Feet),
            5 => Ok(Self::Legs),
            6 => Ok(Self::Chest),
            7 => Ok(Self::Head),
            8 => Ok(Self::Armor),
            9 => Ok(Self::Body),
            10 => Ok(Self::Saddle),
            _ => Err(Error::Invalid("AttributeSlot")),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum FireworkShape {
    SmallBall = 0,
    LargeBall = 1,
    Star = 2,
    Creeper = 3,
    Burst = 4,
}
impl FireworkShape {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::SmallBall),
            1 => Ok(Self::LargeBall),
            2 => Ok(Self::Star),
            3 => Ok(Self::Creeper),
            4 => Ok(Self::Burst),
            _ => Err(Error::Invalid("FireworkShape")),
        }
    }
}

// The pinned schemas contain historical backports. Cached official release
// bytecode (FoodProperties, PotionContents, Filterable, and BeehiveBlockEntity
// Occupant STREAM_CODEC declarations) establishes these corrections:
// * 766 food has no converter; 767 uses boolean + nonempty ItemStack.
// * 766–767 food effects are full MobEffectInstance values, not just IDs.
// * Potion custom_name begins at 768.
// * Written-book filtered pages use boolean + required anonymous NBT.
// * Bees acquire a registry entity type before their NBT in 773.
fn optional<T>(
    r: &mut Reader<'_>,
    f: impl FnOnce(&mut Reader<'_>) -> Result<T>,
) -> Result<Option<T>> {
    if r.bool()? {
        Ok(Some(f(r)?))
    } else {
        Ok(None)
    }
}
fn write_optional<T>(
    value: Option<&T>,
    w: &mut Writer,
    b: &mut Budget,
    f: impl FnOnce(&T, &mut Writer, &mut Budget) -> Result<()>,
) -> Result<()> {
    w.bool(value.is_some());
    if let Some(value) = value {
        f(value, w, b)?;
    }
    b.check_bytes(w)
}
fn limited_count(r: &mut Reader<'_>, b: &mut Budget, maximum: usize) -> Result<usize> {
    let n = b.count(r)?;
    if n > maximum {
        return Err(Error::Limit("item component collection"));
    }
    Ok(n)
}
fn write_limited_count(n: usize, w: &mut Writer, b: &mut Budget, maximum: usize) -> Result<()> {
    if n > maximum {
        return Err(Error::Limit("item component collection"));
    }
    b.write_count(n, w)
}
fn limited_string(w: &mut Writer, value: &str, b: &Budget, maximum: usize) -> Result<()> {
    if value.encode_utf16().count() > maximum {
        return Err(Error::Limit("item component string"));
    }
    string(w, value, b)
}
fn read_filterable_string(r: &mut Reader<'_>, maximum: usize) -> Result<Filterable<String>> {
    Ok(Filterable {
        raw: r.string(maximum)?.into(),
        filtered: optional(r, |r| Ok(r.string(maximum)?.into()))?,
    })
}
fn write_filterable_string(
    value: &Filterable<String>,
    w: &mut Writer,
    b: &mut Budget,
    maximum: usize,
) -> Result<()> {
    limited_string(w, &value.raw, b, maximum)?;
    write_optional(value.filtered.as_ref(), w, b, |x, w, b| {
        limited_string(w, x, b, maximum)
    })
}
fn read_effect_details(r: &mut Reader<'_>, b: &mut Budget, depth: usize) -> Result<EffectDetails> {
    b.depth(depth)?;
    b.charge(1)?;
    Ok(EffectDetails {
        amplifier: r.var_i32()?,
        duration: r.var_i32()?,
        ambient: r.bool()?,
        show_particles: r.bool()?,
        show_icon: r.bool()?,
        hidden_effect: optional(r, |r| Ok(Box::new(read_effect_details(r, b, depth + 1)?)))?,
    })
}
fn write_effect_details(
    value: &EffectDetails,
    w: &mut Writer,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    b.depth(depth)?;
    b.charge(1)?;
    w.var_i32(value.amplifier);
    w.var_i32(value.duration);
    w.bool(value.ambient);
    w.bool(value.show_particles);
    w.bool(value.show_icon);
    write_optional(value.hidden_effect.as_ref(), w, b, |x, w, b| {
        write_effect_details(x, w, b, depth + 1)
    })
}
fn read_effect(r: &mut Reader<'_>, b: &mut Budget, depth: usize) -> Result<PotionEffect> {
    Ok(PotionEffect {
        id: nonnegative(r.var_i32()?, "mob effect ID")?,
        details: read_effect_details(r, b, depth + 1)?,
    })
}
fn write_effect(value: &PotionEffect, w: &mut Writer, b: &mut Budget, depth: usize) -> Result<()> {
    w.var_i32(nonnegative(value.id, "mob effect ID")?);
    write_effect_details(&value.details, w, b, depth + 1)
}
fn read_food(r: &mut Reader<'_>, version: Version, b: &mut Budget, depth: usize) -> Result<Food> {
    let nutrition = r.var_i32()?;
    let saturation_modifier = r.f32()?;
    let can_always_eat = r.bool()?;
    let legacy_consumption = if version.protocol() <= 767 {
        let seconds_to_eat = r.f32()?;
        let using_converts_to = if version.protocol() == 767 {
            optional(r, |r| {
                let slot = read_slot(r, version, b, depth + 1)?;
                if slot == Slot::Empty {
                    return Err(Error::Invalid("empty food conversion stack"));
                }
                Ok(Box::new(slot))
            })?
        } else {
            None
        };
        let mut effects = Vec::new();
        for _ in 0..b.count(r)? {
            effects.push(FoodEffect {
                effect: read_effect(r, b, depth)?,
                probability: r.f32()?,
            });
        }
        Some(LegacyFoodConsumption {
            seconds_to_eat,
            using_converts_to,
            effects,
        })
    } else {
        None
    };
    Ok(Food {
        nutrition,
        saturation_modifier,
        can_always_eat,
        legacy_consumption,
    })
}
fn write_food(
    value: &Food,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    if value.legacy_consumption.is_some() != (version.protocol() <= 767) {
        return Err(Error::Invalid("food consumption version"));
    }
    w.var_i32(value.nutrition);
    w.f32(value.saturation_modifier);
    w.bool(value.can_always_eat);
    if let Some(legacy) = &value.legacy_consumption {
        w.f32(legacy.seconds_to_eat);
        if version.protocol() == 767 {
            write_optional(legacy.using_converts_to.as_ref(), w, b, |x, w, b| {
                if **x == Slot::Empty {
                    return Err(Error::Invalid("empty food conversion stack"));
                }
                write_slot(x, w, version, b, depth + 1)
            })?;
        } else if legacy.using_converts_to.is_some() {
            return Err(Error::Invalid("food conversion version"));
        }
        b.write_count(legacy.effects.len(), w)?;
        for effect in &legacy.effects {
            write_effect(&effect.effect, w, b, depth)?;
            w.f32(effect.probability);
            b.check_bytes(w)?;
        }
    }
    b.check_bytes(w)
}
fn read_attributes(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<AttributeModifiers> {
    let mut entries = Vec::new();
    for _ in 0..b.count(r)? {
        let attribute_id = nonnegative(r.var_i32()?, "attribute ID")?;
        let modifier_id = if version.protocol() == 766 {
            AttributeModifierId::Legacy {
                uuid: r.uuid()?,
                name: r.string(32767)?.into(),
            }
        } else {
            AttributeModifierId::Identifier(r.string(32767)?.into())
        };
        let amount = r.f64()?;
        let operation = AttributeOperation::read(r)?;
        let slot = AttributeSlot::read(r)?;
        if slot == AttributeSlot::Saddle && version.protocol() < 770 {
            return Err(Error::Invalid("attribute saddle slot version"));
        }
        let display = if version.protocol() >= 771 {
            Some(match r.var_i32()? {
                0 => AttributeDisplay::Default,
                1 => AttributeDisplay::Hidden,
                2 => AttributeDisplay::Override(required_nbt(r, b)?),
                _ => return Err(Error::Invalid("attribute display type")),
            })
        } else {
            None
        };
        entries.push(AttributeModifier {
            attribute_id,
            modifier_id,
            amount,
            operation,
            slot,
            display,
        });
    }
    let show_tooltip = if version.protocol() < 770 {
        Some(r.bool()?)
    } else {
        None
    };
    Ok(AttributeModifiers {
        entries,
        show_tooltip,
    })
}
fn write_attributes(
    value: &AttributeModifiers,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    if value.show_tooltip.is_some() != (version.protocol() < 770) {
        return Err(Error::Invalid("attribute tooltip version"));
    }
    b.write_count(value.entries.len(), w)?;
    for entry in &value.entries {
        w.var_i32(nonnegative(entry.attribute_id, "attribute ID")?);
        match (&entry.modifier_id, version.protocol()) {
            (AttributeModifierId::Legacy { uuid, name }, 766) => {
                w.raw(uuid);
                string(w, name, b)?;
            }
            (AttributeModifierId::Identifier(name), 767..) => string(w, name, b)?,
            _ => return Err(Error::Invalid("attribute modifier ID version")),
        }
        w.f64(entry.amount);
        w.var_i32(entry.operation as i32);
        if entry.slot == AttributeSlot::Saddle && version.protocol() < 770 {
            return Err(Error::Invalid("attribute saddle slot version"));
        }
        w.var_i32(entry.slot as i32);
        if entry.display.is_some() != (version.protocol() >= 771) {
            return Err(Error::Invalid("attribute display version"));
        }
        if let Some(display) = &entry.display {
            match display {
                AttributeDisplay::Default => w.var_i32(0),
                AttributeDisplay::Hidden => w.var_i32(1),
                AttributeDisplay::Override(nbt) => {
                    w.var_i32(2);
                    write_nbt(Some(nbt), w, RootFormat::Anonymous, b)?;
                }
            }
        }
        b.check_bytes(w)?;
    }
    if let Some(show) = value.show_tooltip {
        w.bool(show);
    }
    b.check_bytes(w)
}
fn read_explosion(r: &mut Reader<'_>, b: &mut Budget) -> Result<FireworkExplosion> {
    let shape = FireworkShape::read(r)?;
    let mut colors = Vec::new();
    for _ in 0..b.count(r)? {
        colors.push(r.i32()?);
    }
    let mut fade_colors = Vec::new();
    for _ in 0..b.count(r)? {
        fade_colors.push(r.i32()?);
    }
    Ok(FireworkExplosion {
        shape,
        colors,
        fade_colors,
        has_trail: r.bool()?,
        has_twinkle: r.bool()?,
    })
}
fn write_explosion(value: &FireworkExplosion, w: &mut Writer, b: &mut Budget) -> Result<()> {
    w.var_i32(value.shape as i32);
    b.write_count(value.colors.len(), w)?;
    for color in &value.colors {
        w.i32(*color);
        b.check_bytes(w)?;
    }
    b.write_count(value.fade_colors.len(), w)?;
    for color in &value.fade_colors {
        w.i32(*color);
        b.check_bytes(w)?;
    }
    w.bool(value.has_trail);
    w.bool(value.has_twinkle);
    b.check_bytes(w)
}

pub(super) fn read(
    r: &mut Reader<'_>,
    wire: ComponentWire,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<ComponentValue> {
    use ComponentValue as V;
    use ComponentWire as W;
    Ok(match wire {
        W::ItemTemplate => V::ItemTemplate(Box::new(read_template(r, version, b, depth + 1)?)),
        W::ItemTemplates(maximum) => {
            let mut items = Vec::new();
            for _ in 0..limited_count(r, b, maximum)? {
                items.push(read_template(r, version, b, depth + 1)?);
            }
            V::ItemTemplates(items)
        }
        W::OptionalItemTemplates(maximum) => {
            let mut items = Vec::new();
            for _ in 0..limited_count(r, b, maximum)? {
                items.push(optional(r, |r| read_template(r, version, b, depth + 1))?);
            }
            V::OptionalItemTemplates(items)
        }
        W::Food | W::FoodLegacy => V::Food(read_food(r, version, b, depth)?),
        W::PotionContents => {
            let potion_id = optional(r, |r| nonnegative(r.var_i32()?, "potion ID"))?;
            let custom_color = optional(r, |r| r.i32())?;
            let mut custom_effects = Vec::new();
            for _ in 0..b.count(r)? {
                custom_effects.push(read_effect(r, b, depth)?);
            }
            let custom_name = if version.protocol() >= 768 {
                optional(r, |r| Ok(r.string(32767)?.into()))?
            } else {
                None
            };
            V::PotionContents(PotionContents {
                potion_id,
                custom_color,
                custom_effects,
                custom_name,
            })
        }
        W::StewEffects => {
            let mut effects = Vec::new();
            for _ in 0..b.count(r)? {
                effects.push(StewEffect {
                    id: nonnegative(r.var_i32()?, "mob effect ID")?,
                    duration: r.var_i32()?,
                });
            }
            V::StewEffects(effects)
        }
        W::WritableBook => {
            let mut pages = Vec::new();
            for _ in 0..limited_count(r, b, 100)? {
                pages.push(read_filterable_string(r, 1024)?);
            }
            V::WritableBook(pages)
        }
        W::WrittenBook => {
            let title = read_filterable_string(r, 32)?;
            let author = r.string(32767)?.into();
            let generation = r.var_i32()?;
            let mut pages = Vec::new();
            for _ in 0..b.count(r)? {
                pages.push(Filterable {
                    raw: required_nbt(r, b)?,
                    filtered: optional(r, |r| required_nbt(r, b))?,
                });
            }
            V::WrittenBook(WrittenBook {
                title,
                author,
                generation,
                pages,
                resolved: r.bool()?,
            })
        }
        W::AttributeModifiers => V::AttributeModifiers(read_attributes(r, version, b)?),
        W::LodestoneTracker => V::LodestoneTracker(LodestoneTracker {
            target: optional(r, |r| {
                Ok(LodestoneTarget {
                    dimension: r.string(32767)?.into(),
                    position: BlockPosition::unpack(r.i64()?),
                })
            })?,
            tracked: r.bool()?,
        }),
        W::FireworkExplosion => V::FireworkExplosion(read_explosion(r, b)?),
        W::Fireworks => {
            let flight_duration = r.var_i32()?;
            let mut explosions = Vec::new();
            for _ in 0..limited_count(r, b, 256)? {
                explosions.push(read_explosion(r, b)?);
            }
            V::Fireworks(Fireworks {
                flight_duration,
                explosions,
            })
        }
        W::Bees => {
            let mut bees = Vec::new();
            for _ in 0..b.count(r)? {
                let entity_type = if version.protocol() >= 773 {
                    Some(nonnegative(r.var_i32()?, "bee entity type")?)
                } else {
                    None
                };
                let entity_data = required_nbt(r, b)?;
                if !matches!(&entity_data.root, Tag::Compound(_)) {
                    return Err(Error::Invalid("bee entity NBT compound"));
                }
                bees.push(BeeOccupant {
                    entity_type,
                    entity_data,
                    ticks_in_hive: r.var_i32()?,
                    minimum_ticks_in_hive: r.var_i32()?,
                });
            }
            V::Bees(bees)
        }
        W::Tool => {
            let mut rules = Vec::new();
            for _ in 0..b.count(r)? {
                rules.push(ToolRule {
                    blocks: read_holder_set(r, b)?,
                    speed: optional(r, |r| r.f32())?,
                    correct_for_drops: optional(r, |r| r.bool())?,
                });
            }
            let default_mining_speed = r.f32()?;
            let damage_per_block = r.var_i32()?;
            let can_destroy_blocks_in_creative = if version.protocol() >= 770 {
                Some(r.bool()?)
            } else {
                None
            };
            V::Tool(Tool {
                rules,
                default_mining_speed,
                damage_per_block,
                can_destroy_blocks_in_creative,
            })
        }
        W::Repairable => V::Repairable(read_holder_set(r, b)?),
        _ => return Err(Error::Invalid("ordinary component wire layout")),
    })
}
pub(super) fn write(
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
        (W::ItemTemplate, V::ItemTemplate(item)) => write_template(item, w, version, b, depth + 1)?,
        (W::ItemTemplates(maximum), V::ItemTemplates(items)) => {
            write_limited_count(items.len(), w, b, maximum)?;
            for item in items {
                write_template(item, w, version, b, depth + 1)?;
            }
        }
        (W::OptionalItemTemplates(maximum), V::OptionalItemTemplates(items)) => {
            write_limited_count(items.len(), w, b, maximum)?;
            for item in items {
                write_optional(item.as_ref(), w, b, |item, w, b| {
                    write_template(item, w, version, b, depth + 1)
                })?;
            }
        }
        (W::Food | W::FoodLegacy, V::Food(value)) => write_food(value, w, version, b, depth)?,
        (W::PotionContents, V::PotionContents(value)) => {
            if version.protocol() < 768 && value.custom_name.is_some() {
                return Err(Error::Invalid("potion custom name version"));
            }
            write_optional(value.potion_id.as_ref(), w, b, |x, w, _| {
                w.var_i32(nonnegative(*x, "potion ID")?);
                Ok(())
            })?;
            write_optional(value.custom_color.as_ref(), w, b, |x, w, _| {
                w.i32(*x);
                Ok(())
            })?;
            b.write_count(value.custom_effects.len(), w)?;
            for effect in &value.custom_effects {
                write_effect(effect, w, b, depth)?;
            }
            if version.protocol() >= 768 {
                write_optional(value.custom_name.as_ref(), w, b, |x, w, b| string(w, x, b))?;
            }
        }
        (W::StewEffects, V::StewEffects(value)) => {
            b.write_count(value.len(), w)?;
            for effect in value {
                w.var_i32(nonnegative(effect.id, "mob effect ID")?);
                w.var_i32(effect.duration);
                b.check_bytes(w)?;
            }
        }
        (W::WritableBook, V::WritableBook(pages)) => {
            write_limited_count(pages.len(), w, b, 100)?;
            for page in pages {
                write_filterable_string(page, w, b, 1024)?;
            }
        }
        (W::WrittenBook, V::WrittenBook(value)) => {
            write_filterable_string(&value.title, w, b, 32)?;
            string(w, &value.author, b)?;
            w.var_i32(value.generation);
            b.write_count(value.pages.len(), w)?;
            for page in &value.pages {
                write_nbt(Some(&page.raw), w, RootFormat::Anonymous, b)?;
                write_optional(page.filtered.as_ref(), w, b, |x, w, b| {
                    write_nbt(Some(x), w, RootFormat::Anonymous, b)
                })?;
            }
            w.bool(value.resolved);
        }
        (W::AttributeModifiers, V::AttributeModifiers(value)) => {
            write_attributes(value, w, version, b)?
        }
        (W::LodestoneTracker, V::LodestoneTracker(value)) => {
            write_optional(value.target.as_ref(), w, b, |x, w, b| {
                string(w, &x.dimension, b)?;
                w.i64(x.position.pack()?);
                Ok(())
            })?;
            w.bool(value.tracked);
        }
        (W::FireworkExplosion, V::FireworkExplosion(value)) => write_explosion(value, w, b)?,
        (W::Fireworks, V::Fireworks(value)) => {
            w.var_i32(value.flight_duration);
            write_limited_count(value.explosions.len(), w, b, 256)?;
            for explosion in &value.explosions {
                write_explosion(explosion, w, b)?;
            }
        }
        (W::Bees, V::Bees(value)) => {
            b.write_count(value.len(), w)?;
            for bee in value {
                if bee.entity_type.is_some() != (version.protocol() >= 773) {
                    return Err(Error::Invalid("bee entity type version"));
                }
                if let Some(id) = bee.entity_type {
                    w.var_i32(nonnegative(id, "bee entity type")?);
                }
                if !matches!(&bee.entity_data.root, Tag::Compound(_)) {
                    return Err(Error::Invalid("bee entity NBT compound"));
                }
                write_nbt(Some(&bee.entity_data), w, RootFormat::Anonymous, b)?;
                w.var_i32(bee.ticks_in_hive);
                w.var_i32(bee.minimum_ticks_in_hive);
                b.check_bytes(w)?;
            }
        }
        (W::Tool, V::Tool(value)) => {
            if value.can_destroy_blocks_in_creative.is_some() != (version.protocol() >= 770) {
                return Err(Error::Invalid("tool creative flag version"));
            }
            b.write_count(value.rules.len(), w)?;
            for rule in &value.rules {
                write_holder_set(&rule.blocks, w, b)?;
                write_optional(rule.speed.as_ref(), w, b, |x, w, _| {
                    w.f32(*x);
                    Ok(())
                })?;
                write_optional(rule.correct_for_drops.as_ref(), w, b, |x, w, _| {
                    w.bool(*x);
                    Ok(())
                })?;
            }
            w.f32(value.default_mining_speed);
            w.var_i32(value.damage_per_block);
            if let Some(flag) = value.can_destroy_blocks_in_creative {
                w.bool(flag);
            }
        }
        (W::Repairable, V::Repairable(value)) => write_holder_set(value, w, b)?,
        _ => {
            return Err(Error::Invalid(
                "component value does not match release layout",
            ))
        }
    }
    b.check_bytes(w)
}
