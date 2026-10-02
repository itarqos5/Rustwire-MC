use rustwire_mc::{
    packet::{self, movement::PlayerMovement},
    Error, Limits, Version,
};
#[test]
fn four_golden_forms_all_release_families() {
    for &v in Version::ALL {
        for position in [None, Some([1., 2., -3.])] {
            for rotation in [None, Some([90., -45.])] {
                let value = PlayerMovement {
                    position,
                    rotation,
                    on_ground: true,
                    horizontal_collision: false,
                };
                let mut expected = vec![];
                if position.is_some() {
                    expected.extend_from_slice(&[
                        0x3f, 0xf0, 0, 0, 0, 0, 0, 0, 0x40, 0, 0, 0, 0, 0, 0, 0, 0xc0, 8, 0, 0, 0,
                        0, 0, 0,
                    ]);
                }
                if rotation.is_some() {
                    expected.extend_from_slice(&[0x42, 0xb4, 0, 0, 0xc2, 0x34, 0, 0]);
                }
                expected.push(1);
                assert_eq!(value.encode(v, Limits::default()).unwrap(), expected);
                assert_eq!(
                    PlayerMovement::decode(value.name(), &expected, v, Limits::default()).unwrap(),
                    value
                );
                assert_eq!(value.packet(v, Limits::default()).unwrap().data, expected);
                for end in 0..expected.len() {
                    assert!(PlayerMovement::decode(
                        value.name(),
                        &expected[..end],
                        v,
                        Limits::default()
                    )
                    .is_err());
                }
                expected.push(0);
                assert!(
                    PlayerMovement::decode(value.name(), &expected, v, Limits::default()).is_err()
                );
            }
        }
        assert_eq!(
            packet::player_position(v, 1., 2., -3., true, false).unwrap(),
            PlayerMovement {
                position: Some([1., 2., -3.]),
                on_ground: true,
                ..Default::default()
            }
            .packet(v, Limits::default())
            .unwrap()
        );
    }
}
#[test]
fn flags_versions_limits_and_non_finite_values() {
    for &v in Version::ALL {
        let value = PlayerMovement {
            horizontal_collision: true,
            ..Default::default()
        };
        assert_eq!(
            value.encode(v, Limits::default()).is_ok(),
            v.protocol() >= 768
        );
        if v.protocol() >= 768 {
            assert_eq!(value.encode(v, Limits::default()).unwrap(), [2]);
        }
        assert!(PlayerMovement::decode("flying", &[4], v, Limits::default()).is_err());
        assert!(PlayerMovement::default()
            .encode(
                v,
                Limits {
                    max_packet: 0,
                    ..Limits::default()
                }
            )
            .is_err());
        assert!(PlayerMovement {
            position: Some([f64::NAN, 0., 0.]),
            ..Default::default()
        }
        .packet(v, Limits::default())
        .is_err());
        assert!(PlayerMovement {
            rotation: Some([0., f32::INFINITY]),
            ..Default::default()
        }
        .packet(v, Limits::default())
        .is_err());
        assert!(matches!(
            PlayerMovement::decode("vehicle_move", &[], v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
    }
}
