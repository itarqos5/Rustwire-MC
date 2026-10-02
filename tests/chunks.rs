use rustwire_mc::{
    chunk::*,
    codec::{Reader, Writer},
    nbt::{Nbt, RootFormat, Tag, TagType},
    registry::*,
    Limits, Version,
};
fn empty_nbt(version: Version) -> Nbt {
    Nbt {
        name: if version.protocol() < 764 {
            Some("".into())
        } else {
            None
        },
        root: Tag::Compound(vec![]),
    }
}
fn chunk(version: Version) -> ChunkData {
    ChunkData {
        x: -2,
        z: 3,
        heightmaps: if version.protocol() < 770 {
            Heightmaps::Nbt(empty_nbt(version))
        } else {
            Heightmaps::Typed(vec![Heightmap {
                kind: 4,
                data: vec![0; 37],
            }])
        },
        sections: vec![ChunkSection {
            non_air_count: 0,
            fluid_count: if version.protocol() >= 775 {
                Some(0)
            } else {
                None
            },
            blocks: PaletteContainer::single(ContainerKind::Blocks, 0),
            biomes: PaletteContainer::single(ContainerKind::Biomes, 1),
        }],
        block_entities: vec![BlockEntity {
            local_x: 15,
            local_z: 2,
            y: -64,
            kind: 3,
            data: Some(empty_nbt(version)),
        }],
        light: LightData {
            sky_mask: vec![1],
            block_mask: vec![],
            empty_sky_mask: vec![2],
            empty_block_mask: vec![3],
            sky_arrays: vec![vec![0xff; 2048]],
            block_arrays: vec![],
        },
    }
}
#[test]
fn chunks_roundtrip_all_release_families() {
    for &version in Version::ALL {
        let c = chunk(version);
        let bytes = c.encode(version, Limits::default()).unwrap();
        assert_eq!(
            ChunkData::decode(&bytes, version, 1, Limits::default()).unwrap(),
            c,
            "{version}"
        );
    }
}
#[test]
fn golden_singleton_section_version_boundaries() {
    for (version, expected) in [
        (Version::V1_20, vec![0, 0, 0, 0, 0, 0, 1, 0]),
        (Version::V1_21_5, vec![0, 0, 0, 0, 0, 1]),
        (Version::V26_1, vec![0, 0, 0, 0, 0, 0, 0, 1]),
    ] {
        let section = chunk(version).sections.remove(0);
        let mut w = Writer::new();
        section.write(&mut w, version, Limits::default()).unwrap();
        assert_eq!(w.as_slice(), expected);
        let mut r = Reader::new(&expected, Limits::default());
        assert_eq!(ChunkSection::read(&mut r, version).unwrap(), section);
        r.finish().unwrap();
    }
}
#[test]
fn indirect_and_direct_no_span_palette_roundtrips() {
    for kind in [ContainerKind::Blocks, ContainerKind::Biomes] {
        let variants = if kind == ContainerKind::Blocks {
            vec![2, 19, 257]
        } else {
            vec![2, 5, 9]
        };
        for count in variants {
            let values: Vec<u32> = (0..kind.entries()).map(|i| (i % count) as u32).collect();
            let p = PaletteContainer::from_values(kind, &values, 15).unwrap();
            for &version in Version::ALL {
                let mut w = Writer::new();
                p.write(&mut w, version, Limits::default()).unwrap();
                let mut r = Reader::new(w.as_slice(), Limits::default());
                let decoded = PaletteContainer::read(&mut r, version, kind).unwrap();
                r.finish().unwrap();
                assert_eq!(
                    decoded.values().collect::<Option<Vec<_>>>().unwrap(),
                    values
                );
                assert_eq!(decoded, p);
            }
        }
    }
}
#[test]
fn golden_five_bit_no_span_and_coordinate_order() {
    let values: Vec<u32> = (0..4096).map(|i| (i % 17) as u32).collect();
    let p = PaletteContainer::from_values(ContainerKind::Blocks, &values, 15).unwrap();
    assert_eq!(p.bits_per_entry, 5);
    assert_eq!(p.data.len(), 342); // ceil(4096 / floor(64/5))
    assert_eq!(
        p.data[0],
        (0..12).map(|n| (n as u64) << (5 * n)).sum::<u64>()
    );
    assert_eq!(p.get_xyz(1, 0, 0), Some(1));
    assert_eq!(p.get_xyz(0, 0, 1), Some(16));
    assert_eq!(p.get_xyz(0, 1, 0), Some(1));
    assert_eq!(p.get_xyz(16, 0, 0), None);
}
#[test]
fn malformed_palettes_and_lengths_rejected() {
    for bytes in [
        vec![1],
        vec![32],
        vec![0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        vec![4, 0],
        vec![4, 1, 0, 0],
    ] {
        let mut r = Reader::new(&bytes, Limits::default());
        assert!(PaletteContainer::read(&mut r, Version::V1_20, ContainerKind::Blocks).is_err());
    }
    let mut bad = PaletteContainer::from_values(
        ContainerKind::Blocks,
        &(0..4096).map(|i| (i % 2) as u32).collect::<Vec<_>>(),
        15,
    )
    .unwrap();
    bad.data[0] |= 15;
    assert!(bad
        .write(&mut Writer::new(), Version::V1_21_5, Limits::default())
        .is_err());
    assert!(PaletteContainer::from_values(ContainerKind::Blocks, &[0; 4096], 32).is_ok()); // unused direct bits
    assert!(PaletteContainer::from_values(
        ContainerKind::Blocks,
        &(0..4096).collect::<Vec<_>>(),
        9
    )
    .is_err());
}
#[test]
fn section_count_trailing_bytes_and_light_consistency_rejected() {
    let v = Version::V26_2;
    let c = chunk(v);
    let bytes = c.encode(v, Limits::default()).unwrap();
    assert!(ChunkData::decode(&bytes, v, 2, Limits::default()).is_err());
    for n in [0, 1, 8, bytes.len() - 1] {
        assert!(ChunkData::decode(&bytes[..n], v, 1, Limits::default()).is_err());
    }
    let mut b = bytes.clone();
    b.push(0);
    assert!(ChunkData::decode(&b, v, 1, Limits::default()).is_err());
    let mut bad = c.clone();
    bad.light.sky_mask[0] = 3;
    assert!(bad.encode(v, Limits::default()).is_err());
    let mut bad = c.clone();
    bad.light.sky_arrays[0].pop();
    assert!(bad.encode(v, Limits::default()).is_err());
    let mut bad = c;
    bad.sections[0].fluid_count = None;
    assert!(bad.encode(v, Limits::default()).is_err());
}
fn dimension() -> Nbt {
    Nbt::anonymous(Tag::Compound(vec![
        ("min_y".into(), Tag::Int(-64)),
        ("height".into(), Tag::Int(384)),
        ("logical_height".into(), Tag::Int(384)),
        ("has_skylight".into(), Tag::Byte(1)),
    ]))
}
#[test]
fn modern_registry_optional_data_and_dimension_lookup() {
    let packet = RegistryData::Entries {
        registry: "minecraft:dimension_type".into(),
        entries: vec![
            RegistryEntry {
                key: "minecraft:overworld".into(),
                data: Some(dimension()),
            },
            RegistryEntry {
                key: "custom:omitted".into(),
                data: None,
            },
        ],
    };
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        let b = packet.encode(v, Limits::default()).unwrap();
        assert_eq!(
            RegistryData::decode(&b, v, Limits::default()).unwrap(),
            packet
        );
    }
    let mut store = RegistryStore::default();
    store.apply(packet).unwrap();
    let d = store.dimension_by_id(0).unwrap();
    assert_eq!(d.section_count(), 24);
    assert_eq!(d.min_section_y(), -4);
    assert!(store.dimension_by_id(1).is_err());
    assert_eq!(store.dimension_by_key("minecraft:overworld").unwrap(), d);
}
#[test]
fn legacy_registry_explicit_ids_and_atomic_rejection() {
    let entry = Tag::Compound(vec![
        ("name".into(), Tag::String("minecraft:overworld".into())),
        ("id".into(), Tag::Int(7)),
        ("element".into(), dimension().root),
    ]);
    let tag = Tag::Compound(vec![(
        "minecraft:dimension_type".into(),
        Tag::Compound(vec![
            (
                "type".into(),
                Tag::String("minecraft:dimension_type".into()),
            ),
            (
                "value".into(),
                Tag::List {
                    element_type: TagType::Compound,
                    elements: vec![entry],
                },
            ),
        ]),
    )]);
    let mut store = RegistryStore::default();
    for v in [Version::V1_20, Version::V1_20_2, Version::V1_20_3] {
        let mut n = Nbt::anonymous(tag.clone());
        if RootFormat::for_version(v) == RootFormat::Named {
            n.name = Some("".into());
        }
        let packet = RegistryData::Legacy(n);
        let bytes = packet.encode(v, Limits::default()).unwrap();
        assert_eq!(
            RegistryData::decode(&bytes, v, Limits::default()).unwrap(),
            packet
        );
        store.apply(packet).unwrap();
    }
    assert_eq!(store.dimension_by_id(7).unwrap().height, 384);
    assert!(store.dimension_by_id(0).is_err());
    let old = store.clone();
    assert!(store
        .apply(RegistryData::Legacy(Nbt::anonymous(Tag::Byte(0))))
        .is_err());
    assert_eq!(store, old);
}
#[test]
fn registry_duplicate_and_invalid_dimensions_rejected() {
    let mut store = RegistryStore::default();
    let entry = RegistryEntry {
        key: "same:key".into(),
        data: None,
    };
    assert!(store
        .apply(RegistryData::Entries {
            registry: "test:test".into(),
            entries: vec![entry.clone(), entry]
        })
        .is_err());
    assert!(DimensionInfo::from_tag(&Tag::Compound(vec![
        ("min_y".into(), Tag::Int(-63)),
        ("height".into(), Tag::Int(384))
    ]))
    .is_err());
    assert!(RegistryData::decode(
        &[1, b'r', 1, 1, b'e', 1, 0],
        Version::V1_20_5,
        Limits::default()
    )
    .is_err());
}
#[test]
fn independently_encoded_indirect_and_direct_palette_words() {
    // Canonical 4-bit indirect blocks: palette [5,9], indices alternate 0,1.
    let mut bytes = vec![4, 2, 5, 9, 0x80, 0x02]; // VarInt 256 words
    for _ in 0..256 {
        bytes.extend_from_slice(&0x1010_1010_1010_1010u64.to_be_bytes());
    }
    let mut r = Reader::new(&bytes, Limits::default());
    let p = PaletteContainer::read(&mut r, Version::V1_20, ContainerKind::Blocks).unwrap();
    r.finish().unwrap();
    for i in 0..4096 {
        assert_eq!(p.get(i), Some(if i % 2 == 0 { 5 } else { 9 }));
    }
    // 15-bit direct blocks: four entries fit each long, four high padding bits.
    let word = 1u64 | (2u64 << 15) | (3u64 << 30) | (32767u64 << 45);
    let mut bytes = vec![15]; // From 770, no array-length prefix.
    for _ in 0..1024 {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    let mut r = Reader::new(&bytes, Limits::default());
    let p = PaletteContainer::read(&mut r, Version::V1_21_5, ContainerKind::Blocks).unwrap();
    r.finish().unwrap();
    for i in 0..4096 {
        assert_eq!(p.get(i), Some([1, 2, 3, 32767][i % 4]));
    }
}
#[test]
fn heightmap_nine_bit_no_span_storage() {
    let mut map = Heightmap {
        kind: 4,
        data: vec![0; 37],
    };
    for i in 0..256 {
        map.data[i / 7] |= ((i + 1) as i64) << ((i % 7) * 9);
    }
    let heights = map.heights(384).unwrap();
    assert_eq!(heights[0], 1);
    assert_eq!(heights[255], 256);
    map.data[0] = 511;
    assert!(map.heights(384).is_err());
    map.data.pop();
    assert!(map.heights(384).is_err());
}
#[test]
fn paper_1201_exact_singleton_zero_padding_regression() {
    fn padded(version: Version, padding: &[u8]) -> (ChunkData, Vec<u8>) {
        let mut c = chunk(version);
        c.sections = vec![c.sections[0].clone(); 24];
        let original = c.encode(version, Limits::default()).unwrap();
        let mut r = Reader::new(&original, Limits::default());
        r.i32().unwrap();
        r.i32().unwrap();
        Nbt::read(&mut r, RootFormat::for_version(version)).unwrap();
        let offset = r.position();
        let sections = r.bytes(1_000_000).unwrap();
        let mut w = Writer::new();
        w.raw(&original[..offset]);
        w.var_i32((sections.len() + padding.len()) as i32);
        w.raw(sections);
        w.raw(padding);
        w.raw(r.remaining());
        (c, w.into_inner())
    }
    let (expected, bytes) = padded(Version::V1_20, &[0; 24]);
    assert_eq!(
        ChunkData::decode(&bytes, Version::V1_20, 24, Limits::default()).unwrap(),
        expected
    );
    for padding in [vec![0; 23], vec![0; 25], vec![1; 24]] {
        let (_, bytes) = padded(Version::V1_20, &padding);
        assert!(ChunkData::decode(&bytes, Version::V1_20, 24, Limits::default()).is_err());
    }
    let (_, bytes) = padded(Version::V1_20_2, &[0; 24]);
    assert!(ChunkData::decode(&bytes, Version::V1_20_2, 24, Limits::default()).is_err());
}
#[test]
fn light_encoder_budget_is_preflighted() {
    let light = LightData {
        sky_mask: vec![1],
        sky_arrays: vec![vec![0; 2048]],
        ..LightData::default()
    };
    let mut w = Writer::new();
    w.raw(&[7, 8]);
    assert!(light
        .write(
            &mut w,
            Limits {
                max_packet: 32,
                ..Limits::default()
            }
        )
        .is_err());
    assert_eq!(w.as_slice(), &[7, 8]);
    let mut c = chunk(Version::V1_21);
    c.light = light;
    assert!(c
        .encode(
            Version::V1_21,
            Limits {
                max_packet: 1024,
                ..Limits::default()
            }
        )
        .is_err());
}
