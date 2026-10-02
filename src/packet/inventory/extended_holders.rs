//! Complete inline item-component holders and release-specific key wrappers.
//!
//! Layout boundaries were checked against official release stream codecs. In
//! particular, older trim materials contain a model index and numeric armor
//! keys, early instruments have tick durations, and key wrappers disappear in
//! protocol 775. Chicken and damage-type components never carry inline records.
//!
//! Verification sources are the `STREAM_CODEC` and `DIRECT_STREAM_CODEC`
//! declarations of ArmorTrim, TrimMaterial, TrimPattern, MaterialAssetGroup,
//! Instrument, InstrumentComponent, ProvidesTrimMaterial, JukeboxSong,
//! JukeboxPlayable, BannerPattern, BannerPatternLayers.Layer, EitherHolder,
//! DamageType and DataComponents in the cached official 1.20.6, 1.21.1,
//! 1.21.3, 1.21.4, 1.21.5, 1.21.6, 1.21.8, 1.21.10, 1.21.11, 26.1.2 and
//! 26.2 release jars. No server implementation or disassembly is embedded here.
//!
//! Important corrections to the pinned community schemas:
//! * Trim model index exists in 766–768; numeric armor keys in 766–767;
//!   ingredient/template item IDs disappear in 770.
//! * Instrument tick duration and absent description apply to 766–767.
//! * Banner pattern layers always contain a pattern holder, never a sound.
//! * Instrument key wrappers exist only in 770–774; jukebox wrappers in
//!   767–774; provided trim material wrappers in 770–774.
//! * Chicken variants use boolean + unshifted ID/resource key in 770–774;
//!   zombie-nautilus variants and damage types do so in 774. From 775 these
//!   are plain unshifted IDs, as are chicken sound variants.
use super::extended::{read_sound, write_sound, SoundHolder};
use super::{nonnegative, required_nbt, string, write_nbt, Budget};
use crate::packet::entity_metadata::holders::RegistryHolder;
use crate::{
    codec::{Reader, Writer},
    nbt::{Nbt, RootFormat},
    Error, Result, Version,
};
use std::collections::BTreeSet;

/// A complete registry holder or an unresolved resource key. A key is allowed
/// only in releases whose component uses the boolean-prefixed EitherHolder.
#[derive(Clone, Debug, PartialEq)]
pub enum HolderOrKey<T> {
    Holder(RegistryHolder<T>),
    Key(String),
}
/// A reference to a registry whose stream codec has no inline payload.
/// Numeric IDs are unshifted, including zero. Keys are supported before 775.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryReference {
    RegistryId(i32),
    Key(String),
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrimAssetKey {
    /// Armor-material registry ID, protocols 766–767.
    ArmorMaterialId(i32),
    /// Equipment resource identifier, protocols 768+.
    Identifier(String),
}
#[derive(Clone, Debug, PartialEq)]
pub struct TrimMaterial {
    pub asset_name: String,
    /// Required through protocol 769; absent from protocol 770.
    pub ingredient_id: Option<i32>,
    /// Required through protocol 768; absent from protocol 769.
    pub item_model_index: Option<f32>,
    pub override_armor_assets: Vec<(TrimAssetKey, String)>,
    pub description: Nbt,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TrimPattern {
    pub asset_id: String,
    /// Required through protocol 769; absent from protocol 770.
    pub template_item_id: Option<i32>,
    pub description: Nbt,
    pub decal: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ArmorTrim {
    pub material: RegistryHolder<TrimMaterial>,
    pub pattern: RegistryHolder<TrimPattern>,
    /// Required through protocol 769; absent from protocol 770.
    pub show_tooltip: Option<bool>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum InstrumentDuration {
    /// Protocols 766–767 encode a VarInt tick duration.
    Ticks(i32),
    /// Protocols 768+ encode a floating-point duration in seconds.
    Seconds(f32),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Instrument {
    pub sound: SoundHolder,
    pub use_duration: InstrumentDuration,
    pub range: f32,
    /// Absent in 766–767; required from 768.
    pub description: Option<Nbt>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct JukeboxSong {
    pub sound: SoundHolder,
    pub description: Nbt,
    pub length_seconds: f32,
    pub comparator_output: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct JukeboxPlayable {
    pub song: HolderOrKey<JukeboxSong>,
    /// Required through protocol 769; absent from protocol 770.
    pub show_tooltip: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BannerPattern {
    pub asset_id: String,
    pub translation_key: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BannerPatternLayer {
    pub pattern: RegistryHolder<BannerPattern>,
    /// Dye color ID, in the inclusive range 0–15.
    pub color_id: i32,
}

fn reference(r: &mut Reader<'_>) -> Result<Option<i32>> {
    let marker = nonnegative(r.var_i32()?, "negative registry holder marker")?;
    Ok(if marker == 0 { None } else { Some(marker - 1) })
}
fn write_reference<T>(v: &RegistryHolder<T>, w: &mut Writer, b: &Budget) -> Result<()> {
    let marker = match v {
        RegistryHolder::Inline(_) => 0,
        RegistryHolder::RegistryId(id) => nonnegative(*id, "negative registry holder ID")?
            .checked_add(1)
            .ok_or(Error::Invalid("registry holder ID overflow"))?,
    };
    w.var_i32(marker);
    b.check_bytes(w)
}
fn read_holder<T>(
    r: &mut Reader<'_>,
    b: &mut Budget,
    f: impl FnOnce(&mut Reader<'_>, &mut Budget) -> Result<T>,
) -> Result<RegistryHolder<T>> {
    Ok(match reference(r)? {
        Some(id) => RegistryHolder::RegistryId(id),
        None => RegistryHolder::Inline(f(r, b)?),
    })
}
fn write_holder<T>(
    v: &RegistryHolder<T>,
    w: &mut Writer,
    b: &mut Budget,
    f: impl FnOnce(&T, &mut Writer, &mut Budget) -> Result<()>,
) -> Result<()> {
    write_reference(v, w, b)?;
    if let RegistryHolder::Inline(v) = v {
        f(v, w, b)?;
    }
    b.check_bytes(w)
}
fn read_either<T>(
    r: &mut Reader<'_>,
    b: &mut Budget,
    key_allowed: bool,
    f: impl FnOnce(&mut Reader<'_>, &mut Budget) -> Result<T>,
) -> Result<HolderOrKey<T>> {
    if key_allowed && !r.bool()? {
        Ok(HolderOrKey::Key(r.string(32767)?.into()))
    } else {
        Ok(HolderOrKey::Holder(read_holder(r, b, f)?))
    }
}
fn write_either<T>(
    v: &HolderOrKey<T>,
    w: &mut Writer,
    b: &mut Budget,
    key_allowed: bool,
    f: impl FnOnce(&T, &mut Writer, &mut Budget) -> Result<()>,
) -> Result<()> {
    match v {
        HolderOrKey::Key(key) => {
            if !key_allowed {
                return Err(Error::Unsupported("registry key in selected release"));
            }
            w.bool(false);
            string(w, key, b)?;
        }
        HolderOrKey::Holder(holder) => {
            if key_allowed {
                w.bool(true);
            }
            write_holder(holder, w, b, f)?;
        }
    }
    b.check_bytes(w)
}
fn write_tooltip(v: Option<bool>, w: &mut Writer, version: Version) -> Result<()> {
    if version.protocol() < 770 {
        w.bool(v.ok_or(Error::Invalid("missing legacy holder tooltip"))?);
    } else if v.is_some() {
        return Err(Error::Unsupported(
            "legacy holder tooltip in selected release",
        ));
    }
    Ok(())
}
fn read_material(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<TrimMaterial> {
    let p = version.protocol();
    let asset_name = r.string(32767)?.into();
    let ingredient_id = if p < 770 {
        Some(nonnegative(r.var_i32()?, "trim ingredient ID")?)
    } else {
        None
    };
    let item_model_index = if p < 769 { Some(r.f32()?) } else { None };
    let n = b.count(r)?;
    if n > r.remaining().len() / 2 {
        return Err(Error::Eof);
    }
    let mut override_armor_assets = Vec::with_capacity(n);
    let mut seen = BTreeSet::new();
    for _ in 0..n {
        let key = if p < 768 {
            TrimAssetKey::ArmorMaterialId(nonnegative(r.var_i32()?, "armor material ID")?)
        } else {
            TrimAssetKey::Identifier(r.string(32767)?.into())
        };
        if !seen.insert(key.clone()) {
            return Err(Error::Invalid("duplicate trim asset key"));
        }
        override_armor_assets.push((key, r.string(32767)?.into()));
    }
    Ok(TrimMaterial {
        asset_name,
        ingredient_id,
        item_model_index,
        override_armor_assets,
        description: required_nbt(r, b)?,
    })
}
fn write_material(
    v: &TrimMaterial,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    let p = version.protocol();
    if v.ingredient_id.is_some() != (p < 770) || v.item_model_index.is_some() != (p < 769) {
        return Err(Error::Invalid("trim material fields for selected release"));
    }
    string(w, &v.asset_name, b)?;
    if let Some(id) = v.ingredient_id {
        w.var_i32(nonnegative(id, "trim ingredient ID")?);
    }
    if let Some(index) = v.item_model_index {
        w.f32(index);
    }
    b.write_count(v.override_armor_assets.len(), w)?;
    let mut seen = BTreeSet::new();
    for (key, value) in &v.override_armor_assets {
        if !seen.insert(key) {
            return Err(Error::Invalid("duplicate trim asset key"));
        }
        match key {
            TrimAssetKey::ArmorMaterialId(id) if p < 768 => {
                w.var_i32(nonnegative(*id, "armor material ID")?)
            }
            TrimAssetKey::Identifier(key) if p >= 768 => string(w, key, b)?,
            _ => return Err(Error::Invalid("trim asset key for selected release")),
        }
        string(w, value, b)?;
    }
    write_nbt(Some(&v.description), w, RootFormat::Anonymous, b)
}
fn read_pattern(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<TrimPattern> {
    Ok(TrimPattern {
        asset_id: r.string(32767)?.into(),
        template_item_id: if version.protocol() < 770 {
            Some(nonnegative(r.var_i32()?, "trim template ID")?)
        } else {
            None
        },
        description: required_nbt(r, b)?,
        decal: r.bool()?,
    })
}
fn write_pattern(v: &TrimPattern, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
    if v.template_item_id.is_some() != (version.protocol() < 770) {
        return Err(Error::Invalid("trim template for selected release"));
    }
    string(w, &v.asset_id, b)?;
    if let Some(id) = v.template_item_id {
        w.var_i32(nonnegative(id, "trim template ID")?);
    }
    write_nbt(Some(&v.description), w, RootFormat::Anonymous, b)?;
    w.bool(v.decal);
    b.check_bytes(w)
}
pub(super) fn read_trim(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<ArmorTrim> {
    Ok(ArmorTrim {
        material: read_holder(r, b, |r, b| read_material(r, version, b))?,
        pattern: read_holder(r, b, |r, b| read_pattern(r, version, b))?,
        show_tooltip: if version.protocol() < 770 {
            Some(r.bool()?)
        } else {
            None
        },
    })
}
pub(super) fn write_trim(
    v: &ArmorTrim,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    write_holder(&v.material, w, b, |v, w, b| {
        write_material(v, w, version, b)
    })?;
    write_holder(&v.pattern, w, b, |v, w, b| write_pattern(v, w, version, b))?;
    write_tooltip(v.show_tooltip, w, version)?;
    b.check_bytes(w)
}
pub(super) fn read_provides_trim_material(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<HolderOrKey<TrimMaterial>> {
    read_either(r, b, version.protocol() < 775, |r, b| {
        read_material(r, version, b)
    })
}
pub(super) fn write_provides_trim_material(
    v: &HolderOrKey<TrimMaterial>,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    write_either(v, w, b, version.protocol() < 775, |v, w, b| {
        write_material(v, w, version, b)
    })
}
fn read_instrument_data(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<Instrument> {
    let sound = read_sound(r, b)?;
    let use_duration = if version.protocol() < 768 {
        InstrumentDuration::Ticks(r.var_i32()?)
    } else {
        InstrumentDuration::Seconds(r.f32()?)
    };
    let range = r.f32()?;
    let description = if version.protocol() >= 768 {
        Some(required_nbt(r, b)?)
    } else {
        None
    };
    Ok(Instrument {
        sound,
        use_duration,
        range,
        description,
    })
}
fn write_instrument_data(
    v: &Instrument,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    if v.description.is_some() != (version.protocol() >= 768) {
        return Err(Error::Invalid(
            "instrument description for selected release",
        ));
    }
    write_sound(&v.sound, w, b)?;
    match v.use_duration {
        InstrumentDuration::Ticks(ticks) if version.protocol() < 768 => w.var_i32(ticks),
        InstrumentDuration::Seconds(seconds) if version.protocol() >= 768 => w.f32(seconds),
        _ => return Err(Error::Invalid("instrument duration for selected release")),
    }
    w.f32(v.range);
    if let Some(v) = &v.description {
        write_nbt(Some(v), w, RootFormat::Anonymous, b)?;
    }
    b.check_bytes(w)
}
pub(super) fn read_instrument(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<HolderOrKey<Instrument>> {
    read_either(r, b, (770..775).contains(&version.protocol()), |r, b| {
        read_instrument_data(r, version, b)
    })
}
pub(super) fn write_instrument(
    v: &HolderOrKey<Instrument>,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    write_either(
        v,
        w,
        b,
        (770..775).contains(&version.protocol()),
        |v, w, b| write_instrument_data(v, w, version, b),
    )
}
fn read_song(r: &mut Reader<'_>, b: &mut Budget) -> Result<JukeboxSong> {
    Ok(JukeboxSong {
        sound: read_sound(r, b)?,
        description: required_nbt(r, b)?,
        length_seconds: r.f32()?,
        comparator_output: r.var_i32()?,
    })
}
fn write_song(v: &JukeboxSong, w: &mut Writer, b: &mut Budget) -> Result<()> {
    write_sound(&v.sound, w, b)?;
    write_nbt(Some(&v.description), w, RootFormat::Anonymous, b)?;
    w.f32(v.length_seconds);
    w.var_i32(v.comparator_output);
    b.check_bytes(w)
}
pub(super) fn read_jukebox(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<JukeboxPlayable> {
    Ok(JukeboxPlayable {
        song: read_either(r, b, version.protocol() < 775, read_song)?,
        show_tooltip: if version.protocol() < 770 {
            Some(r.bool()?)
        } else {
            None
        },
    })
}
pub(super) fn write_jukebox(
    v: &JukeboxPlayable,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    write_either(&v.song, w, b, version.protocol() < 775, write_song)?;
    write_tooltip(v.show_tooltip, w, version)?;
    b.check_bytes(w)
}
fn color(v: i32) -> Result<i32> {
    if (0..=15).contains(&v) {
        Ok(v)
    } else {
        Err(Error::Invalid("banner dye color"))
    }
}
pub(super) fn read_banner_patterns(
    r: &mut Reader<'_>,
    _version: Version,
    b: &mut Budget,
) -> Result<Vec<BannerPatternLayer>> {
    let n = b.count(r)?;
    if n > r.remaining().len() / 2 {
        return Err(Error::Eof);
    }
    let mut layers = Vec::with_capacity(n);
    for _ in 0..n {
        layers.push(BannerPatternLayer {
            pattern: read_holder(r, b, |r, _| {
                Ok(BannerPattern {
                    asset_id: r.string(32767)?.into(),
                    translation_key: r.string(32767)?.into(),
                })
            })?,
            color_id: color(r.var_i32()?)?,
        });
    }
    Ok(layers)
}
pub(super) fn write_banner_patterns(
    v: &[BannerPatternLayer],
    w: &mut Writer,
    _version: Version,
    b: &mut Budget,
) -> Result<()> {
    b.write_count(v.len(), w)?;
    for layer in v {
        write_holder(&layer.pattern, w, b, |v, w, b| {
            string(w, &v.asset_id, b)?;
            string(w, &v.translation_key, b)
        })?;
        w.var_i32(color(layer.color_id)?);
        b.check_bytes(w)?;
    }
    Ok(())
}
/// Chicken variants, zombie-nautilus variants and damage types use a
/// holder-registry codec, so no zero-inline marker is present in the ID arm.
pub(super) fn read_registry_reference(
    r: &mut Reader<'_>,
    version: Version,
    _b: &mut Budget,
) -> Result<RegistryReference> {
    Ok(if version.protocol() < 775 && !r.bool()? {
        RegistryReference::Key(r.string(32767)?.into())
    } else {
        RegistryReference::RegistryId(nonnegative(r.var_i32()?, "negative registry reference ID")?)
    })
}
pub(super) fn write_registry_reference(
    v: &RegistryReference,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    match v {
        RegistryReference::Key(key) => {
            if version.protocol() >= 775 {
                return Err(Error::Unsupported("registry key in selected release"));
            }
            w.bool(false);
            string(w, key, b)?;
        }
        RegistryReference::RegistryId(id) => {
            if version.protocol() < 775 {
                w.bool(true);
            }
            w.var_i32(nonnegative(*id, "negative registry reference ID")?);
        }
    }
    b.check_bytes(w)
}
