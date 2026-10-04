use rustwire_mc::{
    packet::{
        entity::Angle,
        entity_control::{EntityControlPacket, MinecartStep, MoveMinecart},
        typed::DecodedPacket,
    },
    version::{Direction, State},
    Error, Limits, Version,
};
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn rows() -> Vec<(Version, i32, &'static str, Vec<u8>)> {
    include_str!("fixtures/minecart.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
        .map(|s| {
            let f: Vec<_> = s.split('\t').collect();
            (
                v(f[0].parse().unwrap()),
                f[1].parse().unwrap(),
                f[2],
                (0..f[3].len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&f[3][i..i + 2], 16).unwrap())
                    .collect(),
            )
        })
        .collect()
}
#[test]
fn independent_minecart_bytes_semantics_and_all_prefixes() {
    let rows = rows();
    assert_eq!(rows.len(), 27);
    for (version, id, case, bytes) in rows {
        let limits = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        let value = MoveMinecart::decode(&bytes, version, limits).unwrap();
        assert_eq!(value.encode(version, limits).unwrap(), bytes);
        assert_eq!(value.packet(version, limits).unwrap().id, id);
        assert_eq!(
            version
                .packet_id(State::Play, Direction::Clientbound, "move_minecart")
                .unwrap(),
            id
        );
        match case {
            "empty" => {
                assert_eq!(value.entity_id, -1);
                assert!(value.steps.is_empty());
            }
            "bits" => {
                assert_eq!(value.entity_id, 300);
                assert_eq!(value.steps.len(), 1);
                let s = &value.steps[0];
                assert_eq!(
                    s.position.map(f64::to_bits),
                    [0x3ff4000000000000, 0xc004000000000000, 0x433fffffffffffff]
                );
                assert_eq!(
                    s.velocity.map(f64::to_bits),
                    [0x8000000000000000, 0x7ff8000000000001, 0x7ff0000000000000]
                );
                assert_eq!((s.yaw, s.pitch), (Angle(255), Angle(128)));
                assert_eq!(s.weight.to_bits(), 0x7fc00001);
            }
            "multiple" => {
                assert_eq!(value.entity_id, i32::MIN);
                assert_eq!(value.steps.len(), 2);
                assert_eq!(value.steps[0].weight.to_bits(), 0x80000000);
            }
            _ => panic!(),
        }
        for end in 0..bytes.len() {
            assert!(MoveMinecart::decode(&bytes[..end], version, limits).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(MoveMinecart::decode(&trailing, version, Limits::default()).is_err());
        let small = Limits {
            max_packet: bytes.len() - 1,
            ..limits
        };
        assert!(MoveMinecart::decode(&bytes, version, small).is_err());
        assert!(value.encode(version, small).is_err());
        let Some(DecodedPacket::EntityControl(EntityControlPacket::Minecart(p))) =
            DecodedPacket::decode(State::Play, "move_minecart", &bytes, version, limits).unwrap()
        else {
            panic!()
        };
        assert_eq!(p.encode(version, limits).unwrap(), bytes);
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(
                DecodedPacket::decode(state, "move_minecart", &bytes, version, limits)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
#[test]
fn counts_bytes_and_availability_fail_closed() {
    let value = MoveMinecart {
        entity_id: 0,
        steps: vec![
            MinecartStep {
                position: [0.; 3],
                velocity: [0.; 3],
                yaw: Angle(0),
                pitch: Angle(0),
                weight: 0.
            };
            2
        ],
    };
    for &version in Version::ALL {
        if version.protocol() < 768 {
            assert!(value.encode(version, Limits::default()).is_err());
            assert!(MoveMinecart::decode(&[0, 0], version, Limits::default()).is_err());
            continue;
        }
        let bytes = value.encode(version, Limits::default()).unwrap();
        let limit = Limits {
            max_collection: 1,
            ..Limits::default()
        };
        assert!(value.encode(version, limit).is_err());
        assert!(MoveMinecart::decode(&bytes, version, limit).is_err());
        let permissive = Limits {
            max_collection: usize::MAX,
            max_packet: usize::MAX,
            ..Limits::default()
        };
        for body in [
            vec![0, 255, 255, 255, 255, 7],
            vec![0, 255, 255, 255, 255, 15],
            vec![0, 128, 128, 128, 128, 16],
        ] {
            assert!(MoveMinecart::decode(&body, version, permissive).is_err());
        }
        // The stale schema would encode nine floats (36 bytes), not a 54-byte step.
        let mut wrong = vec![0, 1];
        wrong.extend([0; 36]);
        assert!(matches!(
            MoveMinecart::decode(&wrong, version, Limits::default()),
            Err(Error::Eof)
        ));
    }
}
#[test]
fn all_byte_mutations_remain_bounded_and_reencode_stably() {
    for (version, _, _, body) in rows() {
        for i in 0..body.len() {
            let mut changed = body.clone();
            changed[i] ^= 255;
            if let Ok(value) = MoveMinecart::decode(&changed, version, Limits::default()) {
                let encoded = value.encode(version, Limits::default()).unwrap();
                assert_eq!(
                    MoveMinecart::decode(&encoded, version, Limits::default())
                        .unwrap()
                        .encode(version, Limits::default())
                        .unwrap(),
                    encoded
                );
            }
        }
    }
}
