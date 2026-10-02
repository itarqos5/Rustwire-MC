use rustwire_mc::{
    nbt::{Nbt, Tag, TagType},
    registry::{RegistryData, RegistryEntry, RegistryStore},
    version::State,
    Limits, Version,
};
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn modern() -> RegistryData {
    RegistryData::Entries {
        registry: "test:registry".into(),
        entries: vec![
            RegistryEntry {
                key: "test:omitted".into(),
                data: None,
            },
            RegistryEntry {
                key: "test:present".into(),
                data: Some(Nbt::anonymous(Tag::Int(7))),
            },
        ],
    }
}
#[test]
fn independent_registry_fixtures_and_state_boundaries() {
    let modern_bytes=hex("0d746573743a7265676973747279020c746573743a6f6d6974746564000c746573743a70726573656e74010300000007");
    for &v in Version::ALL {
        let (expected, bytes) = if v.protocol() < 766 {
            (
                RegistryData::Legacy(if v.protocol() == 763 {
                    Nbt {
                        name: Some("".into()),
                        root: Tag::Compound(vec![]),
                    }
                } else {
                    Nbt::anonymous(Tag::Compound(vec![]))
                }),
                if v.protocol() == 763 {
                    vec![10, 0, 0, 0]
                } else {
                    vec![10, 0]
                },
            )
        } else {
            (modern(), modern_bytes.clone())
        };
        assert_eq!(
            RegistryData::decode(&bytes, v, Limits::default()).unwrap(),
            expected
        );
        assert_eq!(expected.encode(v, Limits::default()).unwrap(), bytes);
        for state in [State::Handshake, State::Status, State::Login, State::Play] {
            assert!(expected.packet(v, state, Limits::default()).is_err());
            assert!(RegistryData::decode_in_state(&bytes, v, state, Limits::default()).is_err());
        }
        if v.protocol() == 763 {
            assert!(expected
                .packet(v, State::Configuration, Limits::default())
                .is_err());
        } else {
            let p = expected
                .packet(v, State::Configuration, Limits::default())
                .unwrap();
            assert_eq!(p.id, if v.protocol() < 766 { 5 } else { 7 });
            assert_eq!(p.data, bytes);
            assert_eq!(
                RegistryData::decode_in_state(&bytes, v, State::Configuration, Limits::default())
                    .unwrap(),
                expected
            );
        }
        for n in 0..bytes.len() {
            assert!(RegistryData::decode(&bytes[..n], v, Limits::default()).is_err());
        }
        let mut tail = bytes;
        tail.push(0);
        assert!(RegistryData::decode(&tail, v, Limits::default()).is_err());
    }
}
#[test]
fn identifiers_and_present_nbt_are_validated_without_forcing_compounds() {
    for &v in &Version::ALL[3..] {
        for bytes in [
            hex("014100"),
            hex("016101014100"),
            hex("016101016102"),
            hex("01610101610100"),
            hex("0161ffffffff0f"),
            hex("0161ffffffff07"),
        ] {
            assert!(RegistryData::decode(&bytes, v, Limits::default()).is_err());
        }
        for identifier in ["", "a", ":a", ":", "a:", "a:../x", "..:x"] {
            let p = RegistryData::Entries {
                registry: identifier.into(),
                entries: vec![RegistryEntry {
                    key: identifier.into(),
                    data: None,
                }],
            };
            let ok = identifier != "..:x" || v.protocol() < 775;
            assert_eq!(p.encode(v, Limits::default()).is_ok(), ok);
            if ok {
                let b = p.encode(v, Limits::default()).unwrap();
                assert_eq!(RegistryData::decode(&b, v, Limits::default()).unwrap(), p);
            }
        }
        let mut p = modern();
        if let RegistryData::Entries { entries, .. } = &mut p {
            entries[0].key = "Invalid".into();
        }
        assert!(p.encode(v, Limits::default()).is_err());
    }
}
#[test]
fn aggregate_nbt_nodes_cover_all_registry_entries() {
    let p = RegistryData::Entries {
        registry: "a".into(),
        entries: vec![
            RegistryEntry {
                key: "a".into(),
                data: Some(Nbt::anonymous(Tag::ByteArray(vec![1, 2]))),
            },
            RegistryEntry {
                key: "b".into(),
                data: Some(Nbt::anonymous(Tag::List {
                    element_type: TagType::Int,
                    elements: vec![Tag::Int(3), Tag::Int(4)],
                })),
            },
        ],
    };
    for &v in &Version::ALL[3..] {
        let bytes = p.encode(v, Limits::default()).unwrap();
        for nodes in 0..=6 {
            let limits = Limits {
                max_nbt_nodes: nodes,
                ..Limits::default()
            };
            assert_eq!(p.encode(v, limits).is_ok(), nodes == 6);
            assert_eq!(RegistryData::decode(&bytes, v, limits).is_ok(), nodes == 6);
        }
        for max_packet in 0..=bytes.len() {
            let limits = Limits {
                max_packet,
                ..Limits::default()
            };
            assert_eq!(p.encode(v, limits).is_ok(), max_packet == bytes.len());
            assert_eq!(
                RegistryData::decode(&bytes, v, limits).is_ok(),
                max_packet == bytes.len()
            );
        }
        let limits = Limits {
            max_collection: 1,
            ..Limits::default()
        };
        assert!(p.encode(v, limits).is_err());
        assert!(RegistryData::decode(&bytes, v, limits).is_err());
    }
}
#[test]
fn duplicate_wire_keys_remain_ordered_but_store_application_is_atomic() {
    let mut p = modern();
    if let RegistryData::Entries { entries, .. } = &mut p {
        entries[1].key = entries[0].key.clone();
    }
    let bytes = p.encode(Version::V26_2, Limits::default()).unwrap();
    assert_eq!(
        RegistryData::decode(&bytes, Version::V26_2, Limits::default()).unwrap(),
        p
    );
    let mut store = RegistryStore::default();
    store.apply(modern()).unwrap();
    let before = store.clone();
    assert!(store.apply(p).is_err());
    assert_eq!(store, before);
}
