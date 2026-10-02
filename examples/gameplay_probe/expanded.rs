//! Value assertions for the bounded, offline-only release-server scenario.
use super::record;
use rustwire_mc::{
    packet::{
        entity_metadata::holders::{RegistryHolder, RegistryHolderSet, ResolvableProfile},
        entity_state::{AttributeModifierId, AttributeOperation, EntityStatePacket, EquipmentSlot},
        inventory::{
            BlockPropertyValue, ComponentPredicateKind, ComponentValue, ConsumeEffect, HolderOrKey,
            InstrumentDuration, ItemData, ItemEquipmentSlot, Slot, UseAnimation,
        },
        player::{GameMode, PlayerPacket},
    },
    registry::RegistryStore,
};
use std::collections::{BTreeMap, BTreeSet};

fn value(
    counts: &mut BTreeMap<&'static str, u64>,
    category: &'static str,
    details: impl std::fmt::Display,
) {
    record(counts, category);
    println!("VALUE category={category} {details}");
}
fn marker(nbt: &rustwire_mc::nbt::Nbt) -> bool {
    nbt.root.get("rustwire_probe").and_then(|v| v.as_i32()) == Some(1)
}
fn partial_marker(nbt: &rustwire_mc::nbt::Nbt) -> bool {
    marker(nbt)
        || matches!(&nbt.root, rustwire_mc::nbt::Tag::String(s)
        if s.to_string_lossy().replace(' ', "") == "{rustwire_probe:1}")
}
fn named<T>(
    holder: &RegistryHolder<T>,
    registry: &str,
    key: &str,
    registries: &RegistryStore,
) -> bool {
    match holder {
        RegistryHolder::RegistryId(id) => registries
            .registries
            .get(registry)
            .and_then(|r| r.by_id(*id as u32))
            .is_some_and(|entry| entry.key == key),
        _ => false,
    }
}
pub fn slot(slot: &Slot, registries: &RegistryStore, counts: &mut BTreeMap<&'static str, u64>) {
    let Slot::Item(item) = slot else { return };
    let ItemData::Components(patch) = &item.data else {
        return;
    };
    for component in &patch.added {
        if matches!(
            component.name,
            "can_break"
                | "can_place_on"
                | "trim"
                | "banner_patterns"
                | "profile"
                | "instrument"
                | "jukebox_playable"
                | "consumable"
                | "equippable"
                | "use_cooldown"
                | "death_protection"
                | "weapon"
                | "blocks_attacks"
        ) {
            println!(
                "COMPONENT item_id={} name={} value={:?}",
                item.item_id, component.name, component.value
            );
        }
        match &component.value {
            ComponentValue::BlockPredicates(v) => {
                for p in &v.predicates {
                    if component.name == "can_break"
                        && p.nbt.as_ref().is_some_and(marker)
                        && p.properties.as_ref().is_some_and(|props| {
                            props.iter().any(|p| {
                                p.name == "axis" && p.value == BlockPropertyValue::Exact("x".into())
                            })
                        })
                        && matches!(&p.blocks, Some(RegistryHolderSet::Ids(ids)) if ids.len() == 1)
                    {
                        value(
                            counts,
                            "item_block_predicate",
                            "component=can_break axis=x nbt_marker=1 block_ids=1",
                        );
                    }
                    if component.name == "can_place_on"
                        && p.blocks == Some(RegistryHolderSet::Tag("minecraft:logs".into()))
                    {
                        value(
                            counts,
                            "item_block_tag",
                            "component=can_place_on tag=minecraft:logs",
                        );
                    }
                    if let Some(m) = &p.components {
                        let exact = m.exact.iter().any(|c| {
                            c.name == "custom_data"
                                && matches!(&c.value, ComponentValue::Nbt(nbt) if marker(nbt))
                        });
                        let partial = m.partial.iter().any(|p| {
                            p.kind == ComponentPredicateKind::CustomData && partial_marker(&p.data)
                        });
                        if exact && partial {
                            value(
                                counts,
                                "item_component_matchers",
                                "exact_custom_data=1 partial_custom_data=1",
                            );
                        }
                    }
                }
            }
            ComponentValue::ArmorTrim(v)
                if named(
                    &v.material,
                    "minecraft:trim_material",
                    "minecraft:quartz",
                    registries,
                ) && named(
                    &v.pattern,
                    "minecraft:trim_pattern",
                    "minecraft:sentry",
                    registries,
                ) =>
            {
                value(
                    counts,
                    "item_trim",
                    "material=minecraft:quartz pattern=minecraft:sentry",
                );
            }
            ComponentValue::BannerPatterns(v)
                if v.len() == 1
                    && v[0].color_id == 14
                    && named(
                        &v[0].pattern,
                        "minecraft:banner_pattern",
                        "minecraft:stripe_top",
                        registries,
                    ) =>
            {
                value(
                    counts,
                    "item_banner",
                    "pattern=minecraft:stripe_top color=14",
                );
            }
            ComponentValue::Profile(v) => {
                let properties = match v {
                    ResolvableProfile::Partial { properties, .. }
                    | ResolvableProfile::Complete { properties, .. } => properties,
                };
                if properties.iter().any(|p| {
                    p.name == "rustwire_probe" && p.value == "local" && p.signature.is_none()
                }) {
                    value(
                        counts,
                        "item_profile",
                        "property=rustwire_probe value=local signed=false",
                    );
                }
            }
            ComponentValue::Consumable(v)
                if v.consume_seconds == 3.0
                    && v.animation == UseAnimation::Eat
                    && !v.makes_particles
                    && v.effects.len() == 1
                    && v.effects[0] == ConsumeEffect::ClearAllEffects =>
            {
                value(
                    counts,
                    "item_consumable",
                    "seconds=3 animation=Eat particles=false effect=ClearAllEffects",
                );
            }
            ComponentValue::Equippable(v)
                if v.slot == ItemEquipmentSlot::Head
                    && !v.dispensable
                    && !v.swappable
                    && !v.damage_on_hurt =>
            {
                value(
                    counts,
                    "item_equippable",
                    "slot=Head dispensable=false swappable=false damage_on_hurt=false",
                );
            }
            ComponentValue::UseCooldown(v)
                if v.seconds == 1.5 && v.group.as_deref() == Some("rustwire:probe") =>
            {
                value(
                    counts,
                    "item_use_cooldown",
                    "seconds=1.5 group=rustwire:probe",
                );
            }
            ComponentValue::DeathProtection(v)
                if v.as_slice() == [ConsumeEffect::ClearAllEffects] =>
            {
                value(counts, "item_death_protection", "effect=ClearAllEffects");
            }
            ComponentValue::Instrument(HolderOrKey::Holder(RegistryHolder::Inline(v)))
                if v.range == 32.0 && matches!(&v.sound, RegistryHolder::Inline(sound)
                    if sound.name == "rustwire:probe" && sound.fixed_range == Some(12.0)) => {
                match v.use_duration {
                    InstrumentDuration::Ticks(140) if v.description.is_none() => value(counts, "item_instrument", "inline=true duration=140 unit=ticks range=32 sound=rustwire:probe sound_range=12"),
                    InstrumentDuration::Seconds(seconds) if seconds == 3.5 && v.description.as_ref().is_some_and(|nbt| super::nbt_contains(&nbt.root, "Rustwire horn")) => value(counts, "item_instrument", "inline=true duration=3.5 unit=seconds range=32 sound=rustwire:probe sound_range=12"),
                    _ => {}
                }
            }
            ComponentValue::JukeboxPlayable(v) => {
                let matches = match &v.song {
                    HolderOrKey::Key(key) => key == "minecraft:cat",
                    HolderOrKey::Holder(holder) => named(
                        holder,
                        "minecraft:jukebox_song",
                        "minecraft:cat",
                        registries,
                    ),
                };
                if matches {
                    value(counts, "item_jukebox", "song=minecraft:cat");
                }
            }
            ComponentValue::Weapon(v)
                if v.item_damage_per_attack == 2 && v.disable_blocking_seconds == 3.5 =>
            {
                value(
                    counts,
                    "item_weapon",
                    "damage_per_attack=2 disable_seconds=3.5",
                );
            }
            ComponentValue::BlocksAttacks(v)
                if v.block_delay_seconds == 0.75 && v.disable_cooldown_scale == 1.5 =>
            {
                value(
                    counts,
                    "item_blocks_attacks",
                    "delay_seconds=0.75 cooldown_scale=1.5",
                );
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct State {
    pub own_entity: Option<i32>,
    profiles: BTreeMap<[u8; 16], String>,
    spawned: BTreeSet<i32>,
    effect: Option<(i32, i32)>,
}
impl State {
    pub fn spawn(&mut self, id: i32) {
        self.spawned.insert(id);
    }
    pub fn player(&mut self, packet: &PlayerPacket, counts: &mut BTreeMap<&'static str, u64>) {
        match packet {
            PlayerPacket::Info(p) => {
                println!(
                    "PLAYER_INFO actions={:?} entries={:?}",
                    p.actions, p.entries
                );
                for e in &p.entries {
                    if let Some(profile) = &e.profile {
                        self.profiles.insert(e.uuid, profile.name.clone());
                        if profile.name == "Rustwire"
                            && e.listed == Some(true)
                            && e.game_mode == Some(GameMode::Creative)
                        {
                            value(
                                counts,
                                "player_self_add",
                                "name=Rustwire listed=true mode=Creative",
                            );
                        }
                        if profile.name == "RustwirePeer" && e.listed == Some(true) {
                            value(counts, "player_peer_add", "name=RustwirePeer listed=true");
                        }
                    }
                    if self
                        .profiles
                        .get(&e.uuid)
                        .is_some_and(|name| name == "Rustwire")
                        && e.game_mode == Some(GameMode::Survival)
                    {
                        value(counts, "player_mode_update", "name=Rustwire mode=Survival");
                    }
                }
            }
            PlayerPacket::Remove(p) => {
                println!("PLAYER_REMOVE uuids={:?}", p.players);
                for uuid in &p.players {
                    if self.profiles.remove(uuid).as_deref() == Some("RustwirePeer") {
                        value(
                            counts,
                            "player_peer_remove",
                            "name=RustwirePeer matched_added_uuid=true",
                        );
                    }
                }
            }
        }
    }
    pub fn entity(&mut self, packet: &EntityStatePacket, counts: &mut BTreeMap<&'static str, u64>) {
        println!("ENTITY_STATE {packet:?}");
        match packet {
            EntityStatePacket::Equipment(p) if self.spawned.contains(&p.entity_id) => {
                for e in &p.equipment {
                    if e.slot == EquipmentSlot::Head {
                        if let Slot::Item(item) = &e.item {
                            let damage = match &item.data {
                                ItemData::Legacy(Some(nbt)) => {
                                    nbt.root.get("Damage").and_then(|v| v.as_i32()) == Some(7)
                                }
                                ItemData::Components(p) => p.added.iter().any(|c| {
                                    c.name == "damage" && c.value == ComponentValue::VarInt(7)
                                }),
                                _ => false,
                            };
                            if damage && item.count == 1 {
                                value(
                                    counts,
                                    "entity_equipment",
                                    format!(
                                        "slot=Head damage=7 count=1 entity={} item_id={}",
                                        p.entity_id, item.item_id
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            EntityStatePacket::Attributes(p) if Some(p.entity_id) == self.own_entity => {
                for a in &p.attributes {
                    for m in &a.modifiers {
                        let id = match &m.id {
                            AttributeModifierId::Resource(v) => v == "rustwire:probe",
                            AttributeModifierId::Uuid(v) => {
                                *v == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
                            }
                        };
                        if id && m.amount == 0.125 && m.operation == AttributeOperation::AddValue {
                            value(
                                counts,
                                "entity_attribute_modifier",
                                format!(
                                    "amount=0.125 operation=AddValue own_entity=true key={:?}",
                                    a.key
                                ),
                            );
                        }
                    }
                }
            }
            EntityStatePacket::Effect(p)
                if Some(p.entity_id) == self.own_entity
                    && p.amplifier == 2
                    && p.duration > 1100
                    && p.duration <= 1200
                    && p.flags.visible
                    && p.flags.show_icon =>
            {
                self.effect = Some((p.entity_id, p.effect_id));
                value(counts, "entity_effect_add", format!("amplifier=2 duration={} visible=true icon=true effect_id={} own_entity=true", p.duration, p.effect_id));
            }
            EntityStatePacket::RemoveEffect(p)
                if self.effect == Some((p.entity_id, p.effect_id)) =>
            {
                value(
                    counts,
                    "entity_effect_remove",
                    "matched_added_effect=true own_entity=true",
                );
                self.effect = None;
            }
            _ => {}
        }
    }
}
