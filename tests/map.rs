use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, Tag},
    packet::{chat::ChatComponent, map::*},
    Error, Limits, Version,
};

// Hand-authored wire bytes, not output from Rustwire's Writer or map encoder.
// These exercise the pinned layouts, not a release-API/live-server oracle.
fn fixture(version: Version) -> Vec<u8> {
    let mut bytes = vec![0xac, 2, 3, 1, 1, 2, 0, 0x80, 0x7f, 15, 0, 1, 0xff, 1, 0, 1];
    if version.protocol() < 765 {
        bytes.extend(b"\x03\"x\"");
    } else {
        bytes.extend([8, 0, 1, b'x']);
    }
    bytes.extend([2, 2, 10, 20, 4, 0, 1, 127, 255]);
    bytes
}
fn expected(version: Version) -> MapData {
    MapData {
        map_id: 300,
        scale: 3,
        locked: true,
        decorations: Some(vec![
            MapDecoration {
                kind_id: 0,
                x: -128,
                z: 127,
                rotation: 15,
                label: None,
            },
            MapDecoration {
                kind_id: 1,
                x: -1,
                z: 1,
                rotation: 0,
                label: Some(if version.protocol() < 765 {
                    ChatComponent::Json("\"x\"".into())
                } else {
                    ChatComponent::Nbt(Nbt::anonymous(Tag::String("x".into())))
                }),
            },
        ]),
        patch: Some(MapPatch {
            columns: 2,
            rows: 2,
            x: 10,
            z: 20,
            colors: vec![0, 1, 127, 255],
        }),
    }
}

#[test]
fn independent_map_fixtures_all_families() {
    for &v in Version::ALL {
        let bytes = fixture(v);
        assert_eq!(
            MapData::decode(&bytes, v, Limits::default()).unwrap(),
            expected(v)
        );
        assert_eq!(expected(v).encode(v, Limits::default()).unwrap(), bytes);
    }
}

#[test]
fn absent_and_empty_decorations_and_patch_are_distinct() {
    for &v in Version::ALL {
        for (bytes, decorations, patch) in [
            (vec![0, 0, 0, 0, 0], None, None),
            (vec![0, 0, 0, 1, 0, 0], Some(vec![]), None),
            (
                vec![0, 0, 0, 0, 1, 1, 0, 0, 1, 42],
                None,
                Some(MapPatch {
                    columns: 1,
                    rows: 1,
                    x: 0,
                    z: 0,
                    colors: vec![42],
                }),
            ),
        ] {
            let value = MapData {
                map_id: 0,
                scale: 0,
                locked: false,
                decorations,
                patch,
            };
            assert_eq!(
                MapData::decode(&bytes, v, Limits::default()).unwrap(),
                value
            );
            assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
        }
    }
}

#[test]
fn map_fixtures_reject_every_truncation_and_trailing_byte() {
    for &v in Version::ALL {
        let bytes = fixture(v);
        for end in 0..bytes.len() {
            assert!(
                MapData::decode(&bytes[..end], v, Limits::default()).is_err(),
                "{v} {end}"
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(MapData::decode(&trailing, v, Limits::default()).is_err());
    }
}

#[test]
fn map_label_boundary_is_exact() {
    let old = Version::V1_20_2;
    let new = Version::V1_20_3;
    assert!(MapData::decode(&fixture(old), new, Limits::default()).is_err());
    assert!(MapData::decode(&fixture(new), old, Limits::default()).is_err());
    assert!(expected(old).encode(new, Limits::default()).is_err());
    assert!(expected(new).encode(old, Limits::default()).is_err());
}

#[test]
fn map_rejects_malformed_flags_lengths_varints_and_labels() {
    for &v in Version::ALL {
        for offset in [3, 4, 10, 15] {
            let mut bytes = fixture(v);
            bytes[offset] = 2;
            assert!(MapData::decode(&bytes, v, Limits::default()).is_err());
        }
        for bytes in [
            vec![0xff, 0xff, 0xff, 0xff, 0x10],             // VarInt overflow
            vec![0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff, 0x0f], // negative count
            vec![0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff, 7],    // enormous, impossible count
            vec![0, 0, 0, 0, 1, 1, 0, 0, 0xff, 0xff, 0xff, 0xff, 0x0f], // negative colors
        ] {
            assert!(MapData::decode(
                &bytes,
                v,
                Limits {
                    max_collection: usize::MAX,
                    ..Limits::default()
                }
            )
            .is_err());
        }
        let mut bytes = vec![0, 0, 0, 1, 1, 0, 0, 0, 0, 1];
        if v.protocol() < 765 {
            bytes.extend([1, 255, 0]); // invalid UTF-8 label
        } else {
            bytes.extend([0, 0]); // present label cannot be absent NBT
        }
        assert!(MapData::decode(&bytes, v, Limits::default()).is_err());
    }
}

#[test]
fn map_collection_budget_is_shared_by_decorations_and_colors() {
    for &v in Version::ALL {
        let bytes = fixture(v);
        let value = expected(v);
        for max_collection in 0..6 {
            let limits = Limits {
                max_collection,
                ..Limits::default()
            };
            assert!(matches!(
                MapData::decode(&bytes, v, limits),
                Err(Error::Limit(_))
            ));
            assert!(matches!(value.encode(v, limits), Err(Error::Limit(_))));
        }
        let limits = Limits {
            max_collection: 6,
            ..Limits::default()
        };
        assert_eq!(MapData::decode(&bytes, v, limits).unwrap(), value);
        assert_eq!(value.encode(v, limits).unwrap(), bytes);
    }
}

#[test]
fn map_packet_byte_limits_apply_to_decode_encode_and_stream_read() {
    for &v in Version::ALL {
        let bytes = fixture(v);
        let value = expected(v);
        for max_packet in 0..bytes.len() {
            let limits = Limits {
                max_packet,
                ..Limits::default()
            };
            assert!(MapData::decode(&bytes, v, limits).is_err());
            assert!(value.encode(v, limits).is_err());
            let mut r = Reader::new(&bytes, limits);
            assert!(MapData::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
        }
        let limits = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        assert_eq!(value.encode(v, limits).unwrap(), bytes);
        assert_eq!(MapData::decode(&bytes, v, limits).unwrap(), value);
    }
}

#[test]
fn map_label_string_and_shared_nbt_node_budgets_apply_both_ways() {
    for &v in Version::ALL {
        let mut value = expected(v);
        let label = value.decorations.as_ref().unwrap()[1].label.clone();
        value.decorations.as_mut().unwrap()[0].label = label;
        let bytes = value.encode(v, Limits::default()).unwrap();
        let limits = Limits {
            max_string_chars: 0,
            ..Limits::default()
        };
        assert!(MapData::decode(&bytes, v, limits).is_err());
        assert!(value.encode(v, limits).is_err());
        if v.protocol() >= 765 {
            for max_nbt_nodes in [0, 1] {
                let limits = Limits {
                    max_nbt_nodes,
                    ..Limits::default()
                };
                assert!(MapData::decode(&bytes, v, limits).is_err());
                assert!(value.encode(v, limits).is_err());
            }
            let limits = Limits {
                max_nbt_nodes: 2,
                ..Limits::default()
            };
            assert_eq!(MapData::decode(&bytes, v, limits).unwrap(), value);
            assert_eq!(value.encode(v, limits).unwrap(), bytes);
        }
    }
}

#[test]
fn map_read_one_body_and_write_failures_are_transactional() {
    for &v in Version::ALL {
        let bytes = fixture(v);
        let joined = [bytes.as_slice(), &[42]].concat();
        let mut r = Reader::new(&joined, Limits::default());
        assert_eq!(MapData::read(&mut r, v).unwrap(), expected(v));
        assert_eq!(r.remaining(), &[42]);
        let mut r = Reader::new(&bytes[..bytes.len() - 1], Limits::default());
        assert!(MapData::read(&mut r, v).is_err());
        assert_eq!(r.position(), 0);
        let mut w = Writer::new();
        w.u8(42);
        let mut invalid = expected(v);
        invalid.patch.as_mut().unwrap().columns = 0;
        assert!(invalid.write(&mut w, v, Limits::default()).is_err());
        assert_eq!(w.as_slice(), &[42]);
        let limits = Limits {
            max_packet: bytes.len() - 1,
            ..Limits::default()
        };
        assert!(expected(v).write(&mut w, v, limits).is_err());
        assert_eq!(w.as_slice(), &[42]);
        expected(v).write(&mut w, v, Limits::default()).unwrap();
        assert_eq!(w.as_slice(), [vec![42], bytes].concat());
    }
}

#[test]
fn map_wire_scalars_are_preserved_without_gameplay_clamps() {
    for &v in Version::ALL {
        // Negative map/kind IDs are unresolved wire scalars, not registry claims.
        // Rotation, dimensions and colors deliberately exceed canvas conventions.
        let bytes = [
            255, 255, 255, 255, 15, 128, 0, 1, 1, 255, 255, 255, 255, 15, 128, 127, 255, 0, 255, 0,
            255, 255, 1, 255,
        ];
        let value = MapData::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(value.map_id, -1);
        assert_eq!(value.scale, -128);
        assert_eq!(value.decorations.as_ref().unwrap()[0].kind_id, -1);
        assert_eq!(value.decorations.as_ref().unwrap()[0].rotation, 255);
        assert_eq!(value.patch.as_ref().unwrap().rows, 0);
        assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
    }
}

#[test]
fn map_patch_canvas_validation_checks_geometry_and_exact_color_count() {
    let full = MapPatch {
        columns: 128,
        rows: 128,
        x: 0,
        z: 0,
        colors: vec![0; 128 * 128],
    };
    assert!(full.validate_canvas().is_ok());
    let edge = MapPatch {
        columns: 1,
        rows: 1,
        x: 127,
        z: 127,
        colors: vec![0],
    };
    assert!(edge.validate_canvas().is_ok());
    for invalid in [
        MapPatch {
            columns: 0,
            ..edge.clone()
        },
        MapPatch {
            rows: 0,
            ..edge.clone()
        },
        MapPatch {
            columns: 2,
            ..edge.clone()
        },
        MapPatch {
            rows: 2,
            ..edge.clone()
        },
        MapPatch {
            x: 255,
            ..edge.clone()
        },
        MapPatch {
            z: 255,
            ..edge.clone()
        },
        MapPatch {
            colors: vec![],
            ..edge.clone()
        },
        MapPatch {
            colors: vec![0, 0],
            ..edge
        },
    ] {
        assert!(invalid.validate_canvas().is_err());
    }
}

#[test]
fn map_label_nested_depth_and_array_node_limits_are_enforced() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 765) {
        let mut value = expected(v);
        let labels = value.decorations.as_mut().unwrap();
        for label in labels {
            label.label = Some(ChatComponent::Nbt(Nbt::anonymous(Tag::Compound(vec![(
                "text".into(),
                Tag::String("x".into()),
            )]))));
        }
        let bytes = value.encode(v, Limits::default()).unwrap();
        let shallow = Limits {
            max_nbt_depth: 0,
            ..Limits::default()
        };
        assert!(MapData::decode(&bytes, v, shallow).is_err());
        assert!(value.encode(v, shallow).is_err());
        let too_few_nodes = Limits {
            max_nbt_nodes: 3,
            ..Limits::default()
        };
        assert!(MapData::decode(&bytes, v, too_few_nodes).is_err());
        assert!(value.encode(v, too_few_nodes).is_err());
        let exact = Limits {
            max_nbt_nodes: 4,
            max_nbt_depth: 1,
            ..Limits::default()
        };
        assert_eq!(MapData::decode(&bytes, v, exact).unwrap(), value);
        assert_eq!(value.encode(v, exact).unwrap(), bytes);
        // Primitive-array elements consume the shared NBT-node budget as well.
        for decoration in value.decorations.as_mut().unwrap() {
            decoration.label = Some(ChatComponent::Nbt(Nbt::anonymous(Tag::ByteArray(vec![
                1, 2,
            ]))));
        }
        let bytes = value.encode(v, Limits::default()).unwrap();
        let limits = Limits {
            max_nbt_nodes: 5,
            ..Limits::default()
        };
        assert!(MapData::decode(&bytes, v, limits).is_err());
        assert!(value.encode(v, limits).is_err());
    }
}
