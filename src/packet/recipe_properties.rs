//! Modern recipe-property and stonecutter declarations (protocols 768–776).
//!
//! These replace the legacy recipe-serializer list on `declare_recipes`. They
//! describe wire data, without registry lookup, crafting, or recipe-book state.
//! All entries share one collection/NBT budget with their recursive displays.
use super::{
    inventory::Budget,
    recipe_display::{self, DisplayIngredient, SlotDisplay},
};
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

/// A named recipe property set. Wire order and duplicates remain observable;
/// this codec does not apply map/set replacement or resolve registry IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipePropertySet {
    pub name: String,
    pub item_ids: Vec<i32>,
}

/// One stonecutter input holder set and its output display.
#[derive(Debug, Clone, PartialEq)]
pub struct StonecutterRecipe {
    pub input: DisplayIngredient,
    pub result: SlotDisplay,
}

/// Clientbound `declare_recipes` from 768. This is intentionally distinct from
/// [`super::recipe_declarations::LegacyDeclareRecipes`] (763–767).
#[derive(Debug, Clone, PartialEq)]
pub struct ModernDeclareRecipes {
    pub property_sets: Vec<RecipePropertySet>,
    pub stonecutter_recipes: Vec<StonecutterRecipe>,
}

fn available(version: Version) -> Result<()> {
    match version.protocol() {
        768..=776 => Ok(()),
        _ => Err(Error::Unsupported("modern recipe properties version")),
    }
}
fn count(r: &mut Reader<'_>, budget: &mut Budget, minimum: usize) -> Result<usize> {
    let count = budget.count(r)?;
    if count > r.remaining().len() / minimum {
        return Err(Error::Eof);
    }
    Ok(count)
}
fn preflight(w: &Writer, additional: usize, budget: &Budget) -> Result<()> {
    if additional > budget.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("recipe properties packet bytes"));
    }
    budget.check_bytes(w)
}
fn varint(w: &mut Writer, value: i32, budget: &Budget) -> Result<()> {
    let mut value_bits = value as u32;
    let mut size = 1;
    while value_bits > 127 {
        size += 1;
        value_bits >>= 7;
    }
    preflight(w, size, budget)?;
    w.var_i32(value);
    Ok(())
}
fn write_count(count: usize, w: &mut Writer, budget: &mut Budget, minimum: usize) -> Result<()> {
    if count > i32::MAX as usize {
        return Err(Error::Limit("recipe properties collection"));
    }
    budget.charge(count)?;
    varint(w, count as i32, budget)?;
    preflight(
        w,
        count
            .checked_mul(minimum)
            .ok_or(Error::Limit("recipe properties collection bytes"))?,
        budget,
    )
}
fn item_id(value: i32) -> Result<i32> {
    if value < 0 {
        Err(Error::Invalid("negative property item ID"))
    } else {
        Ok(value)
    }
}
impl ModernDeclareRecipes {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        available(version)?;
        version.packet_id(State::Play, Direction::Clientbound, "declare_recipes")?;
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("recipe properties packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let mut budget = Budget::new(limits);
        let length = count(&mut r, &mut budget, 2)?;
        // Grow only for decoded entries instead of reserving an attacker-supplied
        // capacity for large compound values.
        let mut property_sets = Vec::new();
        for _ in 0..length {
            let name = r.string(32767)?;
            identifier::validate(name, version)?;
            let name = name.to_owned();
            let length = count(&mut r, &mut budget, 1)?;
            let mut item_ids = Vec::with_capacity(length);
            for _ in 0..length {
                item_ids.push(item_id(r.var_i32()?)?);
            }
            property_sets.push(RecipePropertySet { name, item_ids });
        }
        let length = count(&mut r, &mut budget, 2)?;
        let mut stonecutter_recipes = Vec::new();
        for _ in 0..length {
            let input = recipe_display::read_ingredient(&mut r, version, &mut budget)?;
            let result = recipe_display::read_slot(&mut r, version, &mut budget, 0)?;
            stonecutter_recipes.push(StonecutterRecipe { input, result });
        }
        r.finish()?;
        Ok(Self {
            property_sets,
            stonecutter_recipes,
        })
    }
    /// Atomic: returns no partial body on a malformed model or exceeded budget.
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        available(version)?;
        version.packet_id(State::Play, Direction::Clientbound, "declare_recipes")?;
        let mut w = Writer::new();
        let mut budget = Budget::new(limits);
        write_count(self.property_sets.len(), &mut w, &mut budget, 2)?;
        for property in &self.property_sets {
            identifier::validate(&property.name, version)?;
            if property.name.len() > limits.max_string_chars.min(32767) {
                return Err(Error::Limit("recipe property identifier length"));
            }
            varint(&mut w, property.name.len() as i32, &budget)?;
            preflight(&w, property.name.len(), &budget)?;
            w.raw(property.name.as_bytes());
            write_count(property.item_ids.len(), &mut w, &mut budget, 1)?;
            for id in &property.item_ids {
                varint(&mut w, item_id(*id)?, &budget)?;
            }
        }
        write_count(self.stonecutter_recipes.len(), &mut w, &mut budget, 2)?;
        for recipe in &self.stonecutter_recipes {
            recipe_display::write_ingredient(&recipe.input, &mut w, version, &mut budget)?;
            recipe_display::write_slot(&recipe.result, &mut w, version, &mut budget, 0)?;
        }
        budget.check_bytes(&w)?;
        Ok(w.into_inner())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(State::Play, Direction::Clientbound, "declare_recipes")?,
            self.encode(version, limits)?,
        ))
    }
}
