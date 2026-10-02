//! Equipment, attributes and status-effect wire envelopes for protocols 763–776.
//!
//! Registry IDs remain version-specific, unresolved identities. These codecs do
//! not simulate attributes or effects. Equipment reuses the inventory codec and
//! its aggregate nested-item/NBT budgets; unsupported item layouts fail closed.
//! Layouts follow pinned schemas with release-server checks, including VarInt
//! attribute operations from 766 (the schemas still label them as signed bytes).
use super::inventory::{self, Budget, Slot};
use crate::{
    codec::{Reader, Writer},
    nbt::{Nbt, RootFormat, Tag},
    Error, Limits, Result, Version,
};
use std::collections::BTreeSet;

fn nonnegative(n: i32) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid("negative entity/registry ID or amplifier"))
    } else {
        Ok(n)
    }
}
fn finite(n: f64) -> Result<f64> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(Error::Invalid("non-finite attribute value"))
    }
}
fn identifier_parts(value: &str) -> (&str, &str) {
    let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value));
    (
        if namespace.is_empty() {
            "minecraft"
        } else {
            namespace
        },
        path,
    )
}
fn canonical_identifier(value: &str) -> String {
    let (namespace, path) = identifier_parts(value);
    format!("{namespace}:{path}")
}
fn identifier(value: &str, version: Version) -> Result<()> {
    let (namespace, path) = identifier_parts(value);
    let valid = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b);
    if (version.protocol() >= 775 && namespace == "..")
        || !namespace.bytes().all(valid)
        || !path.bytes().all(|b| valid(b) || b == b'/')
    {
        Err(Error::Invalid("attribute resource identifier"))
    } else {
        Ok(())
    }
}
fn read_identifier(r: &mut Reader<'_>, version: Version) -> Result<String> {
    let value = r.string(32767)?;
    identifier(value, version)?;
    Ok(value.to_owned())
}
fn write_identifier(w: &mut Writer, value: &str, version: Version, b: &Budget) -> Result<()> {
    identifier(value, version)?;
    if value.len() > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("attribute identifier bytes"));
    }
    w.string(value, 32767.min(b.limits.max_string_chars))?;
    b.check_bytes(w)
}
trait Body: Sized {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()>;
}
macro_rules! codec {
    ($($t:ty),* $(,)?) => {$ (
        impl $t {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet { return Err(Error::Limit("entity state packet bytes")); }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version, &mut Budget::new(limits))?;
                r.finish()?;
                Ok(value)
            }
            /// An error never exposes a partially encoded body.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                let mut b = Budget::new(limits);
                self.write(&mut w, version, &mut b)?;
                b.check_bytes(&w)?;
                Ok(w.into_inner())
            }
        }
    )*};
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentSlot {
    MainHand,
    OffHand,
    Feet,
    Legs,
    Chest,
    Head,
    /// Protocol 766 and later.
    Body,
    /// Protocol 770 and later.
    Saddle,
}
impl EquipmentSlot {
    pub fn from_id(id: u8, version: Version) -> Result<Self> {
        match id {
            0 => Ok(Self::MainHand),
            1 => Ok(Self::OffHand),
            2 => Ok(Self::Feet),
            3 => Ok(Self::Legs),
            4 => Ok(Self::Chest),
            5 => Ok(Self::Head),
            6 if version.protocol() >= 766 => Ok(Self::Body),
            7 if version.protocol() >= 770 => Ok(Self::Saddle),
            _ => Err(Error::Invalid("equipment slot for protocol")),
        }
    }
    pub fn id(self, version: Version) -> Result<u8> {
        let id = self as u8;
        Self::from_id(id, version)?;
        Ok(id)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentEntry {
    pub slot: EquipmentSlot,
    pub item: Slot,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EntityEquipment {
    pub entity_id: i32,
    /// Nonempty, unique slots in the transmitted order.
    pub equipment: Vec<EquipmentEntry>,
}
impl Body for EntityEquipment {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let entity_id = nonnegative(r.var_i32()?)?;
        let mut equipment = Vec::new();
        let mut seen = 0u8;
        loop {
            let header = r.u8()?;
            let slot = EquipmentSlot::from_id(header & 127, version)?;
            let bit = 1 << (header & 127);
            if seen & bit != 0 {
                return Err(Error::Invalid("duplicate equipment slot"));
            }
            seen |= bit;
            b.charge(1)?;
            equipment.push(EquipmentEntry {
                slot,
                item: inventory::read_slot(r, version, b, 0)?,
            });
            if header & 128 == 0 {
                break;
            }
        }
        Ok(Self {
            entity_id,
            equipment,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        if self.equipment.is_empty() {
            return Err(Error::Invalid("empty equipment update"));
        }
        w.var_i32(nonnegative(self.entity_id)?);
        let mut seen = 0u8;
        b.charge(self.equipment.len())?;
        for (i, entry) in self.equipment.iter().enumerate() {
            let id = entry.slot.id(version)?;
            if seen & (1 << id) != 0 {
                return Err(Error::Invalid("duplicate equipment slot"));
            }
            seen |= 1 << id;
            w.u8(id | if i + 1 < self.equipment.len() { 128 } else { 0 });
            inventory::write_slot(&entry.item, w, version, b, 0)?;
            b.check_bytes(w)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttributeKey {
    /// Protocols 763–765. Syntax is checked; registry membership is not.
    Resource(String),
    /// Protocol 766+. Raw, unresolved registry ID, not a stable semantic name.
    Registry(i32),
}
impl AttributeKey {
    fn identity(&self) -> Self {
        match self {
            Self::Resource(s) => Self::Resource(canonical_identifier(s)),
            Self::Registry(id) => Self::Registry(*id),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttributeModifierId {
    /// Protocols 763–766.
    Uuid([u8; 16]),
    /// Protocol 767+. Resource identifier, not a UUID string.
    Resource(String),
}
impl AttributeModifierId {
    fn identity(&self) -> Self {
        match self {
            Self::Resource(s) => Self::Resource(canonical_identifier(s)),
            Self::Uuid(id) => Self::Uuid(*id),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeOperation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}
impl AttributeOperation {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let id = if version.protocol() >= 766 {
            r.var_i32()?
        } else {
            r.u8()? as i32
        };
        match id {
            0 => Ok(Self::AddValue),
            1 => Ok(Self::AddMultipliedBase),
            2 => Ok(Self::AddMultipliedTotal),
            _ => Err(Error::Invalid("attribute modifier operation")),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeModifier {
    pub id: AttributeModifierId,
    pub amount: f64,
    pub operation: AttributeOperation,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EntityAttribute {
    pub key: AttributeKey,
    pub base_value: f64,
    pub modifiers: Vec<AttributeModifier>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EntityAttributes {
    pub entity_id: i32,
    pub attributes: Vec<EntityAttribute>,
}
impl Body for EntityAttributes {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let entity_id = nonnegative(r.var_i32()?)?;
        let count = b.count(r)?;
        let mut attributes = Vec::with_capacity(count.min(r.remaining().len() / 10));
        let mut seen = BTreeSet::new();
        for _ in 0..count {
            let key = if version.protocol() >= 766 {
                AttributeKey::Registry(nonnegative(r.var_i32()?)?)
            } else {
                AttributeKey::Resource(read_identifier(r, version)?)
            };
            if !seen.insert(key.identity()) {
                return Err(Error::Invalid("duplicate entity attribute"));
            }
            let base_value = finite(r.f64()?)?;
            let count = b.count(r)?;
            let mut modifiers = Vec::with_capacity(count.min(r.remaining().len() / 10));
            let mut seen = BTreeSet::new();
            for _ in 0..count {
                let id = if version.protocol() >= 767 {
                    AttributeModifierId::Resource(read_identifier(r, version)?)
                } else {
                    AttributeModifierId::Uuid(r.uuid()?)
                };
                if !seen.insert(id.identity()) {
                    return Err(Error::Invalid("duplicate attribute modifier"));
                }
                modifiers.push(AttributeModifier {
                    id,
                    amount: finite(r.f64()?)?,
                    operation: AttributeOperation::read(r, version)?,
                });
            }
            attributes.push(EntityAttribute {
                key,
                base_value,
                modifiers,
            });
        }
        Ok(Self {
            entity_id,
            attributes,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        b.write_count(self.attributes.len(), w)?;
        let mut seen = BTreeSet::new();
        for attr in &self.attributes {
            if !seen.insert(attr.key.identity()) {
                return Err(Error::Invalid("duplicate entity attribute"));
            }
            match &attr.key {
                AttributeKey::Resource(s) if version.protocol() < 766 => {
                    write_identifier(w, s, version, b)?
                }
                AttributeKey::Registry(id) if version.protocol() >= 766 => {
                    w.var_i32(nonnegative(*id)?)
                }
                _ => return Err(Error::Invalid("attribute key representation for protocol")),
            }
            w.f64(finite(attr.base_value)?);
            b.write_count(attr.modifiers.len(), w)?;
            let mut seen = BTreeSet::new();
            for modifier in &attr.modifiers {
                if !seen.insert(modifier.id.identity()) {
                    return Err(Error::Invalid("duplicate attribute modifier"));
                }
                match &modifier.id {
                    AttributeModifierId::Uuid(uuid) if version.protocol() < 767 => w.raw(uuid),
                    AttributeModifierId::Resource(s) if version.protocol() >= 767 => {
                        write_identifier(w, s, version, b)?
                    }
                    _ => {
                        return Err(Error::Invalid(
                            "attribute modifier ID representation for protocol",
                        ))
                    }
                }
                w.f64(finite(modifier.amount)?);
                if version.protocol() >= 766 {
                    w.var_i32(modifier.operation as i32);
                } else {
                    w.u8(modifier.operation as u8);
                }
                b.check_bytes(w)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectFlags {
    pub ambient: bool,
    pub visible: bool,
    pub show_icon: bool,
    /// Protocol 766+, requests blending when an effect is updated.
    pub blend: bool,
}
impl EffectFlags {
    pub fn from_mask(mask: u8, version: Version) -> Result<Self> {
        let allowed = if version.protocol() >= 766 { 15 } else { 7 };
        if mask & !allowed != 0 {
            return Err(Error::Invalid("effect flags for protocol"));
        }
        Ok(Self {
            ambient: mask & 1 != 0,
            visible: mask & 2 != 0,
            show_icon: mask & 4 != 0,
            blend: mask & 8 != 0,
        })
    }
    pub fn mask(self, version: Version) -> Result<u8> {
        let mask = u8::from(self.ambient)
            | (u8::from(self.visible) << 1)
            | (u8::from(self.show_icon) << 2)
            | (u8::from(self.blend) << 3);
        Self::from_mask(mask, version)?;
        Ok(mask)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct EntityEffect {
    pub entity_id: i32,
    /// Raw unresolved registry ID; IDs are version-specific.
    pub effect_id: i32,
    /// Protocols 763–765 use an unsigned byte (0–255); 766+ use a VarInt.
    pub amplifier: i32,
    /// Signed wire duration in ticks; -1 denotes an infinite duration.
    pub duration: i32,
    pub flags: EffectFlags,
    /// Protocols 763–765 only. Bounded opaque compound; factor fields are not
    /// interpreted or simulated. None is the wire's absent optional value.
    pub factor_data: Option<Nbt>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveEntityEffect {
    pub entity_id: i32,
    /// Raw unresolved registry ID; IDs are version-specific.
    pub effect_id: i32,
}
impl Body for EntityEffect {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let entity_id = nonnegative(r.var_i32()?)?;
        let effect_id = nonnegative(r.var_i32()?)?;
        let amplifier = if version.protocol() >= 766 {
            nonnegative(r.var_i32()?)?
        } else {
            r.u8()? as i32
        };
        let duration = r.var_i32()?;
        let flags = EffectFlags::from_mask(r.u8()?, version)?;
        let factor_data = if version.protocol() < 766 && r.bool()? {
            let nbt = inventory::read_nbt(r, RootFormat::for_version(version), b)?
                .ok_or(Error::Invalid("absent effect factor NBT"))?;
            if !matches!(nbt.root, Tag::Compound(_)) {
                return Err(Error::Invalid("effect factor NBT must be compound"));
            }
            Some(nbt)
        } else {
            None
        };
        Ok(Self {
            entity_id,
            effect_id,
            amplifier,
            duration,
            flags,
            factor_data,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        w.var_i32(nonnegative(self.effect_id)?);
        let amplifier = nonnegative(self.amplifier)?;
        if version.protocol() >= 766 {
            w.var_i32(amplifier);
        } else {
            w.u8(u8::try_from(amplifier)
                .map_err(|_| Error::Invalid("legacy effect amplifier range"))?);
        }
        w.var_i32(self.duration);
        w.u8(self.flags.mask(version)?);
        if version.protocol() < 766 {
            w.bool(self.factor_data.is_some());
            if let Some(nbt) = &self.factor_data {
                if !matches!(nbt.root, Tag::Compound(_)) {
                    return Err(Error::Invalid("effect factor NBT must be compound"));
                }
                inventory::write_nbt(Some(nbt), w, RootFormat::for_version(version), b)?;
            }
        } else if self.factor_data.is_some() {
            return Err(Error::Invalid("effect factor data removed in protocol 766"));
        }
        Ok(())
    }
}
impl Body for RemoveEntityEffect {
    fn read(r: &mut Reader<'_>, _version: Version, _b: &mut Budget) -> Result<Self> {
        Ok(Self {
            entity_id: nonnegative(r.var_i32()?)?,
            effect_id: nonnegative(r.var_i32()?)?,
        })
    }
    fn write(&self, w: &mut Writer, _version: Version, _b: &mut Budget) -> Result<()> {
        w.var_i32(nonnegative(self.entity_id)?);
        w.var_i32(nonnegative(self.effect_id)?);
        Ok(())
    }
}
codec!(
    EntityEquipment,
    EntityAttributes,
    EntityEffect,
    RemoveEntityEffect
);

#[derive(Clone, Debug, PartialEq)]
pub enum EntityStatePacket {
    Equipment(EntityEquipment),
    Attributes(EntityAttributes),
    Effect(EntityEffect),
    RemoveEffect(RemoveEntityEffect),
}
impl EntityStatePacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        match name {
            "entity_equipment" => Ok(Self::Equipment(EntityEquipment::decode(
                bytes, version, limits,
            )?)),
            "entity_update_attributes" => Ok(Self::Attributes(EntityAttributes::decode(
                bytes, version, limits,
            )?)),
            "entity_effect" => Ok(Self::Effect(EntityEffect::decode(bytes, version, limits)?)),
            "remove_entity_effect" => Ok(Self::RemoveEffect(RemoveEntityEffect::decode(
                bytes, version, limits,
            )?)),
            _ => Err(Error::Unsupported("typed entity state packet")),
        }
    }
}
