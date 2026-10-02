//! Independent byte fixtures for official consumption/equipment/combat/profile
//! stream layouts. Every fixture tests encode, decode, trailing bytes and every
//! truncation; roundtrips alone are not wire-format evidence.
use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, NbtString, Tag},
    packet::{
        entity_metadata::holders::{
            PaintingVariant, PlayerModel, PlayerSkinPatch, RegistryHolder, RegistryHolderSet,
            ResolvableProfile,
        },
        inventory::*,
        ProfileProperty,
    },
    Error, Limits, Version,
};
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .flat_map(|s| {
            s.as_bytes()
                .chunks(2)
                .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
        })
        .collect()
}
fn id(p: i32, n: &str) -> u8 {
    let a = match n {
        "consumable" => [255, 255, 22, 22, 21, 21, 21, 21, 24, 24, 24],
        "use_cooldown" => [255, 255, 24, 24, 23, 23, 23, 23, 26, 26, 26],
        "equippable" => [255, 255, 28, 28, 28, 28, 28, 28, 32, 32, 32],
        "death_protection" => [255, 255, 32, 32, 32, 32, 32, 32, 36, 36, 36],
        "weapon" => [255, 255, 255, 255, 26, 26, 26, 26, 29, 29, 29],
        "attack_range" => [255, 255, 255, 255, 255, 255, 255, 255, 30, 30, 30],
        "blocks_attacks" => [255, 255, 255, 255, 33, 33, 33, 33, 37, 37, 37],
        "use_effects" => [255, 255, 255, 255, 255, 255, 255, 255, 5, 5, 5],
        "piercing_weapon" => [255, 255, 255, 255, 255, 255, 255, 255, 38, 38, 38],
        "kinetic_weapon" => [255, 255, 255, 255, 255, 255, 255, 255, 39, 39, 39],
        "swing_animation" => [255, 255, 255, 255, 255, 255, 255, 255, 40, 40, 40],
        "profile" => [46, 47, 57, 57, 61, 61, 61, 61, 68, 70, 70],
        "painting/variant" => [255, 255, 255, 255, 89, 89, 89, 89, 97, 102, 103],
        "break_sound" => [255, 255, 255, 255, 71, 71, 71, 71, 78, 80, 81],
        _ => panic!("unknown fixture"),
    };
    a[(p - 766) as usize]
}
fn slot(n: &'static str, value: ComponentValue) -> Slot {
    Slot::Item(ItemStack {
        item_id: 5,
        count: 1,
        data: ItemData::Components(ComponentPatch {
            added: vec![Component { name: n, value }],
            removed: vec![],
        }),
    })
}
fn bytes(p: i32, n: &str, payload: &str) -> Vec<u8> {
    let mut b = vec![1, 5, 1, 0, id(p, n)];
    b.extend(hex(payload));
    b
}
fn fixture(p: i32, n: &'static str, payload: &str, value: ComponentValue) {
    let expected = slot(n, value);
    let b = bytes(p, n, payload);
    assert_eq!(component_id(v(p), n).unwrap(), i32::from(id(p, n)));
    assert_eq!(
        Slot::decode(&b, v(p), Limits::default()).unwrap(),
        expected,
        "decode {p} {n}"
    );
    assert_eq!(
        expected.encode(v(p), Limits::default()).unwrap(),
        b,
        "encode {p} {n}"
    );
    for end in 0..b.len() {
        let mut r = Reader::new(&b[..end], Limits::default());
        assert!(Slot::read(&mut r, v(p)).is_err(), "truncated {p} {n} {end}");
        assert_eq!(r.position(), 0);
    }
    let mut trailing = b;
    trailing.push(0);
    assert!(Slot::decode(&trailing, v(p), Limits::default()).is_err());
}
fn sound() -> SoundHolder {
    RegistryHolder::Inline(SoundEvent {
        name: "x".into(),
        fixed_range: Some(2.0),
    })
}
fn effect() -> PotionEffect {
    PotionEffect {
        id: 2,
        details: EffectDetails {
            amplifier: 1,
            duration: 300,
            ambient: false,
            show_particles: true,
            show_icon: false,
            hidden_effect: None,
        },
    }
}
fn effects() -> Vec<ConsumeEffect> {
    vec![
        ConsumeEffect::ApplyEffects {
            effects: vec![effect()],
            probability: 0.5,
        },
        ConsumeEffect::RemoveEffects(RegistryHolderSet::Ids(vec![1, 2])),
        ConsumeEffect::ClearAllEffects,
        ConsumeEffect::TeleportRandomly { diameter: 2.0 },
        ConsumeEffect::PlaySound(sound()),
    ]
}
const EFFECTS: &str =
    "05 00 01 02 01 ac02 00 01 00 00 3f000000 01 03 01 02 02 03 40000000 04 00 01 78 01 40000000";
#[test]
fn consumable_and_death_protection_all_effects_all_releases() {
    for p in 768..=776 {
        fixture(
            p,
            "consumable",
            &format!("3fc00000 01 03 01 {EFFECTS}"),
            ComponentValue::Consumable(Consumable {
                consume_seconds: 1.5,
                animation: UseAnimation::Eat,
                sound: RegistryHolder::RegistryId(2),
                makes_particles: true,
                effects: effects(),
            }),
        );
        fixture(
            p,
            "death_protection",
            EFFECTS,
            ComponentValue::DeathProtection(effects()),
        );
    }
}
#[test]
fn animation_schema_corrections_are_versioned() {
    for (p, a, n) in [
        (768, UseAnimation::Brush, "09"),
        (769, UseAnimation::Bundle, "0a"),
        (773, UseAnimation::Bundle, "0a"),
        (774, UseAnimation::Spear, "0b"),
        (776, UseAnimation::Spear, "0b"),
    ] {
        fixture(
            p,
            "consumable",
            &format!("3f800000 {n} 01 00 00"),
            ComponentValue::Consumable(Consumable {
                consume_seconds: 1.0,
                animation: a,
                sound: RegistryHolder::RegistryId(0),
                makes_particles: false,
                effects: vec![],
            }),
        );
    }
    for (p, a, id) in [
        (768, UseAnimation::Bundle, "0a"),
        (773, UseAnimation::Spear, "0b"),
    ] {
        let s = slot(
            "consumable",
            ComponentValue::Consumable(Consumable {
                consume_seconds: 1.0,
                animation: a,
                sound: RegistryHolder::RegistryId(0),
                makes_particles: false,
                effects: vec![],
            }),
        );
        assert!(s.encode(v(p), Limits::default()).is_err());
        assert!(Slot::decode(
            &bytes(p, "consumable", &format!("3f800000 {id} 01 00 00")),
            v(p),
            Limits::default()
        )
        .is_err());
    }
}
#[test]
fn equippable_fields_and_saddle_version_boundaries() {
    for p in 768..=776 {
        let slot = if p >= 770 {
            ItemEquipmentSlot::Saddle
        } else {
            ItemEquipmentSlot::Head
        };
        let payload = format!(
            "{} 00 01 78 01 40000000 01 01 61 01 01 62 01 03 01 02 01 00 01 {} {}",
            if p >= 770 { "07" } else { "04" },
            if p >= 770 { "01" } else { "" },
            if p >= 771 { "01 03" } else { "" }
        );
        fixture(
            p,
            "equippable",
            &payload,
            ComponentValue::Equippable(Equippable {
                slot,
                equip_sound: sound(),
                asset_id: Some("a".into()),
                camera_overlay: Some("b".into()),
                allowed_entities: Some(RegistryHolderSet::Ids(vec![1, 2])),
                dispensable: true,
                swappable: false,
                damage_on_hurt: true,
                equip_on_interact: (p >= 770).then_some(true),
                shearing: (p >= 771).then_some(EquipmentShearing {
                    shearable: true,
                    sound: RegistryHolder::RegistryId(2),
                }),
            }),
        );
    }
}
#[test]
fn cooldown_and_weapon_golden() {
    for p in 768..=776 {
        fixture(
            p,
            "use_cooldown",
            "3f000000 01 01 78",
            ComponentValue::UseCooldown(UseCooldown {
                seconds: 0.5,
                group: Some("x".into()),
            }),
        );
        fixture(
            p,
            "use_cooldown",
            "3f000000 00",
            ComponentValue::UseCooldown(UseCooldown {
                seconds: 0.5,
                group: None,
            }),
        );
        if p >= 770 {
            fixture(
                p,
                "weapon",
                "ac02 3fc00000",
                ComponentValue::Weapon(Weapon {
                    item_damage_per_attack: 300,
                    disable_blocking_seconds: 1.5,
                }),
            );
        }
    }
}
#[test]
fn sound_and_painting_complete_inline_holders() {
    for p in 770..=776 {
        fixture(
            p,
            "break_sound",
            "00 01 78 01 40000000",
            ComponentValue::Sound(sound()),
        );
        fixture(
            p,
            "break_sound",
            "03",
            ComponentValue::Sound(RegistryHolder::RegistryId(2)),
        );
        fixture(
            p,
            "break_sound",
            "00 01 78 00",
            ComponentValue::Sound(RegistryHolder::Inline(SoundEvent {
                name: "x".into(),
                fixed_range: None,
            })),
        );
        fixture(
            p,
            "painting/variant",
            "00 01 02 01 78 01 08 0001 61 00",
            ComponentValue::PaintingVariant(RegistryHolder::Inline(PaintingVariant {
                width: 1,
                height: 2,
                asset_id: "x".into(),
                title: Some(Nbt::anonymous(Tag::String(NbtString::from("a")))),
                author: None,
            })),
        );
        fixture(
            p,
            "painting/variant",
            "03",
            ComponentValue::PaintingVariant(RegistryHolder::RegistryId(2)),
        );
    }
}
fn shield(bypassed_by: Option<RegistryHolderSet>) -> BlocksAttacks {
    BlocksAttacks {
        block_delay_seconds: 0.5,
        disable_cooldown_scale: 1.0,
        damage_reductions: vec![DamageReduction {
            horizontal_blocking_angle: 90.0,
            damage_types: Some(RegistryHolderSet::Ids(vec![2])),
            base: 1.0,
            factor: 2.0,
        }],
        item_damage: ItemDamageFunction {
            threshold: 1.0,
            base: 2.0,
            factor: 0.5,
        },
        bypassed_by,
        block_sound: Some(RegistryHolder::RegistryId(2)),
        disable_sound: Some(sound()),
    }
}
#[test]
fn shield_bypass_tag_to_holder_set_transition() {
    for p in 770..=776 {
        let payload=format!("3f000000 3f800000 01 42b40000 01 02 02 3f800000 40000000 3f800000 40000000 3f000000 01 {} 01 78 01 03 01 00 01 78 01 40000000",if p>=775{"00"}else{""});
        fixture(
            p,
            "blocks_attacks",
            &payload,
            ComponentValue::BlocksAttacks(shield(Some(RegistryHolderSet::Tag("x".into())))),
        );
    }
    let s = slot(
        "blocks_attacks",
        ComponentValue::BlocksAttacks(shield(Some(RegistryHolderSet::Ids(vec![2])))),
    );
    assert!(s.encode(v(774), Limits::default()).is_err());
    let b = s.encode(v(775), Limits::default()).unwrap();
    assert_eq!(Slot::decode(&b, v(775), Limits::default()).unwrap(), s);
}
#[test]
fn modern_combat_components_golden() {
    for p in 774..=776 {
        fixture(
            p,
            "use_effects",
            "01 00 3f000000",
            ComponentValue::UseEffects(UseEffects {
                can_sprint: true,
                interact_vibrations: false,
                speed_multiplier: 0.5,
            }),
        );
        fixture(
            p,
            "attack_range",
            "3f000000 3f800000 40000000 40400000 40800000 40a00000",
            ComponentValue::AttackRange(AttackRange {
                min_range: 0.5,
                max_range: 1.0,
                min_creative_range: 2.0,
                max_creative_range: 3.0,
                hitbox_margin: 4.0,
                mob_factor: 5.0,
            }),
        );
        fixture(
            p,
            "piercing_weapon",
            "01 00 00 01 00 01 78 01 40000000",
            ComponentValue::PiercingWeapon(PiercingWeapon {
                deals_knockback: true,
                dismounts: false,
                sound: None,
                hit_sound: Some(sound()),
            }),
        );
        fixture(
            p,
            "kinetic_weapon",
            "ac02 04 01 05 3f800000 40000000 00 01 06 3f000000 40400000 40000000 40800000 01 03 00",
            ComponentValue::KineticWeapon(KineticWeapon {
                contact_cooldown_ticks: 300,
                delay_ticks: 4,
                dismount_conditions: Some(KineticWeaponCondition {
                    max_duration_ticks: 5,
                    min_speed: 1.0,
                    min_relative_speed: 2.0,
                }),
                knockback_conditions: None,
                damage_conditions: Some(KineticWeaponCondition {
                    max_duration_ticks: 6,
                    min_speed: 0.5,
                    min_relative_speed: 3.0,
                }),
                forward_movement: 2.0,
                damage_multiplier: 4.0,
                sound: Some(RegistryHolder::RegistryId(2)),
                hit_sound: None,
            }),
        );
        for (k, n) in [
            (SwingAnimationType::None, "00"),
            (SwingAnimationType::Whack, "01"),
            (SwingAnimationType::Stab, "02"),
        ] {
            fixture(
                p,
                "swing_animation",
                &format!("{n} ac02"),
                ComponentValue::SwingAnimation(SwingAnimation {
                    kind: k,
                    duration: 300,
                }),
            );
        }
    }
}
#[test]
fn legacy_and_modern_profile_golden_boundaries() {
    for p in 766..=776 {
        let payload = format!(
            "{} 01 01 61 01 00000000000000000000000000000000 01 01 78 01 79 01 01 7a {}",
            if p >= 773 { "00" } else { "" },
            if p >= 773 { "00 00 00 00" } else { "" }
        );
        fixture(
            p,
            "profile",
            &payload,
            ComponentValue::Profile(ResolvableProfile::Partial {
                name: Some("a".into()),
                uuid: Some([0; 16]),
                properties: vec![ProfileProperty {
                    name: "x".into(),
                    value: "y".into(),
                    signature: Some("z".into()),
                }],
                skin_patch: PlayerSkinPatch::default(),
            }),
        );
        if p >= 773 {
            fixture(
                p,
                "profile",
                "01 00000000000000000000000000000000 01 61 00 01 01 78 00 00 01 01",
                ComponentValue::Profile(ResolvableProfile::Complete {
                    uuid: [0; 16],
                    name: "a".into(),
                    properties: vec![],
                    skin_patch: PlayerSkinPatch {
                        body: Some("x".into()),
                        cape: None,
                        elytra: None,
                        model: Some(PlayerModel::Slim),
                    },
                }),
            );
        }
    }
}
#[test]
fn profile_limits_are_symmetric_including_utf16() {
    for p in [766, 772, 773, 776] {
        for name in ["x".repeat(17), "😀".repeat(9)] {
            let s = slot(
                "profile",
                ComponentValue::Profile(ResolvableProfile::Partial {
                    name: Some(name),
                    uuid: None,
                    properties: vec![],
                    skin_patch: PlayerSkinPatch::default(),
                }),
            );
            assert!(s.encode(v(p), Limits::default()).is_err());
        }
        let s = slot(
            "profile",
            ComponentValue::Profile(ResolvableProfile::Partial {
                name: None,
                uuid: None,
                properties: vec![
                    ProfileProperty {
                        name: "x".into(),
                        value: "y".into(),
                        signature: None
                    };
                    17
                ],
                skin_patch: PlayerSkinPatch::default(),
            }),
        );
        assert!(s.encode(v(p), Limits::default()).is_err());
    }
}
#[test]
fn nested_effect_budgets_and_atomic_writer_failures() {
    let s = slot(
        "death_protection",
        ComponentValue::DeathProtection(effects()),
    );
    let b = s.encode(v(776), Limits::default()).unwrap();
    for limits in [
        Limits {
            max_collection: 5,
            ..Limits::default()
        },
        Limits {
            max_nbt_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_packet: 10,
            ..Limits::default()
        },
    ] {
        assert!(Slot::decode(&b, v(776), limits).is_err());
        let mut w = Writer::new();
        w.u8(42);
        assert!(s.write(&mut w, v(776), limits).is_err());
        assert_eq!(w.as_slice(), &[42]);
    }
}
#[test]
fn malformed_extended_discriminants_fail_closed() {
    for (p, n, payload) in [
        (776, "death_protection", "01 05"),
        (776, "swing_animation", "03 01"),
        (776, "consumable", "3f800000 0c 01 00 00"),
        (769, "equippable", "07"),
        (776, "break_sound", "ffffffff0f"),
    ] {
        assert!(Slot::decode(&bytes(p, n, payload), v(p), Limits::default()).is_err());
    }
    let s = slot(
        "break_sound",
        ComponentValue::Sound(RegistryHolder::RegistryId(i32::MAX)),
    );
    assert!(matches!(
        s.encode(v(776), Limits::default()),
        Err(Error::Invalid(_))
    ));
}
#[test]
fn every_registered_layout_has_an_implementation() {
    for p in 766..=776 {
        for (name, wire) in component_registry(v(p)) {
            assert_ne!(*wire, ComponentWire::Unsupported, "{p} {name}");
        }
    }
}

// EquipmentSlot.STREAM_CODEC uses getId(), not Java enum ordinal. These bytes
// were independently verified from each cached release constructor and codec
// binding (1.21.3,1.21.4,1.21.5,1.21.6,1.21.8,1.21.10,1.21.11,26.1.2,26.2).
#[test]
fn equippable_actual_stream_ids_for_every_slot_and_release() {
    let slots = [
        (ItemEquipmentSlot::MainHand, "00"),
        (ItemEquipmentSlot::Feet, "01"),
        (ItemEquipmentSlot::Legs, "02"),
        (ItemEquipmentSlot::Chest, "03"),
        (ItemEquipmentSlot::Head, "04"),
        (ItemEquipmentSlot::OffHand, "05"),
        (ItemEquipmentSlot::Body, "06"),
        (ItemEquipmentSlot::Saddle, "07"),
    ];
    for p in 768..=776 {
        for (equipment_slot, wire_id) in slots {
            let value = ComponentValue::Equippable(Equippable {
                slot: equipment_slot,
                equip_sound: RegistryHolder::RegistryId(0),
                asset_id: None,
                camera_overlay: None,
                allowed_entities: None,
                dispensable: false,
                swappable: false,
                damage_on_hurt: false,
                equip_on_interact: (p >= 770).then_some(false),
                shearing: (p >= 771).then_some(EquipmentShearing {
                    shearable: false,
                    sound: RegistryHolder::RegistryId(0),
                }),
            });
            let payload = format!(
                "{wire_id} 01 00 00 00 00 00 00 {} {}",
                if p >= 770 { "00" } else { "" },
                if p >= 771 { "00 01" } else { "" }
            );
            if equipment_slot == ItemEquipmentSlot::Saddle && p < 770 {
                assert!(slot("equippable", value)
                    .encode(v(p), Limits::default())
                    .is_err());
                assert!(
                    Slot::decode(&bytes(p, "equippable", &payload), v(p), Limits::default())
                        .is_err()
                );
            } else {
                fixture(p, "equippable", &payload, value);
            }
        }
    }
}
