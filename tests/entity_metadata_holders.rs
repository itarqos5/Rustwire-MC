use rustwire_mc::{
    nbt::{Nbt, Tag},
    packet::{
        entity_metadata::{
            holders::{
                PaintingVariant, PlayerModel, PlayerSkinPatch, RegistryHolder, RegistryHolderSet,
                ResolvableProfile, WolfVariant,
            },
            EntityMetadata, MetadataEntry, MetadataValue,
        },
        ProfileProperty,
    },
    Error, Limits, Version,
};

// Independent wire construction: these serializer numbers are release-specific
// protocol entries, not obtained from the registry implementation under test.
fn painting_id(v: Version) -> u8 {
    match v.protocol() {
        763..=765 => 24,
        766..=769 => 26,
        770..=772 => 30,
        773 => 29,
        774 => 30,
        775..=776 => 34,
        _ => unreachable!(),
    }
}
fn profile_id(v: Version) -> u8 {
    match v.protocol() {
        773 => 36,
        774 => 37,
        775..=776 => 41,
        _ => unreachable!(),
    }
}
fn varint(bytes: &mut Vec<u8>, value: i32) {
    let mut n = value as u32;
    loop {
        let byte = (n & 127) as u8;
        n >>= 7;
        bytes.push(byte | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
fn text(bytes: &mut Vec<u8>, value: &str) {
    varint(bytes, value.len() as i32);
    bytes.extend(value.as_bytes());
}
fn framed(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![7, 4, id];
    bytes.extend(payload);
    // A following metadata value detects over/underconsumption of unframed data.
    bytes.extend([5, 0, 42, 255]);
    bytes
}
fn packet(serializer: &'static str, value: MetadataValue) -> EntityMetadata {
    EntityMetadata {
        entity_id: 7,
        entries: vec![
            MetadataEntry {
                index: 4,
                serializer,
                value,
            },
            MetadataEntry {
                index: 5,
                serializer: "byte",
                value: MetadataValue::Byte(42),
            },
        ],
    }
}
fn golden(v: Version, id: u8, payload: &[u8], expected: EntityMetadata) {
    let bytes = framed(id, payload);
    assert_eq!(
        EntityMetadata::decode(&bytes, v, Limits::default()).unwrap(),
        expected,
        "protocol {}",
        v.protocol()
    );
    assert_eq!(expected.encode(v, Limits::default()).unwrap(), bytes);
    for end in 0..bytes.len() {
        assert!(
            EntityMetadata::decode(&bytes[..end], v, Limits::default()).is_err(),
            "truncation {end} of protocol {}",
            v.protocol()
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(EntityMetadata::decode(&trailing, v, Limits::default()).is_err());
}
fn painting(title: Option<Nbt>, author: Option<Nbt>) -> PaintingVariant {
    PaintingVariant {
        width: 2,
        height: 4,
        asset_id: "minecraft:kebab".into(),
        title,
        author,
    }
}
fn inline_painting_payload(v: Version) -> Vec<u8> {
    let mut bytes = vec![0, 2, 4]; // inline, width VarInt, height VarInt
    text(&mut bytes, "minecraft:kebab");
    if v.protocol() >= 768 {
        // Present title, anonymous TAG_String("Hi"), absent author.
        bytes.extend([1, 8, 0, 2, b'H', b'i', 0]);
    }
    bytes
}

#[test]
fn painting_golden_release_boundaries_and_inline_payloads() {
    for &v in Version::ALL {
        if v.protocol() <= 766 {
            golden(
                v,
                painting_id(v),
                &[0],
                packet("painting_variant", MetadataValue::VarInt(0)),
            );
            continue;
        }
        golden(
            v,
            painting_id(v),
            &[1],
            packet(
                "painting_variant",
                MetadataValue::PaintingVariant(RegistryHolder::RegistryId(0)),
            ),
        );
        golden(
            v,
            painting_id(v),
            &[128, 1],
            packet(
                "painting_variant",
                MetadataValue::PaintingVariant(RegistryHolder::RegistryId(127)),
            ),
        );
        let title = (v.protocol() >= 768).then(|| Nbt::anonymous(Tag::String("Hi".into())));
        golden(
            v,
            painting_id(v),
            &inline_painting_payload(v),
            packet(
                "painting_variant",
                MetadataValue::PaintingVariant(RegistryHolder::Inline(painting(title, None))),
            ),
        );
    }
}

fn wolf(set: RegistryHolderSet) -> WolfVariant {
    WolfVariant {
        wild_texture: "test:wild".into(),
        tame_texture: "test:tame".into(),
        angry_texture: "test:angry".into(),
        biomes: set,
    }
}
fn wolf_prefix() -> Vec<u8> {
    let mut bytes = vec![0];
    for s in ["test:wild", "test:tame", "test:angry"] {
        text(&mut bytes, s);
    }
    bytes
}

#[test]
fn wolf_golden_holders_and_tag_or_explicit_biomes() {
    golden(
        Version::V1_20_5,
        23,
        &[0],
        packet("wolf_variant", MetadataValue::VarInt(0)),
    );
    for v in [Version::V1_21, Version::V1_21_2, Version::V1_21_4] {
        golden(
            v,
            23,
            &[1],
            packet(
                "wolf_variant",
                MetadataValue::WolfVariant(RegistryHolder::RegistryId(0)),
            ),
        );
        for (tail, set) in [
            (vec![1], RegistryHolderSet::Ids(vec![])),
            (vec![3, 0, 128, 1], RegistryHolderSet::Ids(vec![0, 128])),
            (
                {
                    let mut b = vec![0];
                    text(&mut b, "minecraft:is_forest");
                    b
                },
                RegistryHolderSet::Tag("minecraft:is_forest".into()),
            ),
        ] {
            let mut payload = wolf_prefix();
            payload.extend(tail);
            golden(
                v,
                23,
                &payload,
                packet(
                    "wolf_variant",
                    MetadataValue::WolfVariant(RegistryHolder::Inline(wolf(set))),
                ),
            );
        }
    }
    golden(
        Version::V1_21_5,
        24,
        &[0],
        packet("wolf_variant", MetadataValue::VarInt(0)),
    );
}

fn properties() -> Vec<ProfileProperty> {
    vec![ProfileProperty {
        name: "textures".into(),
        value: "value".into(),
        signature: Some("signed".into()),
    }]
}
fn property_payload(bytes: &mut Vec<u8>) {
    bytes.push(1);
    text(bytes, "textures");
    text(bytes, "value");
    bytes.push(1);
    text(bytes, "signed");
}
fn skin() -> PlayerSkinPatch {
    PlayerSkinPatch {
        body: Some("test:skin".into()),
        cape: None,
        elytra: Some("test:wings".into()),
        model: Some(PlayerModel::Slim),
    }
}
fn skin_payload(bytes: &mut Vec<u8>) {
    bytes.push(1);
    text(bytes, "test:skin");
    bytes.extend([0, 1]);
    text(bytes, "test:wings");
    bytes.extend([1, 1]);
}

#[test]
fn profile_golden_partial_complete_and_skin_patch() {
    for v in [
        Version::V1_21_9,
        Version::V1_21_11,
        Version::V26_1,
        Version::V26_2,
    ] {
        let mut partial = vec![0, 1];
        text(&mut partial, "Alex");
        partial.push(1);
        partial.extend([0x12; 16]);
        property_payload(&mut partial);
        skin_payload(&mut partial);
        golden(
            v,
            profile_id(v),
            &partial,
            packet(
                "resolvable_profile",
                MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
                    name: Some("Alex".into()),
                    uuid: Some([0x12; 16]),
                    properties: properties(),
                    skin_patch: skin(),
                }),
            ),
        );
        let mut complete = vec![1];
        complete.extend([0x34; 16]);
        text(&mut complete, "Steve");
        property_payload(&mut complete);
        skin_payload(&mut complete);
        golden(
            v,
            profile_id(v),
            &complete,
            packet(
                "resolvable_profile",
                MetadataValue::ResolvableProfile(ResolvableProfile::Complete {
                    uuid: [0x34; 16],
                    name: "Steve".into(),
                    properties: properties(),
                    skin_patch: skin(),
                }),
            ),
        );
        golden(
            v,
            profile_id(v),
            &[0, 0, 0, 0, 0, 0, 0, 0],
            packet(
                "resolvable_profile",
                MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
                    name: None,
                    uuid: None,
                    properties: vec![],
                    skin_patch: PlayerSkinPatch::default(),
                }),
            ),
        );
        // A wide model is encoded as false, independent of enum declaration order.
        golden(
            v,
            profile_id(v),
            &[0, 0, 0, 0, 0, 0, 0, 1, 0],
            packet(
                "resolvable_profile",
                MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
                    name: None,
                    uuid: None,
                    properties: vec![],
                    skin_patch: PlayerSkinPatch {
                        model: Some(PlayerModel::Wide),
                        ..PlayerSkinPatch::default()
                    },
                }),
            ),
        );
    }
}

#[test]
fn reject_invalid_holder_markers_ids_and_painting_dimensions() {
    let v = Version::V1_21_4;
    let negative = [255, 255, 255, 255, 15];
    for id in [23, 26] {
        assert!(matches!(
            EntityMetadata::decode(&framed(id, &negative), v, Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
    for id in [-1, i32::MAX] {
        assert!(packet(
            "painting_variant",
            MetadataValue::PaintingVariant(RegistryHolder::RegistryId(id))
        )
        .encode(v, Limits::default())
        .is_err());
        assert!(packet(
            "wolf_variant",
            MetadataValue::WolfVariant(RegistryHolder::RegistryId(id))
        )
        .encode(v, Limits::default())
        .is_err());
    }
    for width in [0, 17, -1] {
        let mut p = painting(None, None);
        p.width = width;
        assert!(packet(
            "painting_variant",
            MetadataValue::PaintingVariant(RegistryHolder::Inline(p))
        )
        .encode(v, Limits::default())
        .is_err());
        let mut payload = vec![0];
        varint(&mut payload, width);
        payload.push(4);
        text(&mut payload, "test:a");
        payload.extend([0, 0]);
        assert!(EntityMetadata::decode(&framed(26, &payload), v, Limits::default()).is_err());
    }
    let mut payload = vec![0, 2, 4];
    text(&mut payload, "test:a");
    payload.extend([1, 0, 0]);
    assert!(matches!(
        EntityMetadata::decode(&framed(26, &payload), v, Limits::default()),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn reject_invalid_holder_sets_and_share_collection_budget() {
    let v = Version::V1_21_4;
    for tail in [
        vec![255, 255, 255, 255, 15],
        vec![2, 255, 255, 255, 255, 15],
        vec![255, 255, 255, 255, 7],
    ] {
        let mut p = wolf_prefix();
        p.extend(tail);
        assert!(EntityMetadata::decode(&framed(23, &p), v, Limits::default()).is_err());
    }
    let p = packet(
        "wolf_variant",
        MetadataValue::WolfVariant(RegistryHolder::Inline(wolf(RegistryHolderSet::Ids(vec![
            0, 1,
        ])))),
    );
    let encoded = p.encode(v, Limits::default()).unwrap();
    let small = Limits {
        max_collection: 3,
        ..Limits::default()
    }; // two entries plus two IDs
    assert!(matches!(p.encode(v, small), Err(Error::Limit(_))));
    assert!(matches!(
        EntityMetadata::decode(&encoded, v, small),
        Err(Error::Limit(_))
    ));
    let exact = Limits {
        max_collection: 4,
        ..Limits::default()
    };
    assert_eq!(EntityMetadata::decode(&encoded, v, exact).unwrap(), p);
    let bad = packet(
        "wolf_variant",
        MetadataValue::WolfVariant(RegistryHolder::Inline(wolf(RegistryHolderSet::Ids(vec![
            -1,
        ])))),
    );
    assert!(bad.encode(v, Limits::default()).is_err());
}

#[test]
fn reject_profile_discriminants_properties_and_server_string_limits() {
    let v = Version::V1_21_9;
    for payload in [
        vec![2],
        vec![0, 2],
        vec![0, 0, 2],
        vec![0, 0, 0, 17],
        vec![0, 0, 0, 0, 0, 0, 0, 1, 2],
        vec![128, 0],
    ] {
        assert!(EntityMetadata::decode(&framed(36, &payload), v, Limits::default()).is_err());
    }
    for (name_len, value_len, signature_len) in [(65, 1, 0), (1, 32768, 0), (1, 1, 1025)] {
        let property = ProfileProperty {
            name: "n".repeat(name_len),
            value: "v".repeat(value_len),
            signature: Some("s".repeat(signature_len)),
        };
        let p = packet(
            "resolvable_profile",
            MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
                name: None,
                uuid: None,
                properties: vec![property.clone()],
                skin_patch: PlayerSkinPatch::default(),
            }),
        );
        assert!(matches!(
            p.encode(v, Limits::default()),
            Err(Error::Limit(_))
        ));
        let mut bytes = vec![0, 0, 0, 1];
        text(&mut bytes, &property.name);
        text(&mut bytes, &property.value);
        bytes.push(1);
        text(&mut bytes, property.signature.as_ref().unwrap());
        bytes.extend([0, 0, 0, 0]);
        assert!(matches!(
            EntityMetadata::decode(&framed(36, &bytes), v, Limits::default()),
            Err(Error::Limit(_))
        ));
    }
    for name in ["a".repeat(17), "😀".repeat(9)] {
        let p = packet(
            "resolvable_profile",
            MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
                name: Some(name.clone()),
                uuid: None,
                properties: vec![],
                skin_patch: PlayerSkinPatch::default(),
            }),
        );
        assert!(p.encode(v, Limits::default()).is_err());
        let mut bytes = vec![0, 1];
        text(&mut bytes, &name);
        bytes.extend([0, 0, 0, 0, 0, 0]);
        assert!(EntityMetadata::decode(&framed(36, &bytes), v, Limits::default()).is_err());
    }
}

#[test]
fn share_profile_collection_nbt_and_packet_byte_budgets() {
    let v = Version::V1_21_9;
    let p = packet(
        "resolvable_profile",
        MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
            name: None,
            uuid: None,
            properties: properties(),
            skin_patch: skin(),
        }),
    );
    let bytes = p.encode(v, Limits::default()).unwrap();
    for limits in [
        Limits {
            max_collection: 2,
            ..Limits::default()
        },
        Limits {
            max_packet: bytes.len() - 1,
            ..Limits::default()
        },
        Limits {
            max_string_chars: 4,
            ..Limits::default()
        },
    ] {
        assert!(matches!(p.encode(v, limits), Err(Error::Limit(_))));
        assert!(matches!(
            EntityMetadata::decode(&bytes, v, limits),
            Err(Error::Limit(_))
        ));
    }
    let nbt = Nbt::anonymous(Tag::String("Hi".into()));
    let p = packet(
        "painting_variant",
        MetadataValue::PaintingVariant(RegistryHolder::Inline(painting(
            Some(nbt.clone()),
            Some(nbt),
        ))),
    );
    let bytes = p.encode(v, Limits::default()).unwrap();
    let limits = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(matches!(p.encode(v, limits), Err(Error::Limit(_))));
    assert!(matches!(
        EntityMetadata::decode(&bytes, v, limits),
        Err(Error::Limit(_))
    ));
}

#[test]
fn reject_variant_use_outside_release_and_title_in_767() {
    let p = packet(
        "painting_variant",
        MetadataValue::PaintingVariant(RegistryHolder::Inline(painting(None, None))),
    );
    assert!(p.encode(Version::V1_20_5, Limits::default()).is_err());
    let p = packet(
        "wolf_variant",
        MetadataValue::WolfVariant(RegistryHolder::RegistryId(0)),
    );
    for v in [Version::V1_20_5, Version::V1_21_5] {
        assert!(p.encode(v, Limits::default()).is_err());
    }
    let p = packet(
        "painting_variant",
        MetadataValue::PaintingVariant(RegistryHolder::Inline(painting(
            Some(Nbt::anonymous(Tag::String("Hi".into()))),
            None,
        ))),
    );
    assert!(matches!(
        p.encode(Version::V1_21, Limits::default()),
        Err(Error::Unsupported(_))
    ));
    let p = packet(
        "resolvable_profile",
        MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
            name: None,
            uuid: None,
            properties: vec![],
            skin_patch: PlayerSkinPatch::default(),
        }),
    );
    assert!(matches!(
        p.encode(Version::V1_21_7, Limits::default()),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn profile_property_count_boundary_and_unsigned_properties() {
    let v = Version::V1_21_9;
    let property = ProfileProperty {
        name: "n".repeat(64),
        value: "value".into(),
        signature: None,
    };
    let properties = vec![property.clone(); 16];
    let expected = packet(
        "resolvable_profile",
        MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
            name: Some("A".repeat(16)),
            uuid: None,
            properties,
            skin_patch: PlayerSkinPatch::default(),
        }),
    );
    let mut payload = vec![0, 1];
    text(&mut payload, &"A".repeat(16));
    payload.extend([0, 16]);
    for _ in 0..16 {
        text(&mut payload, &property.name);
        text(&mut payload, &property.value);
        payload.push(0);
    }
    payload.extend([0, 0, 0, 0]);
    golden(v, 36, &payload, expected);
    let oversized = packet(
        "resolvable_profile",
        MetadataValue::ResolvableProfile(ResolvableProfile::Partial {
            name: None,
            uuid: None,
            properties: vec![property; 17],
            skin_patch: PlayerSkinPatch::default(),
        }),
    );
    assert!(matches!(
        oversized.encode(v, Limits::default()),
        Err(Error::Limit(_))
    ));
}

#[test]
fn painting_optional_nbt_booleans_and_local_packet_caps_are_checked() {
    let v = Version::V1_21_2;
    for options in [vec![2], vec![0, 2], vec![1, 0]] {
        let mut payload = vec![0, 2, 4];
        text(&mut payload, "test:a");
        payload.extend(options);
        assert!(EntityMetadata::decode(&framed(26, &payload), v, Limits::default()).is_err());
    }
    let p = packet(
        "painting_variant",
        MetadataValue::PaintingVariant(RegistryHolder::Inline(painting(None, None))),
    );
    let bytes = p.encode(v, Limits::default()).unwrap();
    let exact = Limits {
        max_packet: bytes.len(),
        ..Limits::default()
    };
    assert_eq!(p.encode(v, exact).unwrap(), bytes);
    assert_eq!(EntityMetadata::decode(&bytes, v, exact).unwrap(), p);
    let small = Limits {
        max_packet: bytes.len() - 1,
        ..Limits::default()
    };
    assert!(matches!(p.encode(v, small), Err(Error::Limit(_))));
    assert!(matches!(
        EntityMetadata::decode(&bytes, v, small),
        Err(Error::Limit(_))
    ));
}
