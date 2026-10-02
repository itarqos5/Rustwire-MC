use rustwire_mc::{
    codec::Reader,
    nbt::{Nbt, Tag, TagType},
    packet::JoinGame,
    registry::{RegistryData, RegistryEntry},
    Limits, Version,
};

// Independent synthetic NBT payloads, exercised against the actual 763 Join
// Game and 764/765 configuration packet constructors by LegacyRegistryRootOracle.
// Each pair is a tag type plus its payload, without a root name.
const ROOTS: &[(u8, &[u8])] = &[
    (0, &[]),
    (1, &[7]),
    (2, &[0, 7]),
    (3, &[0, 0, 0, 7]),
    (4, &[0, 0, 0, 0, 0, 0, 0, 7]),
    (5, &[0, 0, 0, 7]),
    (6, &[0, 0, 0, 0, 0, 0, 0, 7]),
    (7, &[0, 0, 0, 0]),
    (8, &[0, 0]),
    (9, &[0, 0, 0, 0, 0]),
    (10, &[0]),
    (11, &[0, 0, 0, 0]),
    (12, &[0, 0, 0, 0]),
];
fn root(id: u8, payload: &[u8], named: bool) -> Vec<u8> {
    let mut bytes = vec![id];
    if named && id != 0 {
        bytes.extend([0, 0]);
    }
    bytes.extend(payload);
    bytes
}
fn values() -> Vec<Tag> {
    vec![
        Tag::Byte(7),
        Tag::Short(7),
        Tag::Int(7),
        Tag::Long(7),
        Tag::Float(f32::from_bits(7)),
        Tag::Double(f64::from_bits(7)),
        Tag::ByteArray(vec![]),
        Tag::String("".into()),
        Tag::List {
            element_type: TagType::End,
            elements: vec![],
        },
        Tag::Compound(vec![]),
        Tag::IntArray(vec![]),
        Tag::LongArray(vec![]),
    ]
}
#[test]
fn legacy_registry_roots_require_compounds_in_both_directions() {
    for &version in &Version::ALL[..3] {
        for &(id, payload) in ROOTS {
            let bytes = root(id, payload, version.protocol() == 763);
            let decoded = RegistryData::decode(&bytes, version, Limits::default());
            assert_eq!(decoded.is_ok(), id == 10, "{version}: root {id}");
            let mut embedded = bytes.clone();
            embedded.push(42);
            let mut reader = Reader::new(&embedded, Limits::default());
            let read = RegistryData::read(&mut reader, version);
            assert_eq!(read.is_ok(), id == 10);
            if id == 10 {
                assert_eq!(reader.remaining(), &[42]);
                assert_eq!(
                    decoded.unwrap().encode(version, Limits::default()).unwrap(),
                    bytes
                );
            }
        }
        for tag in values() {
            let compound = matches!(tag, Tag::Compound(_));
            assert_eq!(
                RegistryData::Legacy(Nbt::anonymous(tag))
                    .encode(version, Limits::default())
                    .is_ok(),
                compound
            );
        }
    }
}
#[test]
fn join_game_763_rejects_noncompound_embedded_registry_roots() {
    for &(id, payload) in ROOTS {
        // Entity ID, hardcore, current/previous game mode, empty world-name list.
        let mut bytes = vec![0, 0, 0, 0, 0, 0, 255, 0];
        bytes.extend(root(id, payload, true));
        bytes.extend(b"\x0etest:dimension\x0atest:world");
        // Seed, max players/view/simulation distance, four flags, absent death,
        // and zero portal cooldown. All remain independent of Rustwire's writer.
        bytes.extend([0; 17]);
        let result = JoinGame::decode(&bytes, Version::V1_20, Limits::default());
        assert_eq!(result.is_ok(), id == 10, "root {id}");
        if id == 10 {
            let joined = result.unwrap();
            assert!(matches!(
                joined.dimension_codec.unwrap().root,
                Tag::Compound(_)
            ));
            assert_eq!(joined.spawn.dimension_name, "test:world");
            for end in 0..bytes.len() {
                assert!(
                    JoinGame::decode(&bytes[..end], Version::V1_20, Limits::default()).is_err()
                );
            }
        }
    }
}
#[test]
fn modern_registry_entries_still_accept_every_nonend_nbt_root() {
    for &version in &Version::ALL[3..] {
        for ((id, payload), tag) in ROOTS[1..].iter().zip(values()) {
            let expected = RegistryData::Entries {
                registry: "a".into(),
                entries: vec![RegistryEntry {
                    key: "b".into(),
                    data: Some(Nbt::anonymous(tag)),
                }],
            };
            let mut bytes = vec![1, b'a', 1, 1, b'b', 1];
            bytes.extend(root(*id, payload, false));
            assert_eq!(
                RegistryData::decode(&bytes, version, Limits::default()).unwrap(),
                expected
            );
            assert_eq!(expected.encode(version, Limits::default()).unwrap(), bytes);
        }
    }
}
