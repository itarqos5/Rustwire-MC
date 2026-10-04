//! Bounded clientbound recipe displays for protocols 768–776.
//!
//! These are wire descriptions, not recipe execution or registry resolution.
//! Unknown unframed display kinds/components return `Unsupported`; the connection
//! keeps the entire raw packet. Recipe declarations are outside this module.
//! See `docs/recipe-display-wire-audit.md` for the pinned-schema discrepancy at 776.
use super::{
    entity_metadata::holders::RegistryHolder,
    inventory::{self, Budget, ItemStack, Slot, TrimPattern},
};
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    nbt::RootFormat,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

/// The smithing-trim pattern changed wire domain at protocol 770.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayTrimPattern {
    /// Protocols 768–769: another recursive slot display.
    Display(Box<SlotDisplay>),
    /// Protocols 770–776: shifted registry ID or inline trim pattern.
    Holder(RegistryHolder<TrimPattern>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlotDisplay {
    Empty,
    AnyFuel,
    Item(i32),
    /// Protocols 768–774. The slot count precedes the item ID.
    ItemStack(Slot),
    /// Protocols 775–776. ID precedes count; zero still carries the patch.
    ItemTemplate(ItemStack),
    Tag(String),
    SmithingTrim {
        base: Box<Self>,
        material: Box<Self>,
        pattern: DisplayTrimPattern,
    },
    WithRemainder {
        input: Box<Self>,
        remainder: Box<Self>,
    },
    Composite(Vec<Self>),
    /// Added at 775.
    WithAnyPotion(Box<Self>),
    /// Added at 775. A component registry ID, without a component payload.
    OnlyWithComponent {
        source: Box<Self>,
        component_id: i32,
    },
    /// Added at 775 (the 775 schema labels this `dyed_slot_demo`).
    Dyed {
        dye: Box<Self>,
        target: Box<Self>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecipeDisplay {
    Shapeless {
        ingredients: Vec<SlotDisplay>,
        result: SlotDisplay,
        station: SlotDisplay,
    },
    Shaped {
        width: i32,
        height: i32,
        ingredients: Vec<SlotDisplay>,
        result: SlotDisplay,
        station: SlotDisplay,
    },
    Furnace {
        ingredient: SlotDisplay,
        fuel: SlotDisplay,
        result: SlotDisplay,
        station: SlotDisplay,
        duration: i32,
        experience: f32,
    },
    Stonecutter {
        ingredient: SlotDisplay,
        result: SlotDisplay,
        station: SlotDisplay,
    },
    Smithing {
        template: SlotDisplay,
        base: SlotDisplay,
        addition: SlotDisplay,
        result: SlotDisplay,
        station: SlotDisplay,
    },
}

/// Holder-set ingredient: a named item tag or explicit item registry IDs.
pub use super::entity_metadata::holders::RegistryHolderSet as DisplayIngredient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RecipeBookCategory {
    CraftingBuildingBlocks = 0,
    CraftingRedstone = 1,
    CraftingEquipment = 2,
    CraftingMisc = 3,
    FurnaceFood = 4,
    FurnaceBlocks = 5,
    FurnaceMisc = 6,
    BlastFurnaceBlocks = 7,
    BlastFurnaceMisc = 8,
    SmokerFood = 9,
    Stonecutter = 10,
    Smithing = 11,
    Campfire = 12,
    /// Unknown framed registry scalar, preserved without guessing a category.
    Unknown(i32),
}
impl RecipeBookCategory {
    pub fn id(self) -> i32 {
        match self {
            Self::CraftingBuildingBlocks => 0,
            Self::CraftingRedstone => 1,
            Self::CraftingEquipment => 2,
            Self::CraftingMisc => 3,
            Self::FurnaceFood => 4,
            Self::FurnaceBlocks => 5,
            Self::FurnaceMisc => 6,
            Self::BlastFurnaceBlocks => 7,
            Self::BlastFurnaceMisc => 8,
            Self::SmokerFood => 9,
            Self::Stonecutter => 10,
            Self::Smithing => 11,
            Self::Campfire => 12,
            Self::Unknown(id) => id,
        }
    }
    pub fn from_id(id: i32) -> Result<Self> {
        Ok(match id {
            0 => Self::CraftingBuildingBlocks,
            1 => Self::CraftingRedstone,
            2 => Self::CraftingEquipment,
            3 => Self::CraftingMisc,
            4 => Self::FurnaceFood,
            5 => Self::FurnaceBlocks,
            6 => Self::FurnaceMisc,
            7 => Self::BlastFurnaceBlocks,
            8 => Self::BlastFurnaceMisc,
            9 => Self::SmokerFood,
            10 => Self::Stonecutter,
            11 => Self::Smithing,
            12 => Self::Campfire,
            _ => Self::Unknown(id),
        })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeBookEntry {
    pub display_id: i32,
    pub display: RecipeDisplay,
    /// Zero wire marker means absent; other signed markers decode with wrapping
    /// subtraction. `Some(-1)` is unrepresentable and rejected when encoding.
    pub group: Option<i32>,
    pub category: RecipeBookCategory,
    pub crafting_requirements: Option<Vec<DisplayIngredient>>,
    /// Raw byte, preserving reserved bits. Bit 0: notification; bit 1: highlight.
    pub flags: u8,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeBookAdd {
    pub entries: Vec<RecipeBookEntry>,
    pub replace: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct CraftRecipeResponse {
    pub window_id: i32,
    pub display: RecipeDisplay,
}
#[derive(Debug, Clone, PartialEq)]
pub enum RecipeDisplayPacket {
    Add(RecipeBookAdd),
    Response(Box<CraftRecipeResponse>),
}

fn available(v: Version) -> Result<()> {
    match v.protocol() {
        768..=776 => Ok(()),
        _ => Err(Error::Unsupported("recipe display version")),
    }
}
fn reader(bytes: &[u8], v: Version, limits: Limits) -> Result<Reader<'_>> {
    available(v)?;
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("recipe display packet bytes"));
    }
    Ok(Reader::new(bytes, limits))
}
fn depth(b: &Budget, n: usize) -> Result<()> {
    if n > b.limits.max_nbt_depth.min(64) {
        Err(Error::Limit("recipe display depth"))
    } else {
        Ok(())
    }
}
fn preflight(w: &Writer, n: usize, b: &Budget) -> Result<()> {
    if n > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        Err(Error::Limit("recipe display packet bytes"))
    } else {
        b.check_bytes(w)
    }
}
fn scalar(w: &mut Writer, value: i32, b: &Budget) -> Result<()> {
    let mut x = value as u32;
    let mut len = 1;
    while x > 127 {
        len += 1;
        x >>= 7;
    }
    preflight(w, len, b)?;
    w.var_i32(value);
    Ok(())
}
fn nonnegative(n: i32) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid("negative recipe registry ID"))
    } else {
        Ok(n)
    }
}
fn read_key(r: &mut Reader<'_>, v: Version) -> Result<String> {
    let s = r.string(32767)?;
    identifier::validate(s, v)?;
    Ok(s.into())
}
fn write_key(s: &str, w: &mut Writer, v: Version, b: &Budget) -> Result<()> {
    identifier::validate(s, v)?;
    if s.len() > 32767.min(b.limits.max_string_chars) {
        return Err(Error::Limit("recipe display identifier"));
    }
    scalar(w, s.len() as i32, b)?;
    preflight(w, s.len(), b)?;
    w.raw(s.as_bytes());
    Ok(())
}
fn count(r: &mut Reader<'_>, b: &mut Budget, minimum: usize) -> Result<usize> {
    let n = b.count(r)?;
    if n > r.remaining().len() / minimum {
        return Err(Error::Eof);
    }
    Ok(n)
}
fn write_count(n: usize, w: &mut Writer, b: &mut Budget, minimum: usize) -> Result<()> {
    if n > i32::MAX as usize {
        return Err(Error::Limit("recipe display collection"));
    }
    b.charge(n)?;
    scalar(w, n as i32, b)?;
    preflight(
        w,
        n.checked_mul(minimum)
            .ok_or(Error::Limit("recipe display collection bytes"))?,
        b,
    )
}
fn read_slots(
    r: &mut Reader<'_>,
    v: Version,
    b: &mut Budget,
    d: usize,
) -> Result<Vec<SlotDisplay>> {
    let n = count(r, b, 1)?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(read_slot(r, v, b, d)?);
    }
    Ok(out)
}
fn write_slots(
    values: &[SlotDisplay],
    w: &mut Writer,
    v: Version,
    b: &mut Budget,
    d: usize,
) -> Result<()> {
    write_count(values.len(), w, b, 1)?;
    for value in values {
        write_slot(value, w, v, b, d)?;
    }
    Ok(())
}

// Leaf codecs retain this packet's aggregate element/NBT budgets. Reduce their
// recursion allowance by the enclosing display depth instead of resetting it.
fn read_item(r: &mut Reader<'_>, v: Version, b: &mut Budget, d: usize) -> Result<SlotDisplay> {
    depth(b, d)?;
    let old_b = b.limits.max_nbt_depth;
    let old_r = r.limits.max_nbt_depth;
    let remaining = old_b.min(64) - d;
    b.limits.max_nbt_depth = remaining;
    r.limits.max_nbt_depth = old_r.min(remaining);
    let result = if v.protocol() >= 775 {
        inventory::read_template(r, v, b, 0).map(SlotDisplay::ItemTemplate)
    } else {
        inventory::read_slot(r, v, b, 0).map(SlotDisplay::ItemStack)
    };
    b.limits.max_nbt_depth = old_b;
    r.limits.max_nbt_depth = old_r;
    result
}
fn write_item(
    value: &SlotDisplay,
    w: &mut Writer,
    v: Version,
    b: &mut Budget,
    d: usize,
) -> Result<()> {
    depth(b, d)?;
    let old = b.limits.max_nbt_depth;
    b.limits.max_nbt_depth = old.min(64) - d;
    let result = match value {
        SlotDisplay::ItemStack(item) if v.protocol() < 775 => {
            inventory::write_slot(item, w, v, b, 0)
        }
        SlotDisplay::ItemTemplate(item) if v.protocol() >= 775 => {
            inventory::write_template(item, w, v, b, 0)
        }
        _ => Err(Error::Invalid("display stack/template version")),
    };
    b.limits.max_nbt_depth = old;
    result
}
fn read_pattern(
    r: &mut Reader<'_>,
    v: Version,
    b: &mut Budget,
    d: usize,
) -> Result<RegistryHolder<TrimPattern>> {
    depth(b, d)?;
    b.charge(1)?;
    let marker = nonnegative(r.var_i32()?)?;
    if marker != 0 {
        return Ok(RegistryHolder::RegistryId(marker - 1));
    }
    let asset_id = read_key(r, v)?;
    let old = r.limits.max_nbt_depth;
    r.limits.max_nbt_depth = old.min(b.limits.max_nbt_depth.min(64) - d);
    let description = inventory::read_nbt(r, RootFormat::Anonymous, b);
    r.limits.max_nbt_depth = old;
    Ok(RegistryHolder::Inline(TrimPattern {
        asset_id,
        template_item_id: None,
        description: description?.ok_or(Error::Invalid("missing trim description"))?,
        decal: r.bool()?,
    }))
}
fn write_pattern(
    value: &RegistryHolder<TrimPattern>,
    w: &mut Writer,
    v: Version,
    b: &mut Budget,
    d: usize,
) -> Result<()> {
    depth(b, d)?;
    b.charge(1)?;
    match value {
        RegistryHolder::RegistryId(id) => scalar(
            w,
            nonnegative(*id)?
                .checked_add(1)
                .ok_or(Error::Invalid("trim holder ID overflow"))?,
            b,
        ),
        RegistryHolder::Inline(value) => {
            if value.template_item_id.is_some() {
                return Err(Error::Invalid("display trim pattern version"));
            }
            scalar(w, 0, b)?;
            write_key(&value.asset_id, w, v, b)?;
            let old = b.limits.max_nbt_depth;
            b.limits.max_nbt_depth = old.min(64) - d;
            let result =
                inventory::write_nbt(Some(&value.description), w, RootFormat::Anonymous, b);
            b.limits.max_nbt_depth = old;
            result?;
            preflight(w, 1, b)?;
            w.bool(value.decal);
            Ok(())
        }
    }
}
pub(crate) fn read_slot(
    r: &mut Reader<'_>,
    v: Version,
    b: &mut Budget,
    d: usize,
) -> Result<SlotDisplay> {
    depth(b, d)?;
    b.charge(1)?;
    let id = r.var_i32()?;
    // Exact registry ordering: 768–774 and 775–776 have different numeric IDs.
    let kind = match (v.protocol(), id) {
        (_, 0) => 0,
        (_, 1) => 1,
        (768..=774, 2..=7) => id,
        (775..=776, 2) => 8,
        (775..=776, 3) => 9,
        (775..=776, 4..=6) => id - 2,
        (775..=776, 7) => 10,
        (775..=776, 8..=10) => id - 3,
        _ => return Err(Error::Unsupported("slot display kind")),
    };
    Ok(match kind {
        0 => SlotDisplay::Empty,
        1 => SlotDisplay::AnyFuel,
        2 => SlotDisplay::Item(nonnegative(r.var_i32()?)?),
        3 => read_item(r, v, b, d + 1)?,
        4 => SlotDisplay::Tag(read_key(r, v)?),
        5 => SlotDisplay::SmithingTrim {
            base: Box::new(read_slot(r, v, b, d + 1)?),
            material: Box::new(read_slot(r, v, b, d + 1)?),
            pattern: if v.protocol() < 770 {
                DisplayTrimPattern::Display(Box::new(read_slot(r, v, b, d + 1)?))
            } else {
                DisplayTrimPattern::Holder(read_pattern(r, v, b, d + 1)?)
            },
        },
        6 => SlotDisplay::WithRemainder {
            input: Box::new(read_slot(r, v, b, d + 1)?),
            remainder: Box::new(read_slot(r, v, b, d + 1)?),
        },
        7 => SlotDisplay::Composite(read_slots(r, v, b, d + 1)?),
        8 => SlotDisplay::WithAnyPotion(Box::new(read_slot(r, v, b, d + 1)?)),
        9 => SlotDisplay::OnlyWithComponent {
            source: Box::new(read_slot(r, v, b, d + 1)?),
            component_id: nonnegative(r.var_i32()?)?,
        },
        10 => SlotDisplay::Dyed {
            dye: Box::new(read_slot(r, v, b, d + 1)?),
            target: Box::new(read_slot(r, v, b, d + 1)?),
        },
        _ => unreachable!(),
    })
}
pub(crate) fn write_slot(
    value: &SlotDisplay,
    w: &mut Writer,
    v: Version,
    b: &mut Budget,
    d: usize,
) -> Result<()> {
    depth(b, d)?;
    b.charge(1)?;
    let old = v.protocol() < 775;
    let id = match value {
        SlotDisplay::Empty => 0,
        SlotDisplay::AnyFuel => 1,
        SlotDisplay::Item(_) => {
            if old {
                2
            } else {
                4
            }
        }
        SlotDisplay::ItemStack(_) | SlotDisplay::ItemTemplate(_) => {
            if old {
                3
            } else {
                5
            }
        }
        SlotDisplay::Tag(_) => {
            if old {
                4
            } else {
                6
            }
        }
        SlotDisplay::SmithingTrim { .. } => {
            if old {
                5
            } else {
                8
            }
        }
        SlotDisplay::WithRemainder { .. } => {
            if old {
                6
            } else {
                9
            }
        }
        SlotDisplay::Composite(_) => {
            if old {
                7
            } else {
                10
            }
        }
        SlotDisplay::WithAnyPotion(_) if !old => 2,
        SlotDisplay::OnlyWithComponent { .. } if !old => 3,
        SlotDisplay::Dyed { .. } if !old => 7,
        _ => return Err(Error::Invalid("slot display kind version")),
    };
    scalar(w, id, b)?;
    match value {
        SlotDisplay::Empty | SlotDisplay::AnyFuel => {}
        SlotDisplay::Item(id) => scalar(w, nonnegative(*id)?, b)?,
        SlotDisplay::ItemStack(_) | SlotDisplay::ItemTemplate(_) => {
            write_item(value, w, v, b, d + 1)?
        }
        SlotDisplay::Tag(tag) => write_key(tag, w, v, b)?,
        SlotDisplay::SmithingTrim {
            base,
            material,
            pattern,
        } => {
            write_slot(base, w, v, b, d + 1)?;
            write_slot(material, w, v, b, d + 1)?;
            match pattern {
                DisplayTrimPattern::Display(value) if v.protocol() < 770 => {
                    write_slot(value, w, v, b, d + 1)?
                }
                DisplayTrimPattern::Holder(value) if v.protocol() >= 770 => {
                    write_pattern(value, w, v, b, d + 1)?
                }
                _ => return Err(Error::Invalid("smithing display pattern version")),
            }
        }
        SlotDisplay::WithRemainder { input, remainder } => {
            write_slot(input, w, v, b, d + 1)?;
            write_slot(remainder, w, v, b, d + 1)?;
        }
        SlotDisplay::Composite(values) => write_slots(values, w, v, b, d + 1)?,
        SlotDisplay::WithAnyPotion(value) => write_slot(value, w, v, b, d + 1)?,
        SlotDisplay::OnlyWithComponent {
            source,
            component_id,
        } => {
            write_slot(source, w, v, b, d + 1)?;
            scalar(w, nonnegative(*component_id)?, b)?;
        }
        SlotDisplay::Dyed { dye, target } => {
            write_slot(dye, w, v, b, d + 1)?;
            write_slot(target, w, v, b, d + 1)?;
        }
    }
    Ok(())
}
fn read_display(r: &mut Reader<'_>, v: Version, b: &mut Budget) -> Result<RecipeDisplay> {
    b.charge(1)?;
    depth(b, 0)?;
    Ok(match r.var_i32()? {
        0 => RecipeDisplay::Shapeless {
            ingredients: read_slots(r, v, b, 1)?,
            result: read_slot(r, v, b, 1)?,
            station: read_slot(r, v, b, 1)?,
        },
        1 => RecipeDisplay::Shaped {
            width: r.var_i32()?,
            height: r.var_i32()?,
            ingredients: read_slots(r, v, b, 1)?,
            result: read_slot(r, v, b, 1)?,
            station: read_slot(r, v, b, 1)?,
        },
        2 => RecipeDisplay::Furnace {
            ingredient: read_slot(r, v, b, 1)?,
            fuel: read_slot(r, v, b, 1)?,
            result: read_slot(r, v, b, 1)?,
            station: read_slot(r, v, b, 1)?,
            duration: r.var_i32()?,
            experience: r.f32()?,
        },
        3 => RecipeDisplay::Stonecutter {
            ingredient: read_slot(r, v, b, 1)?,
            result: read_slot(r, v, b, 1)?,
            station: read_slot(r, v, b, 1)?,
        },
        4 => RecipeDisplay::Smithing {
            template: read_slot(r, v, b, 1)?,
            base: read_slot(r, v, b, 1)?,
            addition: read_slot(r, v, b, 1)?,
            result: read_slot(r, v, b, 1)?,
            station: read_slot(r, v, b, 1)?,
        },
        _ => return Err(Error::Unsupported("recipe display kind")),
    })
}
fn write_display(value: &RecipeDisplay, w: &mut Writer, v: Version, b: &mut Budget) -> Result<()> {
    b.charge(1)?;
    depth(b, 0)?;
    match value {
        RecipeDisplay::Shapeless {
            ingredients,
            result,
            station,
        } => {
            scalar(w, 0, b)?;
            write_slots(ingredients, w, v, b, 1)?;
            write_slot(result, w, v, b, 1)?;
            write_slot(station, w, v, b, 1)?;
        }
        RecipeDisplay::Shaped {
            width,
            height,
            ingredients,
            result,
            station,
        } => {
            scalar(w, 1, b)?;
            scalar(w, *width, b)?;
            scalar(w, *height, b)?;
            write_slots(ingredients, w, v, b, 1)?;
            write_slot(result, w, v, b, 1)?;
            write_slot(station, w, v, b, 1)?;
        }
        RecipeDisplay::Furnace {
            ingredient,
            fuel,
            result,
            station,
            duration,
            experience,
        } => {
            scalar(w, 2, b)?;
            for slot in [ingredient, fuel, result, station] {
                write_slot(slot, w, v, b, 1)?;
            }
            scalar(w, *duration, b)?;
            preflight(w, 4, b)?;
            w.f32(*experience);
        }
        RecipeDisplay::Stonecutter {
            ingredient,
            result,
            station,
        } => {
            scalar(w, 3, b)?;
            for slot in [ingredient, result, station] {
                write_slot(slot, w, v, b, 1)?;
            }
        }
        RecipeDisplay::Smithing {
            template,
            base,
            addition,
            result,
            station,
        } => {
            scalar(w, 4, b)?;
            for slot in [template, base, addition, result, station] {
                write_slot(slot, w, v, b, 1)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn read_ingredient(
    r: &mut Reader<'_>,
    v: Version,
    b: &mut Budget,
) -> Result<DisplayIngredient> {
    let marker = nonnegative(r.var_i32()?)?;
    if marker == 0 {
        return Ok(DisplayIngredient::Tag(read_key(r, v)?));
    }
    let n = (marker - 1) as usize;
    b.charge(n)?;
    if n > r.remaining().len() {
        return Err(Error::Eof);
    }
    let mut ids = Vec::with_capacity(n);
    for _ in 0..n {
        ids.push(nonnegative(r.var_i32()?)?);
    }
    Ok(DisplayIngredient::Ids(ids))
}
pub(crate) fn write_ingredient(
    value: &DisplayIngredient,
    w: &mut Writer,
    v: Version,
    b: &mut Budget,
) -> Result<()> {
    match value {
        DisplayIngredient::Tag(tag) => {
            scalar(w, 0, b)?;
            write_key(tag, w, v, b)
        }
        DisplayIngredient::Ids(ids) => {
            if ids.len() >= i32::MAX as usize {
                return Err(Error::Limit("ingredient IDs"));
            }
            b.charge(ids.len())?;
            scalar(w, ids.len() as i32 + 1, b)?;
            preflight(w, ids.len(), b)?;
            for id in ids {
                scalar(w, nonnegative(*id)?, b)?;
            }
            Ok(())
        }
    }
}
trait Body: Sized {
    const NAME: &'static str;
    fn read(r: &mut Reader<'_>, v: Version, b: &mut Budget) -> Result<Self>;
    fn write(&self, w: &mut Writer, v: Version, b: &mut Budget) -> Result<()>;
}
impl Body for CraftRecipeResponse {
    const NAME: &'static str = "craft_recipe_response";
    fn read(r: &mut Reader<'_>, v: Version, b: &mut Budget) -> Result<Self> {
        Ok(Self {
            window_id: r.var_i32()?,
            display: read_display(r, v, b)?,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, b: &mut Budget) -> Result<()> {
        scalar(w, self.window_id, b)?;
        write_display(&self.display, w, v, b)
    }
}
impl Body for RecipeBookAdd {
    const NAME: &'static str = "recipe_book_add";
    fn read(r: &mut Reader<'_>, v: Version, b: &mut Budget) -> Result<Self> {
        // A minimal recipe display takes four bytes; the remaining fields five.
        let n = count(r, b, 9)?;
        let mut entries = Vec::new();
        for _ in 0..n {
            let display_id = r.var_i32()?;
            let display = read_display(r, v, b)?;
            let marker = r.var_i32()?;
            let group = (marker != 0).then(|| marker.wrapping_sub(1));
            let category = RecipeBookCategory::from_id(r.var_i32()?)?;
            let crafting_requirements = if r.bool()? {
                let n = count(r, b, 1)?;
                let mut values = Vec::new();
                for _ in 0..n {
                    values.push(read_ingredient(r, v, b)?);
                }
                Some(values)
            } else {
                None
            };
            entries.push(RecipeBookEntry {
                display_id,
                display,
                group,
                category,
                crafting_requirements,
                flags: r.u8()?,
            });
        }
        Ok(Self {
            entries,
            replace: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, v: Version, b: &mut Budget) -> Result<()> {
        write_count(self.entries.len(), w, b, 9)?;
        for entry in &self.entries {
            scalar(w, entry.display_id, b)?;
            write_display(&entry.display, w, v, b)?;
            let marker = match entry.group {
                Some(-1) => return Err(Error::Invalid("unrepresentable recipe group")),
                Some(id) => id.wrapping_add(1),
                None => 0,
            };
            scalar(w, marker, b)?;
            scalar(w, entry.category.id(), b)?;
            preflight(w, 1, b)?;
            w.bool(entry.crafting_requirements.is_some());
            if let Some(values) = &entry.crafting_requirements {
                write_count(values.len(), w, b, 1)?;
                for value in values {
                    write_ingredient(value, w, v, b)?;
                }
            }
            preflight(w, 1, b)?;
            w.u8(entry.flags);
        }
        preflight(w, 1, b)?;
        w.bool(self.replace);
        Ok(())
    }
}
macro_rules! packet {
    ($ty:ty) => {
        impl $ty {
            pub fn decode(bytes: &[u8], v: Version, limits: Limits) -> Result<Self> {
                v.packet_id(State::Play, Direction::Clientbound, Self::NAME)?;
                let mut r = reader(bytes, v, limits)?;
                let value = Self::read(&mut r, v, &mut Budget::new(limits))?;
                r.finish()?;
                Ok(value)
            }
            /// Atomic: no partial body is returned on error.
            pub fn encode(&self, v: Version, limits: Limits) -> Result<Vec<u8>> {
                available(v)?;
                v.packet_id(State::Play, Direction::Clientbound, Self::NAME)?;
                let mut w = Writer::new();
                let mut b = Budget::new(limits);
                self.write(&mut w, v, &mut b)?;
                b.check_bytes(&w)?;
                Ok(w.into_inner())
            }
            pub fn packet(&self, v: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    v.packet_id(State::Play, Direction::Clientbound, Self::NAME)?,
                    self.encode(v, limits)?,
                ))
            }
        }
    };
}
packet!(RecipeBookAdd);
packet!(CraftRecipeResponse);
macro_rules! payload {
    ($ty:ty, $read:ident, $write:ident $(, $depth:expr)?) => {
        impl $ty {
            pub fn decode(bytes: &[u8], v: Version, limits: Limits) -> Result<Self> {
                let mut r = reader(bytes, v, limits)?;
                let value = $read(&mut r, v, &mut Budget::new(limits) $(, $depth)?)?;
                r.finish()?; Ok(value)
            }
            pub fn encode(&self, v: Version, limits: Limits) -> Result<Vec<u8>> {
                available(v)?; let mut w = Writer::new(); let mut b = Budget::new(limits);
                $write(self, &mut w, v, &mut b $(, $depth)?)?; b.check_bytes(&w)?; Ok(w.into_inner())
            }
        }
    }
}
payload!(SlotDisplay, read_slot, write_slot, 0);
payload!(RecipeDisplay, read_display, write_display);
impl RecipeDisplayPacket {
    pub fn decode(name: &str, bytes: &[u8], v: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "recipe_book_add" => Self::Add(RecipeBookAdd::decode(bytes, v, limits)?),
            "craft_recipe_response" => {
                Self::Response(Box::new(CraftRecipeResponse::decode(bytes, v, limits)?))
            }
            _ => return Err(Error::Unsupported("recipe display packet")),
        })
    }
    pub fn encode(&self, v: Version, limits: Limits) -> Result<Vec<u8>> {
        match self {
            Self::Add(p) => p.encode(v, limits),
            Self::Response(p) => p.encode(v, limits),
        }
    }
    pub fn packet(&self, v: Version, limits: Limits) -> Result<RawPacket> {
        match self {
            Self::Add(p) => p.packet(v, limits),
            Self::Response(p) => p.packet(v, limits),
        }
    }
}
