use rustwire_mc::{
    chunk::{ContainerKind, LightData, Palette, PaletteContainer},
    codec::{BlockPosition, Reader, Writer},
    nbt::{Nbt, Tag, TagType},
    packet::chunk_updates::*,
    version::{Direction, State},
    Error, Limits, Version,
};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}
// Independently assembled wire bodies. None use the production encoders.
fn light_fixture() -> Vec<u8> {
    let mut b = hex("fe ff ff ff 0f 03 01 00 00 00 00 00 00 00 09 01 00 00 00 00 00 00 00 02 01 00 00 00 00 00 00 00 04 01 00 00 00 00 00 00 00 0d 02 80 10");
    b.extend([0x10; 2048]);
    b.extend([0x80, 0x10]);
    b.extend([0xfe; 2048]);
    b.extend([1, 0x80, 0x10]);
    b.extend([0x32; 2048]);
    b
}
fn entity_fixture(v: Version) -> Vec<u8> {
    let mut b = hex("ff ff ff c0 00 00 2f c0 ac 02 0a");
    if v.protocol() == 763 {
        b.extend([0, 0]);
    }
    b.extend(hex("01 00 01 78 fc 0b 00 01 76 00 00 00 02 00 00 00 01 ff ff ff fe 08 00 01 73 00 08 c0 80 ed a0 bd ed b8 80 00"));
    b
}
fn biome_fixture(v: Version) -> Vec<u8> {
    let mut b = hex("02 00 00 00 03 ff ff ff fe");
    let old = v.protocol() < 770;
    // One singleton and one alternating indirect container.
    b.push(if old { 17 } else { 15 });
    b.extend([0, 0xac, 2]);
    if old {
        b.push(0);
    }
    b.extend([1, 2, 1, 7]);
    if old {
        b.push(1);
    }
    b.extend([0xaa; 8]);
    b.extend(hex("7f ff ff ff 80 00 00 00"));
    // A zero singleton followed by an explicitly unresolved direct palette.
    b.push(if old { 37 } else { 35 });
    b.extend([0, 0]);
    if old {
        b.push(0);
    }
    b.push(4);
    if old {
        b.push(4);
    }
    for _ in 0..4 {
        b.extend(hex("76 54 32 10 76 54 32 10"));
    }
    b
}
fn expected_light() -> UpdateLight {
    UpdateLight {
        chunk_x: -2,
        chunk_z: 3,
        light: LightData {
            sky_mask: vec![9],
            block_mask: vec![2],
            empty_sky_mask: vec![4],
            empty_block_mask: vec![13],
            sky_arrays: vec![vec![0x10; 2048], vec![0xfe; 2048]],
            block_arrays: vec![vec![0x32; 2048]],
        },
    }
}
fn expected_entity(v: Version) -> TileEntityData {
    TileEntityData {
        position: BlockPosition {
            x: -1,
            y: -64,
            z: 2,
        },
        kind: 300,
        data: Some(Nbt {
            name: (v.protocol() == 763).then(|| "".into()),
            root: Tag::Compound(vec![
                ("x".into(), Tag::Byte(-4)),
                ("v".into(), Tag::IntArray(vec![1, -2])),
                ("s".into(), Tag::String("\0😀".into())),
            ]),
        }),
    }
}
fn expected_biomes() -> ChunkBiomes {
    ChunkBiomes {
        chunks: vec![
            ChunkBiomeData {
                chunk_x: -2,
                chunk_z: 3,
                sections: vec![
                    PaletteContainer::single(ContainerKind::Biomes, 300),
                    PaletteContainer {
                        kind: ContainerKind::Biomes,
                        bits_per_entry: 1,
                        palette: Palette::Indirect(vec![1, 7]),
                        data: vec![0xaaaaaaaaaaaaaaaa],
                    },
                ],
            },
            ChunkBiomeData {
                chunk_x: i32::MIN,
                chunk_z: i32::MAX,
                sections: vec![
                    PaletteContainer::single(ContainerKind::Biomes, 0),
                    PaletteContainer {
                        kind: ContainerKind::Biomes,
                        bits_per_entry: 4,
                        palette: Palette::Direct,
                        data: vec![0x7654321076543210; 4],
                    },
                ],
            },
        ],
    }
}

#[test]
fn independent_six_packet_golden_fixtures_all_fourteen_families() {
    let ids = [
        [39, 8, 13, 78, 79, 92],
        [40, 7, 14, 80, 81, 94],
        [40, 7, 14, 82, 83, 96],
        [42, 7, 14, 84, 85, 98],
        [42, 7, 14, 84, 85, 98],
        [43, 7, 14, 88, 89, 105],
        [43, 7, 14, 88, 89, 105],
        [42, 6, 13, 87, 88, 104],
        [42, 6, 13, 87, 88, 104],
        [42, 6, 13, 87, 88, 104],
        [47, 6, 13, 92, 93, 109],
        [47, 6, 13, 92, 93, 109],
        [48, 6, 13, 94, 95, 111],
        [48, 6, 13, 94, 95, 111],
    ];
    for (&v, ids) in Version::ALL.iter().zip(ids) {
        let l = Limits::default();
        let light = UpdateLight::decode(&light_fixture(), v, 2, l).unwrap();
        assert_eq!(light, expected_light(), "{v}");
        let entity = TileEntityData::decode(&entity_fixture(v), v, l).unwrap();
        assert_eq!(entity, expected_entity(v), "{v}");
        let biomes = ChunkBiomes::decode(&biome_fixture(v), v, 2, l).unwrap();
        assert_eq!(biomes, expected_biomes(), "{v}");
        assert_eq!(biomes.chunks[0].sections[1].get_xyz(1, 0, 0), Some(7));
        assert_eq!(biomes.chunks[1].sections[1].get_xyz(0, 0, 1), Some(4));
        assert_eq!(biomes.chunks[1].sections[1].get_xyz(3, 3, 3), Some(7));
        let position = UpdateViewPosition {
            chunk_x: -2,
            chunk_z: 3,
        };
        let distance = UpdateViewDistance { distance: 128 };
        let simulation = SimulationDistance { distance: 6 };
        assert_eq!(
            UpdateViewPosition::decode(&hex("fe ff ff ff 0f 03"), v, l).unwrap(),
            position
        );
        assert_eq!(
            UpdateViewDistance::decode(&[0x80, 1], v, l).unwrap(),
            distance
        );
        assert_eq!(SimulationDistance::decode(&[6], v, l).unwrap(), simulation);
        let packets = [
            light.packet(v, 2, l).unwrap(),
            entity.packet(v, l).unwrap(),
            biomes.packet(v, 2, l).unwrap(),
            position.packet(v, l).unwrap(),
            distance.packet(v, l).unwrap(),
            simulation.packet(v, l).unwrap(),
        ];
        let expected = [
            light_fixture(),
            entity_fixture(v),
            biome_fixture(v),
            hex("fe ff ff ff 0f 03"),
            vec![0x80, 1],
            vec![6],
        ];
        for (i, (packet, bytes)) in packets.iter().zip(expected).enumerate() {
            assert_eq!(packet.id, ids[i], "{v} packet {i}");
            assert_eq!(packet.data, bytes, "{v} packet {i}");
        }
        for name in [
            "update_light",
            "tile_entity_data",
            "chunk_biomes",
            "update_view_position",
            "update_view_distance",
            "simulation_distance",
        ] {
            assert!(v
                .packet_id(State::Play, Direction::Clientbound, name)
                .is_ok());
        }
    }
}

#[test]
fn all_packets_reject_every_truncation_and_trailing_byte() {
    for &v in Version::ALL {
        let l = Limits::default();
        let b = light_fixture();
        for n in 0..b.len() {
            assert!(
                UpdateLight::decode(&b[..n], v, 2, l).is_err(),
                "light {v} {n}"
            );
        }
        let mut trailing = b;
        trailing.push(0);
        assert!(UpdateLight::decode(&trailing, v, 2, l).is_err());
        let b = entity_fixture(v);
        for n in 0..b.len() {
            assert!(
                TileEntityData::decode(&b[..n], v, l).is_err(),
                "entity {v} {n}"
            );
        }
        let mut trailing = b;
        trailing.push(0);
        assert!(TileEntityData::decode(&trailing, v, l).is_err());
        let b = biome_fixture(v);
        for n in 0..b.len() {
            assert!(
                ChunkBiomes::decode(&b[..n], v, 2, l).is_err(),
                "biomes {v} {n}"
            );
        }
        let mut trailing = b;
        trailing.push(0);
        assert!(ChunkBiomes::decode(&trailing, v, 2, l).is_err());
        let b = hex("fe ff ff ff 0f 03");
        for n in 0..b.len() {
            assert!(UpdateViewPosition::decode(&b[..n], v, l).is_err());
        }
        assert!(UpdateViewPosition::decode(&[b.as_slice(), &[0]].concat(), v, l).is_err());
        for bytes in [&[][..], &[0x80][..], &[0x80, 1, 0][..]] {
            assert!(UpdateViewDistance::decode(bytes, v, l).is_err());
            assert!(SimulationDistance::decode(bytes, v, l).is_err());
        }
    }
}

#[test]
fn dimension_context_is_explicit_and_checked() {
    for &v in Version::ALL {
        let l = Limits::default();
        for sections in [0, 1, 3, usize::MAX] {
            assert!(
                ChunkBiomes::decode(&biome_fixture(v), v, sections, l).is_err(),
                "{v} {sections}"
            );
            assert!(expected_biomes().encode(v, sections, l).is_err());
        }
        for sections in [0, 1, usize::MAX] {
            assert!(UpdateLight::decode(&light_fixture(), v, sections, l).is_err());
            assert!(expected_light().encode(v, sections, l).is_err());
        }
        for sections in [1, 16, 24, 64, 96] {
            let empty = ChunkBiomes::default();
            assert_eq!(empty.encode(v, sections, l).unwrap(), [0]);
            assert_eq!(ChunkBiomes::decode(&[0], v, sections, l).unwrap(), empty);
            let empty_light = UpdateLight {
                chunk_x: 0,
                chunk_z: 0,
                light: LightData::default(),
            };
            assert_eq!(empty_light.encode(v, sections, l).unwrap(), [0; 8]);
            assert_eq!(
                UpdateLight::decode(&[0; 8], v, sections, l).unwrap(),
                empty_light
            );
        }
        assert!(ChunkBiomes::decode(&[0], v, 0, l).is_err());
        assert!(ChunkBiomes::default().encode(v, 0, l).is_err());
    }
}

#[test]
fn block_entity_named_anonymous_and_required_compound_boundaries() {
    for &v in Version::ALL {
        let l = Limits::default();
        let mut absent = hex("ff ff ff c0 00 00 2f c0 ac 02 00");
        let mut entity = expected_entity(v);
        entity.data = None;
        assert_eq!(entity.encode(v, l).is_ok(), v.protocol() <= 765);
        assert_eq!(
            TileEntityData::decode(&absent, v, l).is_ok(),
            v.protocol() <= 765
        );
        if v.protocol() <= 765 {
            assert_eq!(entity.encode(v, l).unwrap(), absent);
        }
        // Scalar roots are invalid even in legacy optional-compound packets.
        absent[10] = 1;
        if v.protocol() == 763 {
            absent.extend([0, 0]);
        }
        absent.push(0);
        assert!(TileEntityData::decode(&absent, v, l).is_err());
        entity.data = Some(Nbt::anonymous(Tag::Byte(0)));
        assert!(entity.encode(v, l).is_err());
        let mut invalid = entity_fixture(v);
        invalid.splice(8..10, [0xff, 0xff, 0xff, 0xff, 0x0f]);
        assert!(TileEntityData::decode(&invalid, v, l).is_err());
        entity = expected_entity(v);
        entity.kind = u32::MAX;
        assert!(entity.encode(v, l).is_err());
        for position in [
            BlockPosition {
                x: 33_554_432,
                y: 0,
                z: 0,
            },
            BlockPosition {
                x: 0,
                y: 2048,
                z: 0,
            },
            BlockPosition {
                x: 0,
                y: 0,
                z: -33_554_433,
            },
        ] {
            entity = expected_entity(v);
            entity.position = position;
            assert!(entity.encode(v, l).is_err());
        }
    }
}

fn one_biome_payload(palette: &[u8]) -> Vec<u8> {
    assert!(palette.len() < 128);
    let mut bytes = vec![1, 0, 0, 0, 0, 0, 0, 0, 0, palette.len() as u8];
    bytes.extend(palette);
    bytes
}
#[test]
fn invalid_biome_palettes_ids_indices_and_lengths() {
    for &v in Version::ALL {
        let l = Limits::default();
        let mut malformed = vec![
            vec![32],
            vec![255],
            vec![1, 0],
            vec![1, 3, 0, 1, 2],
            hex("00 ff ff ff ff 0f"),
            hex("01 01 ff ff ff ff 0f"),
        ];
        // Indirect palette has only index 0, but storage references index 1.
        let mut index = vec![1, 1, 1];
        if v.protocol() < 770 {
            index.push(1);
        }
        index.extend([0, 0, 0, 0, 0, 0, 0, 1]);
        malformed.push(index);
        if v.protocol() < 770 {
            malformed.extend([vec![0, 1, 1], vec![4, 0], vec![4, 5]]);
        }
        for bytes in malformed {
            assert!(
                ChunkBiomes::decode(&one_biome_payload(&bytes), v, 1, l).is_err(),
                "{v} {bytes:?}"
            );
        }
        let base = expected_biomes();
        let mut invalid = base.clone();
        invalid.chunks[0].sections[0].kind = ContainerKind::Blocks;
        assert!(invalid.encode(v, 2, l).is_err());
        let mut invalid = base.clone();
        invalid.chunks[0].sections[0].palette = Palette::Single(u32::MAX);
        assert!(invalid.encode(v, 2, l).is_err());
        let mut invalid = base.clone();
        invalid.chunks[0].sections[1].data[0] = 1;
        invalid.chunks[0].sections[1].palette = Palette::Indirect(vec![7]);
        assert!(invalid.encode(v, 2, l).is_err());
        let mut invalid = base.clone();
        invalid.chunks[1].sections[1].bits_per_entry = 32;
        assert!(invalid.encode(v, 2, l).is_err());
        let mut invalid = base;
        invalid.chunks[1].sections[1].data.pop();
        assert!(invalid.encode(v, 2, l).is_err());
    }
}

#[test]
fn light_masks_array_lengths_and_dimension_boundary_bits() {
    let l = Limits::default();
    for &v in Version::ALL {
        let mut bad = expected_light();
        bad.light.sky_mask[0] |= 16;
        assert!(bad.encode(v, 2, l).is_err());
        let mut bytes = light_fixture();
        bytes[14] |= 16;
        assert!(UpdateLight::decode(&bytes, v, 2, l).is_err());
        let mut bad = expected_light();
        bad.light.sky_mask.push(0);
        assert!(bad.encode(v, 2, l).is_err());
        let mut bad = expected_light();
        bad.light.empty_sky_mask[0] |= 1;
        assert!(bad.encode(v, 2, l).is_err());
        let mut bytes = light_fixture();
        bytes[32] |= 1;
        assert!(UpdateLight::decode(&bytes, v, 2, l).is_err());
        let mut bad = expected_light();
        bad.light.empty_block_mask[0] |= 2;
        assert!(bad.encode(v, 2, l).is_err());
        let mut bad = expected_light();
        bad.light.sky_arrays.pop();
        assert!(bad.encode(v, 2, l).is_err());
        for size in [0, 2047, 2049] {
            let mut bad = expected_light();
            bad.light.block_arrays[0].resize(size, 0);
            assert!(bad.encode(v, 2, l).is_err());
        }
        let mut bytes = light_fixture();
        bytes[42] = 1;
        assert!(UpdateLight::decode(&bytes, v, 2, l).is_err());
        for prefix in [[0xff, 0x0f], [0x81, 0x10]] {
            let mut bytes = light_fixture();
            bytes[43..45].copy_from_slice(&prefix);
            assert!(UpdateLight::decode(&bytes, v, 2, l).is_err());
        }
        // Section count 63 means 65 lighting sections, spanning two mask words.
        let large = UpdateLight {
            chunk_x: i32::MIN,
            chunk_z: i32::MAX,
            light: LightData {
                sky_mask: vec![i64::MIN, 1],
                sky_arrays: vec![vec![0x56; 2048], vec![0x78; 2048]],
                ..LightData::default()
            },
        };
        let b = large.encode(v, 63, l).unwrap();
        assert_eq!(UpdateLight::decode(&b, v, 63, l).unwrap(), large);
        assert!(UpdateLight::decode(&b, v, 62, l).is_err());
    }
}

#[test]
fn aggregate_collection_budget_covers_nested_palettes_and_light_arrays() {
    for &v in Version::ALL {
        let biomes = expected_biomes();
        // 2 chunks + 4 sections + 2 indirect IDs + 1 + 4 storage words.
        let exact = Limits {
            max_collection: 13,
            ..Limits::default()
        };
        assert!(biomes.encode(v, 2, exact).is_ok());
        assert!(ChunkBiomes::decode(&biome_fixture(v), v, 2, exact).is_ok());
        let short = Limits {
            max_collection: 12,
            ..exact
        };
        assert!(matches!(biomes.encode(v, 2, short), Err(Error::Limit(_))));
        assert!(matches!(
            ChunkBiomes::decode(&biome_fixture(v), v, 2, short),
            Err(Error::Limit(_))
        ));
        // 4 mask words + 3 arrays + 6144 bytes.
        let exact = Limits {
            max_collection: 6151,
            ..Limits::default()
        };
        assert!(expected_light().encode(v, 2, exact).is_ok());
        assert!(UpdateLight::decode(&light_fixture(), v, 2, exact).is_ok());
        let short = Limits {
            max_collection: 6150,
            ..exact
        };
        assert!(matches!(
            expected_light().encode(v, 2, short),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            UpdateLight::decode(&light_fixture(), v, 2, short),
            Err(Error::Limit(_))
        ));
        let mut hostile = vec![0xff, 0xff, 0xff, 0xff, 0x07];
        hostile.extend([0; 64]);
        let low = Limits {
            max_collection: 64,
            ..Limits::default()
        };
        assert!(ChunkBiomes::decode(&hostile, v, 2, low).is_err());
        let mut light = vec![0, 0];
        light.extend(&hostile);
        assert!(UpdateLight::decode(&light, v, 2, low).is_err());
    }
}

#[test]
fn whole_packet_byte_budgets_are_inclusive() {
    for &v in Version::ALL {
        for n in [light_fixture().len() - 1, light_fixture().len()] {
            let limits = Limits {
                max_packet: n,
                ..Limits::default()
            };
            let fits = n == light_fixture().len();
            assert_eq!(expected_light().encode(v, 2, limits).is_ok(), fits);
            assert_eq!(
                UpdateLight::decode(&light_fixture(), v, 2, limits).is_ok(),
                fits
            );
        }
        for n in [biome_fixture(v).len() - 1, biome_fixture(v).len()] {
            let limits = Limits {
                max_packet: n,
                ..Limits::default()
            };
            let fits = n == biome_fixture(v).len();
            assert_eq!(expected_biomes().encode(v, 2, limits).is_ok(), fits);
            assert_eq!(
                ChunkBiomes::decode(&biome_fixture(v), v, 2, limits).is_ok(),
                fits
            );
        }
        for n in [entity_fixture(v).len() - 1, entity_fixture(v).len()] {
            let limits = Limits {
                max_packet: n,
                ..Limits::default()
            };
            let fits = n == entity_fixture(v).len();
            assert_eq!(expected_entity(v).encode(v, limits).is_ok(), fits);
            assert_eq!(
                TileEntityData::decode(&entity_fixture(v), v, limits).is_ok(),
                fits
            );
        }
        let no_bytes = Limits {
            max_packet: 0,
            ..Limits::default()
        };
        assert!(ChunkBiomes::default().encode(v, 1, no_bytes).is_err());
        assert!(UpdateViewPosition {
            chunk_x: 0,
            chunk_z: 0
        }
        .encode(v, no_bytes)
        .is_err());
        assert!(UpdateViewDistance { distance: 0 }
            .encode(v, no_bytes)
            .is_err());
        assert!(SimulationDistance { distance: 0 }
            .encode(v, no_bytes)
            .is_err());
    }
}

#[test]
fn block_entity_aggregate_nbt_nodes_depth_and_collection_budgets() {
    for &v in Version::ALL {
        // root + 3 child tags + 2 int-array elements = 6 nodes.
        let exact = Limits {
            max_nbt_nodes: 6,
            ..Limits::default()
        };
        assert!(expected_entity(v).encode(v, exact).is_ok());
        assert!(TileEntityData::decode(&entity_fixture(v), v, exact).is_ok());
        let short = Limits {
            max_nbt_nodes: 5,
            ..exact
        };
        assert!(expected_entity(v).encode(v, short).is_err());
        assert!(TileEntityData::decode(&entity_fixture(v), v, short).is_err());
        let short = Limits {
            max_nbt_depth: 0,
            ..exact
        };
        assert!(expected_entity(v).encode(v, short).is_err());
        assert!(TileEntityData::decode(&entity_fixture(v), v, short).is_err());
        let short = Limits {
            max_collection: 1,
            ..exact
        };
        assert!(expected_entity(v).encode(v, short).is_err());
        assert!(TileEntityData::decode(&entity_fixture(v), v, short).is_err());
        let mut entity = expected_entity(v);
        entity.data.as_mut().unwrap().root = Tag::Compound(vec![
            ("a".into(), Tag::IntArray(vec![1; 4])),
            ("b".into(), Tag::IntArray(vec![2; 4])),
        ]);
        let b = entity.encode(v, Limits::default()).unwrap();
        let aggregate = Limits {
            max_nbt_nodes: 10,
            ..Limits::default()
        };
        assert!(entity.encode(v, aggregate).is_err());
        assert!(TileEntityData::decode(&b, v, aggregate).is_err());
        let mut nested = Tag::Byte(0);
        for _ in 0..12 {
            nested = Tag::List {
                element_type: nested.tag_type(),
                elements: vec![nested],
            };
        }
        entity.data.as_mut().unwrap().root = Tag::Compound(vec![("deep".into(), nested)]);
        let b = entity.encode(v, Limits::default()).unwrap();
        let shallow = Limits {
            max_nbt_depth: 5,
            ..Limits::default()
        };
        assert!(entity.encode(v, shallow).is_err());
        assert!(TileEntityData::decode(&b, v, shallow).is_err());
        entity.data.as_mut().unwrap().root = Tag::Compound(vec![(
            "bad".into(),
            Tag::List {
                element_type: TagType::Int,
                elements: vec![Tag::Byte(1)],
            },
        )]);
        assert!(entity.encode(v, Limits::default()).is_err());
    }
}

#[test]
fn only_verified_paper_770_biome_padding_is_accepted() {
    let l = Limits::default();
    // One chunk, two singleton sections, followed by two obsolete prefix bytes.
    let padded = hex("01 00 00 00 00 00 00 00 00 06 00 01 00 02 00 00");
    let value = ChunkBiomes::decode(&padded, Version::V1_21_5, 2, l).unwrap();
    assert_eq!(
        value.encode(Version::V1_21_5, 2, l).unwrap(),
        hex("01 00 00 00 00 00 00 00 00 04 00 01 00 02")
    );
    for &v in Version::ALL {
        if v != Version::V1_21_5 {
            assert!(ChunkBiomes::decode(&padded, v, 2, l).is_err());
        }
    }
    let mut nonzero = padded.clone();
    *nonzero.last_mut().unwrap() = 1;
    assert!(ChunkBiomes::decode(&nonzero, Version::V1_21_5, 2, l).is_err());
    for n in [1, 3, 4] {
        let mut wrong = hex("01 00 00 00 00 00 00 00 00");
        wrong.push(4 + n);
        wrong.extend([0, 1, 0, 2]);
        wrong.extend(vec![0; n as usize]);
        assert!(ChunkBiomes::decode(&wrong, Version::V1_21_5, 2, l).is_err());
    }
}

#[test]
fn biome_buffer_obeys_release_two_megabyte_limit() {
    for &v in Version::ALL {
        // An oversized prefix must be rejected before its absent payload is read.
        let over = hex("01 00 00 00 00 00 00 00 00 81 80 80 01");
        assert!(matches!(
            ChunkBiomes::decode(&over, v, 1, Limits::default()),
            Err(Error::Limit(_))
        ));
        let section = PaletteContainer {
            kind: ContainerKind::Biomes,
            bits_per_entry: 31,
            palette: Palette::Direct,
            data: vec![0; 32],
        };
        let large = ChunkBiomes {
            chunks: vec![ChunkBiomeData {
                chunk_x: 0,
                chunk_z: 0,
                sections: vec![section; 8200],
            }],
        };
        assert!(matches!(
            large.encode(v, 8200, Limits::default()),
            Err(Error::Limit(_))
        ));
    }
}

#[test]
fn light_writer_is_transactional_on_all_preflight_failures() {
    let light = expected_light().light;
    let mut w = Writer::new();
    w.raw(&[0x55, 0xaa]);
    for (sections, limits) in [
        (0, Limits::default()),
        (1, Limits::default()),
        (
            2,
            Limits {
                max_collection: 6150,
                ..Limits::default()
            },
        ),
        (
            2,
            Limits {
                max_packet: 100,
                ..Limits::default()
            },
        ),
    ] {
        assert!(light.write_for_sections(&mut w, sections, limits).is_err());
        assert_eq!(w.as_slice(), [0x55, 0xaa]);
    }
    let mut bad = light.clone();
    bad.block_arrays[0].pop();
    assert!(bad
        .write_for_sections(&mut w, 2, Limits::default())
        .is_err());
    assert_eq!(w.as_slice(), [0x55, 0xaa]);
    light
        .write_for_sections(&mut w, 2, Limits::default())
        .unwrap();
    let mut r = Reader::new(&w.as_slice()[2..], Limits::default());
    assert_eq!(LightData::read_for_sections(&mut r, 2).unwrap(), light);
    r.finish().unwrap();
    // The reusable light reader bounds its own slice and commits only on success.
    let limits = Limits {
        max_packet: 100,
        ..Limits::default()
    };
    let mut r = Reader::new(&w.as_slice()[2..], limits);
    assert!(LightData::read_for_sections(&mut r, 2).is_err());
    assert_eq!(r.position(), 0);
}

#[test]
fn view_controls_preserve_full_signed_scalar_range_without_gameplay_policy() {
    for &v in Version::ALL {
        for (value, bytes) in [
            (i32::MIN, hex("80 80 80 80 08")),
            (-1, hex("ff ff ff ff 0f")),
            (0, vec![0]),
            (i32::MAX, hex("ff ff ff ff 07")),
        ] {
            let l = Limits::default();
            assert_eq!(
                UpdateViewDistance { distance: value }.encode(v, l).unwrap(),
                bytes
            );
            assert_eq!(
                UpdateViewDistance::decode(&bytes, v, l).unwrap().distance,
                value
            );
            assert_eq!(
                SimulationDistance { distance: value }.encode(v, l).unwrap(),
                bytes
            );
            assert_eq!(
                SimulationDistance::decode(&bytes, v, l).unwrap().distance,
                value
            );
        }
        for bad in [hex("ff ff ff ff ff 00"), hex("80 80 80 80 10")] {
            assert!(UpdateViewDistance::decode(&bad, v, Limits::default()).is_err());
            assert!(SimulationDistance::decode(&bad, v, Limits::default()).is_err());
        }
    }
}

#[test]
fn bounded_mutations_never_panic_and_successes_reencode() {
    let limits = Limits {
        max_packet: 8192,
        max_collection: 8192,
        max_nbt_depth: 12,
        max_nbt_nodes: 256,
        ..Limits::default()
    };
    for &v in Version::ALL {
        for family in 0..3 {
            let original = match family {
                0 => light_fixture(),
                1 => entity_fixture(v),
                _ => biome_fixture(v),
            };
            // Include every structural byte plus deterministic interior samples.
            let indices = (0..original.len().min(100)).chain((100..original.len()).step_by(101));
            for index in indices {
                for byte in [0, 1, 0x7f, 0x80, 0xff] {
                    let mut b = original.clone();
                    b[index] = byte;
                    match family {
                        0 => {
                            if let Ok(value) = UpdateLight::decode(&b, v, 2, limits) {
                                let canonical = value.encode(v, 2, limits).unwrap();
                                assert_eq!(
                                    UpdateLight::decode(&canonical, v, 2, limits).unwrap(),
                                    value
                                );
                            }
                        }
                        1 => {
                            if let Ok(value) = TileEntityData::decode(&b, v, limits) {
                                let canonical = value.encode(v, limits).unwrap();
                                assert_eq!(
                                    TileEntityData::decode(&canonical, v, limits).unwrap(),
                                    value
                                );
                            }
                        }
                        _ => {
                            if let Ok(value) = ChunkBiomes::decode(&b, v, 2, limits) {
                                let canonical = value.encode(v, 2, limits).unwrap();
                                assert_eq!(
                                    ChunkBiomes::decode(&canonical, v, 2, limits).unwrap(),
                                    value
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
