use rustwire_mc::{
    codec::{BlockPosition, Writer},
    packet::world_state::*,
    version::{Direction, State},
    Error, Limits, Version,
};

fn hex(s: &str) -> Vec<u8> {
    if s == "-" {
        return Vec::new();
    }
    assert_eq!(s.len() % 2, 0);
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| {
            let digit = |b: u8| match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                _ => panic!("bad hex"),
            };
            digit(b[0]) * 16 + digit(b[1])
        })
        .collect()
}
fn fixtures() -> Vec<(Version, String, i32, Vec<u8>)> {
    include_str!("fixtures/world-state.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
        .map(|s| {
            let p: Vec<_> = s.split('\t').collect();
            (
                Version::from_protocol(p[0].parse().unwrap()).unwrap(),
                p[1].into(),
                p[2].parse().unwrap(),
                hex(p[3]),
            )
        })
        .collect()
}
fn semantic_assertions(value: &WorldStatePacket, v: Version) {
    match value {
        WorldStatePacket::GameState(p) => {
            assert_eq!(p.reason, GameEventReason::RainLevelChange);
            assert_eq!(p.value, -3.5);
        }
        WorldStatePacket::Difficulty(p) => {
            assert_eq!(p.raw_difficulty, 2);
            assert_eq!(p.level(), DifficultyLevel::Normal);
            assert!(p.locked);
        }
        WorldStatePacket::Time(p) => {
            assert_eq!(p.world_age, -123456789);
            match &p.time {
                TimeData::Legacy { day_time } => {
                    assert!(v.protocol() < 768);
                    assert_eq!(*day_time, i64::MIN);
                }
                TimeData::DayTime {
                    day_time,
                    tick_day_time,
                } => {
                    assert!((768..=774).contains(&v.protocol()));
                    assert_eq!(*day_time, i64::MIN);
                    assert!(*tick_day_time);
                }
                TimeData::Clocks(clocks) => {
                    assert!(v.protocol() >= 775);
                    assert_eq!(
                        clocks,
                        &[ClockUpdate {
                            clock_id: 0,
                            total_ticks: -1,
                            partial_tick: 0.25,
                            rate: -2.5
                        }]
                    );
                }
            }
        }
        WorldStatePacket::Spawn(p) => {
            assert_eq!(
                p.position,
                BlockPosition {
                    x: -33554432,
                    y: 2047,
                    z: -33554432
                }
            );
            assert_eq!(p.yaw, -123.5);
            assert_eq!(
                p.dimension.as_deref(),
                if v.protocol() >= 773 {
                    Some("minecraft:overworld")
                } else {
                    None
                }
            );
            assert_eq!(
                p.pitch,
                if v.protocol() >= 773 {
                    Some(-91.5)
                } else {
                    None
                }
            );
        }
        WorldStatePacket::BorderInitialize(p) => {
            assert_eq!(
                (p.x, p.z, p.old_diameter, p.new_diameter),
                (-1.25, 2.5, -3.5, 4.75)
            );
            assert_eq!(p.duration_ms, 1 << 40);
            assert_eq!(
                (p.portal_teleport_boundary, p.warning_blocks, p.warning_time),
                (-1, i32::MIN, i32::MAX)
            );
        }
        WorldStatePacket::BorderCenter(p) => {
            assert_eq!(p.x.to_bits(), 0x7ff8_0000_0000_0000);
            assert_eq!(p.z, f64::NEG_INFINITY);
        }
        WorldStatePacket::BorderLerp(p) => {
            assert_eq!(p.old_diameter, -1.25);
            assert_eq!(p.new_diameter, f64::INFINITY);
            assert_eq!(p.duration_ms, i64::MIN);
        }
        WorldStatePacket::BorderSize(p) => assert_eq!(p.diameter, -3.5),
        WorldStatePacket::BorderWarningDelay(p) => assert_eq!(p.warning_time, i32::MIN),
        WorldStatePacket::BorderWarningDistance(p) => assert_eq!(p.warning_blocks, -1),
        WorldStatePacket::BlockChangeAck(p) => assert_eq!(p.sequence, i32::MIN),
    }
}

#[test]
fn all_fourteen_release_api_goldens_and_every_truncation() {
    let data = fixtures();
    assert_eq!(data.len(), 162);
    for (v, name, id, bytes) in data {
        if name == "player_loaded" {
            let p = PlayerLoaded::decode(&bytes, v, Limits::default()).unwrap();
            assert_eq!(p.packet(v, Limits::default()).unwrap().id, id);
            assert_eq!(p.encode(v, Limits::default()).unwrap(), bytes);
            assert!(PlayerLoaded::decode(&[0], v, Limits::default()).is_err());
            continue;
        }
        let value = WorldStatePacket::decode(&name, &bytes, v, Limits::default()).unwrap();
        semantic_assertions(&value, v);
        let packet = value.packet(v, Limits::default()).unwrap();
        assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
        assert_eq!(packet.id, id, "{v} {name}");
        assert_eq!(packet.data, bytes, "{v} {name}");
        for n in 0..bytes.len() {
            assert!(
                WorldStatePacket::decode(&name, &bytes[..n], v, Limits::default()).is_err(),
                "{v} {name} prefix {n}"
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(WorldStatePacket::decode(&name, &trailing, v, Limits::default()).is_err());
        let limited = Limits {
            max_packet: bytes.len() - 1,
            ..Limits::default()
        };
        assert!(matches!(
            WorldStatePacket::decode(&name, &bytes, v, limited),
            Err(Error::Limit(_))
        ));
        assert!(matches!(value.encode(v, limited), Err(Error::Limit(_))));
        let exact = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        assert_eq!(value.encode(v, exact).unwrap(), bytes);
    }
}

#[test]
fn every_golden_single_byte_mutation_is_bounded_and_stable() {
    let mut count = 0;
    for (v, name, _, bytes) in fixtures() {
        if name == "player_loaded" {
            continue;
        }
        for index in 0..bytes.len() {
            for mask in [1, 0x80, 0xff] {
                let mut mutated = bytes.clone();
                mutated[index] ^= mask;
                count += 1;
                let limits = Limits {
                    max_packet: 4096,
                    max_collection: 32,
                    ..Limits::default()
                };
                if let Ok(value) = WorldStatePacket::decode(&name, &mutated, v, limits) {
                    let canonical = value.encode(v, limits).unwrap();
                    let decoded = WorldStatePacket::decode(&name, &canonical, v, limits).unwrap();
                    assert_eq!(decoded.encode(v, limits).unwrap(), canonical);
                }
            }
        }
    }
    assert_eq!(count, 6807);
}

#[test]
fn game_event_boundaries_and_raw_float_bits() {
    for &v in Version::ALL {
        let maximum = if v.protocol() == 763 {
            11
        } else if v.protocol() == 764 {
            12
        } else {
            13
        };
        for id in 0..=255u8 {
            let mut bytes = vec![id];
            bytes.extend((-3.5f32).to_be_bytes());
            let result = GameStateChange::decode(&bytes, v, Limits::default());
            if id <= maximum {
                assert_eq!(result.unwrap().encode(v, Limits::default()).unwrap(), bytes);
            } else {
                assert!(matches!(result, Err(Error::Unsupported(_))));
                for len in 0..5 {
                    assert!(!matches!(
                        GameStateChange::decode(&bytes[..len], v, Limits::default()),
                        Err(Error::Unsupported(_))
                    ));
                }
                bytes.push(0);
                assert!(!matches!(
                    GameStateChange::decode(&bytes, v, Limits::default()),
                    Err(Error::Unsupported(_))
                ));
            }
        }
        for bits in [
            0x7fc0_0000,
            0x7fc0_1234,
            0xffc0_4321,
            0x7f80_0000,
            0xff80_0000,
            0x8000_0000,
        ] {
            let p = GameStateChange {
                reason: GameEventReason::RainLevelChange,
                value: f32::from_bits(bits),
            };
            let bytes = p.encode(v, Limits::default()).unwrap();
            assert_eq!(
                GameStateChange::decode(&bytes, v, Limits::default())
                    .unwrap()
                    .value
                    .to_bits(),
                bits
            );
        }
    }
    assert!(GameStateChange {
        reason: GameEventReason::LimitedCrafting,
        value: 0.0
    }
    .encode(Version::V1_20, Limits::default())
    .is_err());
    assert!(GameStateChange {
        reason: GameEventReason::LevelChunksLoadStart,
        value: 0.0
    }
    .encode(Version::V1_20_2, Limits::default())
    .is_err());
}

#[test]
fn difficulty_wrapping_preserves_wire_integer() {
    for &v in Version::ALL {
        for n in [i32::MIN, -1, 0, 1, 2, 3, 4, 127, 255, 256, i32::MAX] {
            let value = Difficulty {
                raw_difficulty: n,
                locked: false,
            };
            if v.protocol() < 771 && !(0..=255).contains(&n) {
                assert!(value.encode(v, Limits::default()).is_err());
                continue;
            }
            let mut bytes = Writer::new();
            if v.protocol() < 771 {
                bytes.u8(n as u8);
            } else {
                bytes.var_i32(n);
            }
            bytes.bool(false);
            assert_eq!(
                value.encode(v, Limits::default()).unwrap(),
                bytes.as_slice()
            );
            assert_eq!(
                Difficulty::decode(bytes.as_slice(), v, Limits::default()).unwrap(),
                value
            );
            assert_eq!(
                value.level(),
                [
                    DifficultyLevel::Peaceful,
                    DifficultyLevel::Easy,
                    DifficultyLevel::Normal,
                    DifficultyLevel::Hard
                ][n.rem_euclid(4) as usize]
            );
        }
        assert!(Difficulty::decode(&[0, 2], v, Limits::default()).is_err());
    }
}

#[test]
fn time_layouts_are_exact_and_clock_allocations_are_bounded() {
    let variants = [
        TimeData::Legacy { day_time: -1 },
        TimeData::DayTime {
            day_time: 0,
            tick_day_time: false,
        },
        TimeData::Clocks(vec![]),
    ];
    for &v in Version::ALL {
        let expected = if v.protocol() < 768 {
            0
        } else if v.protocol() < 775 {
            1
        } else {
            2
        };
        for (i, time) in variants.iter().enumerate() {
            assert_eq!(
                UpdateTime {
                    world_age: 0,
                    time: time.clone()
                }
                .encode(v, Limits::default())
                .is_ok(),
                i == expected
            );
        }
    }
    for v in [Version::V26_1, Version::V26_2] {
        for count in [-1, i32::MAX] {
            let mut w = Writer::new();
            w.i64(0);
            w.var_i32(count);
            assert!(UpdateTime::decode(w.as_slice(), v, Limits::default()).is_err());
        }
        let c = ClockUpdate {
            clock_id: i32::MAX,
            total_ticks: i64::MIN,
            partial_tick: f32::INFINITY,
            rate: f32::from_bits(0xffc0_1234),
        };
        let value = UpdateTime {
            world_age: i64::MIN,
            time: TimeData::Clocks(vec![c, c]),
        };
        let bytes = value.encode(v, Limits::default()).unwrap();
        let decoded = UpdateTime::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(decoded.encode(v, Limits::default()).unwrap(), bytes);
        let TimeData::Clocks(clocks) = decoded.time else {
            panic!()
        };
        assert_eq!(clocks.len(), 2);
        assert_eq!(clocks[0].clock_id, i32::MAX);
        assert_eq!(clocks[0].rate.to_bits(), 0xffc0_1234);
        let small = Limits {
            max_collection: 1,
            ..Limits::default()
        };
        assert!(matches!(
            UpdateTime::decode(&bytes, v, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(value.encode(v, small), Err(Error::Limit(_))));
        assert!(UpdateTime {
            world_age: 0,
            time: TimeData::Clocks(vec![ClockUpdate { clock_id: -1, ..c }])
        }
        .encode(v, Limits::default())
        .is_err());
        let bad = hex("000000000000000001ffffffff0f01000000003f800000");
        assert!(UpdateTime::decode(&bad, v, Limits::default()).is_err());
        let no = Limits {
            max_collection: 0,
            ..Limits::default()
        };
        assert!(UpdateTime {
            world_age: 0,
            time: TimeData::Clocks(vec![])
        }
        .encode(v, no)
        .is_ok());
    }
}

#[test]
fn spawn_version_identifier_and_position_domains() {
    for &v in Version::ALL {
        let modern = v.protocol() >= 773;
        let mut p = SpawnPosition {
            dimension: modern.then(|| "minecraft:overworld".into()),
            position: BlockPosition {
                x: -33554432,
                y: -2048,
                z: 33554431,
            },
            yaw: f32::from_bits(0x7fc0_1234),
            pitch: modern.then_some(f32::NEG_INFINITY),
        };
        let bytes = p.encode(v, Limits::default()).unwrap();
        let got = SpawnPosition::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(got.position, p.position);
        assert_eq!(got.yaw.to_bits(), p.yaw.to_bits());
        assert_eq!(got.pitch, p.pitch);
        p.dimension = if modern {
            None
        } else {
            Some("minecraft:overworld".into())
        };
        assert!(p.encode(v, Limits::default()).is_err());
        p.dimension = modern.then(|| "minecraft:overworld".into());
        p.pitch = if modern { None } else { Some(0.0) };
        assert!(p.encode(v, Limits::default()).is_err());
        p.pitch = modern.then_some(0.0);
        p.position.y = 2048;
        assert!(p.encode(v, Limits::default()).is_err());
        p.position.y = 0;
        if modern {
            for id in ["", ":", "minecraft:", "..:x", "UPPER:x"] {
                p.dimension = Some(id.into());
                let valid = id != "UPPER:x" && (v.protocol() < 775 || id != "..:x");
                assert_eq!(p.encode(v, Limits::default()).is_ok(), valid);
                let mut w = Writer::new();
                w.string(id, 32767).unwrap();
                w.i64(0);
                w.f32(0.0);
                w.f32(0.0);
                assert_eq!(
                    SpawnPosition::decode(w.as_slice(), v, Limits::default()).is_ok(),
                    valid
                );
            }
            p.dimension = Some("minecraft:overworld".into());
            let limited = Limits {
                max_string_chars: 2,
                ..Limits::default()
            };
            assert!(p.encode(v, limited).is_err());
            assert!(SpawnPosition::decode(&bytes, v, limited).is_err());
        }
    }
}

#[test]
fn signed_border_duration_and_sequence_domains() {
    for &v in Version::ALL {
        for n in [i64::MIN, -1, 0, 1, i64::MAX] {
            let p = WorldBorderLerpSize {
                old_diameter: f64::from_bits(0xfff8_0000_0000_4321),
                new_diameter: -0.0,
                duration_ms: n,
            };
            let b = p.encode(v, Limits::default()).unwrap();
            let q = WorldBorderLerpSize::decode(&b, v, Limits::default()).unwrap();
            assert_eq!(q.duration_ms, n);
            assert_eq!(q.old_diameter.to_bits(), p.old_diameter.to_bits());
            assert_eq!(q.new_diameter.to_bits(), p.new_diameter.to_bits());
        }
        for n in [i32::MIN, -1, 0, 1, i32::MAX] {
            let p = BlockChangeAck { sequence: n };
            let b = p.encode(v, Limits::default()).unwrap();
            assert_eq!(BlockChangeAck::decode(&b, v, Limits::default()).unwrap(), p);
        }
        for bad in [
            vec![0x80; 11],
            vec![0x80; 9].into_iter().chain([2]).collect(),
        ] {
            let mut b = vec![0; 16];
            b.extend(bad);
            assert!(WorldBorderLerpSize::decode(&b, v, Limits::default()).is_err());
        }
        assert!(
            BlockChangeAck::decode(&[0xff, 0xff, 0xff, 0xff, 0x7f], v, Limits::default()).is_err()
        );
    }
}

#[test]
fn player_loaded_presence_direction_and_empty_body() {
    for &v in Version::ALL {
        let limits = Limits {
            max_packet: 0,
            ..Limits::default()
        };
        if v.protocol() < 769 {
            assert!(PlayerLoaded.encode(v, limits).is_err());
            assert!(PlayerLoaded::decode(&[], v, limits).is_err());
            assert!(PlayerLoaded.packet(v, limits).is_err());
        } else {
            assert_eq!(PlayerLoaded::decode(&[], v, limits).unwrap(), PlayerLoaded);
            let p = PlayerLoaded.packet(v, limits).unwrap();
            assert!(p.data.is_empty());
            assert_eq!(
                p.id,
                v.packet_id(State::Play, Direction::Serverbound, "player_loaded")
                    .unwrap()
            );
            assert!(PlayerLoaded::decode(&[0], v, limits).is_err());
        }
    }
    assert!(matches!(
        WorldStatePacket::decode("unknown", &[], Version::V26_2, Limits::default()),
        Err(Error::Unsupported(_))
    ));
}
