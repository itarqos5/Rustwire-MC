//! Independent stream-layout fixtures for adventure-mode block predicates.
//! The schema's optional-NBT and partial-matcher shortcuts are intentionally
//! not used: fixtures follow the corresponding release server stream codecs.
use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, Tag},
    packet::{entity_metadata::holders::RegistryHolderSet, inventory::*},
    Error, Limits, Version,
};

fn v(protocol: i32) -> Version {
    Version::from_protocol(protocol).unwrap()
}
fn bytes(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn empty_nbt() -> Nbt {
    Nbt::anonymous(Tag::Compound(vec![]))
}
fn slot(name: &'static str, value: BlockPredicates) -> Slot {
    Slot::Item(ItemStack {
        item_id: 5,
        count: 1,
        data: ItemData::Components(ComponentPatch {
            added: vec![Component {
                name,
                value: ComponentValue::BlockPredicates(value),
            }],
            removed: vec![],
        }),
    })
}
// Independently transcribed component IDs, indexed by protocol minus 766.
fn wire(protocol: i32, name: &str, payload: &[u8]) -> Vec<u8> {
    let base = [10, 10, 11, 11, 11, 11, 11, 11, 14, 14, 14][(protocol - 766) as usize];
    let id = base + u8::from(name == "can_break");
    assert_eq!(component_id(v(protocol), name).unwrap(), i32::from(id));
    let mut wire = vec![1, 5, 1, 0, id];
    wire.extend_from_slice(payload);
    wire
}
fn fixture(protocol: i32, name: &'static str, payload: &[u8], value: BlockPredicates) {
    let wire = wire(protocol, name, payload);
    let expected = slot(name, value);
    assert_eq!(
        Slot::decode(&wire, v(protocol), Limits::default()).unwrap(),
        expected,
        "decode {protocol} {name}"
    );
    assert_eq!(
        expected.encode(v(protocol), Limits::default()).unwrap(),
        wire,
        "encode {protocol} {name}"
    );
    for end in 0..wire.len() {
        let mut r = Reader::new(&wire[..end], Limits::default());
        assert!(
            Slot::read(&mut r, v(protocol)).is_err(),
            "truncation at {end}"
        );
        assert_eq!(r.position(), 0, "atomic truncation at {end}");
    }
    let mut tail = wire;
    tail.push(0);
    assert!(Slot::decode(&tail, v(protocol), Limits::default()).is_err());
}
fn predicate(protocol: i32) -> ItemBlockPredicate {
    ItemBlockPredicate {
        components: (protocol >= 770).then(DataComponentMatchers::default),
        ..ItemBlockPredicate::default()
    }
}
fn predicates(protocol: i32, predicates: Vec<ItemBlockPredicate>) -> BlockPredicates {
    BlockPredicates {
        predicates,
        show_tooltip: (protocol < 770).then_some(true),
    }
}

#[test]
fn absent_and_present_empty_fields_all_releases() {
    for p in 766..=776 {
        for name in ["can_place_on", "can_break"] {
            let mut payload = bytes("02 00 00 00");
            if p >= 770 {
                payload.extend_from_slice(&[0, 0]);
            }
            payload.extend(bytes("01 01 01 00 00"));
            if p >= 770 {
                payload.extend_from_slice(&[0, 0]);
            } else {
                payload.push(1);
            }
            let empty = ItemBlockPredicate {
                blocks: Some(RegistryHolderSet::Ids(vec![])),
                properties: Some(vec![]),
                ..predicate(p)
            };
            fixture(p, name, &payload, predicates(p, vec![predicate(p), empty]));
        }
    }
}

#[test]
fn tagged_blocks_exact_and_ranged_properties_boolean_prefixed_nbt() {
    for p in 766..=776 {
        let mut payload = bytes(
            "01 01 00 01 74 01 03
             04 61 78 69 73 01 01 78
             03 61 67 65 00 01 01 31 00
             03 61 67 65 00 00 01 01 37
             01 0a 03 00 01 78 00 00 00 07 00",
        );
        if p >= 770 {
            payload.extend_from_slice(&[0, 0]);
        } else {
            payload.push(1);
        }
        let value = ItemBlockPredicate {
            blocks: Some(RegistryHolderSet::Tag("t".into())),
            properties: Some(vec![
                BlockPropertyMatcher {
                    name: "axis".into(),
                    value: BlockPropertyValue::Exact("x".into()),
                },
                BlockPropertyMatcher {
                    name: "age".into(),
                    value: BlockPropertyValue::Range {
                        minimum: Some("1".into()),
                        maximum: None,
                    },
                },
                BlockPropertyMatcher {
                    name: "age".into(),
                    value: BlockPropertyValue::Range {
                        minimum: None,
                        maximum: Some("7".into()),
                    },
                },
            ]),
            nbt: Some(Nbt::anonymous(Tag::Compound(vec![(
                "x".into(),
                Tag::Int(7),
            )]))),
            ..predicate(p)
        };
        fixture(p, "can_break", &payload, predicates(p, vec![value]));
    }
}

#[test]
fn explicit_holder_ids_and_unbounded_range() {
    for p in 766..=776 {
        let mut payload = bytes("01 01 03 05 ac 02 01 01 01 78 00 00 00 00");
        if p >= 770 {
            payload.extend_from_slice(&[0, 0]);
        } else {
            payload.push(1);
        }
        let value = ItemBlockPredicate {
            blocks: Some(RegistryHolderSet::Ids(vec![5, 300])),
            properties: Some(vec![BlockPropertyMatcher {
                name: "x".into(),
                value: BlockPropertyValue::Range {
                    minimum: None,
                    maximum: None,
                },
            }]),
            ..predicate(p)
        };
        fixture(p, "can_place_on", &payload, predicates(p, vec![value]));
    }
}

#[test]
fn exact_components_are_typed_and_use_no_removal_count() {
    for p in 770..=776 {
        fixture(
            p,
            "can_place_on",
            &bytes("01 00 00 00 02 03 ac 02 00 0a 00 00"),
            predicates(
                p,
                vec![ItemBlockPredicate {
                    components: Some(DataComponentMatchers {
                        exact: vec![
                            Component {
                                name: "damage",
                                value: ComponentValue::VarInt(300),
                            },
                            Component {
                                name: "custom_data",
                                value: ComponentValue::Nbt(empty_nbt()),
                            },
                        ],
                        partial: vec![],
                    }),
                    ..predicate(p)
                }],
            ),
        );
    }
}

#[test]
fn concrete_partial_matchers_include_nbt_and_gain_a_selector_in_774() {
    for p in 770..=776 {
        let mut payload = bytes("01 00 00 00 00 01");
        if p >= 774 {
            payload.push(1);
        }
        payload.extend(bytes("00 0a 00"));
        fixture(
            p,
            "can_break",
            &payload,
            predicates(
                p,
                vec![ItemBlockPredicate {
                    components: Some(DataComponentMatchers {
                        exact: vec![],
                        partial: vec![PartialComponentMatcher {
                            kind: ComponentPredicateKind::Damage,
                            data: empty_nbt(),
                        }],
                    }),
                    ..predicate(p)
                }],
            ),
        );
    }
}

#[test]
fn any_value_partial_matcher_includes_its_nbt_unit_payload() {
    for p in 774..=776 {
        fixture(
            p,
            "can_break",
            &bytes("01 00 00 00 00 01 00 03 0a 00"),
            predicates(
                p,
                vec![ItemBlockPredicate {
                    components: Some(DataComponentMatchers {
                        exact: vec![],
                        partial: vec![PartialComponentMatcher {
                            kind: ComponentPredicateKind::AnyValue("damage"),
                            data: empty_nbt(),
                        }],
                    }),
                    ..predicate(p)
                }],
            ),
        );
    }
}

#[test]
fn nested_exact_block_predicates_share_depth_and_collection_budgets() {
    let p = 776;
    let nested = BlockPredicates {
        predicates: vec![],
        show_tooltip: None,
    };
    let value = predicates(
        p,
        vec![ItemBlockPredicate {
            components: Some(DataComponentMatchers {
                exact: vec![Component {
                    name: "can_break",
                    value: ComponentValue::BlockPredicates(nested),
                }],
                partial: vec![],
            }),
            ..predicate(p)
        }],
    );
    let payload = bytes("01 00 00 00 01 0f 00 00");
    fixture(p, "can_place_on", &payload, value.clone());
    let encoded = wire(p, "can_place_on", &payload);
    for limits in [
        Limits {
            max_nbt_depth: 0,
            ..Limits::default()
        },
        Limits {
            max_collection: 3,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            Slot::decode(&encoded, v(p), limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            slot("can_place_on", value.clone()).encode(v(p), limits),
            Err(Error::Limit(_))
        ));
    }
    let limits = Limits {
        max_nbt_depth: 1,
        max_collection: 4,
        ..Limits::default()
    };
    assert_eq!(
        Slot::decode(&encoded, v(p), limits).unwrap(),
        slot("can_place_on", value)
    );
}

#[test]
fn block_and_partial_matcher_nbt_share_node_budget() {
    let p = 776;
    let value = predicates(
        p,
        vec![ItemBlockPredicate {
            nbt: Some(empty_nbt()),
            components: Some(DataComponentMatchers {
                exact: vec![],
                partial: vec![PartialComponentMatcher {
                    kind: ComponentPredicateKind::Damage,
                    data: empty_nbt(),
                }],
            }),
            ..predicate(p)
        }],
    );
    let encoded = wire(
        p,
        "can_break",
        &bytes("01 00 00 01 0a 00 00 01 01 00 0a 00"),
    );
    let limits = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        Slot::decode(&encoded, v(p), limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        slot("can_break", value).encode(v(p), limits),
        Err(Error::Limit(_))
    ));
}

#[test]
fn rejects_wrong_release_fields_noncompound_nbt_and_unknown_kinds() {
    let p = 776;
    for payload in [
        // Present block NBT cannot be TAG_End or an integer.
        "01 00 00 01 00 00 00",
        "01 00 00 01 03 00 00 00 01 00 00",
        // Concrete partial kind 15 is not registered.
        "01 00 00 00 00 01 01 0f 0a 00",
        // Partial NBT is required, including any-value predicates.
        "01 00 00 00 00 01 00 03 00",
        // The partial matcher limit is 64, independently of global budget.
        "01 00 00 00 00 41",
        // Duplicate concrete matcher keys are invalid.
        "01 00 00 00 00 02 01 00 0a 00 01 00 0a 00",
    ] {
        assert!(Slot::decode(
            &wire(p, "can_break", &bytes(payload)),
            v(p),
            Limits::default()
        )
        .is_err());
    }
    let mut invalid = predicates(769, vec![predicate(769)]);
    assert!(slot("can_break", invalid.clone())
        .encode(v(p), Limits::default())
        .is_err());
    invalid.show_tooltip = None;
    assert!(slot("can_break", invalid)
        .encode(v(769), Limits::default())
        .is_err());
    let mut invalid = predicates(p, vec![predicate(p)]);
    invalid.predicates[0].nbt = Some(Nbt::anonymous(Tag::Int(1)));
    let mut writer = Writer::new();
    writer.u8(0xaa);
    assert!(slot("can_break", invalid)
        .write(&mut writer, v(p), Limits::default())
        .is_err());
    assert_eq!(writer.as_slice(), &[0xaa]);
}

#[test]
fn partial_kind_release_boundaries_are_fail_closed() {
    for (p, kind) in [
        (773, ComponentPredicateKind::AnyValue("damage")),
        (774, ComponentPredicateKind::VillagerVariant),
        (776, ComponentPredicateKind::AnyValue("not_a_component")),
    ] {
        let value = predicates(
            p,
            vec![ItemBlockPredicate {
                components: Some(DataComponentMatchers {
                    exact: vec![],
                    partial: vec![PartialComponentMatcher {
                        kind,
                        data: empty_nbt(),
                    }],
                }),
                ..predicate(p)
            }],
        );
        assert!(matches!(
            slot("can_break", value).encode(v(p), Limits::default()),
            Err(Error::Unsupported(_))
        ));
    }
    assert!(matches!(
        Slot::decode(
            &wire(774, "can_break", &bytes("01 00 00 00 00 01 01 0e 0a 00")),
            v(774),
            Limits::default()
        ),
        Err(Error::Unsupported(_))
    ));
}
