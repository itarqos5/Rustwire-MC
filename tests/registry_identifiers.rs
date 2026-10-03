use rustwire_mc::{
    nbt::{Nbt, Tag, TagType},
    registry::{RegistryData, RegistryEntry, RegistryStore},
    Error, Limits, Version,
};

const REGISTRIES: [&str; 3] = [
    "dimension_type",
    ":dimension_type",
    "minecraft:dimension_type",
];
const ENTRIES: [&str; 3] = ["overworld", ":overworld", "minecraft:overworld"];

fn dimension(height: i32) -> Nbt {
    Nbt::anonymous(Tag::Compound(vec![
        ("min_y".into(), Tag::Int(0)),
        ("height".into(), Tag::Int(height)),
    ]))
}

fn modern(registry: &str, key: &str, data: Option<Nbt>) -> RegistryData {
    RegistryData::Entries {
        registry: registry.into(),
        entries: vec![RegistryEntry {
            key: key.into(),
            data,
        }],
    }
}

fn legacy_entry(id: i32, key: &str, height: i32) -> Tag {
    Tag::Compound(vec![
        ("id".into(), Tag::Int(id)),
        ("name".into(), Tag::String(key.into())),
        ("element".into(), dimension(height).root),
    ])
}

fn legacy_registry(entries: Vec<Tag>) -> Tag {
    Tag::Compound(vec![(
        "value".into(),
        Tag::List {
            element_type: TagType::Compound,
            elements: entries,
        },
    )])
}

fn legacy(registries: Vec<(&str, Vec<Tag>)>) -> RegistryData {
    RegistryData::Legacy(Nbt::anonymous(Tag::Compound(
        registries
            .into_iter()
            .map(|(key, entries)| (key.into(), legacy_registry(entries)))
            .collect(),
    )))
}

#[test]
fn modern_default_namespace_aliases_resolve_without_rewriting_wire_data() {
    // ResourceLocation's default namespace is independently established by
    // RegistryTagsOracle.java and docs/validation/registry-tags-oracle.json.
    for &version in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        for registry in REGISTRIES {
            for key in ENTRIES {
                let packet = RegistryData::Entries {
                    registry: registry.into(),
                    entries: vec![
                        RegistryEntry {
                            key: key.into(),
                            data: Some(dimension(384)),
                        },
                        RegistryEntry {
                            key: "omitted".into(),
                            data: None,
                        },
                        RegistryEntry {
                            key: "integer".into(),
                            data: Some(Nbt::anonymous(Tag::Int(7))),
                        },
                    ],
                };
                let bytes = packet.encode(version, Limits::default()).unwrap();
                let decoded = RegistryData::decode(&bytes, version, Limits::default()).unwrap();
                assert_eq!(decoded, packet);
                assert_eq!(decoded.encode(version, Limits::default()).unwrap(), bytes);
                let mut store = RegistryStore::default();
                store.apply(decoded).unwrap();
                assert_eq!(store.dimension_by_id(0).unwrap().height, 384);
                for registry_alias in REGISTRIES {
                    let entries = store.get(registry_alias).unwrap();
                    for key_alias in ENTRIES {
                        let (id, entry) = entries.by_key(key_alias).unwrap();
                        assert_eq!(id, 0);
                        assert_eq!(entry.key, key);
                        assert_eq!(store.dimension_by_key(key_alias).unwrap().height, 384);
                    }
                    assert_eq!(entries.by_key(":omitted").unwrap().0, 1);
                    assert!(entries
                        .by_key("minecraft:omitted")
                        .unwrap()
                        .1
                        .data
                        .is_none());
                    assert_eq!(
                        entries.by_key("minecraft:integer").unwrap().1.data,
                        Some(Nbt::anonymous(Tag::Int(7)))
                    );
                    assert!(entries.by_key("custom:overworld").is_none());
                }
                assert!(store.get("custom:dimension_type").is_none());
                assert!(matches!(
                    store.dimension_by_id(1).unwrap_err(),
                    Error::State(
                        "dimension data omitted by known packs; supply matching pack data"
                    )
                ));
            }
        }
    }
}

#[test]
fn modern_alias_duplicates_are_rejected_atomically_but_preserved_on_wire() {
    for aliases in [ENTRIES, ["", ":", "minecraft:"]] {
        for (i, first) in aliases.iter().enumerate() {
            for second in &aliases[i..] {
                let packet = RegistryData::Entries {
                    registry: ":dimension_type".into(),
                    entries: vec![
                        RegistryEntry {
                            key: (*first).into(),
                            data: None,
                        },
                        RegistryEntry {
                            key: (*second).into(),
                            data: Some(Nbt::anonymous(Tag::Int(7))),
                        },
                    ],
                };
                for &version in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
                    let bytes = packet.encode(version, Limits::default()).unwrap();
                    assert_eq!(
                        RegistryData::decode(&bytes, version, Limits::default()).unwrap(),
                        packet
                    );
                }
                let mut store = RegistryStore::default();
                store
                    .apply(modern("dimension_type", "overworld", Some(dimension(384))))
                    .unwrap();
                let before = store.clone();
                assert!(matches!(
                    store.apply(packet),
                    Err(Error::Invalid("duplicate registry key"))
                ));
                assert_eq!(store, before);
            }
        }
    }
}

#[test]
fn equivalent_registry_packets_replace_one_registry_across_spellings() {
    let mut store = RegistryStore::default();
    store
        .apply(modern("custom:dimension_type", "overworld", None))
        .unwrap();
    for (i, registry) in REGISTRIES.iter().cycle().take(6).enumerate() {
        let height = 16 * (i as i32 + 1);
        if i % 2 == 0 {
            store
                .apply(modern(registry, "overworld", Some(dimension(height))))
                .unwrap();
        } else {
            store
                .apply(legacy(vec![(
                    registry,
                    vec![legacy_entry(0, ":overworld", height)],
                )]))
                .unwrap();
        }
        assert_eq!(store.registries.len(), 2);
        assert!(store.registries.contains_key(*registry));
        for alias in REGISTRIES {
            assert_eq!(store.get(alias).unwrap().entries.len(), 1);
        }
        for alias in ENTRIES {
            assert_eq!(store.dimension_by_key(alias).unwrap().height, height);
        }
    }
    assert!(store
        .get("custom:dimension_type")
        .unwrap()
        .by_id(0)
        .unwrap()
        .data
        .is_none());
}

#[test]
fn fixed_modern_wire_fixture_preserves_aliases_and_detects_duplicate_identity() {
    // Hand-authored wire fixture: default-namespace registry, omitted entry,
    // and a present Int NBT payload. This is not encoder-generated evidence.
    let bytes = b"\x08registry\x02\x08:omitted\x00\x11minecraft:present\x01\x03\x00\x00\x00\x07";
    let duplicate = b"\x08registry\x02\x07present\x00\x08:present\x00";
    for &version in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        let decoded = RegistryData::decode(bytes, version, Limits::default()).unwrap();
        assert_eq!(decoded.encode(version, Limits::default()).unwrap(), bytes);
        let mut store = RegistryStore::default();
        store.apply(decoded).unwrap();
        let registry = store.get("minecraft:registry").unwrap();
        assert!(registry.by_key("omitted").unwrap().1.data.is_none());
        assert_eq!(
            registry.by_key(":present").unwrap().1.data,
            Some(Nbt::anonymous(Tag::Int(7)))
        );
        for end in 0..bytes.len() {
            assert!(RegistryData::decode(&bytes[..end], version, Limits::default()).is_err());
        }
        let before = store.clone();
        let decoded = RegistryData::decode(duplicate, version, Limits::default()).unwrap();
        assert_eq!(
            decoded.encode(version, Limits::default()).unwrap(),
            duplicate
        );
        assert!(matches!(
            store.apply(decoded),
            Err(Error::Invalid("duplicate registry key"))
        ));
        assert_eq!(store, before);
    }
}

#[test]
fn legacy_alias_identifiers_retain_nbt_wire_data() {
    for &version in Version::ALL.iter().filter(|v| v.protocol() < 766) {
        for registry in REGISTRIES {
            for key in ENTRIES {
                let mut packet = legacy(vec![(registry, vec![legacy_entry(7, key, 384)])]);
                if version.protocol() == 763 {
                    let RegistryData::Legacy(nbt) = &mut packet else {
                        unreachable!()
                    };
                    nbt.name = Some("".into());
                }
                let bytes = packet.encode(version, Limits::default()).unwrap();
                let decoded = RegistryData::decode(&bytes, version, Limits::default()).unwrap();
                assert_eq!(decoded, packet);
                assert_eq!(decoded.encode(version, Limits::default()).unwrap(), bytes);
                let mut store = RegistryStore::default();
                store.apply(decoded).unwrap();
                assert!(store.registries.contains_key(registry));
                assert_eq!(store.dimension_by_id(7).unwrap().height, 384);
                assert_eq!(
                    store
                        .get("minecraft:dimension_type")
                        .unwrap()
                        .by_key("minecraft:overworld")
                        .unwrap()
                        .1
                        .key,
                    key
                );
            }
        }
    }
}

#[test]
fn replacement_cleans_equivalent_public_map_keys_and_keeps_latest_spelling() {
    // Public map edits remain supported, without a hidden index that could
    // become stale. Applying a packet replaces all equivalent old map keys.
    let mut store = RegistryStore::default();
    store
        .apply(modern(
            ":dimension_type",
            ":overworld",
            Some(dimension(384)),
        ))
        .unwrap();
    let registry = store.get("dimension_type").unwrap().clone();
    for alias in REGISTRIES {
        store.registries.insert(alias.into(), registry.clone());
    }
    store
        .registries
        .insert("custom:dimension_type".into(), registry);
    store
        .apply(modern("dimension_type", "overworld", Some(dimension(16))))
        .unwrap();
    assert_eq!(store.registries.len(), 2);
    assert!(store.registries.contains_key("dimension_type"));
    for alias in REGISTRIES {
        assert_eq!(store.get(alias).unwrap().entries.len(), 1);
    }
    assert_eq!(store.dimension_by_id(0).unwrap().height, 16);
    assert_eq!(
        store
            .get("custom:dimension_type")
            .unwrap()
            .by_key("overworld")
            .unwrap()
            .1
            .data,
        Some(dimension(384))
    );
}

#[test]
fn legacy_aliases_use_explicit_ids_and_reject_duplicate_names_and_ids_atomically() {
    for registry in REGISTRIES {
        for key in ENTRIES {
            let mut store = RegistryStore::default();
            store
                .apply(legacy(vec![(registry, vec![legacy_entry(7, key, 384)])]))
                .unwrap();
            assert_eq!(store.dimension_by_id(7).unwrap().height, 384);
            for alias in ENTRIES {
                assert_eq!(
                    store
                        .get("minecraft:dimension_type")
                        .unwrap()
                        .by_key(alias)
                        .unwrap()
                        .0,
                    7
                );
            }
            let before = store.clone();
            for alias in ENTRIES {
                assert!(matches!(
                    store.apply(legacy(vec![(
                        registry,
                        vec![legacy_entry(1, key, 16), legacy_entry(2, alias, 32),]
                    )])),
                    Err(Error::Invalid("duplicate registry entry"))
                ));
                assert_eq!(store, before);
            }
            assert!(matches!(
                store.apply(legacy(vec![(
                    registry,
                    vec![
                        legacy_entry(1, "custom:first", 16),
                        legacy_entry(1, "custom:second", 32),
                    ]
                )])),
                Err(Error::Invalid("duplicate registry entry"))
            ));
            assert_eq!(store, before);
            for alias in REGISTRIES {
                assert!(matches!(
                    store.apply(legacy(vec![
                        ("custom:new", vec![]),
                        (registry, vec![]),
                        (alias, vec![]),
                    ])),
                    Err(Error::Invalid("duplicate registry"))
                ));
                assert_eq!(store, before);
            }
        }
    }
}

#[test]
fn empty_path_aliases_and_custom_namespaces_remain_distinct() {
    let mut store = RegistryStore::default();
    store.apply(modern("", "", None)).unwrap();
    store.apply(modern("custom:", "custom:", None)).unwrap();
    for alias in ["", ":", "minecraft:"] {
        assert_eq!(store.get(alias).unwrap().by_key(alias).unwrap().0, 0);
        assert!(store.get(alias).unwrap().by_key("custom:").is_none());
    }
    assert!(store.get("custom:").unwrap().by_key("").is_none());
}

#[test]
fn legacy_nbt_field_duplicates_keep_existing_last_key_semantics() {
    let entry = Tag::Compound(vec![
        ("id".into(), Tag::Int(-1)),
        ("id".into(), Tag::Int(7)),
        ("name".into(), Tag::String("custom:old".into())),
        ("name".into(), Tag::String(":overworld".into())),
        ("element".into(), Tag::Int(1)),
        ("element".into(), dimension(384).root),
    ]);
    let mut registry = legacy_registry(vec![entry]);
    if let Tag::Compound(fields) = &mut registry {
        fields.insert(0, ("value".into(), Tag::Int(1)));
    }
    let mut store = RegistryStore::default();
    store
        .apply(RegistryData::Legacy(Nbt::anonymous(Tag::Compound(vec![(
            "dimension_type".into(),
            registry,
        )]))))
        .unwrap();
    assert_eq!(store.dimension_by_id(7).unwrap().height, 384);
    assert_eq!(store.dimension_by_key("overworld").unwrap().height, 384);
    assert!(store
        .get("dimension_type")
        .unwrap()
        .by_key("custom:old")
        .is_none());
}
