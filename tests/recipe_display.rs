use rustwire_mc::{
    frame::RawPacket,
    nbt::{Nbt, Tag},
    packet::{
        entity_metadata::holders::RegistryHolder,
        inventory::{
            Component, ComponentPatch, ComponentValue, ItemData, ItemStack, Slot, TrimPattern,
        },
        recipe_display::*,
        typed::DecodedPacket,
    },
    version::{Direction, State},
    Error, Limits, Version,
};
#[path = "support/recipe_display_fixtures.rs"]
mod support;
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn encode_fixture(
    f: &support::Fixture,
    bytes: &[u8],
    limits: Limits,
) -> rustwire_mc::Result<Vec<u8>> {
    match f.name {
        "SlotDisplay" => SlotDisplay::decode(bytes, f.version, limits)?.encode(f.version, limits),
        "RecipeDisplay" => {
            RecipeDisplay::decode(bytes, f.version, limits)?.encode(f.version, limits)
        }
        _ => {
            RecipeDisplayPacket::decode(f.name, bytes, f.version, limits)?.encode(f.version, limits)
        }
    }
}
#[test]
fn independent_fixtures_every_prefix_trailing_and_exact_byte_limits() {
    let fixtures = support::fixtures();
    assert_eq!(fixtures.len(), 256);
    for f in fixtures {
        let context = format!("{} {} {}", f.version.protocol(), f.name, f.case);
        assert_eq!(
            encode_fixture(&f, &f.bytes, Limits::default())
                .unwrap_or_else(|e| panic!("{context}: {e:?}")),
            f.bytes,
            "{context}"
        );
        for end in 0..f.bytes.len() {
            assert!(
                matches!(
                    encode_fixture(&f, &f.bytes[..end], Limits::default()),
                    Err(Error::Eof | Error::Invalid(_))
                ),
                "{context} prefix {end}"
            );
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(
            matches!(
                encode_fixture(&f, &trailing, Limits::default()),
                Err(Error::Invalid("trailing bytes"))
            ),
            "{context}"
        );
        let exact = Limits {
            max_packet: f.bytes.len(),
            ..Limits::default()
        };
        assert_eq!(encode_fixture(&f, &f.bytes, exact).unwrap(), f.bytes);
        let small = Limits {
            max_packet: f.bytes.len() - 1,
            ..exact
        };
        assert!(matches!(
            encode_fixture(&f, &f.bytes, small),
            Err(Error::Limit(_))
        ));
        let encoded = match f.name {
            "SlotDisplay" => SlotDisplay::decode(&f.bytes, f.version, exact)
                .unwrap()
                .encode(f.version, small),
            "RecipeDisplay" => RecipeDisplay::decode(&f.bytes, f.version, exact)
                .unwrap()
                .encode(f.version, small),
            _ => RecipeDisplayPacket::decode(f.name, &f.bytes, f.version, exact)
                .unwrap()
                .encode(f.version, small),
        };
        assert!(matches!(encoded, Err(Error::Limit(_))), "{context}");
        if f.id >= 0 {
            assert_eq!(
                f.version
                    .packet_id(State::Play, Direction::Clientbound, f.name)
                    .unwrap(),
                f.id
            );
            let Some(DecodedPacket::RecipeDisplay(packet)) =
                DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, exact).unwrap()
            else {
                panic!("wrong family")
            };
            assert_eq!(
                packet.packet(f.version, exact).unwrap(),
                RawPacket::new(f.id, f.bytes.clone())
            );
            for state in [
                State::Handshake,
                State::Status,
                State::Login,
                State::Configuration,
            ] {
                assert!(
                    DecodedPacket::decode(state, f.name, &f.bytes, f.version, exact)
                        .unwrap()
                        .is_none()
                );
            }
        }
    }
}
fn stack() -> ItemStack {
    ItemStack {
        item_id: 300,
        count: 2,
        data: ItemData::Components(ComponentPatch::default()),
    }
}
fn minimal() -> RecipeDisplay {
    RecipeDisplay::Stonecutter {
        ingredient: SlotDisplay::Empty,
        result: SlotDisplay::Empty,
        station: SlotDisplay::Empty,
    }
}
#[test]
fn versioned_stack_and_trim_domains_are_explicit() {
    let limits = Limits::default();
    for &version in Version::ALL {
        if version.protocol() < 768 {
            assert!(matches!(
                minimal().encode(version, limits),
                Err(Error::Unsupported(_))
            ));
            assert!(RecipeBookAdd {
                entries: vec![],
                replace: false
            }
            .encode(version, limits)
            .is_err());
            assert!(CraftRecipeResponse {
                window_id: 0,
                display: minimal()
            }
            .encode(version, limits)
            .is_err());
            continue;
        }
        let old = SlotDisplay::ItemStack(Slot::Item(stack()));
        let new = SlotDisplay::ItemTemplate(stack());
        assert_eq!(
            old.encode(version, limits).is_ok(),
            version.protocol() < 775
        );
        assert_eq!(
            new.encode(version, limits).is_ok(),
            version.protocol() >= 775
        );
        let trim = |pattern| SlotDisplay::SmithingTrim {
            base: Box::new(SlotDisplay::Empty),
            material: Box::new(SlotDisplay::Empty),
            pattern,
        };
        assert_eq!(
            trim(DisplayTrimPattern::Display(Box::new(SlotDisplay::Empty)))
                .encode(version, limits)
                .is_ok(),
            version.protocol() < 770
        );
        assert_eq!(
            trim(DisplayTrimPattern::Holder(RegistryHolder::RegistryId(300)))
                .encode(version, limits)
                .is_ok(),
            version.protocol() >= 770
        );
        assert_eq!(
            SlotDisplay::WithAnyPotion(Box::new(SlotDisplay::Empty))
                .encode(version, limits)
                .is_ok(),
            version.protocol() >= 775
        );
    }
    // Exact 775 and 776 bytes are independently source-verified template order.
    for version in [v(775), v(776)] {
        assert_eq!(
            SlotDisplay::ItemTemplate(stack())
                .encode(version, limits)
                .unwrap(),
            [5, 172, 2, 2, 0, 0]
        );
    }
}
#[test]
fn aggregate_arrays_display_nodes_items_and_requirements_are_bounded_together() {
    let p = RecipeBookAdd {
        entries: vec![RecipeBookEntry {
            display_id: 1,
            display: minimal(),
            group: None,
            category: RecipeBookCategory::Campfire,
            crafting_requirements: Some(vec![DisplayIngredient::Ids(vec![1, 2])]),
            flags: 255,
        }],
        replace: false,
    };
    // Entry + recipe + three slots + requirement + two IDs = eight charges.
    let exact = Limits {
        max_collection: 8,
        ..Limits::default()
    };
    let bytes = p.encode(v(776), exact).unwrap();
    assert_eq!(RecipeBookAdd::decode(&bytes, v(776), exact).unwrap(), p);
    let small = Limits {
        max_collection: 7,
        ..exact
    };
    assert!(matches!(p.encode(v(776), small), Err(Error::Limit(_))));
    assert!(matches!(
        RecipeBookAdd::decode(&bytes, v(776), small),
        Err(Error::Limit(_))
    ));
    let many = SlotDisplay::Composite(vec![SlotDisplay::Composite(vec![SlotDisplay::Empty]); 4]);
    let limits = Limits {
        max_collection: 12,
        ..Limits::default()
    };
    assert!(matches!(many.encode(v(775), limits), Err(Error::Limit(_))));
    // Counts that cannot fit are rejected before capacity allocation.
    for bytes in [
        vec![10, 255, 255, 255, 255, 7],
        vec![10, 255, 255, 255, 255, 15],
    ] {
        assert!(SlotDisplay::decode(
            &bytes,
            v(776),
            Limits {
                max_collection: usize::MAX,
                ..Limits::default()
            }
        )
        .is_err());
    }
}
fn name_slot() -> SlotDisplay {
    let mut item = stack();
    item.data = ItemData::Components(ComponentPatch {
        added: vec![Component {
            name: "custom_name",
            value: ComponentValue::Nbt(Nbt {
                name: None,
                root: Tag::String("x".into()),
            }),
        }],
        removed: vec![],
    });
    SlotDisplay::ItemTemplate(item)
}
#[test]
fn nbt_node_and_depth_budgets_are_shared_across_leaf_kinds() {
    let pattern = TrimPattern {
        asset_id: "rustwire:trim".into(),
        template_item_id: None,
        description: Nbt {
            name: None,
            root: Tag::String("Trim".into()),
        },
        decal: false,
    };
    let display = SlotDisplay::Composite(vec![
        name_slot(),
        SlotDisplay::SmithingTrim {
            base: Box::new(SlotDisplay::Empty),
            material: Box::new(SlotDisplay::Empty),
            pattern: DisplayTrimPattern::Holder(RegistryHolder::Inline(pattern)),
        },
    ]);
    let bytes = display.encode(v(776), Limits::default()).unwrap();
    let small = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(
        display.encode(v(776), small),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        SlotDisplay::decode(&bytes, v(776), small),
        Err(Error::Limit(_))
    ));
    let mut nested = SlotDisplay::Empty;
    for _ in 0..64 {
        nested = SlotDisplay::WithAnyPotion(Box::new(nested));
    }
    let bytes = nested.encode(v(776), Limits::default()).unwrap();
    assert!(SlotDisplay::decode(&bytes, v(776), Limits::default()).is_ok());
    let too_deep = SlotDisplay::WithAnyPotion(Box::new(nested));
    let permissive = Limits {
        max_nbt_depth: usize::MAX,
        ..Limits::default()
    };
    assert!(matches!(
        too_deep.encode(v(776), permissive),
        Err(Error::Limit(_))
    ));
    let mut bytes = vec![2];
    bytes.extend_from_slice(&too_deep_bytes());
    assert!(matches!(
        SlotDisplay::decode(&bytes, v(776), permissive),
        Err(Error::Limit(_))
    ));
    // Display depth is deducted from the nested item's/NBT's allowance.
    let nested = SlotDisplay::WithAnyPotion(Box::new(name_slot()));
    let tight = Limits {
        max_nbt_depth: 1,
        ..Limits::default()
    };
    assert!(matches!(nested.encode(v(776), tight), Err(Error::Limit(_))));
}
fn too_deep_bytes() -> Vec<u8> {
    let mut b = vec![2; 64];
    b.push(0);
    b
}
#[test]
fn malformed_known_layouts_unknown_unframed_kinds_and_scalar_preservation() {
    let l = Limits::default();
    assert!(matches!(
        SlotDisplay::decode(&[255, 255, 255, 255, 15], v(776), l),
        Err(Error::Unsupported("slot display kind"))
    ));
    assert!(matches!(
        RecipeDisplay::decode(&[5], v(776), l),
        Err(Error::Unsupported("recipe display kind"))
    ));
    for bytes in [
        vec![6, 1, b'A'],
        vec![5, 1, 0, 1, 0, 255, 255, 255, 255, 15],
        vec![4, 255, 255, 255, 255, 15],
    ] {
        assert!(SlotDisplay::decode(&bytes, v(776), l).is_err());
    }
    assert!(matches!(
        RecipeBookAdd::decode(&[0, 2], v(776), l),
        Err(Error::Invalid("boolean"))
    ));
    let mut p = RecipeBookAdd {
        entries: vec![RecipeBookEntry {
            display_id: -1,
            display: minimal(),
            group: Some(i32::MAX),
            category: RecipeBookCategory::Unknown(-1),
            crafting_requirements: None,
            flags: 255,
        }],
        replace: true,
    };
    let bytes = p.encode(v(776), l).unwrap();
    assert_eq!(RecipeBookAdd::decode(&bytes, v(776), l).unwrap(), p);
    p.entries[0].group = Some(-1);
    assert!(matches!(p.encode(v(776), l), Err(Error::Invalid(_))));
    let display = SlotDisplay::OnlyWithComponent {
        source: Box::new(SlotDisplay::Empty),
        component_id: i32::MAX,
    };
    assert_eq!(
        SlotDisplay::decode(&display.encode(v(776), l).unwrap(), v(776), l).unwrap(),
        display
    );
}

#[test]
fn nested_item_patches_charge_the_enclosing_display_collection() {
    let value = SlotDisplay::Composite(vec![name_slot(), name_slot()]);
    let exact = Limits {
        max_collection: 9,
        ..Limits::default()
    };
    let bytes = value.encode(v(776), exact).unwrap();
    assert_eq!(SlotDisplay::decode(&bytes, v(776), exact).unwrap(), value);
    let small = Limits {
        max_collection: 8,
        ..exact
    };
    assert!(matches!(value.encode(v(776), small), Err(Error::Limit(_))));
    assert!(matches!(
        SlotDisplay::decode(&bytes, v(776), small),
        Err(Error::Limit(_))
    ));
}
#[test]
fn furnace_float_bits_and_signed_dimensions_are_wire_values() {
    for bits in [
        0, 0x80000000, 0x7f800000, 0xff800000, 0x7fc01234, 0x7fa01234,
    ] {
        let value = RecipeDisplay::Furnace {
            ingredient: SlotDisplay::Empty,
            fuel: SlotDisplay::Empty,
            result: SlotDisplay::Empty,
            station: SlotDisplay::Empty,
            duration: -1,
            experience: f32::from_bits(bits),
        };
        let bytes = value.encode(v(776), Limits::default()).unwrap();
        let decoded = RecipeDisplay::decode(&bytes, v(776), Limits::default()).unwrap();
        let RecipeDisplay::Furnace {
            experience,
            duration,
            ..
        } = decoded
        else {
            panic!("wrong variant")
        };
        assert_eq!(duration, -1);
        assert_eq!(experience.to_bits(), bits);
        assert_eq!(decoded.encode(v(776), Limits::default()).unwrap(), bytes);
    }
    let value = RecipeDisplay::Shaped {
        width: i32::MIN,
        height: -1,
        ingredients: vec![],
        result: SlotDisplay::Empty,
        station: SlotDisplay::Empty,
    };
    let bytes = value.encode(v(768), Limits::default()).unwrap();
    assert_eq!(
        RecipeDisplay::decode(&bytes, v(768), Limits::default()).unwrap(),
        value
    );
}
