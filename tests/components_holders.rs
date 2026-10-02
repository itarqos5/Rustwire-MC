//! Independent item-holder fixtures covering stream-codec boundaries corrected
//! against the cached official release jars, including old EitherHolder arms.
use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, NbtString, Tag},
    packet::{entity_metadata::holders::RegistryHolder, inventory::*},
    Error, Limits, Version,
};
fn version(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|x| u8::from_str_radix(x, 16).unwrap())
        .collect()
}
fn text(s: &str) -> Nbt {
    Nbt::anonymous(Tag::String(NbtString::from(s)))
}
fn id(p: i32, name: &str) -> u8 {
    let values = match name {
        "trim" => [35, 35, 45, 45, 47, 47, 47, 47, 54, 56, 56],
        "instrument" => [40, 40, 50, 50, 52, 52, 52, 52, 59, 61, 61],
        "provides_trim_material" => [255, 255, 255, 255, 53, 53, 53, 53, 60, 62, 62],
        "jukebox_playable" => [255, 42, 52, 52, 55, 55, 55, 55, 62, 64, 64],
        "banner_patterns" => [48, 49, 59, 59, 63, 63, 63, 63, 70, 72, 72],
        "chicken/variant" => [255, 255, 255, 255, 86, 86, 86, 86, 93, 97, 98],
        "zombie_nautilus/variant" => [255, 255, 255, 255, 255, 255, 255, 255, 94, 99, 100],
        "damage_type" => [255, 255, 255, 255, 255, 255, 255, 255, 8, 8, 8],
        "chicken/sound_variant" => [255, 255, 255, 255, 255, 255, 255, 255, 255, 98, 99],
        _ => panic!("unknown fixture component"),
    };
    values[(p - 766) as usize]
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
fn wire(p: i32, name: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![1, 5, 1, 0, id(p, name)];
    out.extend_from_slice(payload);
    out
}
fn fixture(p: i32, name: &'static str, payload: &[u8], value: ComponentValue) {
    let v = version(p);
    let encoded = wire(p, name, payload);
    let expected = slot(name, value);
    assert_eq!(component_id(v, name).unwrap(), i32::from(id(p, name)));
    assert_eq!(
        Slot::decode(&encoded, v, Limits::default()).unwrap(),
        expected,
        "decode {p} {name}"
    );
    assert_eq!(
        expected.encode(v, Limits::default()).unwrap(),
        encoded,
        "encode {p} {name}"
    );
    for end in 0..encoded.len() {
        let mut reader = Reader::new(&encoded[..end], Limits::default());
        assert!(
            Slot::read(&mut reader, v).is_err(),
            "truncated {p} {name} at {end}"
        );
        assert_eq!(reader.position(), 0, "atomic {p} {name} at {end}");
    }
    let mut trailing = encoded;
    trailing.push(0);
    assert!(Slot::decode(&trailing, v, Limits::default()).is_err());
}
fn material(p: i32) -> TrimMaterial {
    TrimMaterial {
        asset_name: "q".into(),
        ingredient_id: (p < 770).then_some(2),
        item_model_index: (p < 769).then_some(0.5),
        override_armor_assets: vec![(
            if p < 768 {
                TrimAssetKey::ArmorMaterialId(3)
            } else {
                TrimAssetKey::Identifier("x:y".into())
            },
            "v".into(),
        )],
        description: text("a"),
    }
}
fn material_bytes(p: i32) -> Vec<u8> {
    let mut b = hex("01 71");
    if p < 770 {
        b.push(2);
    }
    if p < 769 {
        b.extend(hex("3f 00 00 00"));
    }
    b.push(1);
    b.extend(if p < 768 { vec![3] } else { hex("03 78 3a 79") });
    b.extend(hex("01 76 08 00 01 61"));
    b
}
fn trim(p: i32) -> ArmorTrim {
    ArmorTrim {
        material: RegistryHolder::Inline(material(p)),
        pattern: RegistryHolder::Inline(TrimPattern {
            asset_id: "x:p".into(),
            template_item_id: (p < 770).then_some(4),
            description: text("b"),
            decal: true,
        }),
        show_tooltip: (p < 770).then_some(true),
    }
}
fn trim_bytes(p: i32) -> Vec<u8> {
    let mut b = vec![0];
    b.extend(material_bytes(p));
    b.extend(hex("00 03 78 3a 70"));
    if p < 770 {
        b.push(4);
    }
    b.extend(hex("08 00 01 62 01"));
    if p < 770 {
        b.push(1);
    }
    b
}
#[test]
fn inline_trim_includes_historical_model_index_and_registry_keys() {
    for p in 766..=776 {
        fixture(
            p,
            "trim",
            &trim_bytes(p),
            ComponentValue::ArmorTrim(trim(p)),
        );
    }
}
#[test]
fn reference_trim_and_provided_material() {
    for p in 766..=776 {
        let mut b = vec![2, 4];
        if p < 770 {
            b.push(0);
        }
        fixture(
            p,
            "trim",
            &b,
            ComponentValue::ArmorTrim(ArmorTrim {
                material: RegistryHolder::RegistryId(1),
                pattern: RegistryHolder::RegistryId(3),
                show_tooltip: (p < 770).then_some(false),
            }),
        );
    }
    for p in 770..=776 {
        let mut b = vec![];
        if p < 775 {
            b.push(1);
        }
        b.push(0);
        b.extend(material_bytes(p));
        fixture(
            p,
            "provides_trim_material",
            &b,
            ComponentValue::ProvidesTrimMaterial(HolderOrKey::Holder(RegistryHolder::Inline(
                material(p),
            ))),
        );
        let b = if p < 775 { vec![1, 1] } else { vec![1] };
        fixture(
            p,
            "provides_trim_material",
            &b,
            ComponentValue::ProvidesTrimMaterial(HolderOrKey::Holder(RegistryHolder::RegistryId(
                0,
            ))),
        );
        if p < 775 {
            fixture(
                p,
                "provides_trim_material",
                &hex("00 03 78 3a 6b"),
                ComponentValue::ProvidesTrimMaterial(HolderOrKey::Key("x:k".into())),
            );
        }
    }
}
#[test]
fn instruments_change_duration_description_and_key_wrapper() {
    for p in 766..=776 {
        let mut b = vec![];
        if (770..775).contains(&p) {
            b.push(1);
        }
        b.extend([0, 3]);
        let duration = if p < 768 {
            b.extend(hex("8c 01"));
            InstrumentDuration::Ticks(140)
        } else {
            b.extend(hex("3f 00 00 00"));
            InstrumentDuration::Seconds(0.5)
        };
        b.extend(hex("3f c0 00 00"));
        if p >= 768 {
            b.extend(hex("08 00 01 61"));
        }
        fixture(
            p,
            "instrument",
            &b,
            ComponentValue::Instrument(HolderOrKey::Holder(RegistryHolder::Inline(Instrument {
                sound: RegistryHolder::RegistryId(2),
                use_duration: duration,
                range: 1.5,
                description: (p >= 768).then(|| text("a")),
            }))),
        );
        let b = if (770..775).contains(&p) {
            vec![1, 1]
        } else {
            vec![1]
        };
        fixture(
            p,
            "instrument",
            &b,
            ComponentValue::Instrument(HolderOrKey::Holder(RegistryHolder::RegistryId(0))),
        );
        if (770..775).contains(&p) {
            fixture(
                p,
                "instrument",
                &hex("00 03 78 3a 6b"),
                ComponentValue::Instrument(HolderOrKey::Key("x:k".into())),
            );
        }
    }
}
#[test]
fn jukebox_inline_song_and_legacy_tooltip() {
    for p in 767..=776 {
        let mut b = vec![];
        if p < 775 {
            b.push(1);
        }
        b.extend(hex(
            "00 00 03 78 3a 73 01 40 00 00 00 08 00 01 61 40 00 00 00 0e",
        ));
        if p < 770 {
            b.push(1);
        }
        fixture(
            p,
            "jukebox_playable",
            &b,
            ComponentValue::JukeboxPlayable(JukeboxPlayable {
                song: HolderOrKey::Holder(RegistryHolder::Inline(JukeboxSong {
                    sound: RegistryHolder::Inline(SoundEvent {
                        name: "x:s".into(),
                        fixed_range: Some(2.0),
                    }),
                    description: text("a"),
                    length_seconds: 2.0,
                    comparator_output: 14,
                })),
                show_tooltip: (p < 770).then_some(true),
            }),
        );
        if p < 775 {
            let mut b = hex("00 03 78 3a 6b");
            if p < 770 {
                b.push(0);
            }
            fixture(
                p,
                "jukebox_playable",
                &b,
                ComponentValue::JukeboxPlayable(JukeboxPlayable {
                    song: HolderOrKey::Key("x:k".into()),
                    show_tooltip: (p < 770).then_some(false),
                }),
            );
        }
    }
}
#[test]
fn banner_patterns_have_real_inline_pattern_not_sound_payload() {
    for p in 766..=776 {
        fixture(
            p,
            "banner_patterns",
            &hex("02 00 03 78 3a 70 01 74 0f 03 00"),
            ComponentValue::BannerPatterns(vec![
                BannerPatternLayer {
                    pattern: RegistryHolder::Inline(BannerPattern {
                        asset_id: "x:p".into(),
                        translation_key: "t".into(),
                    }),
                    color_id: 15,
                },
                BannerPatternLayer {
                    pattern: RegistryHolder::RegistryId(2),
                    color_id: 0,
                },
            ]),
        );
        fixture(
            p,
            "banner_patterns",
            &[0],
            ComponentValue::BannerPatterns(vec![]),
        );
    }
}
#[test]
fn references_are_unshifted_ids_and_keys_not_inline_variant_data() {
    for (name, start) in [
        ("chicken/variant", 770),
        ("zombie_nautilus/variant", 774),
        ("damage_type", 774),
    ] {
        for p in start..=776 {
            fixture(
                p,
                name,
                if p < 775 { &[1, 0] } else { &[0] },
                ComponentValue::RegistryReference(RegistryReference::RegistryId(0)),
            );
            if p < 775 {
                fixture(
                    p,
                    name,
                    &hex("00 03 78 3a 6b"),
                    ComponentValue::RegistryReference(RegistryReference::Key("x:k".into())),
                );
            }
        }
    }
}
#[test]
fn holder_collections_and_nbt_roots_share_inventory_budgets() {
    let v = version(776);
    let value = slot("trim", ComponentValue::ArmorTrim(trim(776)));
    let encoded = wire(776, "trim", &trim_bytes(776));
    for limits in [
        Limits {
            max_collection: 1,
            ..Limits::default()
        },
        Limits {
            max_nbt_nodes: 1,
            ..Limits::default()
        },
        Limits {
            max_packet: encoded.len() - 1,
            ..Limits::default()
        },
        Limits {
            max_string_chars: 2,
            ..Limits::default()
        },
    ] {
        assert!(matches!(value.encode(v, limits), Err(Error::Limit(_))));
        assert!(Slot::decode(&encoded, v, limits).is_err());
    }
    let encoded = wire(776, "banner_patterns", &hex("ff ff ff ff 07"));
    assert!(Slot::decode(&encoded, v, Limits::default()).is_err());
}
#[test]
fn wrong_release_fields_invalid_ids_and_colors_fail_atomically() {
    let v = version(776);
    let values = [
        slot(
            "instrument",
            ComponentValue::Instrument(HolderOrKey::Key("x:k".into())),
        ),
        slot(
            "provides_trim_material",
            ComponentValue::ProvidesTrimMaterial(HolderOrKey::Key("x:k".into())),
        ),
        slot(
            "damage_type",
            ComponentValue::RegistryReference(RegistryReference::Key("x:k".into())),
        ),
        slot("trim", ComponentValue::ArmorTrim(trim(768))),
        slot(
            "banner_patterns",
            ComponentValue::BannerPatterns(vec![BannerPatternLayer {
                pattern: RegistryHolder::RegistryId(0),
                color_id: 16,
            }]),
        ),
        slot(
            "banner_patterns",
            ComponentValue::BannerPatterns(vec![BannerPatternLayer {
                pattern: RegistryHolder::RegistryId(i32::MAX),
                color_id: 0,
            }]),
        ),
        slot(
            "damage_type",
            ComponentValue::RegistryReference(RegistryReference::RegistryId(-1)),
        ),
    ];
    for value in values {
        let mut w = Writer::new();
        w.u8(0xaa);
        assert!(value.write(&mut w, v, Limits::default()).is_err());
        assert_eq!(w.as_slice(), &[0xaa]);
    }
    for (name, payload) in [
        ("trim", hex("ff ff ff ff 0f")),
        ("banner_patterns", hex("01 01 10")),
        ("damage_type", hex("ff ff ff ff 0f")),
    ] {
        assert!(Slot::decode(&wire(776, name, &payload), v, Limits::default()).is_err());
    }
}
#[test]
fn duplicate_trim_assets_are_rejected() {
    let p = 776;
    let mut t = trim(p);
    if let RegistryHolder::Inline(material) = &mut t.material {
        material
            .override_armor_assets
            .push(material.override_armor_assets[0].clone());
    }
    assert!(slot("trim", ComponentValue::ArmorTrim(t))
        .encode(version(p), Limits::default())
        .is_err());
    let b = hex("00 01 71 02 03 78 3a 79 01 76 03 78 3a 79 01 77 08 00 01 61 01");
    assert!(Slot::decode(&wire(p, "trim", &b), version(p), Limits::default()).is_err());
}

#[test]
fn chicken_sound_variants_are_plain_ids_in_both_modern_releases() {
    for p in 775..=776 {
        fixture(p, "chicken/sound_variant", &[0], ComponentValue::VarInt(0));
        fixture(
            p,
            "chicken/sound_variant",
            &[0xac, 0x02],
            ComponentValue::VarInt(300),
        );
    }
}
