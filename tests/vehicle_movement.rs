use rustwire_mc::{
    codec::{Reader, Writer},
    frame::FrameCodec,
    packet::{
        movement::VehicleMovement,
        typed::DecodedPacket,
        world_control::{VehicleMove, WorldControlPacket},
    },
    version::{Direction, State},
    Error, Limits, Version,
};

struct Fixture {
    version: Version,
    direction: Direction,
    id: i32,
    case: &'static str,
    bytes: Vec<u8>,
}
fn fixtures() -> Vec<Fixture> {
    include_str!("fixtures/vehicle-movement.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            Fixture {
                version: Version::from_protocol(fields[0].parse().unwrap()).unwrap(),
                direction: match fields[1] {
                    "serverbound" => Direction::Serverbound,
                    "clientbound" => Direction::Clientbound,
                    _ => panic!("fixture direction"),
                },
                id: fields[2].parse().unwrap(),
                case: fields[3],
                bytes: (0..fields[4].len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&fields[4][i..i + 2], 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}
fn expected(f: &Fixture) -> VehicleMovement {
    let (position, yaw, pitch) = match f.case.split('-').next().unwrap() {
        "finite" => ([-1.25, 2.5, -3.75], 450., -180.),
        "extrema" => (
            [f64::MAX, f64::MIN_POSITIVE, f64::from_bits(1)],
            f32::MAX,
            f32::from_bits(1),
        ),
        "bits" => (
            [f64::from_bits(0x7ff8000000001234), -0., f64::INFINITY],
            f32::NEG_INFINITY,
            f32::from_bits(0x7fc01234),
        ),
        _ => panic!("fixture case"),
    };
    VehicleMovement {
        position,
        yaw,
        pitch,
        on_ground: if f.direction == Direction::Serverbound && f.version.protocol() >= 769 {
            Some(f.case.ends_with("-ground"))
        } else {
            None
        },
    }
}
fn assert_bits(actual: &VehicleMovement, expected: &VehicleMovement) {
    assert_eq!(
        actual.position.map(f64::to_bits),
        expected.position.map(f64::to_bits)
    );
    assert_eq!(actual.yaw.to_bits(), expected.yaw.to_bits());
    assert_eq!(actual.pitch.to_bits(), expected.pitch.to_bits());
    assert_eq!(actual.on_ground, expected.on_ground);
}
fn blank(version: Version) -> VehicleMovement {
    VehicleMovement {
        position: [0.; 3],
        yaw: 0.,
        pitch: 0.,
        on_ground: (version.protocol() >= 769).then_some(false),
    }
}

#[test]
fn original_outbound_fixtures_encode_decode_semantics_ids_and_full_frames() {
    let rows = fixtures();
    assert_eq!(rows.len(), 108);
    let mut count = 0;
    for f in rows
        .into_iter()
        .filter(|f| f.direction == Direction::Serverbound)
    {
        count += 1;
        let value = expected(&f);
        assert_eq!(value.encode(f.version, Limits::default()).unwrap(), f.bytes);
        assert_bits(
            &VehicleMovement::decode(&f.bytes, f.version, Limits::default()).unwrap(),
            &value,
        );
        let packet = value.packet(f.version, Limits::default()).unwrap();
        assert_eq!(packet.id, f.id);
        assert_eq!(packet.data, f.bytes);
        assert_eq!(
            f.version
                .packet_id(State::Play, Direction::Serverbound, "vehicle_move")
                .unwrap(),
            f.id
        );
        assert_ne!(
            f.id,
            f.version
                .packet_id(State::Play, Direction::Clientbound, "vehicle_move")
                .unwrap()
        );
        // All fixture IDs/lengths fit one byte, so no Rust VarInt encoder is used
        // to construct these expected, uncompressed whole frames.
        let mut expected_frame = vec![(f.bytes.len() + 1) as u8, f.id as u8];
        expected_frame.extend_from_slice(&f.bytes);
        assert_eq!(
            FrameCodec::default().encode(&packet).unwrap(),
            expected_frame
        );
    }
    assert_eq!(count, 66);
}

#[test]
fn clientbound_layout_dispatch_and_packet_identity_remain_unchanged() {
    let mut count = 0;
    for f in fixtures()
        .into_iter()
        .filter(|f| f.direction == Direction::Clientbound)
    {
        count += 1;
        let value = expected(&f);
        let inbound = VehicleMove {
            position: value.position,
            yaw: value.yaw,
            pitch: value.pitch,
        };
        assert_eq!(f.bytes.len(), 32);
        assert_eq!(
            inbound.encode(f.version, Limits::default()).unwrap(),
            f.bytes
        );
        let raw = inbound.packet(f.version, Limits::default()).unwrap();
        assert_eq!(raw.id, f.id);
        assert_eq!(raw.data, f.bytes);
        let Some(DecodedPacket::WorldControl(WorldControlPacket::Vehicle(decoded))) =
            DecodedPacket::decode(
                State::Play,
                "vehicle_move",
                &f.bytes,
                f.version,
                Limits::default(),
            )
            .unwrap()
        else {
            panic!("clientbound dispatch changed")
        };
        assert_bits(
            &VehicleMovement {
                position: decoded.position,
                yaw: decoded.yaw,
                pitch: decoded.pitch,
                on_ground: None,
            },
            &value,
        );
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(DecodedPacket::decode(
                state,
                "vehicle_move",
                &f.bytes,
                f.version,
                Limits::default()
            )
            .unwrap()
            .is_none());
        }
    }
    assert_eq!(count, 42);
}

#[test]
fn every_strict_prefix_and_trailing_byte_is_rejected() {
    let mut prefixes = 0;
    for f in fixtures() {
        for end in 0..f.bytes.len() {
            prefixes += 1;
            if f.direction == Direction::Serverbound {
                assert!(matches!(
                    VehicleMovement::decode(&f.bytes[..end], f.version, Limits::default()),
                    Err(Error::Eof)
                ));
            } else {
                assert!(matches!(
                    VehicleMove::decode(&f.bytes[..end], f.version, Limits::default()),
                    Err(Error::Eof)
                ));
                assert!(matches!(
                    DecodedPacket::decode(
                        State::Play,
                        "vehicle_move",
                        &f.bytes[..end],
                        f.version,
                        Limits::default()
                    ),
                    Err(Error::Eof)
                ));
            }
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        if f.direction == Direction::Serverbound {
            assert!(matches!(
                VehicleMovement::decode(&trailing, f.version, Limits::default()),
                Err(Error::Invalid("trailing bytes"))
            ));
        } else {
            assert!(matches!(
                VehicleMove::decode(&trailing, f.version, Limits::default()),
                Err(Error::Invalid("trailing bytes"))
            ));
            assert!(matches!(
                DecodedPacket::decode(
                    State::Play,
                    "vehicle_move",
                    &trailing,
                    f.version,
                    Limits::default()
                ),
                Err(Error::Invalid("trailing bytes"))
            ));
        }
    }
    assert_eq!(prefixes, 3504);
}

#[test]
fn version_presence_is_explicit_and_cannot_drop_or_invent_ground_state() {
    for &version in Version::ALL {
        for on_ground in [None, Some(false), Some(true)] {
            let value = VehicleMovement {
                on_ground,
                ..blank(version)
            };
            let valid = on_ground.is_some() == (version.protocol() >= 769);
            if valid {
                let bytes = value.encode(version, Limits::default()).unwrap();
                assert_eq!(bytes.len(), 32 + usize::from(version.protocol() >= 769));
                assert_eq!(
                    VehicleMovement::decode(&bytes, version, Limits::default()).unwrap(),
                    value
                );
            } else {
                assert!(matches!(
                    value.encode(version, Limits::default()),
                    Err(Error::Unsupported(_))
                ));
                assert!(matches!(
                    value.packet(version, Limits::default()),
                    Err(Error::Unsupported(_))
                ));
                let mut w = Writer::new();
                w.raw(&[8, 9]);
                assert!(matches!(
                    value.write(&mut w, version, Limits::default()),
                    Err(Error::Unsupported(_))
                ));
                assert_eq!(w.as_slice(), &[8, 9]);
            }
        }
        if version.protocol() >= 769 {
            assert!(matches!(
                VehicleMovement::decode(&[0; 32], version, Limits::default()),
                Err(Error::Eof)
            ));
        } else {
            for flag in [0, 1] {
                let mut bytes = vec![0; 32];
                bytes.push(flag);
                assert!(matches!(
                    VehicleMovement::decode(&bytes, version, Limits::default()),
                    Err(Error::Invalid("trailing bytes"))
                ));
            }
        }
    }
}

#[test]
fn every_invalid_boolean_is_rejected_without_consuming_the_reader() {
    for &version in Version::ALL.iter().filter(|v| v.protocol() >= 769) {
        for invalid in 2..=255u8 {
            let mut bytes = vec![0; 33];
            bytes[32] = invalid;
            assert!(matches!(
                VehicleMovement::decode(&bytes, version, Limits::default()),
                Err(Error::Invalid("boolean"))
            ));
            let mut r = Reader::new(&bytes, Limits::default());
            assert!(matches!(
                VehicleMovement::read(&mut r, version),
                Err(Error::Invalid("boolean"))
            ));
            assert_eq!(r.position(), 0);
        }
    }
}

#[test]
fn exact_packet_budgets_and_every_smaller_limit_are_enforced() {
    for f in fixtures()
        .into_iter()
        .filter(|f| f.direction == Direction::Serverbound)
    {
        let value = expected(&f);
        let exact = Limits {
            max_packet: f.bytes.len(),
            max_collection: 0,
            max_string_chars: 0,
            max_nbt_depth: 0,
            max_nbt_nodes: 0,
            ..Limits::default()
        };
        assert_eq!(value.encode(f.version, exact).unwrap(), f.bytes);
        assert_bits(
            &VehicleMovement::decode(&f.bytes, f.version, exact).unwrap(),
            &value,
        );
        for max_packet in 0..f.bytes.len() {
            let small = Limits {
                max_packet,
                ..exact
            };
            assert!(matches!(
                value.encode(f.version, small),
                Err(Error::Limit(_))
            ));
            assert!(matches!(
                value.packet(f.version, small),
                Err(Error::Limit(_))
            ));
            assert!(matches!(
                VehicleMovement::decode(&f.bytes, f.version, small),
                Err(Error::Limit(_))
            ));
            let mut r = Reader::new(&f.bytes, small);
            assert!(matches!(
                VehicleMovement::read(&mut r, f.version),
                Err(Error::Limit(_))
            ));
            assert_eq!(r.position(), 0);
            let mut w = Writer::new();
            w.raw(&[10, 11]);
            assert!(matches!(
                value.write(&mut w, f.version, small),
                Err(Error::Limit(_))
            ));
            assert_eq!(w.as_slice(), &[10, 11]);
        }
    }
}

#[test]
fn reader_is_atomic_at_every_prefix_and_consumes_exactly_one_body() {
    for f in fixtures()
        .into_iter()
        .filter(|f| f.direction == Direction::Serverbound)
    {
        for end in 0..f.bytes.len() {
            let mut bytes = vec![99];
            bytes.extend_from_slice(&f.bytes[..end]);
            let mut r = Reader::new(&bytes, Limits::default());
            assert_eq!(r.u8().unwrap(), 99);
            assert!(matches!(
                VehicleMovement::read(&mut r, f.version),
                Err(Error::Eof)
            ));
            assert_eq!(r.position(), 1);
        }
        let mut bytes = f.bytes.clone();
        bytes.extend_from_slice(&[7, 8]);
        let mut r = Reader::new(
            &bytes,
            Limits {
                max_packet: f.bytes.len(),
                ..Limits::default()
            },
        );
        assert_bits(
            &VehicleMovement::read(&mut r, f.version).unwrap(),
            &expected(&f),
        );
        assert_eq!(r.remaining(), &[7, 8]);
        let mut w = Writer::new();
        w.raw(&[9]);
        expected(&f)
            .write(&mut w, f.version, Limits::default())
            .unwrap();
        assert_eq!(&w.as_slice()[1..], f.bytes);
        assert_eq!(w.as_slice()[0], 9);
    }
}

#[test]
fn each_coordinate_and_angle_retains_signed_zero_extrema_and_non_finite_bits() {
    for &version in Version::ALL {
        let limits = Limits::default();
        for bits in [
            0,
            0x8000000000000000,
            1,
            0x8000000000000001,
            0x7fefffffffffffff,
            0xffefffffffffffff,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8000000001234,
            0xfff800000000abcd,
        ] {
            for index in 0..3 {
                let mut value = blank(version);
                value.position[index] = f64::from_bits(bits);
                let mut independent = vec![0; 32];
                independent[index * 8..index * 8 + 8].copy_from_slice(&bits.to_be_bytes());
                if version.protocol() >= 769 {
                    independent.push(0);
                }
                assert_eq!(value.encode(version, limits).unwrap(), independent);
                assert_bits(
                    &VehicleMovement::decode(&independent, version, limits).unwrap(),
                    &value,
                );
            }
        }
        for bits in [
            0, 0x80000000, 1, 0x80000001, 0x7f7fffff, 0xff7fffff, 0x7f800000, 0xff800000,
            0x7fc01234, 0xffc0abcd,
        ] {
            for index in 0..2 {
                let mut value = blank(version);
                if index == 0 {
                    value.yaw = f32::from_bits(bits)
                } else {
                    value.pitch = f32::from_bits(bits)
                }
                let mut independent = vec![0; 32];
                independent[24 + index * 4..28 + index * 4].copy_from_slice(&bits.to_be_bytes());
                if version.protocol() >= 769 {
                    independent.push(0);
                }
                assert_eq!(value.encode(version, limits).unwrap(), independent);
                assert_bits(
                    &VehicleMovement::decode(&independent, version, limits).unwrap(),
                    &value,
                );
            }
        }
    }
}

#[test]
fn modern_outbound_bodies_are_never_mistaken_for_clientbound_corrections() {
    for f in fixtures()
        .into_iter()
        .filter(|f| f.direction == Direction::Serverbound && f.version.protocol() >= 769)
    {
        assert!(matches!(
            VehicleMove::decode(&f.bytes, f.version, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
        assert!(matches!(
            DecodedPacket::decode(
                State::Play,
                "vehicle_move",
                &f.bytes,
                f.version,
                Limits::default()
            ),
            Err(Error::Invalid("trailing bytes"))
        ));
        assert!(matches!(
            VehicleMovement::decode(&f.bytes[..32], f.version, Limits::default()),
            Err(Error::Eof)
        ));
    }
}
