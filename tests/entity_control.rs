use rustwire_mc::{
    packet::{
        entity::Angle,
        entity_control::{
            AttachEntity, DamageEvent, EntityAnimation, EntityControlPacket, EntityHeadRotation,
            HurtAnimation, SetCamera, SetPassengers,
        },
    },
    version::{Direction, State},
    Error, Limits, Version,
};

fn fixture_bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn python_golden_bodies_and_exact_packet_ids_across_fourteen_protocols() {
    let mut count = 0;
    for line in include_str!("fixtures/entity-control.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 5);
        let version = Version::from_protocol(fields[0].parse().unwrap()).unwrap();
        let name = fields[1];
        let expected_id = fields[2].parse::<i32>().unwrap();
        let bytes = fixture_bytes(fields[4]);
        let value = EntityControlPacket::decode(name, &bytes, version, Limits::default()).unwrap();
        // Independent fixture values establish field meaning/order, not merely
        // a decode/encode cycle that could hide matching implementation errors.
        match (fields[3], &value) {
            ("ordered_signed", EntityControlPacket::Passengers(value)) => {
                assert_eq!(value.entity_id, -1);
                assert_eq!(value.passengers, [127, 128, -1, i32::MIN, i32::MAX, 128]);
            }
            ("zero_holder", EntityControlPacket::Attach(value)) => {
                assert_eq!(value.entity_id, 300);
                assert_eq!(value.holder_id, 0);
            }
            ("byte_high", EntityControlPacket::HeadRotation(value)) => {
                assert_eq!(value.entity_id, 300);
                assert_eq!(value.head_yaw, Angle(255));
            }
            ("ordinary", EntityControlPacket::Camera(value)) => {
                assert_eq!(value.camera_id, 300);
            }
            ("ordinary", EntityControlPacket::Animation(value)) => {
                assert_eq!(value.entity_id, 300);
                assert_eq!(value.animation, 4);
            }
            ("position", EntityControlPacket::Damage(value)) => {
                assert_eq!(value.entity_id, 300);
                assert_eq!(value.source_type_id, 2);
                assert_eq!(value.source_cause_id, 129);
                assert_eq!(value.source_direct_id, 256);
                assert_eq!(value.source_position, Some([-1.25, 64.5, 30000000.0]));
            }
            ("ordinary", EntityControlPacket::HurtAnimation(value)) => {
                assert_eq!(value.entity_id, 300);
                assert_eq!(value.yaw, -90.5);
            }
            _ => {}
        }
        assert_eq!(value.encode(version, Limits::default()).unwrap(), bytes);
        let packet = value.packet(version, Limits::default()).unwrap();
        assert_eq!(packet.id, expected_id, "{version} {name}");
        assert_eq!(packet.data, bytes);
        assert_eq!(
            version
                .packet(State::Play, Direction::Clientbound, expected_id)
                .unwrap()
                .name,
            name
        );
        let exact = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        assert!(EntityControlPacket::decode(name, &bytes, version, exact).is_ok());
        assert_eq!(value.encode(version, exact).unwrap(), bytes);
        let small = Limits {
            max_packet: bytes.len() - 1,
            ..Limits::default()
        };
        assert!(matches!(
            EntityControlPacket::decode(name, &bytes, version, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(value.encode(version, small), Err(Error::Limit(_))));
        for length in 0..bytes.len() {
            assert!(
                EntityControlPacket::decode(name, &bytes[..length], version, Limits::default())
                    .is_err(),
                "{version} {name}/{} prefix {length}",
                fields[3]
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            EntityControlPacket::decode(name, &trailing, version, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
        count += 1;
    }
    assert_eq!(count, 224);
}

#[test]
fn passengers_preserve_order_duplicates_and_signed_references() {
    let value = SetPassengers {
        entity_id: i32::MIN,
        passengers: vec![i32::MAX, -1, 0, i32::MIN, 0],
    };
    for &version in Version::ALL {
        let limits = Limits {
            max_collection: value.passengers.len(),
            ..Limits::default()
        };
        let bytes = value.encode(version, limits).unwrap();
        assert_eq!(
            SetPassengers::decode(&bytes, version, limits).unwrap(),
            value
        );
        let too_few = Limits {
            max_collection: value.passengers.len() - 1,
            ..limits
        };
        assert!(matches!(
            value.encode(version, too_few),
            Err(Error::Limit("passenger count"))
        ));
        assert!(matches!(
            SetPassengers::decode(&bytes, version, too_few),
            Err(Error::Limit("collection length"))
        ));
        let empty = SetPassengers {
            entity_id: 1,
            passengers: vec![],
        };
        let zero = Limits {
            max_collection: 0,
            max_packet: 2,
            ..Limits::default()
        };
        assert_eq!(empty.encode(version, zero).unwrap(), [1, 0]);
        assert_eq!(
            SetPassengers::decode(&[1, 0], version, zero).unwrap(),
            empty
        );
    }
}

#[test]
fn passenger_count_is_checked_before_allocation() {
    let unlimited_collection = Limits {
        max_collection: usize::MAX,
        ..Limits::default()
    };
    for &version in Version::ALL {
        // An i32::MAX count has no following entries: no huge allocation.
        assert!(matches!(
            SetPassengers::decode(&[0, 255, 255, 255, 255, 7], version, unlimited_collection),
            Err(Error::Eof)
        ));
        assert!(matches!(
            SetPassengers::decode(&[0, 255, 255, 255, 255, 15], version, Limits::default()),
            Err(Error::Invalid("negative length"))
        ));
        assert!(matches!(
            SetPassengers::decode(&[0, 2, 1], version, Limits::default()),
            Err(Error::Eof)
        ));
        let value = SetPassengers {
            entity_id: -1,
            passengers: vec![-1; 129],
        };
        let encoded = value.encode(version, Limits::default()).unwrap();
        assert_eq!(encoded.len(), 5 + 2 + 5 * 129);
        for maximum in [0, 1, 6, 7, 8, 130, encoded.len() - 1] {
            assert!(matches!(
                value.encode(
                    version,
                    Limits {
                        max_packet: maximum,
                        ..Limits::default()
                    }
                ),
                Err(Error::Limit(_))
            ));
        }
    }
}

#[test]
fn attachment_uses_two_fixed_ints_including_negative_holder() {
    for &version in Version::ALL {
        let value = AttachEntity {
            entity_id: 300,
            holder_id: -1,
        };
        let bytes = [0, 0, 1, 44, 255, 255, 255, 255];
        assert_eq!(value.encode(version, Limits::default()).unwrap(), bytes);
        assert_eq!(
            AttachEntity::decode(&bytes, version, Limits::default()).unwrap(),
            value
        );
        // Two VarInts are not an attachment body.
        assert!(AttachEntity::decode(&[172, 2, 1], version, Limits::default()).is_err());
    }
}

#[test]
fn all_head_angles_and_animation_bytes_are_lossless() {
    for &version in Version::ALL {
        for byte in 0..=255 {
            let rotation = EntityHeadRotation {
                entity_id: -1,
                head_yaw: Angle(byte),
            };
            let bytes = rotation.encode(version, Limits::default()).unwrap();
            assert_eq!(bytes.len(), 6);
            assert_eq!(
                EntityHeadRotation::decode(&bytes, version, Limits::default()).unwrap(),
                rotation
            );
            let animation = EntityAnimation {
                entity_id: 128,
                animation: byte,
            };
            let bytes = animation.encode(version, Limits::default()).unwrap();
            assert_eq!(bytes, [128, 1, byte]);
            assert_eq!(
                EntityAnimation::decode(&bytes, version, Limits::default()).unwrap(),
                animation
            );
        }
        // An attempted VarInt animation 128 leaves a trailing byte.
        assert!(matches!(
            EntityAnimation::decode(&[0, 128, 1], version, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
    }
}

#[test]
fn camera_and_damage_references_retain_raw_signed_domains() {
    for &version in Version::ALL {
        for id in [i32::MIN, -1, 0, 1, 127, 128, i32::MAX] {
            let value = SetCamera { camera_id: id };
            assert_eq!(
                SetCamera::decode(
                    &value.encode(version, Limits::default()).unwrap(),
                    version,
                    Limits::default()
                )
                .unwrap(),
                value
            );
            let damage = DamageEvent {
                entity_id: id,
                source_type_id: id,
                source_cause_id: id,
                source_direct_id: id,
                source_position: None,
            };
            assert_eq!(
                DamageEvent::decode(
                    &damage.encode(version, Limits::default()).unwrap(),
                    version,
                    Limits::default()
                )
                .unwrap(),
                damage
            );
        }
        let damage =
            DamageEvent::decode(&[1, 2, 0, 129, 1, 0], version, Limits::default()).unwrap();
        assert_eq!(damage.source_cause_id, 0);
        assert_eq!(damage.source_direct_id, 129); // Retain the wire offset; do not subtract one.
        assert_eq!(damage.source_position, None);
    }
}

#[test]
fn damage_option_flag_and_varints_fail_closed() {
    let bad_varints: &[&[u8]] = &[
        &[128],
        &[128, 128, 128, 128, 16],
        &[128, 128, 128, 128, 128, 0],
    ];
    for &version in Version::ALL {
        for flag in 2..=255 {
            assert!(matches!(
                DamageEvent::decode(&[1, 2, 0, 0, flag], version, Limits::default()),
                Err(Error::Invalid("boolean"))
            ));
        }
        assert!(matches!(
            DamageEvent::decode(&[1, 2, 0, 0, 1], version, Limits::default()),
            Err(Error::Eof)
        ));
        for bad in bad_varints {
            assert!(SetCamera::decode(bad, version, Limits::default()).is_err());
            assert!(SetPassengers::decode(bad, version, Limits::default()).is_err());
            for index in 0..4 {
                let mut bytes = vec![0; index];
                bytes.extend_from_slice(bad);
                assert!(DamageEvent::decode(&bytes, version, Limits::default()).is_err());
            }
            let mut passenger = vec![0, 1];
            passenger.extend_from_slice(bad);
            assert!(SetPassengers::decode(&passenger, version, Limits::default()).is_err());
        }
    }
}

#[test]
fn float_bits_are_retained_without_gameplay_range_checks() {
    for &version in Version::ALL {
        for bits in [
            0, 0x80000000, 0x7f800000, 0xff800000, 0x7fc00123, 0x7fa00123,
        ] {
            let value = HurtAnimation {
                entity_id: 0,
                yaw: f32::from_bits(bits),
            };
            let encoded = value.encode(version, Limits::default()).unwrap();
            let decoded = HurtAnimation::decode(&encoded, version, Limits::default()).unwrap();
            assert_eq!(decoded.yaw.to_bits(), bits);
            assert_eq!(decoded.encode(version, Limits::default()).unwrap(), encoded);
        }
        for bits in [
            0,
            0x8000000000000000,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8000000000123,
            0x7ff0000000000123,
        ] {
            let value = DamageEvent {
                entity_id: 0,
                source_type_id: 0,
                source_cause_id: 0,
                source_direct_id: 0,
                source_position: Some([f64::from_bits(bits); 3]),
            };
            let encoded = value.encode(version, Limits::default()).unwrap();
            let decoded = DamageEvent::decode(&encoded, version, Limits::default()).unwrap();
            assert_eq!(
                decoded.source_position.unwrap().map(f64::to_bits),
                [bits; 3]
            );
            assert_eq!(decoded.encode(version, Limits::default()).unwrap(), encoded);
        }
    }
}

#[test]
fn unknown_names_and_unlisted_protocols_are_not_guessed() {
    for &version in Version::ALL {
        assert!(matches!(
            EntityControlPacket::decode("future_entity_control", &[], version, Limits::default()),
            Err(Error::Unsupported("typed entity-control packet"))
        ));
    }
    for protocol in [i32::MIN, 0, 762, 777, i32::MAX] {
        assert!(matches!(
            Version::from_protocol(protocol),
            Err(Error::Unsupported(_))
        ));
    }
}
