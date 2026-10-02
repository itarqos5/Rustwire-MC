//! Semantic registry holders used by entity metadata.
//!
//! Registry references retain their numeric IDs; resolving them requires the
//! connection's registries. Inline payloads are not opaque byte strings. Painting
//! and wolf metadata used plain IDs in protocol 766; inline holders start at 767.
//! Painting dimensions are VarInts, with title/author added in protocol 768.
//! These boundaries were checked against the corresponding server stream codecs,
//! correcting stale fields in the pinned protocol schemas.
use crate::{
    codec::{Reader, Writer},
    nbt::{Nbt, RootFormat},
    packet::{
        inventory::{self, Budget},
        ProfileProperty,
    },
    Error, Result, Version,
};

/// A registry reference or a complete inline value. Reference IDs are unshifted
/// here; the holder wire format uses zero for inline and ID plus one otherwise.
#[derive(Debug, Clone, PartialEq)]
pub enum RegistryHolder<T> {
    RegistryId(i32),
    Inline(T),
}

/// A named registry tag or an explicit list of unshifted registry IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryHolderSet {
    Tag(String),
    Ids(Vec<i32>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintingVariant {
    /// Width in blocks, from 1 through 16.
    pub width: i32,
    /// Height in blocks, from 1 through 16.
    pub height: i32,
    pub asset_id: String,
    /// Available beginning with protocol 768; must be absent in protocol 767.
    pub title: Option<Nbt>,
    /// Available beginning with protocol 768; must be absent in protocol 767.
    pub author: Option<Nbt>,
}

/// The inline wolf-variant payload supported by protocols 767 through 769.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WolfVariant {
    pub wild_texture: String,
    pub tame_texture: String,
    pub angry_texture: String,
    pub biomes: RegistryHolderSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerModel {
    Wide,
    Slim,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerSkinPatch {
    pub body: Option<String>,
    pub cape: Option<String>,
    pub elytra: Option<String>,
    pub model: Option<PlayerModel>,
}

/// Protocol 773 and later profile data. Partial profiles can omit either name or
/// UUID; the wire does not imply that account or texture data has been resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvableProfile {
    Partial {
        name: Option<String>,
        uuid: Option<[u8; 16]>,
        properties: Vec<ProfileProperty>,
        skin_patch: PlayerSkinPatch,
    },
    Complete {
        uuid: [u8; 16],
        name: String,
        properties: Vec<ProfileProperty>,
        skin_patch: PlayerSkinPatch,
    },
}

fn nonnegative(value: i32) -> Result<i32> {
    if value < 0 {
        Err(Error::Invalid("negative holder registry ID"))
    } else {
        Ok(value)
    }
}

fn string(value: &str, max_chars: usize, w: &mut Writer, b: &Budget) -> Result<()> {
    let mut len = value.len();
    let mut prefix = 1usize;
    while len > 127 {
        len >>= 7;
        prefix += 1;
    }
    let required = value
        .len()
        .checked_add(prefix)
        .ok_or(Error::Limit("holder string bytes"))?;
    if required > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("holder string bytes"));
    }
    w.string(value, max_chars.min(b.limits.max_string_chars))?;
    b.check_bytes(w)
}

fn optional_string(r: &mut Reader<'_>, max_chars: usize) -> Result<Option<String>> {
    Ok(if r.bool()? {
        Some(r.string(max_chars)?.into())
    } else {
        None
    })
}

fn write_optional_string(
    value: &Option<String>,
    max_chars: usize,
    w: &mut Writer,
    b: &Budget,
) -> Result<()> {
    w.bool(value.is_some());
    if let Some(value) = value {
        string(value, max_chars, w, b)?;
    }
    b.check_bytes(w)
}

pub(crate) fn read_holder_set(r: &mut Reader<'_>, b: &mut Budget) -> Result<RegistryHolderSet> {
    let marker = nonnegative(r.var_i32()?)?;
    if marker == 0 {
        return Ok(RegistryHolderSet::Tag(r.string(32767)?.into()));
    }
    let count = (marker - 1) as usize;
    if count > b.limits.max_collection {
        return Err(Error::Limit("holder set length"));
    }
    b.charge(count)?;
    // Every ID consumes at least one byte; reject impossible counts before allocation.
    if count > r.remaining().len() {
        return Err(Error::Eof);
    }
    let mut ids = Vec::with_capacity(count);
    for _ in 0..count {
        ids.push(nonnegative(r.var_i32()?)?);
    }
    Ok(RegistryHolderSet::Ids(ids))
}

pub(crate) fn write_holder_set(
    value: &RegistryHolderSet,
    w: &mut Writer,
    b: &mut Budget,
) -> Result<()> {
    match value {
        RegistryHolderSet::Tag(tag) => {
            w.var_i32(0);
            string(tag, 32767, w, b)?;
        }
        RegistryHolderSet::Ids(ids) => {
            if ids.len() > b.limits.max_collection || ids.len() >= i32::MAX as usize {
                return Err(Error::Limit("holder set length"));
            }
            b.charge(ids.len())?;
            w.var_i32(ids.len() as i32 + 1);
            b.check_bytes(w)?;
            for &id in ids {
                w.var_i32(nonnegative(id)?);
                b.check_bytes(w)?;
            }
        }
    }
    b.check_bytes(w)
}

fn read_reference(r: &mut Reader<'_>) -> Result<Option<i32>> {
    let marker = nonnegative(r.var_i32()?)?;
    Ok(if marker == 0 { None } else { Some(marker - 1) })
}

fn write_reference<T>(value: &RegistryHolder<T>, w: &mut Writer) -> Result<()> {
    w.var_i32(match value {
        RegistryHolder::RegistryId(id) => nonnegative(*id)?
            .checked_add(1)
            .ok_or(Error::Invalid("holder registry ID overflow"))?,
        RegistryHolder::Inline(_) => 0,
    });
    Ok(())
}

fn dimension(value: i32) -> Result<i32> {
    if (1..=16).contains(&value) {
        Ok(value)
    } else {
        Err(Error::Invalid("painting dimensions"))
    }
}

fn read_optional_nbt(r: &mut Reader<'_>, b: &mut Budget) -> Result<Option<Nbt>> {
    if r.bool()? {
        Ok(Some(
            inventory::read_nbt(r, RootFormat::Anonymous, b)?
                .ok_or(Error::Invalid("missing painting component NBT"))?,
        ))
    } else {
        Ok(None)
    }
}

fn write_optional_nbt(value: &Option<Nbt>, w: &mut Writer, b: &mut Budget) -> Result<()> {
    w.bool(value.is_some());
    if let Some(value) = value {
        inventory::write_nbt(Some(value), w, RootFormat::Anonymous, b)?;
    }
    b.check_bytes(w)
}

pub(crate) fn read_painting(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<RegistryHolder<PaintingVariant>> {
    if version.protocol() < 767 {
        return Err(Error::Unsupported(
            "inline painting metadata in selected release",
        ));
    }
    if let Some(id) = read_reference(r)? {
        return Ok(RegistryHolder::RegistryId(id));
    }
    let width = dimension(r.var_i32()?)?;
    let height = dimension(r.var_i32()?)?;
    let asset_id = r.string(32767)?.into();
    let (title, author) = if version.protocol() >= 768 {
        (read_optional_nbt(r, b)?, read_optional_nbt(r, b)?)
    } else {
        (None, None)
    };
    Ok(RegistryHolder::Inline(PaintingVariant {
        width,
        height,
        asset_id,
        title,
        author,
    }))
}

pub(crate) fn write_painting(
    value: &RegistryHolder<PaintingVariant>,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    if version.protocol() < 767 {
        return Err(Error::Unsupported(
            "inline painting metadata in selected release",
        ));
    }
    write_reference(value, w)?;
    b.check_bytes(w)?;
    if let RegistryHolder::Inline(value) = value {
        if version.protocol() == 767 && (value.title.is_some() || value.author.is_some()) {
            return Err(Error::Unsupported(
                "painting title and author in selected release",
            ));
        }
        w.var_i32(dimension(value.width)?);
        w.var_i32(dimension(value.height)?);
        string(&value.asset_id, 32767, w, b)?;
        if version.protocol() >= 768 {
            write_optional_nbt(&value.title, w, b)?;
            write_optional_nbt(&value.author, w, b)?;
        }
    }
    b.check_bytes(w)
}

pub(super) fn read_wolf(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<RegistryHolder<WolfVariant>> {
    if !(767..=769).contains(&version.protocol()) {
        return Err(Error::Unsupported(
            "inline wolf metadata in selected release",
        ));
    }
    if let Some(id) = read_reference(r)? {
        return Ok(RegistryHolder::RegistryId(id));
    }
    Ok(RegistryHolder::Inline(WolfVariant {
        wild_texture: r.string(32767)?.into(),
        tame_texture: r.string(32767)?.into(),
        angry_texture: r.string(32767)?.into(),
        biomes: read_holder_set(r, b)?,
    }))
}

pub(super) fn write_wolf(
    value: &RegistryHolder<WolfVariant>,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    if !(767..=769).contains(&version.protocol()) {
        return Err(Error::Unsupported(
            "inline wolf metadata in selected release",
        ));
    }
    write_reference(value, w)?;
    b.check_bytes(w)?;
    if let RegistryHolder::Inline(value) = value {
        string(&value.wild_texture, 32767, w, b)?;
        string(&value.tame_texture, 32767, w, b)?;
        string(&value.angry_texture, 32767, w, b)?;
        write_holder_set(&value.biomes, w, b)?;
    }
    b.check_bytes(w)
}

fn read_properties(r: &mut Reader<'_>, b: &mut Budget) -> Result<Vec<ProfileProperty>> {
    let count = r.count(16.min(b.limits.max_collection))?;
    b.charge(count)?;
    let mut properties = Vec::with_capacity(count);
    for _ in 0..count {
        properties.push(ProfileProperty {
            name: r.string(64)?.into(),
            value: r.string(32767)?.into(),
            signature: optional_string(r, 1024)?,
        });
    }
    Ok(properties)
}

fn write_properties(properties: &[ProfileProperty], w: &mut Writer, b: &mut Budget) -> Result<()> {
    if properties.len() > 16 {
        return Err(Error::Limit("profile property count"));
    }
    b.write_count(properties.len(), w)?;
    for property in properties {
        string(&property.name, 64, w, b)?;
        string(&property.value, 32767, w, b)?;
        write_optional_string(&property.signature, 1024, w, b)?;
    }
    Ok(())
}

fn read_skin_patch(r: &mut Reader<'_>) -> Result<PlayerSkinPatch> {
    Ok(PlayerSkinPatch {
        body: optional_string(r, 32767)?,
        cape: optional_string(r, 32767)?,
        elytra: optional_string(r, 32767)?,
        model: if r.bool()? {
            Some(if r.bool()? {
                PlayerModel::Slim
            } else {
                PlayerModel::Wide
            })
        } else {
            None
        },
    })
}

fn write_skin_patch(value: &PlayerSkinPatch, w: &mut Writer, b: &Budget) -> Result<()> {
    write_optional_string(&value.body, 32767, w, b)?;
    write_optional_string(&value.cape, 32767, w, b)?;
    write_optional_string(&value.elytra, 32767, w, b)?;
    w.bool(value.model.is_some());
    if let Some(model) = value.model {
        w.bool(model == PlayerModel::Slim);
    }
    b.check_bytes(w)
}

pub(crate) fn read_profile(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<ResolvableProfile> {
    if version.protocol() < 773 {
        return Err(Error::Unsupported(
            "resolvable metadata profile in selected release",
        ));
    }
    if r.bool()? {
        Ok(ResolvableProfile::Complete {
            uuid: r.uuid()?,
            name: r.string(16)?.into(),
            properties: read_properties(r, b)?,
            skin_patch: read_skin_patch(r)?,
        })
    } else {
        Ok(ResolvableProfile::Partial {
            name: optional_string(r, 16)?,
            uuid: if r.bool()? { Some(r.uuid()?) } else { None },
            properties: read_properties(r, b)?,
            skin_patch: read_skin_patch(r)?,
        })
    }
}

pub(crate) fn write_profile(
    value: &ResolvableProfile,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    if version.protocol() < 773 {
        return Err(Error::Unsupported(
            "resolvable metadata profile in selected release",
        ));
    }
    match value {
        ResolvableProfile::Complete {
            uuid,
            name,
            properties,
            skin_patch,
        } => {
            w.bool(true);
            w.raw(uuid);
            string(name, 16, w, b)?;
            write_properties(properties, w, b)?;
            write_skin_patch(skin_patch, w, b)?;
        }
        ResolvableProfile::Partial {
            name,
            uuid,
            properties,
            skin_patch,
        } => {
            w.bool(false);
            write_optional_string(name, 16, w, b)?;
            w.bool(uuid.is_some());
            if let Some(uuid) = uuid {
                w.raw(uuid);
            }
            b.check_bytes(w)?;
            write_properties(properties, w, b)?;
            write_skin_patch(skin_patch, w, b)?;
        }
    }
    b.check_bytes(w)
}
