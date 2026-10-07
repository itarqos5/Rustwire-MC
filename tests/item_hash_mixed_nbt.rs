use rustwire_mc::{
    nbt::{Nbt, RootFormat, Tag, TagType},
    packet::{
        inventory::{
            Component, ComponentPatch, ComponentValue, HashedItemStack, ItemData, ItemStack, Slot,
        },
        item_hash::{hash_component, hash_nbt},
    },
    Error, Limits, Version,
};

fn fixtures() -> Vec<(&'static str, Vec<u8>, i32)> {
    include_str!("fixtures/mixed-nbt-hash.tsv")
        .lines()
        .filter(|s| !s.starts_with('#') && !s.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('|').collect();
            assert_eq!(fields.len(), 3);
            let (pairs, remainder) = fields[1].as_bytes().as_chunks::<2>();
            assert!(remainder.is_empty());
            let bytes = pairs
                .iter()
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            (fields[0], bytes, fields[2].parse().unwrap())
        })
        .collect()
}

#[test]
fn official_logical_nbt_hashes_preserve_lossless_binary_roundtrips() {
    let fixtures = fixtures();
    assert_eq!(fixtures.len(), 14);
    for (name, bytes, expected) in fixtures {
        let nbt = Nbt::decode(&bytes, RootFormat::Anonymous, Limits::default()).unwrap();
        assert_eq!(
            nbt.encode(RootFormat::Anonymous, Limits::default())
                .unwrap(),
            bytes,
            "{name}"
        );
        assert_eq!(
            hash_nbt(&nbt, Limits::default()).unwrap(),
            expected,
            "{name}"
        );
    }
}

#[test]
fn both_supported_nbt_components_and_predicted_stacks_use_logical_values() {
    for protocol in 770..=776 {
        let version = Version::from_protocol(protocol).unwrap();
        for (case, bytes, expected) in fixtures() {
            let nbt = Nbt::decode(&bytes, RootFormat::Anonymous, Limits::default()).unwrap();
            for name in ["custom_data", "bucket_entity_data"] {
                let component = Component {
                    name,
                    value: ComponentValue::Nbt(nbt.clone()),
                };
                assert_eq!(
                    hash_component(&component, version, Limits::default()).unwrap(),
                    expected,
                    "{protocol} {name} {case}"
                );
                let slot = Slot::Item(ItemStack {
                    item_id: 1,
                    count: 1,
                    data: ItemData::Components(ComponentPatch {
                        added: vec![component],
                        removed: vec![],
                    }),
                });
                let hashed = HashedItemStack::from_slot(&slot, version, Limits::default())
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    hashed.components,
                    vec![(name, expected)],
                    "{protocol} {name} {case}"
                );
            }
        }
    }
}

#[test]
fn wrapper_normalization_cannot_bypass_raw_nbt_budgets() {
    let (_, bytes, _) = fixtures()
        .into_iter()
        .find(|r| r.0 == "single_wrapper")
        .unwrap();
    let nbt = Nbt::decode(&bytes, RootFormat::Anonymous, Limits::default()).unwrap();
    for limits in [
        Limits {
            max_packet: bytes.len() - 1,
            ..Limits::default()
        },
        Limits {
            max_nbt_depth: 2,
            ..Limits::default()
        },
        Limits {
            max_nbt_nodes: 3,
            ..Limits::default()
        },
        Limits {
            max_collection: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(hash_nbt(&nbt, limits), Err(Error::Limit(_))));
        let component = Component {
            name: "custom_data",
            value: ComponentValue::Nbt(nbt.clone()),
        };
        assert!(hash_component(&component, Version::V26_2, limits).is_err());
        let slot = Slot::Item(ItemStack {
            item_id: 1,
            count: 1,
            data: ItemData::Components(ComponentPatch {
                added: vec![component],
                removed: vec![],
            }),
        });
        assert!(HashedItemStack::from_slot(&slot, Version::V26_2, limits).is_err());
    }
    let malformed = Nbt::anonymous(Tag::List {
        element_type: TagType::Compound,
        elements: vec![Tag::Byte(1), Tag::String("a".into())],
    });
    assert!(matches!(
        hash_nbt(&malformed, Limits::default()),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn binary_fixture_prefixes_remain_rejected() {
    for (name, bytes, _) in fixtures() {
        for end in 0..bytes.len() {
            assert!(
                Nbt::decode(&bytes[..end], RootFormat::Anonymous, Limits::default()).is_err(),
                "{name} prefix {end}"
            );
        }
    }
}

#[test]
fn overwritten_wrapper_values_still_count_against_raw_limits() {
    let limits_and_discarded = [
        (
            Limits {
                max_string_chars: 1,
                ..Limits::default()
            },
            Tag::String("too long".into()),
        ),
        (
            Limits {
                max_nbt_depth: 3,
                ..Limits::default()
            },
            Tag::Compound(vec![(
                "a".into(),
                Tag::Compound(vec![("b".into(), Tag::Byte(2))]),
            )]),
        ),
        (
            Limits {
                max_nbt_nodes: 5,
                ..Limits::default()
            },
            Tag::Compound(vec![("a".into(), Tag::Byte(2)), ("b".into(), Tag::Byte(3))]),
        ),
    ];
    let expected = fixtures()
        .into_iter()
        .find(|row| row.0 == "single_wrapper")
        .unwrap()
        .2;
    for (limits, discarded) in limits_and_discarded {
        // Last-write-wins makes the first value semantically invisible. It is
        // still present on the wire and must be charged before normalization.
        let nbt = Nbt::anonymous(Tag::Compound(vec![(
            "x".into(),
            Tag::List {
                element_type: TagType::Compound,
                elements: vec![Tag::Compound(vec![
                    ("".into(), discarded),
                    ("".into(), Tag::Byte(1)),
                ])],
            },
        )]));
        assert_eq!(hash_nbt(&nbt, Limits::default()).unwrap(), expected);
        assert!(matches!(hash_nbt(&nbt, limits), Err(Error::Limit(_))));
        let raw = nbt
            .encode(RootFormat::Anonymous, Limits::default())
            .unwrap();
        assert!(matches!(
            Nbt::decode(&raw, RootFormat::Anonymous, limits),
            Err(Error::Limit(_))
        ));
        for protocol in 770..=776 {
            let version = Version::from_protocol(protocol).unwrap();
            for name in ["custom_data", "bucket_entity_data"] {
                let component = Component {
                    name,
                    value: ComponentValue::Nbt(nbt.clone()),
                };
                assert_eq!(
                    hash_component(&component, version, Limits::default()).unwrap(),
                    expected
                );
                assert!(matches!(
                    hash_component(&component, version, limits),
                    Err(Error::Limit(_))
                ));
                let slot = Slot::Item(ItemStack {
                    item_id: 1,
                    count: 1,
                    data: ItemData::Components(ComponentPatch {
                        added: vec![component],
                        removed: vec![],
                    }),
                });
                assert!(matches!(
                    HashedItemStack::from_slot(&slot, version, limits),
                    Err(Error::Limit(_))
                ));
            }
        }
    }
}
