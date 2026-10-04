//! Legacy clientbound recipe declarations (protocols 763–767).
//!
//! This is a wire model, not a recipe registry or crafting engine. Unknown
//! unframed serializers/components return `Unsupported`; preserve the complete
//! raw packet. Modern declaration packets (768+) have a different format and
//! are deliberately excluded. See `docs/legacy-recipe-wire-audit.md`.
use super::inventory::{self, Budget, Slot};
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

/// The wire spelling is preserved, including an omitted default namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyRecipeSerializer {
    Name(String),
    Id(i32),
}

/// Known vanilla serializer kinds. The obsolete banner-add-pattern serializer
/// is not a vanilla kind in these releases; it remains an unknown custom name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LegacyRecipeKind {
    Shaped = 0,
    Shapeless = 1,
    ArmorDye = 2,
    BookCloning = 3,
    MapCloning = 4,
    MapExtending = 5,
    FireworkRocket = 6,
    FireworkStar = 7,
    FireworkStarFade = 8,
    TippedArrow = 9,
    BannerDuplicate = 10,
    ShieldDecoration = 11,
    ShulkerBoxColoring = 12,
    SuspiciousStew = 13,
    RepairItem = 14,
    Smelting = 15,
    Blasting = 16,
    Smoking = 17,
    CampfireCooking = 18,
    Stonecutting = 19,
    SmithingTransform = 20,
    SmithingTrim = 21,
    DecoratedPot = 22,
}
impl LegacyRecipeKind {
    pub const ALL: &'static [Self] = &[
        Self::Shaped,
        Self::Shapeless,
        Self::ArmorDye,
        Self::BookCloning,
        Self::MapCloning,
        Self::MapExtending,
        Self::FireworkRocket,
        Self::FireworkStar,
        Self::FireworkStarFade,
        Self::TippedArrow,
        Self::BannerDuplicate,
        Self::ShieldDecoration,
        Self::ShulkerBoxColoring,
        Self::SuspiciousStew,
        Self::RepairItem,
        Self::Smelting,
        Self::Blasting,
        Self::Smoking,
        Self::CampfireCooking,
        Self::Stonecutting,
        Self::SmithingTransform,
        Self::SmithingTrim,
        Self::DecoratedPot,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Shaped => "crafting_shaped",
            Self::Shapeless => "crafting_shapeless",
            Self::ArmorDye => "crafting_special_armordye",
            Self::BookCloning => "crafting_special_bookcloning",
            Self::MapCloning => "crafting_special_mapcloning",
            Self::MapExtending => "crafting_special_mapextending",
            Self::FireworkRocket => "crafting_special_firework_rocket",
            Self::FireworkStar => "crafting_special_firework_star",
            Self::FireworkStarFade => "crafting_special_firework_star_fade",
            Self::TippedArrow => "crafting_special_tippedarrow",
            Self::BannerDuplicate => "crafting_special_bannerduplicate",
            Self::ShieldDecoration => "crafting_special_shielddecoration",
            Self::ShulkerBoxColoring => "crafting_special_shulkerboxcoloring",
            Self::SuspiciousStew => "crafting_special_suspiciousstew",
            Self::RepairItem => "crafting_special_repairitem",
            Self::Smelting => "smelting",
            Self::Blasting => "blasting",
            Self::Smoking => "smoking",
            Self::CampfireCooking => "campfire_cooking",
            Self::Stonecutting => "stonecutting",
            Self::SmithingTransform => "smithing_transform",
            Self::SmithingTrim => "smithing_trim",
            Self::DecoratedPot => "crafting_decorated_pot",
        }
    }
    pub fn serializer(self, version: Version) -> Result<LegacyRecipeSerializer> {
        available(version)?;
        Ok(if version.protocol() < 766 {
            LegacyRecipeSerializer::Name(format!("minecraft:{}", self.name()))
        } else {
            LegacyRecipeSerializer::Id(self as i32)
        })
    }
    fn special(self) -> bool {
        matches!(
            self,
            Self::ArmorDye
                | Self::BookCloning
                | Self::MapCloning
                | Self::MapExtending
                | Self::FireworkRocket
                | Self::FireworkStar
                | Self::FireworkStarFade
                | Self::TippedArrow
                | Self::BannerDuplicate
                | Self::ShieldDecoration
                | Self::ShulkerBoxColoring
                | Self::SuspiciousStew
                | Self::RepairItem
                | Self::DecoratedPot
        )
    }
    fn cooking(self) -> bool {
        matches!(
            self,
            Self::Smelting | Self::Blasting | Self::Smoking | Self::CampfireCooking
        )
    }
}
impl LegacyRecipeSerializer {
    pub fn kind(&self, version: Version) -> Result<LegacyRecipeKind> {
        available(version)?;
        let kind = match self {
            Self::Name(name) if version.protocol() < 766 => {
                identifier::validate(name, version)?;
                let (namespace, path) = identifier::parts(name);
                LegacyRecipeKind::ALL
                    .iter()
                    .copied()
                    .find(|k| namespace == "minecraft" && path == k.name())
            }
            Self::Id(id) if version.protocol() >= 766 => {
                if *id < 0 {
                    return Err(Error::Invalid("negative recipe serializer ID"));
                }
                LegacyRecipeKind::ALL.get(*id as usize).copied()
            }
            _ => return Err(Error::Invalid("recipe serializer version")),
        };
        kind.ok_or(Error::Unsupported("legacy recipe serializer"))
    }
}

/// Ordered slot alternatives. An empty list is the empty ingredient; no item
/// registry lookup, deduplication, tag expansion or matching is performed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LegacyIngredient {
    pub alternatives: Vec<Slot>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum LegacyRecipeData {
    Shaped {
        width: usize,
        height: usize,
        group: String,
        /// Crafting category ordinal: building, redstone, equipment, misc.
        category: i32,
        /// Flat row-major wire sequence, exactly width * height ingredients.
        ingredients: Vec<LegacyIngredient>,
        result: Slot,
        show_notification: bool,
    },
    Shapeless {
        group: String,
        category: i32,
        ingredients: Vec<LegacyIngredient>,
        result: Slot,
    },
    Special {
        category: i32,
    },
    Cooking {
        group: String,
        /// Cooking category ordinal: food, blocks, misc.
        category: i32,
        ingredient: LegacyIngredient,
        result: Slot,
        experience: f32,
        cook_time: i32,
    },
    Stonecutting {
        group: String,
        ingredient: LegacyIngredient,
        result: Slot,
    },
    SmithingTransform {
        template: LegacyIngredient,
        base: LegacyIngredient,
        addition: LegacyIngredient,
        result: Slot,
    },
    SmithingTrim {
        template: LegacyIngredient,
        base: LegacyIngredient,
        addition: LegacyIngredient,
    },
}
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyRecipeDeclaration {
    pub key: String,
    pub serializer: LegacyRecipeSerializer,
    pub data: LegacyRecipeData,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LegacyDeclareRecipes {
    pub recipes: Vec<LegacyRecipeDeclaration>,
}
fn available(version: Version) -> Result<()> {
    if !(763..=767).contains(&version.protocol()) {
        return Err(Error::Unsupported("modern recipe declarations"));
    }
    Ok(())
}
fn preflight(n: usize, r: &Reader<'_>, minimum: usize) -> Result<()> {
    if n > r.remaining().len() / minimum {
        Err(Error::Eof)
    } else {
        Ok(())
    }
}
fn string_read(r: &mut Reader<'_>, version: Version, key: bool) -> Result<String> {
    let value = r.string(32767)?;
    if key {
        identifier::validate(value, version)?;
    }
    Ok(value.to_owned())
}
fn string_write(
    value: &str,
    w: &mut Writer,
    version: Version,
    b: &Budget,
    key: bool,
) -> Result<()> {
    if key {
        identifier::validate(value, version)?;
    }
    // Five bytes is a conservative VarInt prefix bound; calculate the exact
    // prefix to permit an output that fits its packet budget exactly.
    let mut prefix = 1;
    let mut len = value.len();
    while len > 127 {
        prefix += 1;
        len >>= 7;
    }
    if value.len().saturating_add(prefix) > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("recipe string bytes"));
    }
    w.string(value, b.limits.max_string_chars.min(32767))?;
    b.check_bytes(w)
}
fn category(value: i32, cooking: bool) -> Result<i32> {
    if !(0..=if cooking { 2 } else { 3 }).contains(&value) {
        Err(Error::Invalid("recipe category"))
    } else {
        Ok(value)
    }
}
fn dimensions(width: usize, height: usize, b: &Budget) -> Result<usize> {
    // Counts, not a crafting-grid simulator: do not assume a 3x3 menu.
    if width > i32::MAX as usize
        || height > i32::MAX as usize
        || width > b.limits.max_collection
        || height > b.limits.max_collection
    {
        return Err(Error::Limit("recipe dimensions"));
    }
    let n = width
        .checked_mul(height)
        .ok_or(Error::Limit("recipe dimensions"))?;
    if n > b.limits.max_collection {
        return Err(Error::Limit("recipe dimensions"));
    }
    Ok(n)
}
fn ingredient_read(r: &mut Reader<'_>, v: Version, b: &mut Budget) -> Result<LegacyIngredient> {
    let n = b.count(r)?;
    preflight(n, r, 1)?;
    let mut alternatives = Vec::with_capacity(n);
    for _ in 0..n {
        alternatives.push(inventory::read_slot(r, v, b, 0)?);
    }
    Ok(LegacyIngredient { alternatives })
}
fn ingredient_write(
    value: &LegacyIngredient,
    w: &mut Writer,
    v: Version,
    b: &mut Budget,
) -> Result<()> {
    b.write_count(value.alternatives.len(), w)?;
    for slot in &value.alternatives {
        inventory::write_slot(slot, w, v, b, 0)?;
    }
    Ok(())
}
fn ingredients_read(
    n: usize,
    r: &mut Reader<'_>,
    v: Version,
    b: &mut Budget,
) -> Result<Vec<LegacyIngredient>> {
    preflight(n, r, 1)?;
    let mut values = Vec::with_capacity(n);
    for _ in 0..n {
        values.push(ingredient_read(r, v, b)?);
    }
    Ok(values)
}
fn data_read(
    kind: LegacyRecipeKind,
    r: &mut Reader<'_>,
    v: Version,
    b: &mut Budget,
) -> Result<LegacyRecipeData> {
    use LegacyRecipeData as D;
    use LegacyRecipeKind as K;
    Ok(match kind {
        K::Shaped => {
            let old = if v.protocol() < 765 {
                Some((
                    r.count(b.limits.max_collection)?,
                    r.count(b.limits.max_collection)?,
                ))
            } else {
                None
            };
            let group = string_read(r, v, false)?;
            let category = category(r.var_i32()?, false)?;
            let (width, height) = match old {
                Some(pair) => pair,
                None => (
                    r.count(b.limits.max_collection)?,
                    r.count(b.limits.max_collection)?,
                ),
            };
            let n = dimensions(width, height, b)?;
            b.charge(n)?;
            let ingredients = ingredients_read(n, r, v, b)?;
            let result = inventory::read_slot(r, v, b, 0)?;
            D::Shaped {
                width,
                height,
                group,
                category,
                ingredients,
                result,
                show_notification: r.bool()?,
            }
        }
        K::Shapeless => {
            let group = string_read(r, v, false)?;
            let category = category(r.var_i32()?, false)?;
            let n = b.count(r)?;
            let ingredients = ingredients_read(n, r, v, b)?;
            D::Shapeless {
                group,
                category,
                ingredients,
                result: inventory::read_slot(r, v, b, 0)?,
            }
        }
        K::Stonecutting => D::Stonecutting {
            group: string_read(r, v, false)?,
            ingredient: {
                b.charge(1)?;
                ingredient_read(r, v, b)?
            },
            result: inventory::read_slot(r, v, b, 0)?,
        },
        K::SmithingTransform | K::SmithingTrim => {
            b.charge(3)?;
            let template = ingredient_read(r, v, b)?;
            let base = ingredient_read(r, v, b)?;
            let addition = ingredient_read(r, v, b)?;
            if kind == K::SmithingTrim {
                D::SmithingTrim {
                    template,
                    base,
                    addition,
                }
            } else {
                D::SmithingTransform {
                    template,
                    base,
                    addition,
                    result: inventory::read_slot(r, v, b, 0)?,
                }
            }
        }
        k if k.special() => D::Special {
            category: category(r.var_i32()?, false)?,
        },
        k if k.cooking() => {
            let group = string_read(r, v, false)?;
            let category = category(r.var_i32()?, true)?;
            b.charge(1)?;
            let ingredient = ingredient_read(r, v, b)?;
            let result = inventory::read_slot(r, v, b, 0)?;
            D::Cooking {
                group,
                category,
                ingredient,
                result,
                experience: r.f32()?,
                cook_time: r.var_i32()?,
            }
        }
        _ => return Err(Error::Unsupported("legacy recipe serializer")),
    })
}
fn data_write(
    data: &LegacyRecipeData,
    kind: LegacyRecipeKind,
    w: &mut Writer,
    v: Version,
    b: &mut Budget,
) -> Result<()> {
    use LegacyRecipeData as D;
    use LegacyRecipeKind as K;
    match data {
        D::Shaped {
            width,
            height,
            group,
            category: cat,
            ingredients,
            result,
            show_notification,
        } if kind == K::Shaped => {
            let n = dimensions(*width, *height, b)?;
            if n != ingredients.len() {
                return Err(Error::Invalid("recipe grid ingredient count"));
            }
            b.charge(n)?;
            if v.protocol() < 765 {
                w.var_i32(*width as i32);
                w.var_i32(*height as i32);
            }
            string_write(group, w, v, b, false)?;
            w.var_i32(category(*cat, false)?);
            if v.protocol() >= 765 {
                w.var_i32(*width as i32);
                w.var_i32(*height as i32);
            }
            for i in ingredients {
                ingredient_write(i, w, v, b)?;
            }
            inventory::write_slot(result, w, v, b, 0)?;
            w.bool(*show_notification);
        }
        D::Shapeless {
            group,
            category: cat,
            ingredients,
            result,
        } if kind == K::Shapeless => {
            string_write(group, w, v, b, false)?;
            w.var_i32(category(*cat, false)?);
            b.write_count(ingredients.len(), w)?;
            for i in ingredients {
                ingredient_write(i, w, v, b)?;
            }
            inventory::write_slot(result, w, v, b, 0)?;
        }
        D::Special { category: cat } if kind.special() => w.var_i32(category(*cat, false)?),
        D::Cooking {
            group,
            category: cat,
            ingredient,
            result,
            experience,
            cook_time,
        } if kind.cooking() => {
            string_write(group, w, v, b, false)?;
            w.var_i32(category(*cat, true)?);
            b.charge(1)?;
            ingredient_write(ingredient, w, v, b)?;
            inventory::write_slot(result, w, v, b, 0)?;
            w.f32(*experience);
            w.var_i32(*cook_time);
        }
        D::Stonecutting {
            group,
            ingredient,
            result,
        } if kind == K::Stonecutting => {
            string_write(group, w, v, b, false)?;
            b.charge(1)?;
            ingredient_write(ingredient, w, v, b)?;
            inventory::write_slot(result, w, v, b, 0)?;
        }
        D::SmithingTransform {
            template,
            base,
            addition,
            result,
        } if kind == K::SmithingTransform => {
            b.charge(3)?;
            for i in [template, base, addition] {
                ingredient_write(i, w, v, b)?;
            }
            inventory::write_slot(result, w, v, b, 0)?;
        }
        D::SmithingTrim {
            template,
            base,
            addition,
        } if kind == K::SmithingTrim => {
            b.charge(3)?;
            for i in [template, base, addition] {
                ingredient_write(i, w, v, b)?;
            }
        }
        _ => return Err(Error::Invalid("recipe serializer data kind")),
    }
    b.check_bytes(w)
}
impl LegacyDeclareRecipes {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        available(version)?;
        version.packet_id(State::Play, Direction::Clientbound, "declare_recipes")?;
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("recipe packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let mut b = Budget::new(limits);
        let n = b.count(&mut r)?;
        // An unknown unframed serializer may have no payload; only the key and
        // serializer field are common to all entries.
        preflight(n, &r, 2)?;
        let mut recipes = Vec::with_capacity(n);
        for _ in 0..n {
            let (key, serializer) = if version.protocol() < 766 {
                let name = string_read(&mut r, version, true)?;
                (
                    string_read(&mut r, version, true)?,
                    LegacyRecipeSerializer::Name(name),
                )
            } else {
                (
                    string_read(&mut r, version, true)?,
                    LegacyRecipeSerializer::Id(r.var_i32()?),
                )
            };
            let kind = serializer.kind(version)?;
            let data = data_read(kind, &mut r, version, &mut b)?;
            recipes.push(LegacyRecipeDeclaration {
                key,
                serializer,
                data,
            });
        }
        r.finish()?;
        Ok(Self { recipes })
    }
    /// No partial output is returned on error. Serializer spelling/ID and data
    /// kind must agree; cross-family serializer representations are rejected.
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        available(version)?;
        version.packet_id(State::Play, Direction::Clientbound, "declare_recipes")?;
        let mut b = Budget::new(limits);
        let mut w = Writer::new();
        b.write_count(self.recipes.len(), &mut w)?;
        for recipe in &self.recipes {
            let kind = recipe.serializer.kind(version)?;
            match &recipe.serializer {
                LegacyRecipeSerializer::Name(name) => {
                    string_write(name, &mut w, version, &b, true)?;
                    string_write(&recipe.key, &mut w, version, &b, true)?;
                }
                LegacyRecipeSerializer::Id(id) => {
                    string_write(&recipe.key, &mut w, version, &b, true)?;
                    w.var_i32(*id);
                }
            }
            data_write(&recipe.data, kind, &mut w, version, &mut b)?;
        }
        b.check_bytes(&w)?;
        Ok(w.into_inner())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(State::Play, Direction::Clientbound, "declare_recipes")?,
            self.encode(version, limits)?,
        ))
    }
}
