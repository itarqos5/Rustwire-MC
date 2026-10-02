use rustwire_mc::{
    packet::{
        common,
        tags::{RegistryTag, TaggedRegistry, UpdateTags},
    },
    registry::{Registry, RegistryEntry},
    version::State,
    Error, Limits, Version,
};
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
// Synthetic bytes independently exercised by all 14 cached release serializers.
fn fixture() -> Vec<u8> {
    hex("010d746573743a72656769737472790108746573743a74616704008001ffffffff078001")
}
fn value() -> UpdateTags {
    UpdateTags {
        registries: vec![TaggedRegistry {
            registry: "test:registry".into(),
            tags: vec![RegistryTag {
                name: "test:tag".into(),
                entries: vec![0, 128, i32::MAX as u32, 128],
            }],
        }],
    }
}
#[test]
fn independent_wire_and_catalogue_boundaries() {
    let play = [
        0x6e, 0x70, 0x74, 0x78, 0x78, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 0x84, 0x84, 0x86, 0x86,
    ];
    let config = [
        None,
        Some(8),
        Some(9),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
        Some(13),
    ];
    for (i, &v) in Version::ALL.iter().enumerate() {
        let bytes = fixture();
        let expected = value();
        assert_eq!(
            UpdateTags::decode(&bytes, v, Limits::default()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(v, Limits::default()).unwrap(), bytes);
        let p = expected.packet(v, State::Play, Limits::default()).unwrap();
        assert_eq!(p.id, play[i]);
        assert_eq!(p.data, bytes);
        assert_eq!(
            UpdateTags::decode_in_state(&bytes, v, State::Play, Limits::default()).unwrap(),
            expected
        );
        match config[i] {
            Some(id) => {
                assert_eq!(
                    expected
                        .packet(v, State::Configuration, Limits::default())
                        .unwrap()
                        .id,
                    id
                );
                assert!(UpdateTags::decode_in_state(
                    &bytes,
                    v,
                    State::Configuration,
                    Limits::default()
                )
                .is_ok());
            }
            None => {
                assert!(expected
                    .packet(v, State::Configuration, Limits::default())
                    .is_err());
                assert!(UpdateTags::decode_in_state(
                    &bytes,
                    v,
                    State::Configuration,
                    Limits::default()
                )
                .is_err());
            }
        }
        for state in [State::Handshake, State::Status, State::Login] {
            assert!(expected.packet(v, state, Limits::default()).is_err());
            assert!(UpdateTags::decode_in_state(&bytes, v, state, Limits::default()).is_err());
        }
        // Public compatibility path and existing common dispatcher remain usable.
        assert_eq!(
            common::UpdateTags::decode(&bytes, v, Limits::default()).unwrap(),
            expected
        );
        assert!(matches!(
            common::CommonPacket::decode("tags", &bytes, v, Limits::default()).unwrap(),
            Some(common::CommonPacket::Tags(_))
        ));
        for end in 0..bytes.len() {
            assert!(
                UpdateTags::decode(&bytes[..end], v, Limits::default()).is_err(),
                "{v} prefix {end}"
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(UpdateTags::decode(&trailing, v, Limits::default()).is_err());
        assert_eq!(
            UpdateTags::decode(&[0], v, Limits::default())
                .unwrap()
                .registries,
            vec![]
        );
    }
}
#[test]
fn nested_aggregate_and_byte_limits_are_symmetric() {
    let bytes = fixture();
    let expected = value();
    let v = Version::V26_2;
    for count in 0..=6 {
        let limits = Limits {
            max_collection: count,
            ..Limits::default()
        };
        assert_eq!(UpdateTags::decode(&bytes, v, limits).is_ok(), count == 6);
        assert_eq!(expected.encode(v, limits).is_ok(), count == 6);
    }
    for max_packet in 0..=bytes.len() {
        let limits = Limits {
            max_packet,
            ..Limits::default()
        };
        assert_eq!(
            UpdateTags::decode(&bytes, v, limits).is_ok(),
            max_packet == bytes.len()
        );
        assert_eq!(
            expected.encode(v, limits).is_ok(),
            max_packet == bytes.len()
        );
    }
    let limits = Limits {
        max_string_chars: 12,
        ..Limits::default()
    };
    assert!(UpdateTags::decode(&bytes, v, limits).is_err());
    assert!(expected.encode(v, limits).is_err());
    // Counts cannot allocate from a tiny body, including maximal caller budgets.
    let limits = Limits {
        max_collection: usize::MAX,
        ..Limits::default()
    };
    for b in [
        hex("ffffffff07"),
        hex("0100ffffffff07"),
        hex("01000100ffffffff07"),
    ] {
        assert!(matches!(UpdateTags::decode(&b, v, limits), Err(Error::Eof)));
    }
    // Registry + tag + ID counts all contribute; no nested reset.
    let b = hex("0200010001000001000100");
    assert!(UpdateTags::decode(
        &b,
        v,
        Limits {
            max_collection: 5,
            ..Limits::default()
        }
    )
    .is_err());
}
#[test]
fn malformed_fields_and_versioned_identifiers() {
    for &v in Version::ALL {
        for b in [
            hex("ffffffff0f"),
            hex("0100ffffffff0f"),
            hex("01000100ffffffff0f"),
            hex("0100010001ffffffff0f"),
            hex("01000100018080808010"),
            hex("0101ff00"),
            hex("800000"),
        ] {
            assert!(
                UpdateTags::decode(&b, v, Limits::default()).is_err(),
                "{v} {b:?}"
            );
        }
        for invalid in ["UPPER:path", "a:b:c", "a/b:x", "a:x y", "a:é"] {
            let mut p = value();
            p.registries[0].registry = invalid.into();
            assert!(p.encode(v, Limits::default()).is_err());
            p = value();
            p.registries[0].tags[0].name = invalid.into();
            assert!(p.encode(v, Limits::default()).is_err());
        }
        for accepted in ["", "tag", ":tag", ":", "a:", "a:../x", "a:x//y"] {
            let mut p = value();
            p.registries[0].registry = accepted.into();
            p.registries[0].tags[0].name = accepted.into();
            let b = p.encode(v, Limits::default()).unwrap();
            assert_eq!(UpdateTags::decode(&b, v, Limits::default()).unwrap(), p);
        }
        let mut p = value();
        p.registries[0].registry = "..:x".into();
        assert_eq!(p.encode(v, Limits::default()).is_ok(), v.protocol() < 775);
        let b = hex("01042e2e3a7800");
        assert_eq!(
            UpdateTags::decode(&b, v, Limits::default()).is_ok(),
            v.protocol() < 775
        );
        let mut p = value();
        p.registries[0].tags[0].entries = vec![u32::MAX];
        assert!(p.encode(v, Limits::default()).is_err());
    }
}
#[test]
fn duplicate_order_and_unknown_memberships_remain_explicit() {
    let mut p = UpdateTags {
        registries: vec![
            TaggedRegistry {
                registry: "minecraft:block".into(),
                tags: vec![RegistryTag {
                    name: "old".into(),
                    entries: vec![99],
                }],
            },
            TaggedRegistry {
                registry: "block".into(),
                tags: vec![
                    RegistryTag {
                        name: "tag".into(),
                        entries: vec![1],
                    },
                    RegistryTag {
                        name: ":tag".into(),
                        entries: vec![2, 9, 2],
                    },
                ],
            },
        ],
    };
    let before = p.clone();
    let b = p.encode(Version::V26_2, Limits::default()).unwrap();
    p = UpdateTags::decode(&b, Version::V26_2, Limits::default()).unwrap();
    assert_eq!(before, p);
    assert_eq!(p.encode(Version::V26_2, Limits::default()).unwrap(), b);
    let registry = p.registry(":block").unwrap();
    assert!(registry.tag("old").is_none());
    let tag = registry.tag("minecraft:tag").unwrap();
    assert_eq!(tag.entries, vec![2, 9, 2]);
    assert!(tag.contains(2));
    assert!(!tag.contains(1));
    let mut data = Registry::default();
    data.entries.insert(
        2,
        RegistryEntry {
            key: "test:entry".into(),
            data: None,
        },
    );
    let resolved = tag.resolve(&data).collect::<Vec<_>>();
    assert_eq!(resolved.len(), 3);
    assert_eq!(resolved[0].1.unwrap().key, "test:entry");
    assert!(resolved[0].1.unwrap().data.is_none());
    assert!(resolved[1].1.is_none());
    assert_eq!(resolved[0], resolved[2]);
    assert!(p.registry("unavailable").is_none());
}
