//! Auxiliary container packets and merchant offers, verified against release APIs.
//! These preserve wire values rather than applying menu or trading game rules.
use super::*;

// These packets' modern ContainerID codec accepts the complete signed VarInt
// domain. Do not reuse the stricter stateful inventory helpers above.
fn read_id(r: &mut Reader<'_>, version: Version) -> Result<i32> {
    if version.protocol() < 768 {
        Ok(i32::from(r.u8()?))
    } else {
        r.var_i32()
    }
}
fn write_id(id: i32, w: &mut Writer, version: Version) -> Result<()> {
    if version.protocol() < 768 {
        w.u8(u8::try_from(id).map_err(|_| Error::Invalid("byte container ID"))?);
    } else {
        w.var_i32(id);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerProperty {
    pub window_id: i32,
    pub property: i16,
    pub value: i16,
}
impl ContainerProperty {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let value = Self {
            window_id: read_id(&mut r, version)?,
            property: r.i16()?,
            value: r.i16()?,
        };
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        write_id(self.window_id, &mut w, version)?;
        w.i16(self.property);
        w.i16(self.value);
        finish(w, limits)
    }
}

/// `open_horse_window`, renamed to mount-screen opening in later releases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenMountScreen {
    pub window_id: i32,
    /// Slot count through protocol 766; inventory column count from 767.
    /// Kept as a signed wire value, without guessing the entity's layout.
    pub inventory_size: i32,
    pub entity_id: i32,
}
impl OpenMountScreen {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let value = Self {
            window_id: read_id(&mut r, version)?,
            inventory_size: r.var_i32()?,
            entity_id: r.i32()?,
        };
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        write_id(self.window_id, &mut w, version)?;
        w.var_i32(self.inventory_size);
        w.i32(self.entity_id);
        finish(w, limits)
    }
}

/// Serverbound `enchant_item`; both fields are signed bytes through 765 and
/// signed VarInts from 766, even though historical schemas describe bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerButtonClick {
    pub window_id: i32,
    pub button_id: i32,
}
impl ContainerButtonClick {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let (window_id, button_id) = if version.protocol() < 766 {
            (r.u8()? as i8 as i32, r.u8()? as i8 as i32)
        } else {
            (r.var_i32()?, r.var_i32()?)
        };
        r.finish()?;
        Ok(Self {
            window_id,
            button_id,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        for field in [self.window_id, self.button_id] {
            if version.protocol() < 766 {
                w.u8(i8::try_from(field)
                    .map_err(|_| Error::Invalid("signed-byte container button field"))?
                    as u8);
            } else {
                w.var_i32(field);
            }
        }
        finish(w, limits)
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::super::named(
            version,
            State::Play,
            "enchant_item",
            self.encode(version, limits)?,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectTrade {
    pub slot: i32,
}
impl SelectTrade {
    pub fn decode(bytes: &[u8], _version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let value = Self { slot: r.var_i32()? };
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, _version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        w.var_i32(self.slot);
        finish(w, limits)
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::super::named(
            version,
            State::Play,
            "select_trade",
            self.encode(version, limits)?,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CooldownTarget {
    /// Item registry ID through protocol 767.
    Item(i32),
    /// Resource identifier from protocol 768; no registry lookup is performed.
    Group(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetCooldown {
    pub target: CooldownTarget,
    /// Preserves the signed VarInt wire domain, including zero and negatives.
    pub ticks: i32,
}
impl SetCooldown {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let target = if version.protocol() < 768 {
            CooldownTarget::Item(nonnegative(r.var_i32()?, "cooldown item ID")?)
        } else {
            let group = r.string(32767)?.to_owned();
            crate::codec::identifier::validate(&group, version)?;
            CooldownTarget::Group(group)
        };
        let ticks = r.var_i32()?;
        r.finish()?;
        Ok(Self { target, ticks })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        match &self.target {
            CooldownTarget::Item(id) if version.protocol() < 768 => {
                w.var_i32(nonnegative(*id, "cooldown item ID")?)
            }
            CooldownTarget::Group(group) if version.protocol() >= 768 => {
                crate::codec::identifier::validate(group, version)?;
                string(&mut w, group, &Budget::new(limits))?;
            }
            _ => return Err(Error::Unsupported("cooldown target in selected release")),
        }
        w.var_i32(self.ticks);
        finish(w, limits)
    }
}

/// Modern merchant cost (766+), not a Slot or component patch. There is no
/// empty sentinel or removed-component count, and counts may be nonpositive.
#[derive(Clone, Debug, PartialEq)]
pub struct MerchantItemCost {
    pub item_id: i32,
    pub count: i32,
    /// Exact component values in wire order, sharing the packet's budgets.
    pub components: Vec<Component>,
}
fn read_cost(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<MerchantItemCost> {
    b.charge(1)?;
    let item_id = nonnegative(r.var_i32()?, "merchant item ID")?;
    let count = r.var_i32()?;
    let n = b.count(r)?;
    let mut components = Vec::new();
    for _ in 0..n {
        b.depth(1)?;
        let (name, wire) = component_type(version, r.var_i32()?)?;
        let value = read_component(r, wire, version, b, 1)?;
        components.push(Component { name, value });
    }
    Ok(MerchantItemCost {
        item_id,
        count,
        components,
    })
}
fn write_cost(
    value: &MerchantItemCost,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    b.charge(1)?;
    w.var_i32(nonnegative(value.item_id, "merchant item ID")?);
    w.var_i32(value.count);
    b.write_count(value.components.len(), w)?;
    for component in &value.components {
        b.depth(1)?;
        let id = component_id(version, component.name)?;
        let (_, wire) = component_type(version, id)?;
        w.var_i32(id);
        write_component(&component.value, wire, w, version, b, 1)?;
    }
    b.check_bytes(w)
}

#[derive(Clone, Debug, PartialEq)]
pub enum MerchantInputs {
    /// Slots through protocol 765; an absent second input is `Slot::Empty`.
    Legacy { first: Slot, second: Slot },
    /// Required first cost and boolean-prefixed optional second cost from 766.
    Components {
        first: MerchantItemCost,
        second: Option<MerchantItemCost>,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct MerchantOffer {
    pub inputs: MerchantInputs,
    /// Modern releases require a nonempty result; legacy releases use an ordinary Slot.
    pub result: Slot,
    pub disabled: bool,
    pub uses: i32,
    pub max_uses: i32,
    pub xp: i32,
    pub special_price: i32,
    /// All IEEE-754 bit patterns are retained, including NaN payloads.
    pub price_multiplier: f32,
    pub demand: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MerchantOffers {
    /// Signed VarInt in every supported release, including 766–767.
    pub window_id: i32,
    pub offers: Vec<MerchantOffer>,
    pub villager_level: i32,
    pub experience: i32,
    pub show_progress: bool,
    pub can_restock: bool,
}
impl MerchantOffers {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let mut b = Budget::new(limits);
        let window_id = r.var_i32()?;
        let n = b.count(&mut r)?;
        let mut offers = Vec::new();
        for _ in 0..n {
            let (inputs, result) = if version.protocol() < 766 {
                let first = read_slot(&mut r, version, &mut b, 0)?;
                let result = read_slot(&mut r, version, &mut b, 0)?;
                let second = read_slot(&mut r, version, &mut b, 0)?;
                (MerchantInputs::Legacy { first, second }, result)
            } else {
                let first = read_cost(&mut r, version, &mut b)?;
                let result = read_slot(&mut r, version, &mut b, 0)?;
                if matches!(result, Slot::Empty) {
                    return Err(Error::Invalid("empty merchant result"));
                }
                let second = if r.bool()? {
                    Some(read_cost(&mut r, version, &mut b)?)
                } else {
                    None
                };
                (MerchantInputs::Components { first, second }, result)
            };
            offers.push(MerchantOffer {
                inputs,
                result,
                disabled: r.bool()?,
                uses: r.i32()?,
                max_uses: r.i32()?,
                xp: r.i32()?,
                special_price: r.i32()?,
                price_multiplier: r.f32()?,
                demand: r.i32()?,
            });
        }
        let value = Self {
            window_id,
            offers,
            villager_level: r.var_i32()?,
            experience: r.var_i32()?,
            show_progress: r.bool()?,
            can_restock: r.bool()?,
        };
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        w.var_i32(self.window_id);
        b.write_count(self.offers.len(), &mut w)?;
        for offer in &self.offers {
            match &offer.inputs {
                MerchantInputs::Legacy { first, second } if version.protocol() < 766 => {
                    write_slot(first, &mut w, version, &mut b, 0)?;
                    write_slot(&offer.result, &mut w, version, &mut b, 0)?;
                    write_slot(second, &mut w, version, &mut b, 0)?;
                }
                MerchantInputs::Components { first, second } if version.protocol() >= 766 => {
                    if matches!(offer.result, Slot::Empty) {
                        return Err(Error::Invalid("empty merchant result"));
                    }
                    write_cost(first, &mut w, version, &mut b)?;
                    write_slot(&offer.result, &mut w, version, &mut b, 0)?;
                    w.bool(second.is_some());
                    if let Some(second) = second {
                        write_cost(second, &mut w, version, &mut b)?;
                    }
                }
                _ => {
                    return Err(Error::Unsupported(
                        "merchant input format in selected release",
                    ))
                }
            }
            w.bool(offer.disabled);
            w.i32(offer.uses);
            w.i32(offer.max_uses);
            w.i32(offer.xp);
            w.i32(offer.special_price);
            w.f32(offer.price_multiplier);
            w.i32(offer.demand);
            b.check_bytes(&w)?;
        }
        w.var_i32(self.villager_level);
        w.var_i32(self.experience);
        w.bool(self.show_progress);
        w.bool(self.can_restock);
        finish(w, limits)
    }
}
