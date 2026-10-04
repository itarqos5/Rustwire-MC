use rustwire_mc::{
    codec::Writer,
    frame::RawPacket,
    nbt::{Nbt, Tag},
    packet::{
        inventory::{Component, ComponentPatch, ComponentValue, ItemData, ItemStack},
        recipe_display::{DisplayIngredient, SlotDisplay},
        recipe_properties::*,
        typed::DecodedPacket,
    },
    version::{Direction, State},
    Error, Limits, Version,
};
#[path = "support/recipe_property_fixtures.rs"]
mod support;
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
#[test]
fn independent_fixtures_prefixes_trailing_exact_limits_and_dispatch() {
    let fixtures = support::fixtures();
    assert_eq!(fixtures.len(), 345);
    for f in fixtures {
        let context = format!("{} {}", f.version.protocol(), f.case);
        let limits = Limits::default();
        assert_eq!(
            f.version
                .packet_id(State::Play, Direction::Clientbound, "declare_recipes")
                .unwrap(),
            f.packet_id
        );
        let value = ModernDeclareRecipes::decode(&f.bytes, f.version, limits)
            .unwrap_or_else(|e| panic!("{context}: {e:?}"));
        assert_eq!(
            value.packet(f.version, limits).unwrap(),
            RawPacket::new(f.packet_id, f.bytes.clone()),
            "{context}"
        );
        for end in 0..f.bytes.len() {
            assert!(
                matches!(
                    ModernDeclareRecipes::decode(&f.bytes[..end], f.version, limits),
                    Err(Error::Eof | Error::Invalid(_))
                ),
                "{context} prefix {end}"
            );
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(matches!(
            ModernDeclareRecipes::decode(&trailing, f.version, limits),
            Err(Error::Invalid("trailing bytes"))
        ));
        let exact = Limits {
            max_packet: f.bytes.len(),
            ..limits
        };
        assert_eq!(value.encode(f.version, exact).unwrap(), f.bytes);
        assert_eq!(
            ModernDeclareRecipes::decode(&f.bytes, f.version, exact).unwrap(),
            value
        );
        let small = Limits {
            max_packet: f.bytes.len() - 1,
            ..limits
        };
        assert!(matches!(
            value.encode(f.version, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            ModernDeclareRecipes::decode(&f.bytes, f.version, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            DecodedPacket::decode(State::Play, "declare_recipes", &f.bytes, f.version, limits)
                .unwrap(),
            Some(DecodedPacket::ModernRecipes(_))
        ));
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(
                DecodedPacket::decode(state, "declare_recipes", &f.bytes, f.version, limits)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
fn empty() -> ModernDeclareRecipes {
    ModernDeclareRecipes {
        property_sets: vec![],
        stonecutter_recipes: vec![],
    }
}
fn stone(result: SlotDisplay) -> StonecutterRecipe {
    StonecutterRecipe {
        input: DisplayIngredient::Ids(vec![]),
        result,
    }
}
fn named_stack() -> SlotDisplay {
    SlotDisplay::ItemTemplate(ItemStack {
        item_id: 300,
        count: 0,
        data: ItemData::Components(ComponentPatch {
            added: vec![Component {
                name: "custom_name",
                value: ComponentValue::Nbt(Nbt {
                    name: None,
                    root: Tag::String("A".into()),
                }),
            }],
            removed: vec![],
        }),
    })
}
#[test]
fn exact_version_boundary_and_empty_zero_collection() {
    let limits = Limits {
        max_packet: 2,
        max_collection: 0,
        max_nbt_nodes: 0,
        max_nbt_depth: 0,
        ..Limits::default()
    };
    for &version in Version::ALL {
        if version.protocol() < 768 {
            assert!(matches!(
                empty().encode(version, limits),
                Err(Error::Unsupported("modern recipe properties version"))
            ));
            assert!(matches!(
                ModernDeclareRecipes::decode(&[0, 0], version, limits),
                Err(Error::Unsupported(_))
            ));
        } else {
            assert_eq!(empty().encode(version, limits).unwrap(), [0, 0]);
            assert_eq!(
                ModernDeclareRecipes::decode(&[0, 0], version, limits).unwrap(),
                empty()
            );
        }
    }
}
#[test]
fn all_collections_and_display_nodes_share_one_packet_budget() {
    let value = ModernDeclareRecipes {
        property_sets: vec![RecipePropertySet {
            name: "a".into(),
            item_ids: vec![1, 2],
        }],
        stonecutter_recipes: vec![StonecutterRecipe {
            input: DisplayIngredient::Ids(vec![1, 2]),
            result: SlotDisplay::Composite(vec![SlotDisplay::Item(1), SlotDisplay::Item(2)]),
        }],
    };
    // 1 set + 2 IDs + 1 stonecutter + 2 input IDs + 1 composite +
    // 2 composite entries + 2 child display nodes.
    let exact = Limits {
        max_collection: 11,
        ..Limits::default()
    };
    let bytes = value.encode(v(776), exact).unwrap();
    assert_eq!(
        ModernDeclareRecipes::decode(&bytes, v(776), exact).unwrap(),
        value
    );
    let small = Limits {
        max_collection: 10,
        ..exact
    };
    assert!(matches!(value.encode(v(776), small), Err(Error::Limit(_))));
    assert!(matches!(
        ModernDeclareRecipes::decode(&bytes, v(776), small),
        Err(Error::Limit(_))
    ));
    let mut value = empty();
    value.stonecutter_recipes = vec![stone(named_stack()), stone(named_stack())];
    // Two entry charges + two display/item/component triples = eight.
    let exact = Limits {
        max_collection: 8,
        max_nbt_nodes: 2,
        ..Limits::default()
    };
    let bytes = value.encode(v(776), exact).unwrap();
    assert_eq!(
        ModernDeclareRecipes::decode(&bytes, v(776), exact).unwrap(),
        value
    );
    for small in [
        Limits {
            max_collection: 7,
            ..exact
        },
        Limits {
            max_nbt_nodes: 1,
            ..exact
        },
    ] {
        assert!(matches!(value.encode(v(776), small), Err(Error::Limit(_))));
        assert!(matches!(
            ModernDeclareRecipes::decode(&bytes, v(776), small),
            Err(Error::Limit(_))
        ));
    }
}
#[test]
fn recursive_depth_and_aggregate_nodes_do_not_reset_per_entry() {
    let mut tree = SlotDisplay::Empty;
    for _ in 0..64 {
        tree = SlotDisplay::Composite(vec![tree]);
    }
    let value = ModernDeclareRecipes {
        property_sets: vec![],
        stonecutter_recipes: vec![stone(tree.clone()), stone(tree)],
    };
    // Each chain: 64 composite nodes + 64 elements + 1 empty node.
    let exact = Limits {
        max_collection: 260,
        max_nbt_depth: 64,
        ..Limits::default()
    };
    let bytes = value.encode(v(768), exact).unwrap();
    assert_eq!(
        ModernDeclareRecipes::decode(&bytes, v(768), exact).unwrap(),
        value
    );
    for small in [
        Limits {
            max_collection: 259,
            ..exact
        },
        Limits {
            max_nbt_depth: 63,
            ..exact
        },
    ] {
        assert!(matches!(value.encode(v(768), small), Err(Error::Limit(_))));
        assert!(matches!(
            ModernDeclareRecipes::decode(&bytes, v(768), small),
            Err(Error::Limit(_))
        ));
    }
    let mut deeper = value.clone();
    deeper.stonecutter_recipes[0].result =
        SlotDisplay::Composite(vec![deeper.stonecutter_recipes[0].result.clone()]);
    assert!(matches!(
        deeper.encode(
            v(768),
            Limits {
                max_nbt_depth: usize::MAX,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
    let mut raw = vec![0, 1, 1];
    for _ in 0..65 {
        raw.extend_from_slice(&[7, 1]);
    }
    raw.push(0);
    assert!(matches!(
        ModernDeclareRecipes::decode(
            &raw,
            v(768),
            Limits {
                max_nbt_depth: usize::MAX,
                ..Limits::default()
            }
        ),
        Err(Error::Limit(_))
    ));
}
#[test]
fn identifiers_counts_scalar_domains_and_unknown_unframed_payloads() {
    let version = v(776);
    let limits = Limits::default();
    let mut value = empty();
    value.property_sets.push(RecipePropertySet {
        name: "a".into(),
        item_ids: vec![0, 2147483647, 0],
    });
    let bytes = value.encode(version, limits).unwrap();
    assert_eq!(
        ModernDeclareRecipes::decode(&bytes, version, limits).unwrap(),
        value
    );
    value.property_sets[0].item_ids.push(-1);
    assert!(matches!(
        value.encode(version, limits),
        Err(Error::Invalid(_))
    ));
    for name in ["Upper:case", "bad space", "a:\u{00e9}"] {
        value.property_sets[0] = RecipePropertySet {
            name: name.into(),
            item_ids: vec![],
        };
        assert!(value.encode(version, limits).is_err());
    }
    value.property_sets[0] = RecipePropertySet {
        name: "abc".into(),
        item_ids: vec![],
    };
    let bytes = value.encode(version, limits).unwrap();
    let small = Limits {
        max_string_chars: 2,
        ..limits
    };
    assert!(matches!(value.encode(version, small), Err(Error::Limit(_))));
    assert!(matches!(
        ModernDeclareRecipes::decode(&bytes, version, small),
        Err(Error::Limit(_))
    ));
    for head in [vec![], vec![1, 1, b'a'], vec![0], vec![0, 1]] {
        let mut w = Writer::new();
        w.raw(&head);
        w.var_i32(-1);
        w.raw(&[0; 8]);
        assert!(matches!(
            ModernDeclareRecipes::decode(w.as_slice(), version, limits),
            Err(Error::Invalid(_))
        ));
    }
    for bytes in [
        vec![1, 1, 255, 0, 0],
        vec![0, 1, 1, 127, 99],
        vec![0, 1, 1, 5, 1, 1, 1, 0, 255, 255, 255, 255, 7, 99],
    ] {
        assert!(ModernDeclareRecipes::decode(&bytes, version, limits).is_err());
    }
    // Huge counts with tiny remaining input cannot trigger eager reservations.
    for head in [vec![], vec![1, 1, b'a'], vec![0], vec![0, 1]] {
        let mut w = Writer::new();
        w.raw(&head);
        w.var_i32(i32::MAX);
        assert!(ModernDeclareRecipes::decode(
            w.as_slice(),
            version,
            Limits {
                max_collection: usize::MAX,
                ..limits
            }
        )
        .is_err());
    }
}

#[test]
fn namespace_boundary_and_nested_text_depth_use_selected_version_limits() {
    let mut value = empty();
    value.property_sets.push(RecipePropertySet {
        name: "..:x".into(),
        item_ids: vec![],
    });
    let old = value.encode(v(774), Limits::default()).unwrap();
    assert_eq!(
        ModernDeclareRecipes::decode(&old, v(774), Limits::default()).unwrap(),
        value
    );
    for version in [v(775), v(776)] {
        assert!(matches!(
            value.encode(version, Limits::default()),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            ModernDeclareRecipes::decode(&old, version, Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
    let item = SlotDisplay::ItemTemplate(ItemStack {
        item_id: 1,
        count: 1,
        data: ItemData::Components(ComponentPatch {
            added: vec![Component {
                name: "custom_name",
                value: ComponentValue::Nbt(Nbt {
                    name: None,
                    root: Tag::Compound(vec![(
                        "a".into(),
                        Tag::Compound(vec![("b".into(), Tag::String("c".into()))]),
                    )]),
                }),
            }],
            removed: vec![],
        }),
    });
    let value = ModernDeclareRecipes {
        property_sets: vec![],
        stonecutter_recipes: vec![stone(SlotDisplay::Composite(vec![item]))],
    };
    let bytes = value.encode(v(776), Limits::default()).unwrap();
    let limited = Limits {
        max_nbt_depth: 3,
        ..Limits::default()
    };
    assert!(matches!(
        value.encode(v(776), limited),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        ModernDeclareRecipes::decode(&bytes, v(776), limited),
        Err(Error::Limit(_))
    ));
}
