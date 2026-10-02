//! Consumption, equipment, combat, and profile components. Registry references
//! retain unshifted IDs. All nested collections share the enclosing item budget.
//! Release boundaries were checked against the corresponding server stream
//! codecs, including bundle animation in 769, spear in 774, and holder-set
//! shield bypasses in 775 (the pinned schemas lag these changes).
//!
//! Verification sources: Consumable, ConsumeEffect and its five registered
//! effect codecs, Equippable, EquipmentSlot, ItemUseAnimation, SoundEvent,
//! BlocksAttacks and its DamageReduction/ItemDamageFunction records, Weapon,
//! UseEffects, AttackRange, PiercingWeapon, KineticWeapon and its Condition,
//! SwingAnimation, ResolvableProfile, ByteBufCodecs and DataComponents from
//! the cached 1.20.6, 1.21.3, 1.21.4, 1.21.5, 1.21.6, 1.21.8, 1.21.10,
//! 1.21.11, 26.1.2 and 26.2 release artifacts. Only original wire models are
//! included; no implementation or disassembly is distributed.
use super::{components, nonnegative, string, Budget, ComponentValue, ComponentWire, PotionEffect};
use crate::{
    codec::{Reader, Writer},
    packet::{
        entity_metadata::holders::{
            self, read_holder_set, write_holder_set, PlayerSkinPatch, RegistryHolder,
            RegistryHolderSet, ResolvableProfile,
        },
        ProfileProperty,
    },
    Error, Result, Version,
};

#[derive(Clone, Debug, PartialEq)]
pub struct SoundEvent {
    pub name: String,
    pub fixed_range: Option<f32>,
}
/// Sound reference or inline name and optional fixed range. Registry IDs are
/// unshifted in this type; the wire stores ID + 1 and reserves zero for inline.
pub type SoundHolder = RegistryHolder<SoundEvent>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum UseAnimation {
    None = 0,
    Eat = 1,
    Drink = 2,
    Block = 3,
    Bow = 4,
    /// Called `spear` before protocol 774, when a distinct spear animation was added.
    Trident = 5,
    Crossbow = 6,
    Spyglass = 7,
    TootHorn = 8,
    Brush = 9,
    /// Protocol 769 and later.
    Bundle = 10,
    /// Protocol 774 and later.
    Spear = 11,
}
impl UseAnimation {
    fn from_id(id: i32, version: Version) -> Result<Self> {
        Ok(match id {
            0 => Self::None,
            1 => Self::Eat,
            2 => Self::Drink,
            3 => Self::Block,
            4 => Self::Bow,
            5 => Self::Trident,
            6 => Self::Crossbow,
            7 => Self::Spyglass,
            8 => Self::TootHorn,
            9 => Self::Brush,
            10 if version.protocol() >= 769 => Self::Bundle,
            11 if version.protocol() >= 774 => Self::Spear,
            _ => return Err(Error::Invalid("item use animation in selected release")),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum ConsumeEffect {
    ApplyEffects {
        effects: Vec<PotionEffect>,
        probability: f32,
    },
    RemoveEffects(RegistryHolderSet),
    ClearAllEffects,
    TeleportRandomly {
        diameter: f32,
    },
    PlaySound(SoundHolder),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Consumable {
    pub consume_seconds: f32,
    pub animation: UseAnimation,
    pub sound: SoundHolder,
    pub makes_particles: bool,
    pub effects: Vec<ConsumeEffect>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct UseCooldown {
    pub seconds: f32,
    pub group: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
/// EquipmentSlot's stream-codec IDs, which differ from the ordinal IDs used
/// by the entity-equipment packet. Verified in every release from 768 to 776.
pub enum ItemEquipmentSlot {
    MainHand = 0,
    Feet = 1,
    Legs = 2,
    Chest = 3,
    Head = 4,
    OffHand = 5,
    Body = 6,
    /// Introduced in protocol 770.
    Saddle = 7,
}
impl ItemEquipmentSlot {
    fn from_id(id: i32, version: Version) -> Result<Self> {
        Ok(match id {
            0 => Self::MainHand,
            1 => Self::Feet,
            2 => Self::Legs,
            3 => Self::Chest,
            4 => Self::Head,
            5 => Self::OffHand,
            6 => Self::Body,
            7 if version.protocol() >= 770 => Self::Saddle,
            _ => return Err(Error::Invalid("item equipment slot in selected release")),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Equippable {
    pub slot: ItemEquipmentSlot,
    pub equip_sound: SoundHolder,
    pub asset_id: Option<String>,
    pub camera_overlay: Option<String>,
    pub allowed_entities: Option<RegistryHolderSet>,
    pub dispensable: bool,
    pub swappable: bool,
    pub damage_on_hurt: bool,
    /// Required from 770; absent in 768–769.
    pub equip_on_interact: Option<bool>,
    /// Required from 771; absent before it.
    pub shearing: Option<EquipmentShearing>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct EquipmentShearing {
    pub shearable: bool,
    pub sound: SoundHolder,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    pub item_damage_per_attack: i32,
    pub disable_blocking_seconds: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AttackRange {
    pub min_range: f32,
    pub max_range: f32,
    pub min_creative_range: f32,
    pub max_creative_range: f32,
    pub hitbox_margin: f32,
    pub mob_factor: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DamageReduction {
    pub horizontal_blocking_angle: f32,
    pub damage_types: Option<RegistryHolderSet>,
    pub base: f32,
    pub factor: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ItemDamageFunction {
    pub threshold: f32,
    pub base: f32,
    pub factor: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct BlocksAttacks {
    pub block_delay_seconds: f32,
    pub disable_cooldown_scale: f32,
    pub damage_reductions: Vec<DamageReduction>,
    pub item_damage: ItemDamageFunction,
    /// Before 775 only `RegistryHolderSet::Tag` is supported, encoded without a holder-set marker.
    pub bypassed_by: Option<RegistryHolderSet>,
    pub block_sound: Option<SoundHolder>,
    pub disable_sound: Option<SoundHolder>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct UseEffects {
    pub can_sprint: bool,
    pub interact_vibrations: bool,
    pub speed_multiplier: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PiercingWeapon {
    pub deals_knockback: bool,
    pub dismounts: bool,
    pub sound: Option<SoundHolder>,
    pub hit_sound: Option<SoundHolder>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct KineticWeaponCondition {
    pub max_duration_ticks: i32,
    pub min_speed: f32,
    pub min_relative_speed: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct KineticWeapon {
    pub contact_cooldown_ticks: i32,
    pub delay_ticks: i32,
    pub dismount_conditions: Option<KineticWeaponCondition>,
    pub knockback_conditions: Option<KineticWeaponCondition>,
    pub damage_conditions: Option<KineticWeaponCondition>,
    pub forward_movement: f32,
    pub damage_multiplier: f32,
    pub sound: Option<SoundHolder>,
    pub hit_sound: Option<SoundHolder>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum SwingAnimationType {
    None = 0,
    Whack = 1,
    Stab = 2,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwingAnimation {
    pub kind: SwingAnimationType,
    pub duration: i32,
}

fn optional<T>(
    r: &mut Reader<'_>,
    read: impl FnOnce(&mut Reader<'_>) -> Result<T>,
) -> Result<Option<T>> {
    Ok(if r.bool()? { Some(read(r)?) } else { None })
}
fn write_optional<T>(
    x: Option<&T>,
    w: &mut Writer,
    b: &mut Budget,
    write: impl FnOnce(&T, &mut Writer, &mut Budget) -> Result<()>,
) -> Result<()> {
    w.bool(x.is_some());
    if let Some(x) = x {
        write(x, w, b)?;
    }
    b.check_bytes(w)
}
fn read_string(r: &mut Reader<'_>) -> Result<String> {
    Ok(r.string(32767)?.into())
}
fn limited_string(x: &str, max: usize, w: &mut Writer, b: &Budget) -> Result<()> {
    if x.encode_utf16().count() > max {
        return Err(Error::Limit("profile string length"));
    }
    string(w, x, b)
}
pub(crate) fn read_sound_event(r: &mut Reader<'_>) -> Result<SoundEvent> {
    Ok(SoundEvent {
        name: read_string(r)?,
        fixed_range: optional(r, |r| r.f32())?,
    })
}
pub(crate) fn write_sound_event(x: &SoundEvent, w: &mut Writer, b: &mut Budget) -> Result<()> {
    string(w, &x.name, b)?;
    w.bool(x.fixed_range.is_some());
    if let Some(range) = x.fixed_range {
        w.f32(range);
    }
    b.check_bytes(w)
}
pub(crate) fn read_sound(r: &mut Reader<'_>, _b: &mut Budget) -> Result<SoundHolder> {
    let marker = nonnegative(r.var_i32()?, "sound holder ID")?;
    Ok(if marker == 0 {
        RegistryHolder::Inline(read_sound_event(r)?)
    } else {
        RegistryHolder::RegistryId(marker - 1)
    })
}
pub(crate) fn write_sound(x: &SoundHolder, w: &mut Writer, b: &mut Budget) -> Result<()> {
    match x {
        RegistryHolder::RegistryId(id) => w.var_i32(
            nonnegative(*id, "sound holder ID")?
                .checked_add(1)
                .ok_or(Error::Invalid("sound holder ID overflow"))?,
        ),
        RegistryHolder::Inline(x) => {
            w.var_i32(0);
            write_sound_event(x, w, b)?;
        }
    }
    b.check_bytes(w)
}
fn read_effects(r: &mut Reader<'_>, b: &mut Budget, depth: usize) -> Result<Vec<ConsumeEffect>> {
    b.depth(depth)?;
    let count = b.count(r)?;
    let mut out = Vec::new();
    for _ in 0..count {
        out.push(match r.var_i32()? {
            0 => {
                let n = b.count(r)?;
                let mut effects = Vec::new();
                for _ in 0..n {
                    effects.push(components::read_effect(r, b, depth + 1)?);
                }
                ConsumeEffect::ApplyEffects {
                    effects,
                    probability: r.f32()?,
                }
            }
            1 => ConsumeEffect::RemoveEffects(read_holder_set(r, b)?),
            2 => ConsumeEffect::ClearAllEffects,
            3 => ConsumeEffect::TeleportRandomly { diameter: r.f32()? },
            4 => ConsumeEffect::PlaySound(read_sound(r, b)?),
            _ => return Err(Error::Unsupported("consume effect type")),
        });
    }
    Ok(out)
}
fn write_effects(x: &[ConsumeEffect], w: &mut Writer, b: &mut Budget, depth: usize) -> Result<()> {
    b.depth(depth)?;
    b.write_count(x.len(), w)?;
    for x in x {
        match x {
            ConsumeEffect::ApplyEffects {
                effects,
                probability,
            } => {
                w.var_i32(0);
                b.write_count(effects.len(), w)?;
                for effect in effects {
                    components::write_effect(effect, w, b, depth + 1)?;
                }
                w.f32(*probability);
            }
            ConsumeEffect::RemoveEffects(set) => {
                w.var_i32(1);
                write_holder_set(set, w, b)?;
            }
            ConsumeEffect::ClearAllEffects => w.var_i32(2),
            ConsumeEffect::TeleportRandomly { diameter } => {
                w.var_i32(3);
                w.f32(*diameter);
            }
            ConsumeEffect::PlaySound(sound) => {
                w.var_i32(4);
                write_sound(sound, w, b)?;
            }
        }
        b.check_bytes(w)?;
    }
    Ok(())
}
fn read_equippable(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Equippable> {
    Ok(Equippable {
        slot: ItemEquipmentSlot::from_id(r.var_i32()?, version)?,
        equip_sound: read_sound(r, b)?,
        asset_id: optional(r, read_string)?,
        camera_overlay: optional(r, read_string)?,
        allowed_entities: optional(r, |r| read_holder_set(r, b))?,
        dispensable: r.bool()?,
        swappable: r.bool()?,
        damage_on_hurt: r.bool()?,
        equip_on_interact: if version.protocol() >= 770 {
            Some(r.bool()?)
        } else {
            None
        },
        shearing: if version.protocol() >= 771 {
            Some(EquipmentShearing {
                shearable: r.bool()?,
                sound: read_sound(r, b)?,
            })
        } else {
            None
        },
    })
}
fn write_equippable(
    x: &Equippable,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    if x.equip_on_interact.is_some() != (version.protocol() >= 770)
        || x.shearing.is_some() != (version.protocol() >= 771)
    {
        return Err(Error::Invalid("equippable fields in selected release"));
    }
    ItemEquipmentSlot::from_id(x.slot as i32, version)?;
    w.var_i32(x.slot as i32);
    write_sound(&x.equip_sound, w, b)?;
    write_optional(x.asset_id.as_ref(), w, b, |x, w, b| string(w, x, b))?;
    write_optional(x.camera_overlay.as_ref(), w, b, |x, w, b| string(w, x, b))?;
    write_optional(x.allowed_entities.as_ref(), w, b, write_holder_set)?;
    w.bool(x.dispensable);
    w.bool(x.swappable);
    w.bool(x.damage_on_hurt);
    if let Some(v) = x.equip_on_interact {
        w.bool(v);
    }
    if let Some(v) = &x.shearing {
        w.bool(v.shearable);
        write_sound(&v.sound, w, b)?;
    }
    b.check_bytes(w)
}
fn read_blocks_attacks(
    r: &mut Reader<'_>,
    version: Version,
    b: &mut Budget,
) -> Result<BlocksAttacks> {
    let block_delay_seconds = r.f32()?;
    let disable_cooldown_scale = r.f32()?;
    let n = b.count(r)?;
    let mut damage_reductions = Vec::new();
    for _ in 0..n {
        damage_reductions.push(DamageReduction {
            horizontal_blocking_angle: r.f32()?,
            damage_types: optional(r, |r| read_holder_set(r, b))?,
            base: r.f32()?,
            factor: r.f32()?,
        });
    }
    Ok(BlocksAttacks {
        block_delay_seconds,
        disable_cooldown_scale,
        damage_reductions,
        item_damage: ItemDamageFunction {
            threshold: r.f32()?,
            base: r.f32()?,
            factor: r.f32()?,
        },
        bypassed_by: optional(r, |r| {
            if version.protocol() >= 775 {
                read_holder_set(r, b)
            } else {
                Ok(RegistryHolderSet::Tag(read_string(r)?))
            }
        })?,
        block_sound: optional(r, |r| read_sound(r, b))?,
        disable_sound: optional(r, |r| read_sound(r, b))?,
    })
}
fn write_blocks_attacks(
    x: &BlocksAttacks,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    w.f32(x.block_delay_seconds);
    w.f32(x.disable_cooldown_scale);
    b.write_count(x.damage_reductions.len(), w)?;
    for x in &x.damage_reductions {
        w.f32(x.horizontal_blocking_angle);
        write_optional(x.damage_types.as_ref(), w, b, write_holder_set)?;
        w.f32(x.base);
        w.f32(x.factor);
        b.check_bytes(w)?;
    }
    w.f32(x.item_damage.threshold);
    w.f32(x.item_damage.base);
    w.f32(x.item_damage.factor);
    write_optional(x.bypassed_by.as_ref(), w, b, |x, w, b| {
        if version.protocol() >= 775 {
            write_holder_set(x, w, b)
        } else if let RegistryHolderSet::Tag(tag) = x {
            string(w, tag, b)
        } else {
            Err(Error::Invalid(
                "shield bypass requires a tag before protocol 775",
            ))
        }
    })?;
    write_optional(x.block_sound.as_ref(), w, b, write_sound)?;
    write_optional(x.disable_sound.as_ref(), w, b, write_sound)
}
fn read_condition(r: &mut Reader<'_>) -> Result<KineticWeaponCondition> {
    Ok(KineticWeaponCondition {
        max_duration_ticks: r.var_i32()?,
        min_speed: r.f32()?,
        min_relative_speed: r.f32()?,
    })
}
fn write_condition(x: &KineticWeaponCondition, w: &mut Writer, b: &mut Budget) -> Result<()> {
    w.var_i32(x.max_duration_ticks);
    w.f32(x.min_speed);
    w.f32(x.min_relative_speed);
    b.check_bytes(w)
}
fn read_profile(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<ResolvableProfile> {
    if version.protocol() >= 773 {
        return holders::read_profile(r, version, b);
    }
    let name = optional(r, |r| Ok(r.string(16)?.into()))?;
    let uuid = optional(r, |r| r.uuid())?;
    let n = r.count(16.min(b.limits.max_collection))?;
    b.charge(n)?;
    let mut properties = Vec::new();
    for _ in 0..n {
        properties.push(ProfileProperty {
            name: r.string(64)?.into(),
            value: r.string(32767)?.into(),
            signature: optional(r, |r| Ok(r.string(1024)?.into()))?,
        });
    }
    Ok(ResolvableProfile::Partial {
        name,
        uuid,
        properties,
        skin_patch: PlayerSkinPatch::default(),
    })
}
fn write_profile(
    x: &ResolvableProfile,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    if version.protocol() >= 773 {
        return holders::write_profile(x, w, version, b);
    }
    let ResolvableProfile::Partial {
        name,
        uuid,
        properties,
        skin_patch,
    } = x
    else {
        return Err(Error::Invalid("complete profile requires protocol 773"));
    };
    if *skin_patch != PlayerSkinPatch::default() {
        return Err(Error::Invalid("profile skin patch requires protocol 773"));
    }
    write_optional(name.as_ref(), w, b, |x, w, b| limited_string(x, 16, w, b))?;
    w.bool(uuid.is_some());
    if let Some(uuid) = uuid {
        w.raw(uuid);
    }
    if properties.len() > 16 {
        return Err(Error::Limit("profile property count"));
    }
    b.write_count(properties.len(), w)?;
    for p in properties {
        limited_string(&p.name, 64, w, b)?;
        limited_string(&p.value, 32767, w, b)?;
        write_optional(p.signature.as_ref(), w, b, |x, w, b| {
            limited_string(x, 1024, w, b)
        })?;
    }
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
        W::Sound => V::Sound(read_sound(r, b)?),
        W::Consumable => V::Consumable(Consumable {
            consume_seconds: r.f32()?,
            animation: UseAnimation::from_id(r.var_i32()?, version)?,
            sound: read_sound(r, b)?,
            makes_particles: r.bool()?,
            effects: read_effects(r, b, depth + 1)?,
        }),
        W::UseCooldown => V::UseCooldown(UseCooldown {
            seconds: r.f32()?,
            group: optional(r, read_string)?,
        }),
        W::Equippable => V::Equippable(read_equippable(r, version, b)?),
        W::DeathProtection => V::DeathProtection(read_effects(r, b, depth + 1)?),
        W::Weapon => V::Weapon(Weapon {
            item_damage_per_attack: r.var_i32()?,
            disable_blocking_seconds: r.f32()?,
        }),
        W::AttackRange => V::AttackRange(AttackRange {
            min_range: r.f32()?,
            max_range: r.f32()?,
            min_creative_range: r.f32()?,
            max_creative_range: r.f32()?,
            hitbox_margin: r.f32()?,
            mob_factor: r.f32()?,
        }),
        W::BlocksAttacks => V::BlocksAttacks(read_blocks_attacks(r, version, b)?),
        W::UseEffects => V::UseEffects(UseEffects {
            can_sprint: r.bool()?,
            interact_vibrations: r.bool()?,
            speed_multiplier: r.f32()?,
        }),
        W::PiercingWeapon => V::PiercingWeapon(PiercingWeapon {
            deals_knockback: r.bool()?,
            dismounts: r.bool()?,
            sound: optional(r, |r| read_sound(r, b))?,
            hit_sound: optional(r, |r| read_sound(r, b))?,
        }),
        W::KineticWeapon => V::KineticWeapon(KineticWeapon {
            contact_cooldown_ticks: r.var_i32()?,
            delay_ticks: r.var_i32()?,
            dismount_conditions: optional(r, read_condition)?,
            knockback_conditions: optional(r, read_condition)?,
            damage_conditions: optional(r, read_condition)?,
            forward_movement: r.f32()?,
            damage_multiplier: r.f32()?,
            sound: optional(r, |r| read_sound(r, b))?,
            hit_sound: optional(r, |r| read_sound(r, b))?,
        }),
        W::SwingAnimation => V::SwingAnimation(SwingAnimation {
            kind: match r.var_i32()? {
                0 => SwingAnimationType::None,
                1 => SwingAnimationType::Whack,
                2 => SwingAnimationType::Stab,
                _ => return Err(Error::Invalid("swing animation")),
            },
            duration: r.var_i32()?,
        }),
        W::Profile => V::Profile(read_profile(r, version, b)?),
        W::PaintingVariant => V::PaintingVariant(holders::read_painting(r, version, b)?),
        _ => return Err(Error::Unsupported("extended item component")),
    })
}
pub(super) fn write(
    x: &ComponentValue,
    wire: ComponentWire,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
    depth: usize,
) -> Result<()> {
    use ComponentValue as V;
    use ComponentWire as W;
    match (wire, x) {
        (W::Sound, V::Sound(x)) => write_sound(x, w, b)?,
        (W::Consumable, V::Consumable(x)) => {
            UseAnimation::from_id(x.animation as i32, version)?;
            w.f32(x.consume_seconds);
            w.var_i32(x.animation as i32);
            write_sound(&x.sound, w, b)?;
            w.bool(x.makes_particles);
            write_effects(&x.effects, w, b, depth + 1)?;
        }
        (W::UseCooldown, V::UseCooldown(x)) => {
            w.f32(x.seconds);
            write_optional(x.group.as_ref(), w, b, |x, w, b| string(w, x, b))?;
        }
        (W::Equippable, V::Equippable(x)) => write_equippable(x, w, version, b)?,
        (W::DeathProtection, V::DeathProtection(x)) => write_effects(x, w, b, depth + 1)?,
        (W::Weapon, V::Weapon(x)) => {
            w.var_i32(x.item_damage_per_attack);
            w.f32(x.disable_blocking_seconds);
        }
        (W::AttackRange, V::AttackRange(x)) => {
            for v in [
                x.min_range,
                x.max_range,
                x.min_creative_range,
                x.max_creative_range,
                x.hitbox_margin,
                x.mob_factor,
            ] {
                w.f32(v);
            }
        }
        (W::BlocksAttacks, V::BlocksAttacks(x)) => write_blocks_attacks(x, w, version, b)?,
        (W::UseEffects, V::UseEffects(x)) => {
            w.bool(x.can_sprint);
            w.bool(x.interact_vibrations);
            w.f32(x.speed_multiplier);
        }
        (W::PiercingWeapon, V::PiercingWeapon(x)) => {
            w.bool(x.deals_knockback);
            w.bool(x.dismounts);
            write_optional(x.sound.as_ref(), w, b, write_sound)?;
            write_optional(x.hit_sound.as_ref(), w, b, write_sound)?;
        }
        (W::KineticWeapon, V::KineticWeapon(x)) => {
            w.var_i32(x.contact_cooldown_ticks);
            w.var_i32(x.delay_ticks);
            write_optional(x.dismount_conditions.as_ref(), w, b, write_condition)?;
            write_optional(x.knockback_conditions.as_ref(), w, b, write_condition)?;
            write_optional(x.damage_conditions.as_ref(), w, b, write_condition)?;
            w.f32(x.forward_movement);
            w.f32(x.damage_multiplier);
            write_optional(x.sound.as_ref(), w, b, write_sound)?;
            write_optional(x.hit_sound.as_ref(), w, b, write_sound)?;
        }
        (W::SwingAnimation, V::SwingAnimation(x)) => {
            w.var_i32(x.kind as i32);
            w.var_i32(x.duration);
        }
        (W::Profile, V::Profile(x)) => write_profile(x, w, version, b)?,
        (W::PaintingVariant, V::PaintingVariant(x)) => holders::write_painting(x, w, version, b)?,
        _ => return Err(Error::Invalid("component value does not match wire layout")),
    }
    b.check_bytes(w)
}
