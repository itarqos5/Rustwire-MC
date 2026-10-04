//! Bounded recipe-book control envelopes, without recipe execution or storage.
//!
//! Registry keys through 767 and display IDs from 768 are distinct wire domains.
//! Legacy declarations are provided separately by `recipe_declarations`. Modern
//! displays/additions and ghost responses are outside this control module.
//! See `docs/recipe-control-wire-audit.md`.
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

trait Body: Sized {
    fn available(_version: Version) -> Result<()> {
        Ok(())
    }
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()>;
}
macro_rules! packet {
    ($ty:ident, $direction:ident, $name:literal) => {
        impl $ty {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                version.packet_id(State::Play, Direction::$direction, $name)?;
                Self::available(version)?;
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("recipe packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let result = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(result)
            }
            /// Returns no partial body on error.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                version.packet_id(State::Play, Direction::$direction, $name)?;
                Self::available(version)?;
                let mut w = Writer::new();
                self.write(&mut w, version, limits)?;
                budget(&w, 0, limits)?;
                Ok(w.into_inner())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::$direction, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    };
}
fn budget(w: &Writer, additional: usize, limits: Limits) -> Result<()> {
    if additional > limits.max_packet.saturating_sub(w.as_slice().len())
        || w.as_slice().len() > limits.max_packet
    {
        return Err(Error::Limit("recipe packet bytes"));
    }
    Ok(())
}
fn var_len(value: i32) -> usize {
    let mut n = value as u32;
    let mut len = 1;
    while n > 127 {
        len += 1;
        n >>= 7;
    }
    len
}
fn key_read(r: &mut Reader<'_>, version: Version) -> Result<String> {
    let value = r.string(32767)?;
    identifier::validate(value, version)?;
    Ok(value.to_owned())
}
fn key_write(value: &str, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
    identifier::validate(value, version)?;
    // Valid identifiers are ASCII, so byte length equals UTF-16 character count.
    if value.len() > 32767.min(limits.max_string_chars) {
        return Err(Error::Limit("recipe identifier length"));
    }
    budget(w, value.len() + var_len(value.len() as i32), limits)?;
    w.string(value, 32767.min(limits.max_string_chars))
}
/// Versioned container wire representation. Legacy readers disagree about the
/// signed interpretation of the byte, so this API preserves the byte itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipeWindowId {
    /// Exactly one wire byte through 767; 0x80 and 0xff remain 128 and 255 here.
    LegacyByte(u8),
    /// Signed VarInt from 768, with no container lookup or range clamp.
    VarInt(i32),
}
impl RecipeWindowId {
    pub fn from_legacy_signed(value: i8) -> Self {
        Self::LegacyByte(value as u8)
    }
    pub fn legacy_signed(self) -> Option<i8> {
        match self {
            Self::LegacyByte(value) => Some(value as i8),
            Self::VarInt(_) => None,
        }
    }
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(if version.protocol() < 768 {
            Self::LegacyByte(r.u8()?)
        } else {
            Self::VarInt(r.var_i32()?)
        })
    }
    fn write(self, w: &mut Writer, version: Version) -> Result<()> {
        match self {
            Self::LegacyByte(value) if version.protocol() < 768 => w.u8(value),
            Self::VarInt(value) if version.protocol() >= 768 => w.var_i32(value),
            _ => return Err(Error::Invalid("recipe window ID version")),
        }
        Ok(())
    }
}

/// Wire identity only: no registry lookup or display-ID allocation is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeReference {
    /// Resource-location recipe key through protocol 767.
    RegistryKey(String),
    /// Raw signed VarInt display ID from protocol 768, not a recipe registry key.
    DisplayId(i32),
}
impl RecipeReference {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(if version.protocol() < 768 {
            Self::RegistryKey(key_read(r, version)?)
        } else {
            Self::DisplayId(r.var_i32()?)
        })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        match self {
            Self::RegistryKey(key) if version.protocol() < 768 => {
                key_write(key, w, version, limits)
            }
            Self::DisplayId(id) if version.protocol() >= 768 => {
                w.var_i32(*id);
                Ok(())
            }
            _ => Err(Error::Invalid("recipe reference version")),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftRecipeRequest {
    /// Raw byte through 767, signed VarInt from 768. No menu lookup is done.
    pub window_id: RecipeWindowId,
    pub recipe: RecipeReference,
    pub make_all: bool,
}
impl Body for CraftRecipeRequest {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            window_id: RecipeWindowId::read(r, version)?,
            recipe: RecipeReference::read(r, version)?,
            make_all: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        self.window_id.write(w, version)?;
        self.recipe.write(w, version, limits)?;
        w.bool(self.make_all);
        Ok(())
    }
}
packet!(CraftRecipeRequest, Serverbound, "craft_recipe_request");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayedRecipe {
    pub recipe: RecipeReference,
}
impl Body for DisplayedRecipe {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            recipe: RecipeReference::read(r, version)?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        self.recipe.write(w, version, limits)
    }
}
packet!(DisplayedRecipe, Serverbound, "displayed_recipe");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum RecipeBookKind {
    Crafting = 0,
    Furnace = 1,
    BlastFurnace = 2,
    Smoker = 3,
}
impl RecipeBookKind {
    pub fn from_id(id: i32) -> Result<Self> {
        match id {
            0 => Ok(Self::Crafting),
            1 => Ok(Self::Furnace),
            2 => Ok(Self::BlastFurnace),
            3 => Ok(Self::Smoker),
            _ => Err(Error::Unsupported("recipe book kind")),
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecipeBookSetting {
    pub open: bool,
    pub filtering: bool,
}
impl RecipeBookSetting {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            open: r.bool()?,
            filtering: r.bool()?,
        })
    }
    fn write(self, w: &mut Writer) {
        w.bool(self.open);
        w.bool(self.filtering);
    }
}
/// Serverbound change to one book's settings (`recipe_book`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeBookChangeSettings {
    pub book: RecipeBookKind,
    pub setting: RecipeBookSetting,
}
impl Body for RecipeBookChangeSettings {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let id = r.var_i32()?;
        let setting = RecipeBookSetting::read(r)?;
        r.finish()?; // Malformed bodies must not be disguised as unknown-enum raw fallback.
        Ok(Self {
            book: RecipeBookKind::from_id(id)?,
            setting,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.book as i32);
        self.setting.write(w);
        Ok(())
    }
}
packet!(RecipeBookChangeSettings, Serverbound, "recipe_book");

/// Four settings pairs in crafting, furnace, blast-furnace, smoker wire order.
/// As a standalone clientbound packet this is available from 768. The same body
/// is embedded in legacy `UnlockRecipes` through 767.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecipeBookSettings {
    pub crafting: RecipeBookSetting,
    pub furnace: RecipeBookSetting,
    pub blast_furnace: RecipeBookSetting,
    pub smoker: RecipeBookSetting,
}
impl RecipeBookSettings {
    fn read_fields(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            crafting: RecipeBookSetting::read(r)?,
            furnace: RecipeBookSetting::read(r)?,
            blast_furnace: RecipeBookSetting::read(r)?,
            smoker: RecipeBookSetting::read(r)?,
        })
    }
    fn write_fields(self, w: &mut Writer) {
        for pair in [self.crafting, self.furnace, self.blast_furnace, self.smoker] {
            pair.write(w);
        }
    }
}
impl Body for RecipeBookSettings {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Self::read_fields(r)
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        self.write_fields(w);
        Ok(())
    }
}
packet!(RecipeBookSettings, Clientbound, "recipe_book_settings");

fn count_read(r: &mut Reader<'_>, remaining: &mut usize) -> Result<usize> {
    let n = r.count(*remaining)?;
    if n > r.remaining().len() {
        return Err(Error::Eof);
    }
    *remaining -= n;
    Ok(n)
}
fn count_write(n: usize, w: &mut Writer, remaining: &mut usize, limits: Limits) -> Result<()> {
    if n > *remaining || n > i32::MAX as usize {
        return Err(Error::Limit("recipe collection length"));
    }
    budget(w, var_len(n as i32), limits)?;
    w.var_i32(n as i32);
    *remaining -= n;
    Ok(())
}
fn keys_read(r: &mut Reader<'_>, version: Version, remaining: &mut usize) -> Result<Vec<String>> {
    let n = count_read(r, remaining)?;
    let mut values = Vec::with_capacity(n);
    for _ in 0..n {
        values.push(key_read(r, version)?);
    }
    Ok(values)
}
fn keys_write(
    values: &[String],
    w: &mut Writer,
    version: Version,
    remaining: &mut usize,
    limits: Limits,
) -> Result<()> {
    count_write(values.len(), w, remaining, limits)?;
    for value in values {
        key_write(value, w, version, limits)?;
    }
    Ok(())
}
/// Init alone has a second list of highlighted recipe keys. All variants have
/// the primary list in `UnlockRecipes::recipes`; list order/duplicates survive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnlockRecipesAction {
    Init { highlighted: Vec<String> },
    Add,
    Remove,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockRecipes {
    pub action: UnlockRecipesAction,
    pub settings: RecipeBookSettings,
    pub recipes: Vec<String>,
}
impl Body for UnlockRecipes {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let id = r.var_i32()?;
        let settings = RecipeBookSettings::read_fields(r)?;
        let mut remaining = r.limits.max_collection;
        let recipes = keys_read(r, version, &mut remaining)?;
        let highlighted = if id == 0 {
            Some(keys_read(r, version, &mut remaining)?)
        } else {
            None
        };
        r.finish()?;
        let action = match id {
            0 => UnlockRecipesAction::Init {
                highlighted: highlighted.unwrap(),
            },
            1 => UnlockRecipesAction::Add,
            2 => UnlockRecipesAction::Remove,
            _ => return Err(Error::Unsupported("unlock recipes action")),
        };
        Ok(Self {
            action,
            settings,
            recipes,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.var_i32(match self.action {
            UnlockRecipesAction::Init { .. } => 0,
            UnlockRecipesAction::Add => 1,
            UnlockRecipesAction::Remove => 2,
        });
        self.settings.write_fields(w);
        let mut remaining = limits.max_collection;
        keys_write(&self.recipes, w, version, &mut remaining, limits)?;
        if let UnlockRecipesAction::Init { highlighted } = &self.action {
            keys_write(highlighted, w, version, &mut remaining, limits)?;
        }
        Ok(())
    }
}
packet!(UnlockRecipes, Clientbound, "unlock_recipes");

/// Modern display IDs, retained as signed VarInt wire scalars without lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeBookRemove {
    pub display_ids: Vec<i32>,
}
impl Body for RecipeBookRemove {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let mut remaining = r.limits.max_collection;
        let n = count_read(r, &mut remaining)?;
        let mut display_ids = Vec::with_capacity(n);
        for _ in 0..n {
            display_ids.push(r.var_i32()?);
        }
        Ok(Self { display_ids })
    }
    fn write(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        let mut remaining = limits.max_collection;
        count_write(self.display_ids.len(), w, &mut remaining, limits)?;
        for &id in &self.display_ids {
            budget(w, var_len(id), limits)?;
            w.var_i32(id);
        }
        Ok(())
    }
}
packet!(RecipeBookRemove, Clientbound, "recipe_book_remove");

/// Only the legacy (763–767) ghost-recipe response. Modern responses contain an
/// unimplemented RecipeDisplay and are deliberately not represented by this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyCraftRecipeResponse {
    /// Raw one-byte ID; use `RecipeWindowId::LegacyByte(id).legacy_signed()` for a signed view.
    pub window_id: u8,
    pub recipe_key: String,
}
impl Body for LegacyCraftRecipeResponse {
    fn available(version: Version) -> Result<()> {
        if version.protocol() >= 768 {
            return Err(Error::Unsupported("modern recipe display"));
        }
        Ok(())
    }
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            window_id: r.u8()?,
            recipe_key: key_read(r, version)?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.u8(self.window_id);
        key_write(&self.recipe_key, w, version, limits)
    }
}
packet!(
    LegacyCraftRecipeResponse,
    Clientbound,
    "craft_recipe_response"
);

/// Clientbound recipe-control subset. Serverbound packets have separate helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeControlPacket {
    Unlock(UnlockRecipes),
    Settings(RecipeBookSettings),
    Remove(RecipeBookRemove),
    LegacyResponse(LegacyCraftRecipeResponse),
}
impl RecipeControlPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "unlock_recipes" => Self::Unlock(UnlockRecipes::decode(bytes, version, limits)?),
            "recipe_book_settings" => {
                Self::Settings(RecipeBookSettings::decode(bytes, version, limits)?)
            }
            "recipe_book_remove" => Self::Remove(RecipeBookRemove::decode(bytes, version, limits)?),
            "craft_recipe_response" => {
                Self::LegacyResponse(LegacyCraftRecipeResponse::decode(bytes, version, limits)?)
            }
            _ => return Err(Error::Unsupported("typed recipe-control packet")),
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        match self {
            Self::Unlock(p) => p.encode(version, limits),
            Self::Settings(p) => p.encode(version, limits),
            Self::Remove(p) => p.encode(version, limits),
            Self::LegacyResponse(p) => p.encode(version, limits),
        }
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        match self {
            Self::Unlock(p) => p.packet(version, limits),
            Self::Settings(p) => p.packet(version, limits),
            Self::Remove(p) => p.packet(version, limits),
            Self::LegacyResponse(p) => p.packet(version, limits),
        }
    }
}
