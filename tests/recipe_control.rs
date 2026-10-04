use rustwire_mc::{
    frame::RawPacket,
    packet::recipe::*,
    version::{Direction, State},
    Error, Limits, Version,
};
#[path = "support/recipe_control_fixtures.rs"]
mod support;
fn decode_encode(
    f: &support::Fixture,
    bytes: &[u8],
    limits: Limits,
) -> rustwire_mc::Result<RawPacket> {
    let v = f.version;
    match f.name {
        "craft_recipe_request" => CraftRecipeRequest::decode(bytes, v, limits)?.packet(v, limits),
        "recipe_book" => RecipeBookChangeSettings::decode(bytes, v, limits)?.packet(v, limits),
        "displayed_recipe" => DisplayedRecipe::decode(bytes, v, limits)?.packet(v, limits),
        _ => RecipeControlPacket::decode(f.name, bytes, v, limits)?.packet(v, limits),
    }
}
#[test]
fn independently_encoded_fixtures_and_every_prefix_across_fourteen_families() {
    for f in support::fixtures() {
        let limits = Limits::default();
        let direction = if f.direction == "toClient" {
            Direction::Clientbound
        } else {
            Direction::Serverbound
        };
        assert_eq!(
            f.version.packet_id(State::Play, direction, f.name).unwrap(),
            f.id
        );
        if f.case.starts_with("unknown-") {
            assert!(matches!(
                decode_encode(&f, &f.bytes, limits),
                Err(Error::Unsupported(_))
            ));
        } else {
            assert_eq!(
                decode_encode(&f, &f.bytes, limits).unwrap(),
                RawPacket::new(f.id, f.bytes.clone()),
                "{} {} {}",
                f.version.protocol(),
                f.name,
                f.case
            );
        }
        for end in 0..f.bytes.len() {
            assert!(
                matches!(
                    decode_encode(&f, &f.bytes[..end], limits),
                    Err(Error::Eof | Error::Invalid(_))
                ),
                "{} {} {} prefix {end}",
                f.version.protocol(),
                f.name,
                f.case
            );
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode_encode(&f, &trailing, limits),
            Err(Error::Invalid("trailing bytes"))
        ));
        let small = Limits {
            max_packet: f.bytes.len() - 1,
            ..limits
        };
        if !f.case.starts_with("unknown-") {
            let encoded = match f.name {
                "craft_recipe_request" => CraftRecipeRequest::decode(&f.bytes, f.version, limits)
                    .unwrap()
                    .encode(f.version, small),
                "recipe_book" => RecipeBookChangeSettings::decode(&f.bytes, f.version, limits)
                    .unwrap()
                    .encode(f.version, small),
                "displayed_recipe" => DisplayedRecipe::decode(&f.bytes, f.version, limits)
                    .unwrap()
                    .encode(f.version, small),
                _ => RecipeControlPacket::decode(f.name, &f.bytes, f.version, limits)
                    .unwrap()
                    .encode(f.version, small),
            };
            assert!(matches!(encoded, Err(Error::Limit(_))));
        }
        assert!(matches!(
            decode_encode(&f, &f.bytes, small),
            Err(Error::Limit(_))
        ));
    }
}
#[test]
fn raw_window_bytes_and_modern_signed_domain_do_not_silently_narrow() {
    for &v in Version::ALL {
        for byte in [0, 127, 128, 255] {
            let mut value = CraftRecipeRequest {
                window_id: RecipeWindowId::LegacyByte(byte),
                recipe: RecipeReference::RegistryKey("a".into()),
                make_all: true,
            };
            if v.protocol() < 768 {
                let bytes = value.encode(v, Limits::default()).unwrap();
                assert_eq!(bytes, [byte, 1, b'a', 1]);
                assert_eq!(
                    CraftRecipeRequest::decode(&bytes, v, Limits::default()).unwrap(),
                    value
                );
                assert_eq!(value.window_id.legacy_signed(), Some(byte as i8));
            } else {
                value.recipe = RecipeReference::DisplayId(0);
                assert!(matches!(
                    value.encode(v, Limits::default()),
                    Err(Error::Invalid("recipe window ID version"))
                ));
            }
        }
        for id in [i32::MIN, -1, 0, 127, 128, 255, 300, i32::MAX] {
            let value = CraftRecipeRequest {
                window_id: RecipeWindowId::VarInt(id),
                recipe: RecipeReference::DisplayId(id),
                make_all: false,
            };
            if v.protocol() >= 768 {
                let bytes = value.encode(v, Limits::default()).unwrap();
                assert_eq!(
                    CraftRecipeRequest::decode(&bytes, v, Limits::default()).unwrap(),
                    value
                );
            } else {
                assert!(matches!(
                    value.encode(v, Limits::default()),
                    Err(Error::Invalid("recipe window ID version"))
                ));
            }
        }
    }
    assert_eq!(
        RecipeWindowId::from_legacy_signed(-1),
        RecipeWindowId::LegacyByte(255)
    );
    assert_eq!(
        RecipeWindowId::from_legacy_signed(-128),
        RecipeWindowId::LegacyByte(128)
    );
}
#[test]
fn recipe_reference_types_and_packet_availability_are_versioned() {
    for &v in Version::ALL {
        let wrong = DisplayedRecipe {
            recipe: if v.protocol() < 768 {
                RecipeReference::DisplayId(0)
            } else {
                RecipeReference::RegistryKey("a".into())
            },
        };
        assert!(matches!(
            wrong.encode(v, Limits::default()),
            Err(Error::Invalid("recipe reference version"))
        ));
        if v.protocol() < 768 {
            assert!(matches!(
                RecipeBookSettings::decode(&[0; 8], v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
            assert!(matches!(
                (RecipeBookRemove {
                    display_ids: vec![]
                })
                .packet(v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
        } else {
            assert!(matches!(
                UnlockRecipes::decode(&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0], v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
            let legacy = LegacyCraftRecipeResponse {
                window_id: 255,
                recipe_key: "a".into(),
            };
            assert!(matches!(
                legacy.encode(v, Limits::default()),
                Err(Error::Unsupported("modern recipe display"))
            ));
            assert!(matches!(
                LegacyCraftRecipeResponse::decode(&[], v, Limits::default()),
                Err(Error::Unsupported("modern recipe display"))
            ));
        }
    }
}
#[test]
fn enum_ordinals_and_init_second_list_have_independent_semantics() {
    let fixtures = support::fixtures();
    let init = fixtures.iter().find(|f| f.case == "init").unwrap();
    let value = UnlockRecipes::decode(&init.bytes, init.version, Limits::default()).unwrap();
    assert_eq!(
        value.recipes,
        ["rustwire:first", "rustwire:second", "rustwire:first"]
    );
    assert_eq!(
        value.action,
        UnlockRecipesAction::Init {
            highlighted: vec!["rustwire:second".into()]
        }
    );
    assert!(value.settings.crafting.open);
    assert!(!value.settings.crafting.filtering);
    assert!(!value.settings.furnace.open);
    assert!(value.settings.furnace.filtering);
    for (id, book) in [
        (0, RecipeBookKind::Crafting),
        (1, RecipeBookKind::Furnace),
        (2, RecipeBookKind::BlastFurnace),
        (3, RecipeBookKind::Smoker),
    ] {
        assert_eq!(RecipeBookKind::from_id(id).unwrap(), book);
    }
}
#[test]
fn aggregate_collection_identifier_and_packet_budgets_apply_both_ways() {
    let legacy = Version::from_protocol(767).unwrap();
    let modern = Version::from_protocol(768).unwrap();
    let init = UnlockRecipes {
        action: UnlockRecipesAction::Init {
            highlighted: vec!["a".into()],
        },
        settings: RecipeBookSettings::default(),
        recipes: vec!["a".into()],
    };
    let bytes = init.encode(legacy, Limits::default()).unwrap();
    let limit = Limits {
        max_collection: 1,
        ..Limits::default()
    };
    assert!(matches!(init.encode(legacy, limit), Err(Error::Limit(_))));
    assert!(matches!(
        UnlockRecipes::decode(&bytes, legacy, limit),
        Err(Error::Limit(_))
    ));
    for malformed in [
        vec![255, 255, 255, 255, 15],
        vec![2, 0],
        vec![255, 255, 255, 255, 127],
    ] {
        assert!(RecipeBookRemove::decode(&malformed, modern, Limits::default()).is_err());
    }
    let ids = RecipeBookRemove {
        display_ids: vec![-1, 300],
    };
    let bytes = ids.encode(modern, Limits::default()).unwrap();
    assert!(matches!(ids.encode(modern, limit), Err(Error::Limit(_))));
    assert!(matches!(
        RecipeBookRemove::decode(&bytes, modern, limit),
        Err(Error::Limit(_))
    ));
    let small = Limits {
        max_packet: 2,
        ..Limits::default()
    };
    assert!(matches!(ids.encode(modern, small), Err(Error::Limit(_))));
    for text in ["Bad:recipe", "a:b:c", "a:bad space", "a:é"] {
        let value = DisplayedRecipe {
            recipe: RecipeReference::RegistryKey(text.into()),
        };
        assert!(matches!(
            value.encode(legacy, Limits::default()),
            Err(Error::Invalid("resource identifier"))
        ));
        let mut raw = vec![text.len() as u8];
        raw.extend(text.as_bytes());
        assert!(matches!(
            DisplayedRecipe::decode(&raw, legacy, Limits::default()),
            Err(Error::Invalid("resource identifier"))
        ));
    }
    let value = DisplayedRecipe {
        recipe: RecipeReference::RegistryKey("abc".into()),
    };
    for limits in [
        Limits {
            max_string_chars: 2,
            ..Limits::default()
        },
        small,
    ] {
        assert!(matches!(value.encode(legacy, limits), Err(Error::Limit(_))));
        assert!(matches!(
            DisplayedRecipe::decode(b"\x03abc", legacy, limits),
            Err(Error::Limit(_))
        ));
    }
    assert!(matches!(
        DisplayedRecipe::decode(&[1, 255], legacy, Limits::default()),
        Err(Error::Invalid("UTF-8 string"))
    ));
}
#[test]
fn strict_booleans_and_unknown_enums_do_not_mask_malformed_bodies() {
    for &v in Version::ALL {
        for bytes in [
            &[4, 2, 0][..],
            &[4, 1][..],
            &[4, 1, 0, 0][..],
            &[255, 255, 255, 255, 127, 1, 0][..],
        ] {
            assert!(matches!(
                RecipeBookChangeSettings::decode(bytes, v, Limits::default()),
                Err(Error::Invalid(_) | Error::Eof)
            ));
        }
        if v.protocol() >= 768 {
            let mut bytes = [0; 8];
            bytes[7] = 2;
            assert!(matches!(
                RecipeBookSettings::decode(&bytes, v, Limits::default()),
                Err(Error::Invalid("boolean"))
            ));
        }
    }
}

#[test]
fn all_boolean_positions_and_both_legacy_counts_are_checked() {
    let legacy = Version::from_protocol(767).unwrap();
    let modern = Version::from_protocol(768).unwrap();
    for offset in 0..8 {
        let mut settings = vec![0; 8];
        settings[offset] = 2;
        assert!(matches!(
            RecipeBookSettings::decode(&settings, modern, Limits::default()),
            Err(Error::Invalid("boolean"))
        ));
        let mut unlock = vec![2];
        unlock.extend(settings);
        unlock.push(0);
        assert!(matches!(
            UnlockRecipes::decode(&unlock, legacy, Limits::default()),
            Err(Error::Invalid("boolean"))
        ));
    }
    for unknown in [false, true] {
        let mut unlock = vec![if unknown { 3 } else { 0 }];
        unlock.extend([0; 8]);
        unlock.extend([255, 255, 255, 255, 15]);
        assert!(matches!(
            UnlockRecipes::decode(&unlock, legacy, Limits::default()),
            Err(Error::Invalid("negative length"))
        ));
    }
    let mut init = vec![0; 10];
    init.extend([255, 255, 255, 255, 15]);
    assert!(matches!(
        UnlockRecipes::decode(&init, legacy, Limits::default()),
        Err(Error::Invalid("negative length"))
    ));
    for &v in Version::ALL {
        let f = support::fixtures()
            .into_iter()
            .find(|f| f.version == v && f.name == "craft_recipe_request")
            .unwrap();
        let mut bytes = f.bytes;
        *bytes.last_mut().unwrap() = 2;
        assert!(matches!(
            CraftRecipeRequest::decode(&bytes, v, Limits::default()),
            Err(Error::Invalid("boolean"))
        ));
    }
    let empty = UnlockRecipes {
        action: UnlockRecipesAction::Init {
            highlighted: vec![],
        },
        settings: RecipeBookSettings::default(),
        recipes: vec![],
    };
    let limits = Limits {
        max_collection: 0,
        ..Limits::default()
    };
    let bytes = empty.encode(legacy, limits).unwrap();
    assert_eq!(
        UnlockRecipes::decode(&bytes, legacy, limits).unwrap(),
        empty
    );
}
