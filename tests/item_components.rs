//! Independent wire fixtures. Historical corrections are checked against the
//! official cached release STREAM_CODEC declarations (not schema backports).
use rustwire_mc::{
    codec::{BlockPosition, Reader, Writer},
    nbt::{Nbt, NbtString, Tag},
    packet::{entity_metadata::holders::RegistryHolderSet, inventory::*},
    Error, Limits, Version,
};

fn version(protocol: i32) -> Version {
    Version::from_protocol(protocol).unwrap()
}
fn bytes(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|n| u8::from_str_radix(n, 16).unwrap())
        .collect()
}
// IDs independently transcribed from pinned per-release registry mappings.
fn id(protocol: i32, name: &str) -> u8 {
    let ids = match name {
        "use_remainder" => [255, 255, 23, 23, 22, 22, 22, 22, 25, 25, 25],
        "charged_projectiles" => [29, 29, 39, 39, 40, 40, 40, 40, 47, 49, 49],
        "bundle_contents" => [30, 30, 40, 40, 41, 41, 41, 41, 48, 50, 50],
        "container" => [51, 52, 62, 62, 66, 66, 66, 66, 73, 75, 75],
        "sulfur_cube_content" => [255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 78],
        "food" => [20, 20, 21, 21, 20, 20, 20, 20, 23, 23, 23],
        "potion_contents" => [31, 31, 41, 41, 42, 42, 42, 42, 49, 51, 51],
        "suspicious_stew_effects" => [32, 32, 42, 42, 44, 44, 44, 44, 51, 53, 53],
        "writable_book_content" => [33, 33, 43, 43, 45, 45, 45, 45, 52, 54, 54],
        "written_book_content" => [34, 34, 44, 44, 46, 46, 46, 46, 53, 55, 55],
        "attribute_modifiers" => [12, 12, 13, 13, 13, 13, 13, 13, 16, 16, 16],
        "lodestone_tracker" => [43, 44, 54, 54, 58, 58, 58, 58, 65, 67, 67],
        "firework_explosion" => [44, 45, 55, 55, 59, 59, 59, 59, 66, 68, 68],
        "fireworks" => [45, 46, 56, 56, 60, 60, 60, 60, 67, 69, 69],
        "bees" => [53, 54, 64, 64, 68, 68, 68, 68, 75, 77, 77],
        "tool" => [22, 22, 26, 26, 25, 25, 25, 25, 28, 28, 28],
        "repairable" => [255, 255, 29, 29, 29, 29, 29, 29, 33, 33, 33],
        _ => panic!("unknown fixture component"),
    };
    ids[(protocol - 766) as usize]
}
fn slot(name: &'static str, value: ComponentValue) -> Slot {
    Slot::Item(ItemStack {
        item_id: 5,
        count: 1,
        data: ItemData::Components(ComponentPatch {
            added: vec![Component { name, value }],
            removed: vec![],
        }),
    })
}
fn fixture_bytes(protocol: i32, name: &str, payload: &[u8]) -> Vec<u8> {
    let mut wire = vec![1, 5, 1, 0, id(protocol, name)];
    wire.extend_from_slice(payload);
    wire
}
fn fixture(protocol: i32, name: &'static str, payload: &str, value: ComponentValue) {
    let expected = slot(name, value);
    let wire = fixture_bytes(protocol, name, &bytes(payload));
    let version = version(protocol);
    assert_eq!(
        component_id(version, name).unwrap(),
        i32::from(id(protocol, name))
    );
    assert_eq!(
        Slot::decode(&wire, version, Limits::default()).unwrap(),
        expected,
        "decode {protocol} {name}"
    );
    assert_eq!(
        expected.encode(version, Limits::default()).unwrap(),
        wire,
        "encode {protocol} {name}"
    );
    for end in 0..wire.len() {
        let mut r = Reader::new(&wire[..end], Limits::default());
        assert!(
            Slot::read(&mut r, version).is_err(),
            "truncation {protocol} {name} at {end}"
        );
        assert_eq!(r.position(), 0);
    }
    let mut tail = wire;
    tail.push(0);
    assert!(Slot::decode(&tail, version, Limits::default()).is_err());
}
fn detail() -> EffectDetails {
    EffectDetails {
        amplifier: 1,
        duration: 300,
        ambient: false,
        show_particles: true,
        show_icon: false,
        hidden_effect: None,
    }
}
fn effect() -> PotionEffect {
    PotionEffect {
        id: 2,
        details: detail(),
    }
}
fn text(s: &str) -> Nbt {
    Nbt::anonymous(Tag::String(NbtString::from(s)))
}
fn compound() -> Nbt {
    Nbt::anonymous(Tag::Compound(vec![]))
}

#[test]
fn food_golden_fixtures_correct_historical_schema_backports() {
    for p in 766..=776 {
        let legacy = (p <= 767).then(|| LegacyFoodConsumption {
            seconds_to_eat: 1.5,
            using_converts_to: None,
            effects: vec![FoodEffect {
                effect: effect(),
                probability: 0.25,
            }],
        });
        let payload = match p {
            766 => "04 3f000000 01 3fc00000 01 02 01 ac02 00 01 00 00 3e800000",
            767 => "04 3f000000 01 3fc00000 00 01 02 01 ac02 00 01 00 00 3e800000",
            _ => "04 3f000000 01",
        };
        fixture(
            p,
            "food",
            &expand(payload),
            ComponentValue::Food(Food {
                nutrition: 4,
                saturation_modifier: 0.5,
                can_always_eat: true,
                legacy_consumption: legacy,
            }),
        );
    }
    fixture(
        767,
        "food",
        &expand("04 3f000000 01 3fc00000 01 01 05 00 00 00"),
        ComponentValue::Food(Food {
            nutrition: 4,
            saturation_modifier: 0.5,
            can_always_eat: true,
            legacy_consumption: Some(LegacyFoodConsumption {
                seconds_to_eat: 1.5,
                using_converts_to: Some(Box::new(Slot::Item(ItemStack {
                    item_id: 5,
                    count: 1,
                    data: ItemData::Components(ComponentPatch::default()),
                }))),
                effects: vec![],
            }),
        }),
    );
}
fn expand(hex: &str) -> String {
    hex.split_whitespace()
        .flat_map(|part| {
            part.as_bytes()
                .chunks(2)
                .map(|chunk| std::str::from_utf8(chunk).unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
#[test]
fn potion_golden_custom_name_and_recursive_effects_all_releases() {
    for p in 766..=776 {
        let mut nested = detail();
        nested.hidden_effect = Some(Box::new(EffectDetails {
            amplifier: 0,
            duration: -1,
            ambient: true,
            show_particles: false,
            show_icon: true,
            hidden_effect: None,
        }));
        let payload = format!(
            "01 04 01 00ff8000 01 02 01 ac02 00 01 00 01 00 ffffffff0f 01 00 01 00 {}",
            if p >= 768 { "01 01 78" } else { "" }
        );
        fixture(
            p,
            "potion_contents",
            &expand(&payload),
            ComponentValue::PotionContents(PotionContents {
                potion_id: Some(4),
                custom_color: Some(0x00ff8000),
                custom_effects: vec![PotionEffect {
                    id: 2,
                    details: nested,
                }],
                custom_name: (p >= 768).then(|| "x".into()),
            }),
        );
    }
}
#[test]
fn stew_and_filtered_books_golden_all_releases() {
    for p in 766..=776 {
        fixture(
            p,
            "suspicious_stew_effects",
            "01 02 ac 02",
            ComponentValue::StewEffects(vec![StewEffect {
                id: 2,
                duration: 300,
            }]),
        );
        fixture(
            p,
            "writable_book_content",
            "02 01 61 01 01 62 01 63 00",
            ComponentValue::WritableBook(vec![
                Filterable {
                    raw: "a".into(),
                    filtered: Some("b".into()),
                },
                Filterable {
                    raw: "c".into(),
                    filtered: None,
                },
            ]),
        );
        fixture(
            p,
            "written_book_content",
            "01 74 01 01 75 01 61 02 02 08 00 01 70 01 08 00 01 71 08 00 01 72 00 01",
            ComponentValue::WrittenBook(WrittenBook {
                title: Filterable {
                    raw: "t".into(),
                    filtered: Some("u".into()),
                },
                author: "a".into(),
                generation: 2,
                pages: vec![
                    Filterable {
                        raw: text("p"),
                        filtered: Some(text("q")),
                    },
                    Filterable {
                        raw: text("r"),
                        filtered: None,
                    },
                ],
                resolved: true,
            }),
        );
    }
}
#[test]
fn attributes_golden_uuid_tooltip_saddle_display_boundaries() {
    for p in 766..=776 {
        let modifier_id = if p == 766 {
            AttributeModifierId::Legacy {
                uuid: [0x11; 16],
                name: "x".into(),
            }
        } else {
            AttributeModifierId::Identifier("a:b".into())
        };
        let slot = if p >= 770 {
            AttributeSlot::Saddle
        } else {
            AttributeSlot::MainHand
        };
        let display = (p >= 771).then(|| AttributeDisplay::Override(text("x")));
        let payload = format!(
            "01 03 {} 4000000000000000 02 {} {} {}",
            if p == 766 {
                "11111111111111111111111111111111 01 78"
            } else {
                "03 613a62"
            },
            if p >= 770 { "0a" } else { "01" },
            if p >= 771 { "02 08 0001 78" } else { "" },
            if p < 770 { "01" } else { "" }
        );
        fixture(
            p,
            "attribute_modifiers",
            &expand(&payload),
            ComponentValue::AttributeModifiers(AttributeModifiers {
                entries: vec![AttributeModifier {
                    attribute_id: 3,
                    modifier_id,
                    amount: 2.0,
                    operation: AttributeOperation::MultiplyTotal,
                    slot,
                    display,
                }],
                show_tooltip: (p < 770).then_some(true),
            }),
        );
    }
}
#[test]
fn attribute_display_default_hidden_and_override_are_distinct() {
    for (marker, display) in [
        (0, AttributeDisplay::Default),
        (1, AttributeDisplay::Hidden),
    ] {
        fixture(
            776,
            "attribute_modifiers",
            &expand(&format!(
                "01 00 03 613a62 0000000000000000 00 00 {marker:02x}"
            )),
            ComponentValue::AttributeModifiers(AttributeModifiers {
                entries: vec![AttributeModifier {
                    attribute_id: 0,
                    modifier_id: AttributeModifierId::Identifier("a:b".into()),
                    amount: 0.0,
                    operation: AttributeOperation::Add,
                    slot: AttributeSlot::Any,
                    display: Some(display),
                }],
                show_tooltip: None,
            }),
        );
    }
}
#[test]
fn lodestone_and_fireworks_golden_all_releases() {
    let explosion = FireworkExplosion {
        shape: FireworkShape::Star,
        colors: vec![0x00ff0000],
        fade_colors: vec![0x0000ff00],
        has_trail: true,
        has_twinkle: false,
    };
    for p in 766..=776 {
        fixture(
            p,
            "lodestone_tracker",
            "01 03 61 3a 62 00 00 00 40 00 00 20 40 01",
            ComponentValue::LodestoneTracker(LodestoneTracker {
                target: Some(LodestoneTarget {
                    dimension: "a:b".into(),
                    position: BlockPosition { x: 1, y: 64, z: 2 },
                }),
                tracked: true,
            }),
        );
        fixture(
            p,
            "lodestone_tracker",
            "00 00",
            ComponentValue::LodestoneTracker(LodestoneTracker {
                target: None,
                tracked: false,
            }),
        );
        fixture(
            p,
            "firework_explosion",
            "02 01 00 ff 00 00 01 00 00 ff 00 01 00",
            ComponentValue::FireworkExplosion(explosion.clone()),
        );
        fixture(
            p,
            "fireworks",
            "03 01 02 01 00 ff 00 00 01 00 00 ff 00 01 00",
            ComponentValue::Fireworks(Fireworks {
                flight_duration: 3,
                explosions: vec![explosion.clone()],
            }),
        );
    }
}
#[test]
fn bee_entity_type_transition_golden_all_releases() {
    for p in 766..=776 {
        fixture(
            p,
            "bees",
            if p >= 773 {
                "01 02 0a 00 05 ac 02"
            } else {
                "01 0a 00 05 ac 02"
            },
            ComponentValue::Bees(vec![BeeOccupant {
                entity_type: (p >= 773).then_some(2),
                entity_data: compound(),
                ticks_in_hive: 5,
                minimum_ticks_in_hive: 300,
            }]),
        );
    }
}
#[test]
fn tool_holder_sets_and_repairable_golden_all_releases() {
    for p in 766..=776 {
        let payload = format!(
            "02 00 03 613a62 01 40800000 01 01 03 02 04 00 01 00 3f800000 01 {}",
            if p >= 770 { "01" } else { "" }
        );
        fixture(
            p,
            "tool",
            &expand(&payload),
            ComponentValue::Tool(Tool {
                rules: vec![
                    ToolRule {
                        blocks: RegistryHolderSet::Tag("a:b".into()),
                        speed: Some(4.0),
                        correct_for_drops: Some(true),
                    },
                    ToolRule {
                        blocks: RegistryHolderSet::Ids(vec![2, 4]),
                        speed: None,
                        correct_for_drops: Some(false),
                    },
                ],
                default_mining_speed: 1.0,
                damage_per_block: 1,
                can_destroy_blocks_in_creative: (p >= 770).then_some(true),
            }),
        );
        if p >= 768 {
            fixture(
                p,
                "repairable",
                "03 02 04",
                ComponentValue::Repairable(RegistryHolderSet::Ids(vec![2, 4])),
            );
            fixture(
                p,
                "repairable",
                "00 03 61 3a 62",
                ComponentValue::Repairable(RegistryHolderSet::Tag("a:b".into())),
            );
            fixture(
                p,
                "repairable",
                "01",
                ComponentValue::Repairable(RegistryHolderSet::Ids(vec![])),
            );
        }
    }
}

#[test]
fn release_incompatible_fields_and_shapes_fail_atomically() {
    let mut cases = vec![
        (
            766,
            slot(
                "food",
                ComponentValue::Food(Food {
                    nutrition: 1,
                    saturation_modifier: 1.0,
                    can_always_eat: false,
                    legacy_consumption: None,
                }),
            ),
        ),
        (
            768,
            slot(
                "food",
                ComponentValue::Food(Food {
                    nutrition: 1,
                    saturation_modifier: 1.0,
                    can_always_eat: false,
                    legacy_consumption: Some(LegacyFoodConsumption {
                        seconds_to_eat: 1.0,
                        using_converts_to: None,
                        effects: vec![],
                    }),
                }),
            ),
        ),
        (
            767,
            slot(
                "potion_contents",
                ComponentValue::PotionContents(PotionContents {
                    custom_name: Some("x".into()),
                    ..Default::default()
                }),
            ),
        ),
        (
            772,
            slot(
                "bees",
                ComponentValue::Bees(vec![BeeOccupant {
                    entity_type: Some(2),
                    entity_data: compound(),
                    ticks_in_hive: 0,
                    minimum_ticks_in_hive: 0,
                }]),
            ),
        ),
        (
            773,
            slot(
                "bees",
                ComponentValue::Bees(vec![BeeOccupant {
                    entity_type: None,
                    entity_data: compound(),
                    ticks_in_hive: 0,
                    minimum_ticks_in_hive: 0,
                }]),
            ),
        ),
        (
            776,
            slot(
                "bees",
                ComponentValue::Bees(vec![BeeOccupant {
                    entity_type: Some(2),
                    entity_data: text("x"),
                    ticks_in_hive: 0,
                    minimum_ticks_in_hive: 0,
                }]),
            ),
        ),
        (
            769,
            slot(
                "tool",
                ComponentValue::Tool(Tool {
                    rules: vec![],
                    default_mining_speed: 1.0,
                    damage_per_block: 1,
                    can_destroy_blocks_in_creative: Some(true),
                }),
            ),
        ),
        (
            770,
            slot(
                "tool",
                ComponentValue::Tool(Tool {
                    rules: vec![],
                    default_mining_speed: 1.0,
                    damage_per_block: 1,
                    can_destroy_blocks_in_creative: None,
                }),
            ),
        ),
        (776, slot("food", ComponentValue::VarInt(3))),
    ];
    for p in [766, 767] {
        cases.push((
            p,
            slot(
                "food",
                ComponentValue::Food(Food {
                    nutrition: 1,
                    saturation_modifier: 1.0,
                    can_always_eat: false,
                    legacy_consumption: Some(LegacyFoodConsumption {
                        seconds_to_eat: 1.0,
                        using_converts_to: Some(Box::new(Slot::Empty)),
                        effects: vec![],
                    }),
                }),
            ),
        ));
    }
    for (p, item) in cases {
        let mut w = Writer::new();
        w.raw(&[7, 8, 9]);
        assert!(item.write(&mut w, version(p), Limits::default()).is_err());
        assert_eq!(w.as_slice(), &[7, 8, 9]);
    }
    for p in 763..=765 {
        assert!(slot(
            "food",
            ComponentValue::Food(Food {
                nutrition: 1,
                saturation_modifier: 1.0,
                can_always_eat: false,
                legacy_consumption: None
            })
        )
        .encode(version(p), Limits::default())
        .is_err());
    }
}
#[test]
fn malformed_components_reject_unknown_discriminants_and_bad_lengths() {
    for (p, name, payload) in [
        (776, "firework_explosion", "05 00 00 00 00"),
        (776, "fireworks", "00 81 02"), // 257 exceeds the official stream limit.
        (776, "writable_book_content", "65"), // 101 pages.
        (
            776,
            "written_book_content",
            "00 00 00 00 01 08 00 00 01 00 00",
        ), // present filtered TAG_End.
        (767, "food", "00 00 00 00 00 00 00 00 00 00 01 00 00"), // present empty converter.
        (776, "bees", "01 00 08 00 00 00 00"), // bee data must be a compound.
        (776, "repairable", "ff ff ff ff 0f"),
        (776, "repairable", "02 ff ff ff ff 0f"),
        (776, "potion_contents", "01 ff ff ff ff 0f 00 00 00"),
        (776, "potion_contents", "00 00 ff ff ff ff 0f 00"),
        (776, "potion_contents", "00 00 01 00 00 00 02 00 00 00 00"),
        (
            776,
            "attribute_modifiers",
            "01 00 00 00 00 00 00 00 00 00 00 03 00 00",
        ),
        (
            769,
            "attribute_modifiers",
            "01 00 00 00 00 00 00 00 00 00 00 00 0a 01",
        ),
        (
            776,
            "attribute_modifiers",
            "01 00 00 00 00 00 00 00 00 00 00 00 00 03",
        ),
    ] {
        let wire = fixture_bytes(p, name, &bytes(payload));
        assert!(
            Slot::decode(&wire, version(p), Limits::default()).is_err(),
            "{p} {name} {payload}"
        );
    }
}
#[test]
fn effect_recursion_obeys_depth_and_aggregate_budgets() {
    let mut details = detail();
    for _ in 0..8 {
        details = EffectDetails {
            hidden_effect: Some(Box::new(details)),
            ..detail()
        };
    }
    let item = slot(
        "potion_contents",
        ComponentValue::PotionContents(PotionContents {
            custom_effects: vec![PotionEffect { id: 1, details }],
            ..Default::default()
        }),
    );
    let wire = item.encode(version(776), Limits::default()).unwrap();
    for limits in [
        Limits {
            max_nbt_depth: 4,
            ..Limits::default()
        },
        Limits {
            max_collection: 8,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            item.encode(version(776), limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            Slot::decode(&wire, version(776), limits),
            Err(Error::Limit(_))
        ));
    }
    // A malicious 65-level chain is rejected even with an oversized caller limit.
    let mut payload = vec![0, 0, 1, 1];
    for _ in 0..65 {
        payload.extend_from_slice(&[0, 0, 0, 0, 0, 1]);
    }
    payload.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0]);
    assert!(matches!(
        Slot::decode(
            &fixture_bytes(776, "potion_contents", &payload),
            version(776),
            Limits {
                max_nbt_depth: usize::MAX,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
#[test]
fn ordinary_components_share_nbt_collection_and_packet_budgets() {
    let book = slot(
        "written_book_content",
        ComponentValue::WrittenBook(WrittenBook {
            title: Filterable {
                raw: "t".into(),
                filtered: None,
            },
            author: "a".into(),
            generation: 0,
            pages: vec![Filterable {
                raw: text("p"),
                filtered: Some(text("q")),
            }],
            resolved: false,
        }),
    );
    let wire = book.encode(version(776), Limits::default()).unwrap();
    let limits = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        book.encode(version(776), limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        Slot::decode(&wire, version(776), limits),
        Err(Error::Limit(_))
    ));
    let holder = slot(
        "repairable",
        ComponentValue::Repairable(RegistryHolderSet::Ids(vec![1, 2, 3])),
    );
    let wire = holder.encode(version(776), Limits::default()).unwrap();
    let limits = Limits {
        max_collection: 4,
        ..Limits::default()
    }; // slot + patch + 3 IDs > 4.
    assert!(matches!(
        holder.encode(version(776), limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        Slot::decode(&wire, version(776), limits),
        Err(Error::Limit(_))
    ));
    let fireworks = slot(
        "fireworks",
        ComponentValue::Fireworks(Fireworks {
            flight_duration: 1,
            explosions: vec![FireworkExplosion {
                shape: FireworkShape::Burst,
                colors: vec![1; 100],
                fade_colors: vec![],
                has_trail: false,
                has_twinkle: false,
            }],
        }),
    );
    assert!(matches!(
        fireworks.encode(
            version(776),
            Limits {
                max_packet: 32,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
#[test]
fn official_book_string_and_page_limits_are_symmetric() {
    for text in ["a".repeat(1025), "😀".repeat(513)] {
        let item = slot(
            "writable_book_content",
            ComponentValue::WritableBook(vec![Filterable {
                raw: text,
                filtered: None,
            }]),
        );
        assert!(matches!(
            item.encode(version(776), Limits::default()),
            Err(Error::Limit(_))
        ));
    }
    let item = slot(
        "writable_book_content",
        ComponentValue::WritableBook(vec![
            Filterable {
                raw: String::new(),
                filtered: None
            };
            101
        ]),
    );
    assert!(matches!(
        item.encode(version(776), Limits::default()),
        Err(Error::Limit(_))
    ));
    let item = slot(
        "written_book_content",
        ComponentValue::WrittenBook(WrittenBook {
            title: Filterable {
                raw: "a".repeat(33),
                filtered: None,
            },
            author: String::new(),
            generation: 0,
            pages: vec![],
            resolved: false,
        }),
    );
    assert!(matches!(
        item.encode(version(776), Limits::default()),
        Err(Error::Limit(_))
    ));
    let mut payload = vec![1, 0x81, 0x08];
    payload.extend(vec![b'a'; 1025]);
    payload.push(0);
    assert!(matches!(
        Slot::decode(
            &fixture_bytes(776, "writable_book_content", &payload),
            version(776),
            Limits::default()
        ),
        Err(Error::Limit(_))
    ));
}

fn template(item_id: i32, count: i32, patch: ComponentPatch) -> ItemStack {
    ItemStack {
        item_id,
        count,
        data: ItemData::Components(patch),
    }
}
#[test]
fn modern_template_component_fixtures_correct_stale_776_slot_schema() {
    let item = template(
        300,
        3,
        ComponentPatch {
            added: vec![Component {
                name: "damage",
                value: ComponentValue::VarInt(7),
            }],
            removed: vec!["max_damage"],
        },
    );
    let empty = template(0, 0, ComponentPatch::default());
    for p in [775, 776] {
        fixture(
            p,
            "use_remainder",
            "ac 02 03 01 01 03 07 02",
            ComponentValue::ItemTemplate(Box::new(item.clone())),
        );
        fixture(
            p,
            "use_remainder",
            "00 00 00 00",
            ComponentValue::ItemTemplate(Box::new(empty.clone())),
        );
        fixture(
            p,
            "use_remainder",
            "05 ac 02 00 00",
            ComponentValue::ItemTemplate(Box::new(template(5, 300, ComponentPatch::default()))),
        );
        for name in ["charged_projectiles", "bundle_contents"] {
            fixture(
                p,
                name,
                "02 ac 02 03 01 01 03 07 02 00 00 00 00",
                ComponentValue::ItemTemplates(vec![item.clone(), empty.clone()]),
            );
            fixture(p, name, "00", ComponentValue::ItemTemplates(vec![]));
        }
        fixture(
            p,
            "container",
            "03 00 01 ac 02 03 01 01 03 07 02 01 00 00 00 00",
            ComponentValue::OptionalItemTemplates(vec![
                None,
                Some(item.clone()),
                Some(empty.clone()),
            ]),
        );
        fixture(
            p,
            "container",
            "00",
            ComponentValue::OptionalItemTemplates(vec![]),
        );
        // A Slot-empty sentinel is only one byte; it is not an empty template.
        assert!(Slot::decode(
            &fixture_bytes(p, "use_remainder", &[0]),
            version(p),
            Limits::default()
        )
        .is_err());
    }
    fixture(
        776,
        "sulfur_cube_content",
        "ac 02 03 01 01 03 07 02",
        ComponentValue::ItemTemplate(Box::new(item)),
    );
}
#[test]
fn template_list_limits_have_real_release_boundaries() {
    let item = template(5, 1, ComponentPatch::default());
    let sixty_five = slot(
        "charged_projectiles",
        ComponentValue::ItemTemplates(vec![item.clone(); 65]),
    );
    assert!(matches!(
        sixty_five.encode(version(775), Limits::default()),
        Err(Error::Limit(_))
    ));
    let wire = sixty_five.encode(version(776), Limits::default()).unwrap();
    assert!(matches!(
        Slot::decode(&wire, version(775), Limits::default()),
        Err(Error::Limit(_))
    ));
    assert_eq!(
        Slot::decode(&wire, version(776), Limits::default()).unwrap(),
        sixty_five
    );
    for (p, name, maximum) in [
        (775, "charged_projectiles", 64),
        (776, "charged_projectiles", 1024),
        (775, "bundle_contents", 256),
        (776, "bundle_contents", 256),
    ] {
        let permitted = slot(
            name,
            ComponentValue::ItemTemplates(vec![item.clone(); maximum]),
        );
        let wire = permitted.encode(version(p), Limits::default()).unwrap();
        assert_eq!(
            Slot::decode(&wire, version(p), Limits::default()).unwrap(),
            permitted
        );
        let excessive = slot(
            name,
            ComponentValue::ItemTemplates(vec![item.clone(); maximum + 1]),
        );
        assert!(matches!(
            excessive.encode(version(p), Limits::default()),
            Err(Error::Limit(_))
        ));
        let mut payload = Writer::new();
        payload.var_i32((maximum + 1) as i32);
        assert!(matches!(
            Slot::decode(
                &fixture_bytes(p, name, payload.as_slice()),
                version(p),
                Limits::default()
            ),
            Err(Error::Limit(_))
        ));
    }
    for p in [775, 776] {
        let permitted = slot(
            "container",
            ComponentValue::OptionalItemTemplates(vec![None; 256]),
        );
        let wire = permitted.encode(version(p), Limits::default()).unwrap();
        assert_eq!(
            Slot::decode(&wire, version(p), Limits::default()).unwrap(),
            permitted
        );
        let excessive = slot(
            "container",
            ComponentValue::OptionalItemTemplates(vec![None; 257]),
        );
        assert!(matches!(
            excessive.encode(version(p), Limits::default()),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            Slot::decode(
                &fixture_bytes(p, "container", &[0x81, 2]),
                version(p),
                Limits::default()
            ),
            Err(Error::Limit(_))
        ));
    }
}
#[test]
fn templates_share_recursive_collection_nbt_and_byte_budgets() {
    let mut item = template(5, 1, ComponentPatch::default());
    for _ in 0..8 {
        item = template(
            5,
            1,
            ComponentPatch {
                added: vec![Component {
                    name: "use_remainder",
                    value: ComponentValue::ItemTemplate(Box::new(item)),
                }],
                removed: vec![],
            },
        );
    }
    let nested = slot(
        "use_remainder",
        ComponentValue::ItemTemplate(Box::new(item)),
    );
    let wire = nested.encode(version(776), Limits::default()).unwrap();
    for limits in [
        Limits {
            max_nbt_depth: 4,
            ..Limits::default()
        },
        Limits {
            max_collection: 4,
            ..Limits::default()
        },
        Limits {
            max_packet: 10,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            nested.encode(version(776), limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            Slot::decode(&wire, version(776), limits),
            Err(Error::Limit(_))
        ));
    }
    let many = slot(
        "bundle_contents",
        ComponentValue::ItemTemplates(vec![template(5, 1, ComponentPatch::default()); 2]),
    );
    let wire = many.encode(version(776), Limits::default()).unwrap();
    let limits = Limits {
        max_collection: 5,
        ..Limits::default()
    }; // slot+patch+list2+two templates =6.
    assert!(matches!(
        many.encode(version(776), limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        Slot::decode(&wire, version(776), limits),
        Err(Error::Limit(_))
    ));
    let rooted = template(
        5,
        1,
        ComponentPatch {
            added: vec![Component {
                name: "custom_data",
                value: ComponentValue::Nbt(compound()),
            }],
            removed: vec![],
        },
    );
    let many = slot(
        "bundle_contents",
        ComponentValue::ItemTemplates(vec![rooted.clone(), rooted]),
    );
    let wire = many.encode(version(776), Limits::default()).unwrap();
    let limits = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        many.encode(version(776), limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        Slot::decode(&wire, version(776), limits),
        Err(Error::Limit(_))
    ));
    let mut payload = Vec::new();
    for _ in 0..65 {
        payload.extend_from_slice(&[5, 1, 1, 0, 25]);
    }
    payload.extend_from_slice(&[5, 1, 0, 0]);
    assert!(matches!(
        Slot::decode(
            &fixture_bytes(776, "use_remainder", &payload),
            version(776),
            Limits {
                max_nbt_depth: usize::MAX,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
#[test]
fn template_malformed_values_wrong_versions_and_old_shapes_fail_atomically() {
    let item = template(5, 1, ComponentPatch::default());
    for (p, name, value) in [
        (
            774,
            "use_remainder",
            ComponentValue::ItemTemplate(Box::new(item.clone())),
        ),
        (
            775,
            "use_remainder",
            ComponentValue::Item(Box::new(Slot::Empty)),
        ),
        (776, "bundle_contents", ComponentValue::Items(vec![])),
        (776, "container", ComponentValue::ItemTemplates(vec![])),
        (
            776,
            "use_remainder",
            ComponentValue::ItemTemplate(Box::new(template(-1, 1, ComponentPatch::default()))),
        ),
        (
            776,
            "use_remainder",
            ComponentValue::ItemTemplate(Box::new(template(5, -1, ComponentPatch::default()))),
        ),
        (
            776,
            "use_remainder",
            ComponentValue::ItemTemplate(Box::new(ItemStack {
                item_id: 5,
                count: 1,
                data: ItemData::Legacy(None),
            })),
        ),
    ] {
        let mut w = Writer::new();
        w.raw(&[7, 8, 9]);
        assert!(slot(name, value)
            .write(&mut w, version(p), Limits::default())
            .is_err());
        assert_eq!(w.as_slice(), &[7, 8, 9]);
    }
    for (name, payload) in [
        ("use_remainder", "ff ff ff ff 0f 01 00 00"),
        ("use_remainder", "05 ff ff ff ff 0f 00 00"),
        ("container", "01 02"),
        ("use_remainder", "05 01 02 00 03 01 03 02"), // duplicate patch additions.
        ("use_remainder", "05 01 01 01 03 01 03"),    // conflicting removal.
    ] {
        let wire = fixture_bytes(776, name, &bytes(payload));
        let mut r = Reader::new(&wire, Limits::default());
        assert!(Slot::read(&mut r, version(776)).is_err());
        assert_eq!(r.position(), 0);
    }
}
#[test]
fn particle_templates_reuse_nested_inventory_budgets_and_zero_count_layout() {
    use rustwire_mc::packet::entity_metadata::particles::{Particle, ParticleData};
    for p in [775, 776] {
        // Item particle IDs are independently taken from the pinned registry,
        // while its template encoding is corrected against official codecs.
        let particle_id = if p == 775 { 47 } else { 54 };
        let particle = Particle {
            kind: "item",
            data: ParticleData::ItemTemplate(Box::new(template(0, 0, ComponentPatch::default()))),
        };
        let expected = vec![particle_id, 0, 0, 0, 0];
        assert_eq!(
            particle.encode(version(p), Limits::default()).unwrap(),
            expected
        );
        assert_eq!(
            Particle::decode(&expected, version(p), Limits::default()).unwrap(),
            particle
        );
        let nested = Particle {
            kind: "item",
            data: ParticleData::ItemTemplate(Box::new(template(
                5,
                1,
                ComponentPatch {
                    added: vec![Component {
                        name: "use_remainder",
                        value: ComponentValue::ItemTemplate(Box::new(template(
                            2,
                            1,
                            ComponentPatch::default(),
                        ))),
                    }],
                    removed: vec![],
                },
            ))),
        };
        let wire = nested.encode(version(p), Limits::default()).unwrap();
        let limits = Limits {
            max_nbt_depth: 0,
            ..Limits::default()
        };
        assert!(matches!(
            nested.encode(version(p), limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            Particle::decode(&wire, version(p), limits),
            Err(Error::Limit(_))
        ));
    }
}

#[test]
fn older_nested_items_distinguish_required_stacks_from_container_empties() {
    for p in 766..=774 {
        let item = Slot::Item(template(5, 1, ComponentPatch::default()));
        fixture(
            p,
            "container",
            "02 00 01 05 00 00",
            ComponentValue::Items(vec![Slot::Empty, item.clone()]),
        );
        for name in ["charged_projectiles", "bundle_contents"] {
            fixture(
                p,
                name,
                "01 01 05 00 00",
                ComponentValue::Items(vec![item.clone()]),
            );
            let empty = slot(name, ComponentValue::Items(vec![Slot::Empty]));
            assert!(matches!(
                empty.encode(version(p), Limits::default()),
                Err(Error::Invalid(_))
            ));
            assert!(matches!(
                Slot::decode(
                    &fixture_bytes(p, name, &[1, 0]),
                    version(p),
                    Limits::default()
                ),
                Err(Error::Invalid(_))
            ));
        }
        if p >= 768 {
            fixture(
                p,
                "use_remainder",
                "01 05 00 00",
                ComponentValue::Item(Box::new(item)),
            );
            assert!(matches!(
                slot("use_remainder", ComponentValue::Item(Box::new(Slot::Empty)))
                    .encode(version(p), Limits::default()),
                Err(Error::Invalid(_))
            ));
            assert!(matches!(
                Slot::decode(
                    &fixture_bytes(p, "use_remainder", &[0]),
                    version(p),
                    Limits::default()
                ),
                Err(Error::Invalid(_))
            ));
        }
        let overfull = slot("container", ComponentValue::Items(vec![Slot::Empty; 257]));
        assert!(matches!(
            overfull.encode(version(p), Limits::default()),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            Slot::decode(
                &fixture_bytes(p, "container", &[0x81, 2]),
                version(p),
                Limits::default()
            ),
            Err(Error::Limit(_))
        ));
    }
}
