//! Byte fixtures are specified from pinned protocol schemas, independently of
//! Rustwire's encoder. Each boundary is exercised both ways; roundtrips alone
//! are not evidence of the correct layout.
use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, NbtString, Tag},
    packet::inventory::*,
    version::{Direction, State},
    Error, Limits, Version,
};
fn v(n: i32) -> Version {
    Version::from_protocol(n).unwrap()
}
fn decode(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect()
}
fn modern(count: i32, added: Vec<Component>, removed: Vec<&'static str>) -> Slot {
    Slot::Item(ItemStack {
        item_id: 5,
        count,
        data: ItemData::Components(ComponentPatch { added, removed }),
    })
}
fn check_slot(protocol: i32, hex: &str, expected: Slot) {
    let bytes = decode(hex);
    let version = v(protocol);
    assert_eq!(
        Slot::decode(&bytes, version, Limits::default()).unwrap(),
        expected,
        "decode {protocol}"
    );
    assert_eq!(
        expected.encode(version, Limits::default()).unwrap(),
        bytes,
        "encode {protocol}"
    );
    for end in 0..bytes.len() {
        assert!(
            Slot::decode(&bytes[..end], version, Limits::default()).is_err(),
            "truncation {protocol} at {end}"
        );
    }
    let mut tail = bytes.clone();
    tail.push(0);
    assert!(Slot::decode(&tail, version, Limits::default()).is_err());
}
#[test]
fn empty_and_plain_slots_all_releases() {
    for version in Version::ALL {
        check_slot(version.protocol(), "00", Slot::Empty);
        let (hex, data) = if version.protocol() < 766 {
            ("01 05 40 00", ItemData::Legacy(None))
        } else {
            (
                "40 05 00 00",
                ItemData::Components(ComponentPatch::default()),
            )
        };
        check_slot(
            version.protocol(),
            hex,
            Slot::Item(ItemStack {
                item_id: 5,
                count: 64,
                data,
            }),
        );
    }
    // Count 128 is multi-byte VarInt from 767; 766 uses signed i8 and rejects it.
    check_slot(767, "80 01 05 00 00", modern(128, vec![], vec![]));
    assert!(Slot::decode(&decode("80 01 05 00 00"), v(766), Limits::default()).is_err());
    assert!(modern(128, vec![], vec![])
        .encode(v(766), Limits::default())
        .is_err());
}
#[test]
fn classic_nbt_named_to_anonymous_boundary() {
    let root = Tag::Compound(vec![(NbtString::from("Damage"), Tag::Int(7))]);
    let named = Nbt {
        name: Some(NbtString::default()),
        root: root.clone(),
    };
    check_slot(
        763,
        "01 05 01 0a 00 00 03 00 06 44 61 6d 61 67 65 00 00 00 07 00",
        Slot::Item(ItemStack {
            item_id: 5,
            count: 1,
            data: ItemData::Legacy(Some(named)),
        }),
    );
    for p in [764, 765] {
        check_slot(
            p,
            "01 05 01 0a 03 00 06 44 61 6d 61 67 65 00 00 00 07 00",
            Slot::Item(ItemStack {
                item_id: 5,
                count: 1,
                data: ItemData::Legacy(Some(Nbt::anonymous(root.clone()))),
            }),
        );
    }
}
#[test]
fn common_components_and_removals_all_modern_releases() {
    // A damage component followed by removal of max_damage. The two counts
    // precede both arrays (they are not interleaved with array payloads).
    for p in 766..=776 {
        check_slot(
            p,
            "01 05 01 01 03 ac 02 02",
            modern(
                1,
                vec![Component {
                    name: "damage",
                    value: ComponentValue::VarInt(300),
                }],
                vec!["max_damage"],
            ),
        );
    }
}
#[test]
fn custom_names_use_release_specific_component_ids() {
    let text = Nbt::anonymous(Tag::String(NbtString::from("A")));
    for p in 766..=776 {
        let hex = if p < 774 {
            "01 05 01 00 05 08 00 01 41"
        } else {
            "01 05 01 00 06 08 00 01 41"
        };
        check_slot(
            p,
            hex,
            modern(
                1,
                vec![Component {
                    name: "custom_name",
                    value: ComponentValue::Nbt(text.clone()),
                }],
                vec![],
            ),
        );
    }
}
#[test]
fn enchantment_tooltip_and_lore_changes() {
    for p in 766..=776 {
        let (id, tooltip) = if p < 768 {
            ("09", true)
        } else if p < 770 {
            ("0a", true)
        } else if p < 774 {
            ("0a", false)
        } else {
            ("0d", false)
        };
        let hex = format!(
            "01 05 01 00 {id} 01 02 03{}",
            if tooltip { " 01" } else { "" }
        );
        check_slot(
            p,
            &hex,
            modern(
                1,
                vec![Component {
                    name: "enchantments",
                    value: ComponentValue::Enchantments {
                        entries: vec![Enchantment { id: 2, level: 3 }],
                        show_tooltip: tooltip.then_some(true),
                    },
                }],
                vec![],
            ),
        );
    }
    check_slot(
        769,
        "01 05 01 00 08 01 00",
        modern(
            1,
            vec![Component {
                name: "lore",
                value: ComponentValue::Lore(vec![None]),
            }],
            vec![],
        ),
    );
    assert!(Slot::decode(&decode("01 05 01 00 08 01 00"), v(770), Limits::default()).is_err());
    check_slot(
        770,
        "01 05 01 00 08 01 08 00 01 41",
        modern(
            1,
            vec![Component {
                name: "lore",
                value: ComponentValue::Lore(vec![Some(Nbt::anonymous(Tag::String(
                    NbtString::from("A"),
                )))]),
            }],
            vec![],
        ),
    );
}
#[test]
fn unit_dyed_color_and_model_data_boundaries() {
    check_slot(
        769,
        "01 05 01 00 04 01",
        modern(
            1,
            vec![Component {
                name: "unbreakable",
                value: ComponentValue::Bool(true),
            }],
            vec![],
        ),
    );
    check_slot(
        770,
        "01 05 01 00 04",
        modern(
            1,
            vec![Component {
                name: "unbreakable",
                value: ComponentValue::Unit,
            }],
            vec![],
        ),
    );
    check_slot(
        769,
        "01 05 01 00 22 00 11 22 33 00",
        modern(
            1,
            vec![Component {
                name: "dyed_color",
                value: ComponentValue::DyedColor {
                    color: 0x112233,
                    show_tooltip: false,
                },
            }],
            vec![],
        ),
    );
    check_slot(
        770,
        "01 05 01 00 23 00 11 22 33",
        modern(
            1,
            vec![Component {
                name: "dyed_color",
                value: ComponentValue::Int(0x112233),
            }],
            vec![],
        ),
    );
    check_slot(
        767,
        "01 05 01 00 0d 07",
        modern(
            1,
            vec![Component {
                name: "custom_model_data",
                value: ComponentValue::VarInt(7),
            }],
            vec![],
        ),
    );
    check_slot(
        768,
        "01 05 01 00 0e 01 3f c0 00 00 01 01 01 01 41 01 00 11 22 33",
        modern(
            1,
            vec![Component {
                name: "custom_model_data",
                value: ComponentValue::CustomModelData(CustomModelData {
                    floats: vec![1.5],
                    flags: vec![true],
                    strings: vec!["A".into()],
                    colors: vec![0x112233],
                }),
            }],
            vec![],
        ),
    );
}
#[test]
fn typed_entity_nbt_change_and_latest_empty_payload_change() {
    check_slot(
        772,
        "01 05 01 00 31 0a 00",
        modern(
            1,
            vec![Component {
                name: "entity_data",
                value: ComponentValue::Nbt(Nbt::anonymous(Tag::Compound(vec![]))),
            }],
            vec![],
        ),
    );
    check_slot(
        773,
        "01 05 01 00 31 02 0a 00",
        modern(
            1,
            vec![Component {
                name: "entity_data",
                value: ComponentValue::TypedNbt {
                    type_id: 2,
                    data: Nbt::anonymous(Tag::Compound(vec![])),
                },
            }],
            vec![],
        ),
    );
    check_slot(
        775,
        "01 05 01 00 16 0a 00",
        modern(
            1,
            vec![Component {
                name: "intangible_projectile",
                value: ComponentValue::Nbt(Nbt::anonymous(Tag::Compound(vec![]))),
            }],
            vec![],
        ),
    );
    check_slot(
        776,
        "01 05 01 00 16",
        modern(
            1,
            vec![Component {
                name: "intangible_projectile",
                value: ComponentValue::Unit,
            }],
            vec![],
        ),
    );
}
#[test]
fn recursive_components_and_templates_have_distinct_layouts() {
    check_slot(
        774,
        "01 05 01 00 19 01 05 00 00",
        modern(
            1,
            vec![Component {
                name: "use_remainder",
                value: ComponentValue::Item(Box::new(modern(1, vec![], vec![]))),
            }],
            vec![],
        ),
    );
    for p in [775, 776] {
        check_slot(
            p,
            "01 05 01 00 19 05 01 00 00",
            modern(
                1,
                vec![Component {
                    name: "use_remainder",
                    value: ComponentValue::ItemTemplate(Box::new(ItemStack {
                        item_id: 5,
                        count: 1,
                        data: ItemData::Components(ComponentPatch::default()),
                    })),
                }],
                vec![],
            ),
        );
        assert!(Slot::decode(&decode("01 05 01 00 19 00"), v(p), Limits::default()).is_err());
    }
    check_slot(
        770,
        "01 05 01 00 29 02 02 05 00 00 01 05 00 00",
        modern(
            1,
            vec![Component {
                name: "bundle_contents",
                value: ComponentValue::Items(vec![
                    modern(2, vec![], vec![]),
                    modern(1, vec![], vec![]),
                ]),
            }],
            vec![],
        ),
    );
}
#[test]
fn coverage_registry_is_release_specific_and_honest() {
    let expected = [
        (766, 56, 5),
        (767, 57, 5),
        (768, 67, 5),
        (769, 67, 5),
        (770, 96, 5),
        (771, 96, 5),
        (772, 96, 5),
        (773, 96, 5),
        (774, 104, 6),
        (775, 110, 6),
        (776, 111, 6),
    ];
    for (p, count, name_id) in expected {
        let registry = component_registry(v(p));
        assert_eq!(registry.len(), count);
        assert_eq!(component_id(v(p), "custom_name").unwrap(), name_id);
        assert_eq!(
            registry[component_id(v(p), "can_place_on").unwrap() as usize].1,
            ComponentWire::BlockPredicates
        );
        for (id, (name, _)) in registry.iter().enumerate() {
            assert_eq!(component_id(v(p), name).unwrap(), id as i32);
        }
    }
    assert!(component_registry(v(765)).is_empty());
    assert!(component_id(v(766), "item_model").is_err());
    assert!(component_id(v(774), "additional_trade_cost").is_err());
}
#[test]
fn unsupported_payload_never_consumes_unknown_layout() {
    // Unknown component ID 255 has no known payload length: fail closed
    // without guessing or scanning for the following component.
    let bytes = decode("01 05 01 00 ff 01 ff aa 55");
    let mut r = Reader::new(&bytes, Limits::default());
    assert!(matches!(
        Slot::read(&mut r, v(770)),
        Err(Error::Unsupported(_))
    ));
    assert_eq!(r.position(), 0); // Failed public reads are atomic.
    assert_eq!(r.remaining(), bytes.as_slice());
    // Removal requires only an ID; complex removed components are safe.
    check_slot(
        770,
        "01 05 00 01 0b",
        modern(1, vec![], vec!["can_place_on"]),
    );
    assert!(matches!(
        Slot::decode(&decode("01 05 01 00 ff 01"), v(770), Limits::default()),
        Err(Error::Unsupported(_))
    ));
}
#[test]
fn malformed_slot_data_is_rejected() {
    for (p, hex) in [
        (763, "02"),
        (766, "ff"),
        (770, "ff ff ff ff 0f"),
        (770, "01 ff ff ff ff 0f 00 00"),
        (770, "01 05 ff ff ff ff 0f 00"),
        (770, "01 05 02 00 03 01 03 02"),
        (770, "01 05 01 01 03 01 03"),
        (770, "01 05 01 00 09 04"),
        (770, "01 05 01 00 00 00"),
    ] {
        assert!(
            Slot::decode(&decode(hex), v(p), Limits::default()).is_err(),
            "accepted {p} {hex}"
        );
    }
    let item = modern(
        1,
        vec![Component {
            name: "damage",
            value: ComponentValue::Bool(true),
        }],
        vec![],
    );
    assert!(item.encode(v(770), Limits::default()).is_err());
    let mut w = Writer::new();
    w.u8(42);
    assert!(item.write(&mut w, v(770), Limits::default()).is_err());
    assert_eq!(w.as_slice(), &[42]);
    assert!(modern(1, vec![], vec![])
        .encode(v(765), Limits::default())
        .is_err());
}
#[test]
fn collection_nbt_and_recursive_depth_budgets() {
    let bytes = decode("01 05 01 00 03 01");
    let limits = Limits {
        max_collection: 1,
        ..Limits::default()
    };
    assert!(matches!(
        Slot::decode(&bytes, v(770), limits),
        Err(Error::Limit(_))
    ));
    assert!(modern(
        1,
        vec![Component {
            name: "damage",
            value: ComponentValue::VarInt(1)
        }],
        vec![]
    )
    .encode(v(770), limits)
    .is_err());
    let limits = Limits {
        max_packet: bytes.len() - 1,
        ..Limits::default()
    };
    assert!(matches!(
        Slot::decode(&bytes, v(770), limits),
        Err(Error::Limit(_))
    ));
    let nested = modern(
        1,
        vec![Component {
            name: "use_remainder",
            value: ComponentValue::Item(Box::new(Slot::Empty)),
        }],
        vec![],
    );
    let limits = Limits {
        max_nbt_depth: 0,
        ..Limits::default()
    };
    assert!(matches!(
        nested.encode(v(770), limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        Slot::decode(&decode("01 05 01 00 16 00"), v(770), limits),
        Err(Error::Limit(_))
    ));
    // Two individually valid empty roots share a packet-wide NBT node budget.
    let bytes = decode("01 05 02 00 00 0a 00 05 08 00 01 41");
    let limits = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        Slot::decode(&bytes, v(770), limits),
        Err(Error::Limit(_))
    ));
    let value = Slot::decode(&bytes, v(770), Limits::default()).unwrap();
    assert!(matches!(value.encode(v(770), limits), Err(Error::Limit(_))));
}

#[test]
fn container_contents_and_set_slot_all_release_fixtures() {
    for version in Version::ALL {
        let p = version.protocol();
        let id = if p < 768 { "82" } else { "82 01" };
        let stack = if p < 766 {
            "01 05 01 00"
        } else {
            "01 05 00 00"
        };
        let item = if p < 766 {
            Slot::Item(ItemStack {
                item_id: 5,
                count: 1,
                data: ItemData::Legacy(None),
            })
        } else {
            modern(1, vec![], vec![])
        };
        let bytes = decode(&format!("{id} ac 02 02 00 {stack} 00"));
        let expected = ContainerContent {
            window_id: 130,
            state_id: 300,
            items: vec![Slot::Empty, item.clone()],
            carried_item: Slot::Empty,
        };
        assert_eq!(
            ContainerContent::decode(&bytes, *version, Limits::default()).unwrap(),
            expected,
            "content {p}"
        );
        assert_eq!(expected.encode(*version, Limits::default()).unwrap(), bytes);
        for end in 0..bytes.len() {
            assert!(
                ContainerContent::decode(&bytes[..end], *version, Limits::default()).is_err(),
                "content truncation {p} {end}"
            );
        }
        let (id, window_id) = if p <= 767 { ("ff", -1) } else { ("82 01", 130) };
        let bytes = decode(&format!("{id} ac 02 ff ff {stack}"));
        let expected = SetContainerSlot {
            window_id,
            state_id: 300,
            slot: -1,
            item,
        };
        assert_eq!(
            SetContainerSlot::decode(&bytes, *version, Limits::default()).unwrap(),
            expected,
            "set slot {p}"
        );
        assert_eq!(expected.encode(*version, Limits::default()).unwrap(), bytes);
        for end in 0..bytes.len() {
            assert!(SetContainerSlot::decode(&bytes[..end], *version, Limits::default()).is_err());
        }
    }
}
#[test]
fn screen_open_close_and_selected_slot_release_fixtures() {
    for version in Version::ALL {
        let p = version.protocol();
        let (hex, title) = if p < 765 {
            ("ac 02 02 02 7b 7d", ScreenTitle::Json("{}".into()))
        } else {
            (
                "ac 02 02 08 00 01 41",
                ScreenTitle::Nbt(Nbt::anonymous(Tag::String(NbtString::from("A")))),
            )
        };
        let expected = OpenScreen {
            window_id: 300,
            menu_type: 2,
            title,
        };
        let bytes = decode(hex);
        assert_eq!(
            OpenScreen::decode(&bytes, *version, Limits::default()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(*version, Limits::default()).unwrap(), bytes);
        let bytes = decode(if p < 768 { "82" } else { "82 01" });
        let close = CloseContainer { window_id: 130 };
        assert_eq!(
            CloseContainer::decode(&bytes, *version, Limits::default()).unwrap(),
            close
        );
        assert_eq!(close.encode(*version, Limits::default()).unwrap(), bytes);
        let packet = close.packet(*version, Limits::default()).unwrap();
        assert_eq!(packet.data, bytes);
        assert_eq!(
            packet.id,
            version
                .packet_id(State::Play, Direction::Serverbound, "close_window")
                .unwrap()
        );
        let selected = SetSelectedSlot { slot: 8 };
        assert_eq!(
            SetSelectedSlot::decode(&[8], *version, Limits::default()).unwrap(),
            selected
        );
        assert_eq!(
            selected.encode(*version, Limits::default()).unwrap(),
            vec![8]
        );
        let packet = selected.packet(*version, Limits::default()).unwrap();
        assert_eq!(packet.data, vec![0, 8]);
        assert_eq!(
            packet.id,
            version
                .packet_id(State::Play, Direction::Serverbound, "held_item_slot")
                .unwrap()
        );
    }
    // A legal non-canonical VarInt distinguishes the 769 field type change.
    assert!(SetSelectedSlot::decode(&[0x88, 0], v(768), Limits::default()).is_err());
    assert_eq!(
        SetSelectedSlot::decode(&[0x88, 0], v(769), Limits::default())
            .unwrap()
            .slot,
        8
    );
    assert!(SetSelectedSlot::decode(&[9], v(770), Limits::default()).is_err());
    assert!(SetSelectedSlot { slot: 9 }
        .packet(v(770), Limits::default())
        .is_err());
    assert!(CloseContainer { window_id: 300 }
        .encode(v(767), Limits::default())
        .is_err());
}
#[test]
fn separate_cursor_and_player_inventory_from_768() {
    for p in 768..=776 {
        let expected = SetPlayerInventory {
            slot: 42,
            item: modern(1, vec![], vec![]),
        };
        let bytes = decode("2a 01 05 00 00");
        assert_eq!(
            SetPlayerInventory::decode(&bytes, v(p), Limits::default()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(v(p), Limits::default()).unwrap(), bytes);
        let expected = SetCursorItem {
            item: modern(1, vec![], vec![]),
        };
        let bytes = decode("01 05 00 00");
        assert_eq!(
            SetCursorItem::decode(&bytes, v(p), Limits::default()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(v(p), Limits::default()).unwrap(), bytes);
    }
    assert!(matches!(
        SetCursorItem::decode(&[0], v(767), Limits::default()),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        SetPlayerInventory::decode(&[0, 0], v(767), Limits::default()),
        Err(Error::Unsupported(_))
    ));
}
fn click_header() -> ClickHeader {
    ClickHeader {
        window_id: 130,
        state_id: 300,
        slot: -999,
        button: 0,
        mode: ClickMode::Pickup,
    }
}
#[test]
fn full_stack_click_fixtures_and_state_ids() {
    for p in 763..=769 {
        let id = if p < 768 { "82" } else { "82 01" };
        let (stack, item) = if p < 766 {
            (
                "01 05 01 00",
                Slot::Item(ItemStack {
                    item_id: 5,
                    count: 1,
                    data: ItemData::Legacy(None),
                }),
            )
        } else {
            ("01 05 00 00", modern(1, vec![], vec![]))
        };
        let bytes = decode(&format!("{id} ac 02 fc 19 00 00 01 00 05 {stack} 00"));
        let expected = ContainerClick {
            header: click_header(),
            changed_slots: vec![(5, item)],
            carried_item: Slot::Empty,
        };
        assert_eq!(
            ContainerClick::decode(&bytes, v(p), Limits::default()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(v(p), Limits::default()).unwrap(), bytes);
        let packet = expected.packet(v(p), Limits::default()).unwrap();
        assert_eq!(packet.data, bytes);
        assert_eq!(
            packet.id,
            v(p).packet_id(State::Play, Direction::Serverbound, "window_click")
                .unwrap()
        );
        for end in 0..bytes.len() {
            assert!(ContainerClick::decode(&bytes[..end], v(p), Limits::default()).is_err());
        }
    }
    let click = ContainerClick {
        header: click_header(),
        changed_slots: vec![],
        carried_item: Slot::Empty,
    };
    assert!(matches!(
        click.encode(v(770), Limits::default()),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        ContainerClick::decode(&[], v(770), Limits::default()),
        Err(Error::Unsupported(_))
    ));
}
#[test]
fn hashed_clicks_from_770_have_separate_optional_stacks() {
    let item = HashedItemStack {
        item_id: 5,
        count: 1,
        components: vec![("damage", 0xdeadbeefu32 as i32)],
        removed_components: vec!["max_damage"],
    };
    let expected = HashedContainerClick {
        header: click_header(),
        changed_slots: vec![(5, Some(item))],
        carried_item: None,
    };
    // Component hashes precede the removed-component array count. The hashed
    // slot is presence + item ID + count, unlike the regular count + item ID.
    let bytes = decode("82 01 ac 02 fc 19 00 00 01 00 05 01 05 01 01 03 de ad be ef 01 02 00");
    for p in 770..=776 {
        assert_eq!(
            HashedContainerClick::decode(&bytes, v(p), Limits::default()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(v(p), Limits::default()).unwrap(), bytes);
        assert_eq!(
            expected.packet(v(p), Limits::default()).unwrap().data,
            bytes
        );
        for end in 0..bytes.len() {
            assert!(HashedContainerClick::decode(&bytes[..end], v(p), Limits::default()).is_err());
        }
    }
    assert!(matches!(
        expected.encode(v(769), Limits::default()),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        HashedContainerClick::decode(&[], v(769), Limits::default()),
        Err(Error::Unsupported(_))
    ));
    // A known complex component can be carried as a caller-provided hash.
    let expected = HashedContainerClick {
        header: click_header(),
        changed_slots: vec![],
        carried_item: Some(HashedItemStack {
            item_id: 5,
            count: 1,
            components: vec![("can_place_on", 1)],
            removed_components: vec![],
        }),
    };
    let bytes = decode("82 01 ac 02 fc 19 00 00 00 01 05 01 01 0b 00 00 00 01 00");
    assert_eq!(
        HashedContainerClick::decode(&bytes, v(770), Limits::default()).unwrap(),
        expected
    );
    assert_eq!(expected.encode(v(770), Limits::default()).unwrap(), bytes);
}
#[test]
fn all_packet_decoders_reject_trailing_bytes_and_oversize_input() {
    let limits = Limits::default();
    let version = v(770);
    assert!(ContainerContent::decode(&decode("00 00 00 00 ff"), version, limits).is_err());
    assert!(SetContainerSlot::decode(&decode("00 00 00 00 00 ff"), version, limits).is_err());
    assert!(OpenScreen::decode(&decode("00 00 08 00 01 41 ff"), version, limits).is_err());
    assert!(CloseContainer::decode(&[0, 1], version, limits).is_err());
    assert!(SetSelectedSlot::decode(&[0, 1], version, limits).is_err());
    assert!(SetPlayerInventory::decode(&[0, 0, 1], version, limits).is_err());
    assert!(SetCursorItem::decode(&[0, 1], version, limits).is_err());
    assert!(ContainerClick::decode(&decode("00 00 00 00 00 00 00 00 ff"), v(769), limits).is_err());
    assert!(
        HashedContainerClick::decode(&decode("00 00 00 00 00 00 00 00 ff"), version, limits)
            .is_err()
    );
    let limits = Limits {
        max_packet: 0,
        ..limits
    };
    assert!(matches!(
        CloseContainer::decode(&[0], version, limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        SetSelectedSlot { slot: 1 }.packet(version, limits),
        Err(Error::Limit(_))
    ));
}
#[test]
fn packet_shared_budgets_prevent_many_small_roots() {
    // Two slots each have a custom_data empty compound. Each is one NBT node,
    // so a budget of one must fail even though either individual slot fits.
    let bytes = decode("00 00 02 01 05 01 00 00 0a 00 01 05 01 00 00 0a 00 00");
    let limits = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        ContainerContent::decode(&bytes, v(770), limits),
        Err(Error::Limit(_))
    ));
    let valid = ContainerContent::decode(&bytes, v(770), Limits::default()).unwrap();
    assert!(matches!(valid.encode(v(770), limits), Err(Error::Limit(_))));
    let limits = Limits {
        max_collection: 2,
        ..Limits::default()
    };
    assert!(matches!(
        ContainerContent::decode(&[0, 0, 3], v(770), limits),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        ContainerContent::decode(&decode("00 00 ff ff ff ff 07"), v(770), Limits::default()),
        Err(Error::Limit(_))
    ));
}

#[test]
fn remaining_supported_payload_shapes_have_golden_bytes() {
    check_slot(
        770,
        "01 05 01 00 07 03 61 3a 62",
        modern(
            1,
            vec![Component {
                name: "item_model",
                value: ComponentValue::String("a:b".into()),
            }],
            vec![],
        ),
    );
    // potion_duration_scale in protocol 770 is component 43 (0x2b).
    check_slot(
        770,
        "01 05 01 00 2b 3f c0 00 00",
        modern(
            1,
            vec![Component {
                name: "potion_duration_scale",
                value: ComponentValue::Float(1.5),
            }],
            vec![],
        ),
    );
    check_slot(
        770,
        "01 05 01 00 0f 01 02 03 0b",
        modern(
            1,
            vec![Component {
                name: "tooltip_display",
                value: ComponentValue::TooltipDisplay {
                    hide_tooltip: true,
                    hidden_components: vec!["damage", "can_place_on"],
                },
            }],
            vec![],
        ),
    );
    check_slot(
        766,
        "01 05 01 00 34 01 01 41 01 42",
        modern(
            1,
            vec![Component {
                name: "block_state",
                value: ComponentValue::BlockState(vec![("A".into(), "B".into())]),
            }],
            vec![],
        ),
    );
    check_slot(
        770,
        "01 05 01 00 41 02 05 ac 02",
        modern(
            1,
            vec![Component {
                name: "pot_decorations",
                value: ComponentValue::IntList(vec![5, 300]),
            }],
            vec![],
        ),
    );
    // 775's DyeColor alias remains a bounded VarInt enum.
    check_slot(
        775,
        "01 05 01 00 2b 0f",
        modern(
            1,
            vec![Component {
                name: "dye",
                value: ComponentValue::VarInt(15),
            }],
            vec![],
        ),
    );
    assert!(Slot::decode(&decode("01 05 01 00 2b 10"), v(775), Limits::default()).is_err());
}
#[test]
fn public_stream_reads_are_byte_bounded_and_atomic() {
    let bytes = decode("01 05 01 00 07 03 61 3a 62");
    let limits = Limits {
        max_packet: 8,
        ..Limits::default()
    };
    let mut reader = Reader::new(&bytes, limits);
    assert!(Slot::read(&mut reader, v(770)).is_err());
    assert_eq!(reader.position(), 0);
    let mut reader = Reader::new(&[0, 42], Limits::default());
    assert_eq!(Slot::read(&mut reader, v(770)).unwrap(), Slot::Empty);
    assert_eq!(reader.remaining(), &[42]);
}

#[test]
fn encoding_large_component_arrays_respects_small_packet_limits() {
    let item = modern(
        1,
        vec![Component {
            name: "custom_model_data",
            value: ComponentValue::CustomModelData(CustomModelData {
                floats: vec![1.0; 1000],
                ..CustomModelData::default()
            }),
        }],
        vec![],
    );
    let limits = Limits {
        max_packet: 16,
        ..Limits::default()
    };
    assert!(matches!(item.encode(v(770), limits), Err(Error::Limit(_))));
    let item = modern(
        1,
        vec![Component {
            name: "item_model",
            value: ComponentValue::String("x".repeat(1000)),
        }],
        vec![],
    );
    assert!(matches!(item.encode(v(770), limits), Err(Error::Limit(_))));
}
#[test]
fn random_small_inputs_do_not_panic_or_bypass_budgets() {
    let limits = Limits {
        max_packet: 128,
        max_collection: 64,
        max_nbt_nodes: 64,
        max_nbt_depth: 8,
        max_string_chars: 32,
        ..Limits::default()
    };
    let mut state = 0x9e3779b9u32;
    for version in Version::ALL {
        for len in 0..128 {
            let mut bytes = Vec::new();
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                bytes.push(state as u8);
            }
            let _ = Slot::decode(&bytes, *version, limits);
            let _ = ContainerContent::decode(&bytes, *version, limits);
            let _ = ContainerClick::decode(&bytes, *version, limits);
            let _ = HashedContainerClick::decode(&bytes, *version, limits);
        }
    }
}

#[test]
fn signed_cursor_container_ids_survive_component_slot_releases() {
    // Official 1.20.6 and 1.21.1 constructors still call readByte, not
    // readUnsignedByte, despite the upstream ContainerID alias.
    for p in 763..=767 {
        for id in [-1, -2] {
            let packet = SetContainerSlot {
                window_id: id,
                state_id: 3,
                slot: -1,
                item: Slot::Empty,
            };
            let bytes = [id as u8, 3, 0xff, 0xff, 0];
            assert_eq!(packet.encode(v(p), Limits::default()).unwrap(), bytes);
            assert_eq!(
                SetContainerSlot::decode(&bytes, v(p), Limits::default()).unwrap(),
                packet
            );
        }
    }
    let old_cursor = SetContainerSlot {
        window_id: -1,
        state_id: 0,
        slot: -1,
        item: Slot::Empty,
    };
    assert!(old_cursor.encode(v(768), Limits::default()).is_err());
}
