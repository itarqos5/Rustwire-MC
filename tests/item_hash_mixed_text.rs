//! Modern text hashes normalize mixed NBT list entries without changing wire NBT.
use rustwire_mc::{
    nbt::{Nbt, Tag, TagType},
    packet::{inventory::*, item_hash::hash_component},
    Error, Limits, Version,
};

fn literal(value: &str) -> Tag {
    Tag::String(value.into())
}
fn wrapped(value: Tag) -> Tag {
    Tag::Compound(vec![("".into(), value)])
}
fn compound_list(elements: Vec<Tag>) -> Tag {
    Tag::List {
        element_type: TagType::Compound,
        elements,
    }
}
fn component(root: Tag) -> Component {
    Component {
        name: "custom_name",
        value: ComponentValue::Nbt(Nbt::anonymous(root)),
    }
}
fn slot(value: Component) -> Slot {
    Slot::Item(ItemStack {
        item_id: 1,
        count: 1,
        data: ItemData::Components(ComponentPatch {
            added: vec![value],
            removed: vec![],
        }),
    })
}
fn hash(root: Tag) -> rustwire_mc::Result<i32> {
    hash_component(&component(root), Version::V26_2, Limits::default())
}

fn hex(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn fixtures() -> Vec<(&'static str, &'static str, Vec<u8>, i32)> {
    include_str!("fixtures/mixed-text-component-hash.tsv")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('|').collect();
            assert_eq!(fields.len(), 4);
            (
                fields[0],
                fields[1],
                hex(fields[2]),
                fields[3].parse().unwrap(),
            )
        })
        .collect()
}

fn incoming(protocol: i32, name: &str, payload: &[u8]) -> Vec<u8> {
    // Component IDs checked against all seven unchanged pinned research schemas.
    let ids = match protocol {
        770..=773 => [5, 6, 8, 46],
        774 => [6, 9, 11, 53],
        775..=776 => [6, 9, 11, 55],
        _ => unreachable!(),
    };
    let index = match name {
        "custom_name" => 0,
        "item_name" => 1,
        "lore" => 2,
        "written_book_content" => 3,
        _ => panic!("unexpected fixture component"),
    };
    // Synthetic ordinary stack: count=1, item=1, one added component, no removals.
    let mut bytes = vec![1, 1, 1, 0, ids[index]];
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn official_component_bytes_decode_and_hash_in_all_seven_families() {
    let fixtures = fixtures();
    assert_eq!(fixtures.len(), 35);
    for protocol in 770..=776 {
        let version = Version::from_protocol(protocol).unwrap();
        for (case, name, payload, expected) in &fixtures {
            let bytes = incoming(protocol, name, payload);
            let decoded = Slot::decode(&bytes, version, Limits::default()).unwrap();
            assert_eq!(
                decoded.encode(version, Limits::default()).unwrap(),
                bytes,
                "raw bytes changed: {protocol} {case}"
            );
            let hashed = HashedItemStack::from_slot(&decoded, version, Limits::default())
                .unwrap()
                .unwrap();
            assert_eq!(
                hashed.components,
                vec![(*name, *expected)],
                "{protocol} {case}"
            );
        }
    }
}

#[test]
fn official_invalid_text_and_out_of_subset_controls_stay_unsupported() {
    for (table, count) in [
        (include_str!("fixtures/mixed-text-component-reject.tsv"), 8),
        (
            include_str!("fixtures/mixed-text-component-unsupported.tsv"),
            4,
        ),
    ] {
        let lines: Vec<_> = table
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        assert_eq!(lines.len(), count);
        for protocol in 770..=776 {
            let version = Version::from_protocol(protocol).unwrap();
            for line in &lines {
                let fields: Vec<_> = line.split('|').collect();
                let bytes = incoming(protocol, fields[1], &hex(fields[2]));
                let decoded = Slot::decode(&bytes, version, Limits::default()).unwrap();
                assert_eq!(decoded.encode(version, Limits::default()).unwrap(), bytes);
                assert!(matches!(
                    HashedItemStack::from_slot(&decoded, version, Limits::default()),
                    Err(Error::Unsupported(_))
                ));
            }
        }
    }
}

#[test]
fn truncated_or_trailing_component_slots_are_rejected() {
    for protocol in 770..=776 {
        let version = Version::from_protocol(protocol).unwrap();
        for (case, name, payload, _) in fixtures() {
            let mut bytes = incoming(protocol, name, &payload);
            for end in 0..bytes.len() {
                assert!(
                    Slot::decode(&bytes[..end], version, Limits::default()).is_err(),
                    "{protocol} {case} prefix {end}"
                );
            }
            bytes.push(0);
            assert!(Slot::decode(&bytes, version, Limits::default()).is_err());
        }
    }
}

#[test]
fn duplicate_empty_keys_use_the_last_value_at_list_boundaries_only() {
    let duplicate = Tag::Compound(vec![
        ("".into(), literal("discarded")),
        ("".into(), literal("a")),
    ]);
    assert_eq!(
        hash(compound_list(vec![duplicate])).unwrap(),
        hash(literal("a")).unwrap()
    );
    // A second distinct key means this is a real map, even when duplicates exist.
    let ordinary = Tag::Compound(vec![
        ("".into(), literal("discarded")),
        ("text".into(), literal("x")),
        ("".into(), literal("a")),
    ]);
    assert!(matches!(
        hash(compound_list(vec![ordinary])),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn root_wrappers_and_double_wrapped_real_maps_remain_unsupported() {
    for root in [
        wrapped(literal("a")),
        compound_list(vec![Tag::Compound(vec![])]),
        compound_list(vec![wrapped(wrapped(literal("a")))]),
        Tag::Compound(vec![
            ("text".into(), literal("x")),
            (
                "extra".into(),
                compound_list(vec![wrapped(wrapped(literal("a")))]),
            ),
        ]),
    ] {
        assert!(matches!(hash(root), Err(Error::Unsupported(_))));
    }
}

#[test]
fn normalization_preserves_unsupported_text_and_style_boundaries() {
    for entries in [
        vec![("translate".into(), literal("item.minecraft.stone"))],
        vec![("text".into(), literal("x")), ("bold".into(), Tag::Byte(2))],
        vec![
            ("text".into(), literal("x")),
            ("click_event".into(), Tag::Compound(vec![])),
        ],
        vec![
            ("text".into(), literal("x")),
            ("unknown_style".into(), Tag::Byte(1)),
        ],
    ] {
        assert!(matches!(
            hash(compound_list(vec![wrapped(Tag::Compound(entries))])),
            Err(Error::Unsupported(_))
        ));
    }
    assert!(hash(compound_list(vec![])).is_err());
    assert!(hash(Tag::Compound(vec![
        ("text".into(), literal("x")),
        ("extra".into(), compound_list(vec![])),
    ]))
    .is_err());
}

#[test]
fn discarded_duplicate_values_still_consume_raw_budgets() {
    for (limits, discarded) in [
        (
            Limits {
                max_string_chars: 1,
                ..Limits::default()
            },
            literal("too long"),
        ),
        (
            Limits {
                max_nbt_depth: 3,
                ..Limits::default()
            },
            wrapped(wrapped(literal("x"))),
        ),
        (
            Limits {
                max_nbt_nodes: 4,
                ..Limits::default()
            },
            Tag::Compound(vec![("a".into(), literal("x")), ("b".into(), literal("y"))]),
        ),
    ] {
        let value = component(compound_list(vec![Tag::Compound(vec![
            ("".into(), discarded),
            ("".into(), literal("a")),
        ])]));
        assert_eq!(
            hash_component(&value, Version::V26_2, Limits::default()).unwrap(),
            hash(literal("a")).unwrap()
        );
        assert!(matches!(
            hash_component(&value, Version::V26_2, limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            HashedItemStack::from_slot(&slot(value), Version::V26_2, limits),
            Err(Error::Limit(_))
        ));
    }
}

#[test]
fn wrappers_do_not_hide_depth_node_byte_or_collection_limits() {
    let value = component(compound_list(vec![wrapped(literal("a"))]));
    for limits in [
        Limits {
            max_packet: 4,
            ..Limits::default()
        },
        Limits {
            max_nbt_depth: 1,
            ..Limits::default()
        },
        Limits {
            max_nbt_nodes: 2,
            ..Limits::default()
        },
        Limits {
            max_collection: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            hash_component(&value, Version::V26_2, limits),
            Err(Error::Limit(_))
        ));
        assert!(HashedItemStack::from_slot(&slot(value.clone()), Version::V26_2, limits).is_err());
    }
    let malformed = component(compound_list(vec![literal("a")]));
    assert!(matches!(
        hash_component(&malformed, Version::V26_2, Limits::default()),
        Err(Error::Invalid(_))
    ));
}
