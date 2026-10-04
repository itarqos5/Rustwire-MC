use rustwire_mc::{
    codec::{Reader, Writer},
    frame::RawPacket,
    packet::{
        inventory::{Component, ComponentPatch, ComponentValue, ItemData, ItemStack, Slot},
        recipe_declarations::*,
    },
    version::{Direction, State},
    Error, Limits, Version,
};
#[path = "support/legacy_recipe_fixtures.rs"]
mod support;
fn version(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn fixture(p: i32, case: &str) -> support::Fixture {
    support::fixtures()
        .into_iter()
        .find(|f| f.version.protocol() == p && f.case == case)
        .unwrap()
}
fn parsed(p: i32, case: &str) -> LegacyDeclareRecipes {
    LegacyDeclareRecipes::decode(&fixture(p, case).bytes, version(p), Limits::default()).unwrap()
}
fn body_offset(f: &support::Fixture) -> usize {
    let mut r = Reader::new(&f.bytes, Limits::default());
    assert_eq!(r.var_i32().unwrap(), 1);
    r.string(32767).unwrap();
    if f.version.protocol() < 766 {
        r.string(32767).unwrap();
    } else {
        r.var_i32().unwrap();
    }
    r.position()
}
fn rejects(p: i32, bytes: &[u8]) {
    assert!(
        matches!(
            LegacyDeclareRecipes::decode(bytes, version(p), Limits::default()),
            Err(Error::Invalid(_) | Error::Eof | Error::Limit(_))
        ),
        "{p}: {bytes:02x?}"
    );
}
#[test]
fn independent_every_kind_boundary_prefix_trailing_and_exact_byte_budget() {
    let mut accepted = 0;
    let mut prefixes = 0;
    for f in support::fixtures() {
        assert_eq!(f.direction, "toClient");
        assert_eq!(f.name, "declare_recipes");
        assert_eq!(
            f.version
                .packet_id(State::Play, Direction::Clientbound, f.name)
                .unwrap(),
            f.id
        );
        assert!(f
            .version
            .packet_id(State::Play, Direction::Serverbound, f.name)
            .is_err());
        if f.case.starts_with("unsupported-") {
            assert!(matches!(
                LegacyDeclareRecipes::decode(&f.bytes, f.version, Limits::default()),
                Err(Error::Unsupported("legacy recipe serializer"))
            ));
            continue; // Custom payload has no framing: completeness cannot be known.
        }
        accepted += 1;
        let exact = Limits {
            max_packet: f.bytes.len(),
            ..Limits::default()
        };
        let decoded = LegacyDeclareRecipes::decode(&f.bytes, f.version, exact).unwrap();
        assert_eq!(
            decoded.packet(f.version, exact).unwrap(),
            RawPacket::new(f.id, f.bytes.clone()),
            "{} {}",
            f.version.protocol(),
            f.case
        );
        let small = Limits {
            max_packet: f.bytes.len() - 1,
            ..exact
        };
        assert!(matches!(
            decoded.encode(f.version, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            LegacyDeclareRecipes::decode(&f.bytes, f.version, small),
            Err(Error::Limit(_))
        ));
        for end in 0..f.bytes.len() {
            prefixes += 1;
            assert!(
                matches!(
                    LegacyDeclareRecipes::decode(&f.bytes[..end], f.version, exact),
                    Err(Error::Eof | Error::Invalid(_))
                ),
                "{} {} prefix {end}",
                f.version.protocol(),
                f.case
            );
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(matches!(
            LegacyDeclareRecipes::decode(&trailing, f.version, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
    }
    assert_eq!(accepted, 131);
    println!("{accepted} accepted independent bodies, {prefixes} strict prefixes");
}
#[test]
fn independent_values_and_corrected_23_kind_registry() {
    for p in 763..=767 {
        let packet = parsed(p, "all-kinds");
        assert_eq!(packet.recipes.len(), 23);
        for (i, (recipe, expected)) in packet.recipes.iter().zip(LegacyRecipeKind::ALL).enumerate()
        {
            assert_eq!(recipe.serializer.kind(version(p)).unwrap(), *expected);
            assert_eq!(recipe.serializer, expected.serializer(version(p)).unwrap());
            assert_eq!(recipe.key, format!("rustwire:recipe/{i}"));
        }
        let LegacyRecipeData::Shaped {
            width,
            height,
            group,
            category,
            ingredients,
            result,
            show_notification,
        } = &packet.recipes[0].data
        else {
            panic!()
        };
        assert_eq!(
            (
                *width,
                *height,
                group.as_str(),
                *category,
                *show_notification
            ),
            (2, 1, "group/雪", 3, true)
        );
        assert!(ingredients[0].alternatives.is_empty());
        assert_eq!(ingredients[1].alternatives.len(), 3);
        assert_eq!(ingredients[1].alternatives[1], Slot::Empty);
        assert_eq!(
            ingredients[1].alternatives[0],
            ingredients[1].alternatives[2]
        );
        let Slot::Item(item) = result else { panic!() };
        assert_eq!(
            (item.item_id, item.count),
            (300, if p <= 765 { 127 } else { 300 })
        );
        match &item.data {
            ItemData::Legacy(Some(nbt)) => {
                assert_eq!(nbt.root.get("v").unwrap(), &rustwire_mc::nbt::Tag::Byte(7));
                assert_eq!(
                    nbt.name.as_ref().map(|s| s.to_string().unwrap()),
                    if p == 763 { Some("r".into()) } else { None }
                );
            }
            ItemData::Components(patch) => {
                assert_eq!(patch.added.len(), 2);
                assert_eq!(patch.added[0].name, "custom_data");
                assert_eq!(patch.added[1].value, ComponentValue::VarInt(5));
                assert_eq!(patch.removed, ["repair_cost"]);
            }
            _ => panic!(),
        }
        let LegacyRecipeData::Cooking {
            experience,
            cook_time,
            category,
            ..
        } = packet.recipes[15].data
        else {
            panic!()
        };
        assert_eq!((experience, cook_time, category), (0.625, 200, 0));
        assert!(matches!(
            packet.recipes[21].data,
            LegacyRecipeData::SmithingTrim { .. }
        ));
        assert_eq!(
            packet.recipes[22].data,
            LegacyRecipeData::Special { category: 2 }
        );
    }
    assert_eq!(LegacyRecipeKind::ShieldDecoration as i32, 11);
    assert_eq!(LegacyRecipeKind::Smelting as i32, 15);
    assert_eq!(LegacyRecipeKind::DecoratedPot as i32, 22);
    for p in 766..=767 {
        assert!(matches!(
            LegacyRecipeSerializer::Id(23).kind(version(p)),
            Err(Error::Unsupported(_))
        ));
    }
}
#[test]
fn malformed_known_fields_counts_identifiers_and_scalar_boundaries() {
    for p in 763..=767 {
        // Negative, overflow, huge count, and truncated count fail before allocation.
        for bytes in [
            &[255, 255, 255, 255, 15][..],
            &[128][..],
            &[255, 255, 255, 255, 127][..],
            &[255, 255, 255, 255, 7][..],
            &[20, 0, 0][..],
        ] {
            rejects(p, bytes);
        }
        let special = fixture(p, "kind-2");
        for cat in [4, 255] {
            let mut bytes = special.bytes.clone();
            *bytes.last_mut().unwrap() = cat;
            rejects(p, &bytes);
        }
        let shaped = fixture(p, "kind-0");
        let mut bytes = shaped.bytes.clone();
        *bytes.last_mut().unwrap() = 2;
        rejects(p, &bytes);
        // Rebuild minimal valid header; malformed widths or ingredient counts.
        let head = &shaped.bytes[..body_offset(&shaped)];
        for (width, height) in [(-1, 2), (i32::MAX, i32::MAX)] {
            let mut w = Writer::new();
            w.raw(head);
            if p >= 765 {
                w.string("", 32767).unwrap();
                w.var_i32(0);
            }
            w.var_i32(width);
            w.var_i32(height);
            if p < 765 {
                w.string("", 32767).unwrap();
                w.var_i32(0);
            }
            w.raw(&[0; 8]);
            rejects(p, &w.into_inner());
        }
        // Known stonecutting with impossible ingredient count.
        let stone = fixture(p, "kind-19");
        let mut w = Writer::new();
        w.raw(&stone.bytes[..body_offset(&stone)]);
        w.string("", 32767).unwrap();
        w.var_i32(i32::MAX);
        w.u8(0);
        rejects(p, &w.into_inner());
        // Invalid UTF-8 and resource identifiers never become raw fallback.
        let mut bad = special.bytes.clone();
        bad[2] = 0xff;
        rejects(p, &bad);
        let mut object = parsed(p, "kind-2");
        object.recipes[0].key = "Bad Key".into();
        assert!(matches!(
            object.encode(version(p), Limits::default()),
            Err(Error::Invalid(_))
        ));
        // Known cooking category is separate from crafting and rejects 3.
        let cooking = fixture(p, "kind-15");
        let mut r = Reader::new(&cooking.bytes, Limits::default());
        r.take(body_offset(&cooking)).unwrap();
        r.string(32767).unwrap();
        let mut bad = cooking.bytes.clone();
        bad[r.position()] = 3;
        rejects(p, &bad);
        // A negative cooking time and negative float are wire scalars, not counts.
        let mut object = parsed(p, "kind-15");
        if let LegacyRecipeData::Cooking {
            experience,
            cook_time,
            ..
        } = &mut object.recipes[0].data
        {
            *experience = -1.25;
            *cook_time = -1;
        }
        let bytes = object.encode(version(p), Limits::default()).unwrap();
        assert_eq!(
            LegacyDeclareRecipes::decode(&bytes, version(p), Limits::default()).unwrap(),
            object
        );
    }
    for p in 766..=767 {
        let f = fixture(p, "kind-2");
        let mut r = Reader::new(&f.bytes, Limits::default());
        r.var_i32().unwrap();
        r.string(32767).unwrap();
        let mut w = Writer::new();
        w.raw(&f.bytes[..r.position()]);
        w.var_i32(-1);
        w.u8(0);
        rejects(p, &w.into_inner());
    }
}
#[test]
fn aggregate_collection_nbt_string_and_output_budgets() {
    for p in 763..=767 {
        for case in [
            "kind-0",
            "kind-1",
            "kind-15",
            "kind-19",
            "kind-20",
            "kind-21",
            "all-kinds",
        ] {
            let f = fixture(p, case);
            let object = parsed(p, case);
            for limits in [
                Limits {
                    max_collection: 2,
                    ..Limits::default()
                },
                Limits {
                    max_nbt_nodes: 2,
                    ..Limits::default()
                },
                Limits {
                    max_nbt_depth: 0,
                    ..Limits::default()
                },
                Limits {
                    max_string_chars: 3,
                    ..Limits::default()
                },
            ] {
                assert!(
                    matches!(
                        LegacyDeclareRecipes::decode(&f.bytes, version(p), limits),
                        Err(Error::Limit(_))
                    ),
                    "{p} {case} {limits:?}"
                );
                assert!(
                    matches!(object.encode(version(p), limits), Err(Error::Limit(_))),
                    "{p} {case} {limits:?}"
                );
            }
        }
        // Each recipe individually fits, but the packet shares one recipe budget.
        let mut pair = parsed(p, "kind-2");
        pair.recipes.push(pair.recipes[0].clone());
        let bytes = pair.encode(version(p), Limits::default()).unwrap();
        let one = Limits {
            max_collection: 1,
            ..Limits::default()
        };
        assert!(matches!(pair.encode(version(p), one), Err(Error::Limit(_))));
        assert!(matches!(
            LegacyDeclareRecipes::decode(&bytes, version(p), one),
            Err(Error::Limit(_))
        ));
    }
}
#[test]
fn nested_item_depth_is_shared_and_capped() {
    let v = version(767);
    let mut slot = Slot::Item(ItemStack {
        item_id: 1,
        count: 1,
        data: ItemData::Components(ComponentPatch::default()),
    });
    for _ in 0..4 {
        slot = Slot::Item(ItemStack {
            item_id: 2,
            count: 1,
            data: ItemData::Components(ComponentPatch {
                added: vec![Component {
                    name: "charged_projectiles",
                    value: ComponentValue::Items(vec![slot]),
                }],
                removed: vec![],
            }),
        });
    }
    let mut recipe = parsed(767, "kind-19");
    if let LegacyRecipeData::Stonecutting { result, .. } = &mut recipe.recipes[0].data {
        *result = slot;
    }
    let bytes = recipe.encode(v, Limits::default()).unwrap();
    let limits = Limits {
        max_nbt_depth: 2,
        ..Limits::default()
    };
    assert!(matches!(recipe.encode(v, limits), Err(Error::Limit(_))));
    assert!(matches!(
        LegacyDeclareRecipes::decode(&bytes, v, limits),
        Err(Error::Limit(_))
    ));
    let generous = Limits {
        max_nbt_depth: usize::MAX,
        ..Limits::default()
    };
    let mut current = Slot::Item(ItemStack {
        item_id: 1,
        count: 1,
        data: ItemData::Components(ComponentPatch::default()),
    });
    for _ in 0..66 {
        current = Slot::Item(ItemStack {
            item_id: 2,
            count: 1,
            data: ItemData::Components(ComponentPatch {
                added: vec![Component {
                    name: "charged_projectiles",
                    value: ComponentValue::Items(vec![current]),
                }],
                removed: vec![],
            }),
        });
    }
    if let LegacyRecipeData::Stonecutting { result, .. } = &mut recipe.recipes[0].data {
        *result = current;
    }
    assert!(matches!(recipe.encode(v, generous), Err(Error::Limit(_))));
    // A manually assembled deep incoming slot must hit the same hard ceiling,
    // even when the caller requests an unbounded depth.
    let mut w = Writer::new();
    w.var_i32(1);
    w.string("rustwire:deep", 32767).unwrap();
    w.var_i32(19);
    w.string("", 32767).unwrap();
    w.var_i32(0);
    let component = rustwire_mc::packet::inventory::component_id(v, "charged_projectiles").unwrap();
    for _ in 0..66 {
        for value in [1, 2, 1, 0, component, 1] {
            w.var_i32(value);
        }
    }
    w.raw(&[1, 1, 0, 0]);
    assert!(matches!(
        LegacyDeclareRecipes::decode(&w.into_inner(), v, generous),
        Err(Error::Limit(_))
    ));
}
#[test]
fn version_representation_data_mismatch_and_modern_exclusion() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 768) {
        assert!(matches!(
            LegacyDeclareRecipes::decode(&[0], v, Limits::default()),
            Err(Error::Unsupported("modern recipe declarations"))
        ));
        assert!(matches!(
            LegacyDeclareRecipes::default().encode(v, Limits::default()),
            Err(Error::Unsupported("modern recipe declarations"))
        ));
    }
    for p in 763..=767 {
        let mut recipe = parsed(p, "kind-0");
        if let LegacyRecipeData::Shaped { ingredients, .. } = &mut recipe.recipes[0].data {
            ingredients.pop();
        }
        assert!(matches!(
            recipe.encode(version(p), Limits::default()),
            Err(Error::Invalid(_))
        ));
        recipe = parsed(p, "kind-0");
        recipe.recipes[0].serializer = LegacyRecipeKind::Shapeless.serializer(version(p)).unwrap();
        assert!(matches!(
            recipe.encode(version(p), Limits::default()),
            Err(Error::Invalid(_))
        ));
        recipe = parsed(p, "kind-2");
        recipe.recipes[0].serializer = if p < 766 {
            LegacyRecipeSerializer::Id(2)
        } else {
            LegacyRecipeSerializer::Name("minecraft:crafting_special_armordye".into())
        };
        assert!(matches!(
            recipe.encode(version(p), Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
}

#[test]
fn zero_area_and_wide_grids_are_wire_counts_not_menu_simulation() {
    for p in 763..=767 {
        for (width, height) in [(0, 2), (3, 0), (0, 0), (4, 1)] {
            let mut packet = parsed(p, "kind-0");
            if let LegacyRecipeData::Shaped {
                width: w,
                height: h,
                ingredients,
                ..
            } = &mut packet.recipes[0].data
            {
                *w = width;
                *h = height;
                *ingredients = vec![LegacyIngredient::default(); width * height];
            }
            let bytes = packet.encode(version(p), Limits::default()).unwrap();
            assert_eq!(
                LegacyDeclareRecipes::decode(&bytes, version(p), Limits::default()).unwrap(),
                packet
            );
        }
    }
}
#[test]
fn random_short_inputs_and_small_budgets_do_not_panic() {
    let limits = Limits {
        max_collection: 32,
        max_nbt_nodes: 32,
        max_packet: 128,
        max_string_chars: 32,
        max_nbt_depth: 4,
        ..Limits::default()
    };
    let mut state = 0x9e37_79b9u32;
    for p in 763..=767 {
        for len in 0..128 {
            let mut bytes = vec![0; len];
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            let _ = LegacyDeclareRecipes::decode(&bytes, version(p), limits);
        }
    }
}

#[test]
fn obsolete_or_misspelled_serializers_never_borrow_a_known_layout() {
    for p in 763..=765 {
        for name in [
            "minecraft:smithing",
            "minecraft:crafting_special_banneraddpattern",
            "minecraft:crafting_special_decoratedpot",
            "example:crafting_shaped",
        ] {
            let v = version(p);
            assert!(matches!(
                LegacyRecipeSerializer::Name(name.into()).kind(v),
                Err(Error::Unsupported("legacy recipe serializer"))
            ));
            let mut w = Writer::new();
            w.var_i32(1);
            w.string(name, 32767).unwrap();
            w.string("rustwire:recipe", 32767).unwrap();
            w.raw(&[0, 255, 254]);
            assert!(matches!(
                LegacyDeclareRecipes::decode(&w.into_inner(), v, Limits::default()),
                Err(Error::Unsupported("legacy recipe serializer"))
            ));
        }
    }
}
