use rustwire_mc::{
    codec::BlockPosition,
    nbt::{Nbt, NbtString, Tag, TagType},
    packet::world_control::*,
    version::{Direction, State},
    Error, Limits, Version,
};
#[path = "support/world_control_fixtures.rs"]
mod support;

fn semantic_assertions(value: &WorldControlPacket, v: Version, case: &str) {
    let position = BlockPosition {
        x: -33554432,
        y: -2048,
        z: 33554431,
    };
    match value {
        WorldControlPacket::BlockBreak(p) => {
            assert_eq!(p.position, position);
            assert_eq!((p.entity_id, p.stage), (-1, i8::MIN));
        }
        WorldControlPacket::BlockAction(p) => {
            assert_eq!(p.position, position);
            assert_eq!(
                (p.action_id, p.action_parameter, p.block_id),
                (255, 128, i32::MIN)
            );
        }
        WorldControlPacket::OpenSign(p) => {
            assert_eq!(p.position, position);
            assert_eq!(p.is_front_text, case == "front");
        }
        WorldControlPacket::NbtQuery(p) => {
            if case == "absent" {
                assert_eq!(p.transaction_id, -1);
                assert_eq!(p.data, None);
            } else {
                assert_eq!(p.transaction_id, i32::MIN);
                let nbt = p.data.as_ref().unwrap();
                assert_eq!(
                    nbt.name,
                    if v.protocol() == 763 {
                        Some("root".into())
                    } else {
                        None
                    }
                );
                assert_eq!(nbt.root.get("x"), Some(&Tag::Int(-1)));
                assert_eq!(nbt.root.get("a"), Some(&Tag::ByteArray(vec![0, -1, 127])));
                assert_eq!(
                    nbt.root.get("l"),
                    Some(&Tag::List {
                        element_type: TagType::Byte,
                        elements: vec![]
                    })
                );
            }
        }
        WorldControlPacket::Collect(p) => assert_eq!(
            (p.collected_entity_id, p.collector_entity_id, p.count),
            (-1, i32::MIN, i32::MAX)
        ),
        WorldControlPacket::Vehicle(p) => {
            assert_eq!(
                p.position.map(f64::to_bits),
                [0x7ff8000000001234, 0x8000000000000000, 0x7ff0000000000000]
            );
            assert_eq!(p.yaw.to_bits(), 0xff800000);
            assert_eq!(p.pitch.to_bits(), 0x7fc01234);
        }
        WorldControlPacket::FacePlayer(p) => {
            assert_eq!(p.source_anchor, EntityAnchor::Eyes);
            assert_eq!(p.position, [-1.25, 2.5, -3.75]);
            assert_eq!(
                p.entity,
                if case == "entity" {
                    Some(FacePlayerEntity {
                        entity_id: -1,
                        anchor: EntityAnchor::Feet,
                    })
                } else {
                    None
                }
            );
        }
        WorldControlPacket::Rotation(p) => {
            assert_eq!(p.yaw.to_bits(), 0x7fc01234);
            assert_eq!(p.pitch.to_bits(), 0x80000000);
            assert_eq!(
                p.relative,
                if v.protocol() >= 773 {
                    Some(RotationRelativity {
                        yaw: true,
                        pitch: false,
                    })
                } else {
                    None
                }
            );
        }
        WorldControlPacket::Projectile(p) => {
            assert_eq!(p.entity_id, i32::MIN);
            match p.power {
                ProjectilePower::Vector(values) => {
                    assert_eq!(v.protocol(), 766);
                    assert_eq!(
                        values.map(f64::to_bits),
                        [0x7ff8000000001234, 0x8000000000000000, 0x7ff0000000000000]
                    );
                }
                ProjectilePower::Scalar(value) => {
                    assert!(v.protocol() >= 767);
                    assert_eq!(value.to_bits(), 0x7ff8000000001234);
                }
            }
        }
        WorldControlPacket::Ticking(p) => {
            assert_eq!(p.tick_rate.to_bits(), 0x7fc01234);
            assert!(p.frozen);
        }
        WorldControlPacket::Step(p) => assert_eq!(p.ticks, i32::MIN),
    }
}

#[test]
fn independent_synthetic_fixtures_all_families_and_all_prefixes() {
    let fixtures = support::fixtures();
    assert_eq!(fixtures.len(), 198);
    for f in fixtures {
        assert_eq!(
            f.version
                .packet_id(State::Play, Direction::Clientbound, f.name)
                .unwrap(),
            f.id
        );
        for end in 0..f.bytes.len() {
            assert!(
                matches!(
                    WorldControlPacket::decode(
                        f.name,
                        &f.bytes[..end],
                        f.version,
                        Limits::default()
                    ),
                    Err(Error::Eof | Error::Invalid(_))
                ),
                "{} {} {} prefix {end}",
                f.version,
                f.name,
                f.case
            );
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(matches!(
            WorldControlPacket::decode(f.name, &trailing, f.version, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
        if f.case == "raw-anchors" {
            assert!(matches!(
                WorldControlPacket::decode(f.name, &f.bytes, f.version, Limits::default()),
                Err(Error::Unsupported("face-player anchor"))
            ));
            continue;
        }
        let value =
            WorldControlPacket::decode(f.name, &f.bytes, f.version, Limits::default()).unwrap();
        semantic_assertions(&value, f.version, f.case);
        let packet = value.packet(f.version, Limits::default()).unwrap();
        assert_eq!(packet.id, f.id);
        assert_eq!(packet.data, f.bytes);
        let exact = Limits {
            max_packet: f.bytes.len(),
            ..Limits::default()
        };
        assert_eq!(value.encode(f.version, exact).unwrap(), f.bytes);
        assert!(WorldControlPacket::decode(f.name, &f.bytes, f.version, exact).is_ok());
        let short = Limits {
            max_packet: f.bytes.len() - 1,
            ..Limits::default()
        };
        assert!(matches!(
            value.encode(f.version, short),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            WorldControlPacket::decode(f.name, &f.bytes, f.version, short),
            Err(Error::Limit(_))
        ));
    }
}

#[test]
fn missing_versions_are_rejected_by_direct_and_named_codecs() {
    for &v in Version::ALL {
        for (name, minimum, packet) in [
            (
                "player_rotation",
                768,
                WorldControlPacket::Rotation(PlayerRotation {
                    yaw: 1.0,
                    pitch: 2.0,
                    relative: None,
                }),
            ),
            (
                "set_projectile_power",
                766,
                WorldControlPacket::Projectile(SetProjectilePower {
                    entity_id: 0,
                    power: ProjectilePower::Vector([0.0; 3]),
                }),
            ),
            (
                "set_ticking_state",
                765,
                WorldControlPacket::Ticking(SetTickingState {
                    tick_rate: 20.0,
                    frozen: false,
                }),
            ),
            (
                "step_tick",
                765,
                WorldControlPacket::Step(StepTick { ticks: 0 }),
            ),
        ] {
            if v.protocol() >= minimum {
                continue;
            }
            assert!(matches!(
                WorldControlPacket::decode(name, &[], v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
            assert!(matches!(
                packet.encode(v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
            assert!(matches!(
                packet.packet(v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
        }
    }
}

#[test]
fn packet_representation_boundaries_are_exact() {
    let v772 = Version::V1_21_7;
    let v773 = Version::V1_21_9;
    let old = PlayerRotation {
        yaw: 0.0,
        pitch: 0.0,
        relative: None,
    };
    let new = PlayerRotation {
        relative: Some(RotationRelativity {
            yaw: true,
            pitch: false,
        }),
        ..old
    };
    assert!(old.encode(v773, Limits::default()).is_err());
    assert!(new.encode(v772, Limits::default()).is_err());
    assert!(PlayerRotation::decode(
        &old.encode(v772, Limits::default()).unwrap(),
        v773,
        Limits::default()
    )
    .is_err());
    assert!(PlayerRotation::decode(
        &new.encode(v773, Limits::default()).unwrap(),
        v772,
        Limits::default()
    )
    .is_err());
    let vector = SetProjectilePower {
        entity_id: 0,
        power: ProjectilePower::Vector([0.0; 3]),
    };
    let scalar = SetProjectilePower {
        entity_id: 0,
        power: ProjectilePower::Scalar(0.0),
    };
    assert!(vector.encode(Version::V1_21, Limits::default()).is_err());
    assert!(scalar.encode(Version::V1_20_5, Limits::default()).is_err());
    assert!(SetProjectilePower::decode(
        &vector.encode(Version::V1_20_5, Limits::default()).unwrap(),
        Version::V1_21,
        Limits::default()
    )
    .is_err());
    assert!(SetProjectilePower::decode(
        &scalar.encode(Version::V1_21, Limits::default()).unwrap(),
        Version::V1_20_5,
        Limits::default()
    )
    .is_err());
}

#[test]
fn packed_positions_reject_each_overflow_and_keep_extrema() {
    for &v in Version::ALL {
        for position in [
            BlockPosition {
                x: -33554432,
                y: -2048,
                z: 33554431,
            },
            BlockPosition {
                x: 33554431,
                y: 2047,
                z: -33554432,
            },
        ] {
            let value = OpenSignEditor {
                position,
                is_front_text: true,
            };
            let bytes = value.encode(v, Limits::default()).unwrap();
            assert_eq!(
                OpenSignEditor::decode(&bytes, v, Limits::default()).unwrap(),
                value
            );
        }
        for position in [
            BlockPosition {
                x: -33554433,
                y: 0,
                z: 0,
            },
            BlockPosition {
                x: 33554432,
                y: 0,
                z: 0,
            },
            BlockPosition {
                x: 0,
                y: -2049,
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
                z: -33554433,
            },
            BlockPosition {
                x: 0,
                y: 0,
                z: 33554432,
            },
        ] {
            assert!(OpenSignEditor {
                position,
                is_front_text: true
            }
            .encode(v, Limits::default())
            .is_err());
            assert!(BlockBreakAnimation {
                position,
                entity_id: 0,
                stage: 0
            }
            .encode(v, Limits::default())
            .is_err());
            assert!(BlockAction {
                position,
                action_id: 0,
                action_parameter: 0,
                block_id: 0
            }
            .encode(v, Limits::default())
            .is_err());
        }
    }
}

#[test]
fn nbt_root_framing_and_shared_node_collection_depth_string_budgets() {
    for f in support::fixtures()
        .into_iter()
        .filter(|f| f.name == "nbt_query_response" && f.case == "compound")
    {
        let value = NbtQueryResponse::decode(&f.bytes, f.version, Limits::default()).unwrap();
        let exact = Limits {
            max_nbt_nodes: 7,
            max_collection: 3,
            max_nbt_depth: 1,
            ..Limits::default()
        };
        assert!(NbtQueryResponse::decode(&f.bytes, f.version, exact).is_ok());
        assert_eq!(value.encode(f.version, exact).unwrap(), f.bytes);
        for limits in [
            Limits {
                max_nbt_nodes: 6,
                ..exact
            },
            Limits {
                max_collection: 2,
                ..exact
            },
            Limits {
                max_nbt_depth: 0,
                ..exact
            },
            Limits {
                max_string_chars: 0,
                ..exact
            },
        ] {
            assert!(matches!(
                NbtQueryResponse::decode(&f.bytes, f.version, limits),
                Err(Error::Limit(_))
            ));
            assert!(matches!(
                value.encode(f.version, limits),
                Err(Error::Limit(_))
            ));
        }
        let other = if f.version.protocol() == 763 {
            Version::V1_20_2
        } else {
            Version::V1_20
        };
        assert!(NbtQueryResponse::decode(&f.bytes, other, Limits::default()).is_err());
    }
    // Separate sibling branches share one node budget: root + two compounds + two ints.
    let tree = NbtQueryResponse {
        transaction_id: -1,
        data: Some(Nbt::anonymous(Tag::Compound(vec![
            ("a".into(), Tag::Compound(vec![("x".into(), Tag::Int(1))])),
            ("b".into(), Tag::Compound(vec![("y".into(), Tag::Int(2))])),
        ]))),
    };
    let v = Version::V1_21_9;
    let bytes = tree.encode(v, Limits::default()).unwrap();
    let limited = Limits {
        max_nbt_nodes: 4,
        ..Limits::default()
    };
    assert!(matches!(
        tree.encode(v, limited),
        Err(Error::Limit("NBT nodes"))
    ));
    assert!(matches!(
        NbtQueryResponse::decode(&bytes, v, limited),
        Err(Error::Limit("NBT nodes"))
    ));
    let absence = NbtQueryResponse {
        transaction_id: i32::MIN,
        data: None,
    };
    let zero_nodes = Limits {
        max_nbt_nodes: 0,
        ..Limits::default()
    };
    assert_eq!(
        NbtQueryResponse::decode(&absence.encode(v, zero_nodes).unwrap(), v, zero_nodes).unwrap(),
        absence
    );
}

#[test]
fn noncompound_roots_are_invalid_and_unknown_anchors_are_bounded_unsupported() {
    for &v in Version::ALL {
        let mut bytes = vec![0, 1];
        if v.protocol() == 763 {
            bytes.extend([0, 0]);
        }
        bytes.push(0x80);
        assert!(matches!(
            NbtQueryResponse::decode(&bytes, v, Limits::default()),
            Err(Error::Invalid("query-response compound NBT"))
        ));
        for end in 0..bytes.len() {
            assert!(matches!(
                NbtQueryResponse::decode(&bytes[..end], v, Limits::default()),
                Err(Error::Eof)
            ));
        }
        bytes.push(0);
        assert!(matches!(
            NbtQueryResponse::decode(&bytes, v, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
        assert!(matches!(
            NbtQueryResponse {
                transaction_id: 0,
                data: Some(Nbt::anonymous(Tag::Byte(1)))
            }
            .encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        for (source, target) in [(2, 0), (0, 2), (-1, 0), (0, -1), (i32::MIN, i32::MAX)] {
            let mut bytes = vec![];
            vint(&mut bytes, source);
            bytes.extend([0; 24]);
            bytes.push(1);
            vint(&mut bytes, -1);
            vint(&mut bytes, target);
            assert!(matches!(
                FacePlayer::decode(&bytes, v, Limits::default()),
                Err(Error::Unsupported("face-player anchor"))
            ));
        }
    }
    assert_eq!(EntityAnchor::from_id(0).unwrap(), EntityAnchor::Feet);
    assert_eq!(EntityAnchor::from_id(1).unwrap(), EntityAnchor::Eyes);
}

fn vint(bytes: &mut Vec<u8>, value: i32) {
    let mut n = value as u32;
    loop {
        let b = (n & 127) as u8;
        n >>= 7;
        bytes.push(b | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}

#[test]
fn malformed_booleans_varints_and_nbt_fail_closed() {
    for &v in Version::ALL {
        let mut sign = vec![0; 9];
        sign[8] = 2;
        assert!(matches!(
            OpenSignEditor::decode(&sign, v, Limits::default()),
            Err(Error::Invalid("boolean"))
        ));
        let mut face = vec![0; 26];
        face[25] = 2;
        assert!(matches!(
            FacePlayer::decode(&face, v, Limits::default()),
            Err(Error::Invalid("boolean"))
        ));
        let overflow = [255, 255, 255, 255, 16];
        assert!(matches!(
            CollectItem::decode(&overflow, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            NbtQueryResponse::decode(&overflow, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        if v.protocol() >= 765 {
            assert!(matches!(
                SetTickingState::decode(&[0, 0, 0, 0, 2], v, Limits::default()),
                Err(Error::Invalid("boolean"))
            ));
            assert!(matches!(
                StepTick::decode(&overflow, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        if v.protocol() >= 773 {
            for index in [4, 9] {
                let mut rotation = [0; 10];
                rotation[index] = 2;
                assert!(matches!(
                    PlayerRotation::decode(&rotation, v, Limits::default()),
                    Err(Error::Invalid("boolean"))
                ));
            }
        }
        // Compound containing a byte array with a negative declared length.
        let mut nbt = vec![0, 10];
        if v.protocol() == 763 {
            nbt.extend([0, 0]);
        }
        nbt.extend([7, 0, 0, 255, 255, 255, 255, 0]);
        assert!(matches!(
            NbtQueryResponse::decode(&nbt, v, Limits::default()),
            Err(Error::Invalid("negative NBT array length"))
        ));
    }
}

#[test]
fn java_nbt_string_units_and_compound_order_are_retained() {
    for &v in Version::ALL {
        let data = NbtQueryResponse {
            transaction_id: 0,
            data: Some(Nbt {
                name: if v.protocol() == 763 {
                    Some("n".into())
                } else {
                    None
                },
                root: Tag::Compound(vec![
                    (
                        "s".into(),
                        Tag::String(NbtString(vec![0, 0xd800, 0xdc00, 0xffff])),
                    ),
                    ("s".into(), Tag::Int(2)),
                ]),
            }),
        };
        let bytes = data.encode(v, Limits::default()).unwrap();
        assert_eq!(
            NbtQueryResponse::decode(&bytes, v, Limits::default()).unwrap(),
            data
        );
    }
}

#[test]
fn scalar_signed_ranges_do_not_spend_allocation_budgets() {
    let limits = Limits {
        max_collection: 0,
        max_nbt_nodes: 0,
        ..Limits::default()
    };
    for &v in Version::ALL {
        for raw in [i32::MIN, -1, 0, 1, i32::MAX] {
            let collect = CollectItem {
                collected_entity_id: raw,
                collector_entity_id: raw,
                count: raw,
            };
            let bytes = collect.encode(v, limits).unwrap();
            assert_eq!(CollectItem::decode(&bytes, v, limits).unwrap(), collect);
            let block = BlockAction {
                position: BlockPosition { x: 0, y: 0, z: 0 },
                action_id: 255,
                action_parameter: 128,
                block_id: raw,
            };
            let bytes = block.encode(v, limits).unwrap();
            assert_eq!(BlockAction::decode(&bytes, v, limits).unwrap(), block);
            if v.protocol() >= 765 {
                let step = StepTick { ticks: raw };
                let bytes = step.encode(v, limits).unwrap();
                assert_eq!(StepTick::decode(&bytes, v, limits).unwrap(), step);
            }
        }
        for stage in [i8::MIN, -1, 0, 9, 10, i8::MAX] {
            let block = BlockBreakAnimation {
                position: BlockPosition { x: 0, y: 0, z: 0 },
                entity_id: 0,
                stage,
            };
            let bytes = block.encode(v, limits).unwrap();
            assert_eq!(
                BlockBreakAnimation::decode(&bytes, v, limits).unwrap(),
                block
            );
        }
    }
}

#[test]
fn unclamped_float_fields_retain_signed_zero_nonfinite_and_payload_bits() {
    for &v in Version::ALL {
        for bits in [
            0x00000000,
            0x80000000,
            0x7f800000,
            0xff800000,
            0x7fc00001,
            0xffc12345,
            (-100.0_f32).to_bits(),
        ] {
            let float = f32::from_bits(bits);
            let vehicle = VehicleMove {
                position: [0.0; 3],
                yaw: float,
                pitch: float,
            };
            let bytes = vehicle.encode(v, Limits::default()).unwrap();
            let decoded = VehicleMove::decode(&bytes, v, Limits::default()).unwrap();
            assert_eq!(decoded.yaw.to_bits(), bits);
            assert_eq!(decoded.pitch.to_bits(), bits);
            if v.protocol() >= 765 {
                let value = SetTickingState {
                    tick_rate: float,
                    frozen: false,
                };
                let bytes = value.encode(v, Limits::default()).unwrap();
                assert_eq!(
                    SetTickingState::decode(&bytes, v, Limits::default())
                        .unwrap()
                        .tick_rate
                        .to_bits(),
                    bits
                );
            }
            if v.protocol() >= 768 {
                let value = PlayerRotation {
                    yaw: float,
                    pitch: float,
                    relative: (v.protocol() >= 773).then_some(RotationRelativity {
                        yaw: false,
                        pitch: true,
                    }),
                };
                let bytes = value.encode(v, Limits::default()).unwrap();
                let decoded = PlayerRotation::decode(&bytes, v, Limits::default()).unwrap();
                assert_eq!(decoded.yaw.to_bits(), bits);
                assert_eq!(decoded.pitch.to_bits(), bits);
            }
        }
        for bits in [
            0,
            0x8000000000000000,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8000000001234,
            0xfff8000000012345,
        ] {
            let float = f64::from_bits(bits);
            let vehicle = VehicleMove {
                position: [float; 3],
                yaw: 0.0,
                pitch: 0.0,
            };
            let bytes = vehicle.encode(v, Limits::default()).unwrap();
            assert_eq!(
                VehicleMove::decode(&bytes, v, Limits::default())
                    .unwrap()
                    .position
                    .map(f64::to_bits),
                [bits; 3]
            );
            let face = FacePlayer {
                source_anchor: EntityAnchor::Feet,
                position: [float; 3],
                entity: None,
            };
            let bytes = face.encode(v, Limits::default()).unwrap();
            assert_eq!(
                FacePlayer::decode(&bytes, v, Limits::default())
                    .unwrap()
                    .position
                    .map(f64::to_bits),
                [bits; 3]
            );
            if v.protocol() >= 766 {
                let value = SetProjectilePower {
                    entity_id: 0,
                    power: if v.protocol() == 766 {
                        ProjectilePower::Vector([float; 3])
                    } else {
                        ProjectilePower::Scalar(float)
                    },
                };
                let bytes = value.encode(v, Limits::default()).unwrap();
                match SetProjectilePower::decode(&bytes, v, Limits::default())
                    .unwrap()
                    .power
                {
                    ProjectilePower::Vector(values) => {
                        assert_eq!(values.map(f64::to_bits), [bits; 3])
                    }
                    ProjectilePower::Scalar(value) => assert_eq!(value.to_bits(), bits),
                }
            }
        }
    }
}

#[test]
fn root_names_follow_shared_nbt_framing_adaptation_explicitly() {
    let anonymous = NbtQueryResponse {
        transaction_id: 0,
        data: Some(Nbt::anonymous(Tag::Compound(vec![]))),
    };
    let named = NbtQueryResponse {
        transaction_id: 0,
        data: Some(Nbt {
            name: Some("root".into()),
            root: Tag::Compound(vec![]),
        }),
    };
    let legacy = Version::V1_20;
    assert_eq!(
        anonymous.encode(legacy, Limits::default()).unwrap(),
        [0, 10, 0, 0, 0]
    );
    assert_eq!(
        NbtQueryResponse::decode(&[0, 10, 0, 0, 0], legacy, Limits::default())
            .unwrap()
            .data
            .unwrap()
            .name,
        Some("".into())
    );
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 764) {
        assert_eq!(named.encode(v, Limits::default()).unwrap(), [0, 10, 0]);
        assert_eq!(
            NbtQueryResponse::decode(&[0, 10, 0], v, Limits::default()).unwrap(),
            anonymous
        );
    }
}
