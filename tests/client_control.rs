use rustwire_mc::{
    codec::{Reader, Writer},
    packet::{client_control::*, typed::DecodedPacket},
    version::{Direction, State},
    Error, Limits, Version,
};
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
struct Fixture {
    version: Version,
    name: &'static str,
    id: i32,
    case: &'static str,
    bytes: Vec<u8>,
}
fn fixtures() -> Vec<Fixture> {
    include_str!("fixtures/client-control.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
        .map(|s| {
            let f: Vec<_> = s.split('\t').collect();
            Fixture {
                version: v(f[0].parse().unwrap()),
                name: f[1],
                id: f[2].parse().unwrap(),
                case: f[3],
                bytes: (0..f[4].len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&f[4][i..i + 2], 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}
#[test]
fn independent_fixture_bytes_ids_all_prefixes_and_exact_budgets() {
    let rows = fixtures();
    assert_eq!(rows.len(), 362);
    for f in rows {
        let limits = Limits {
            max_packet: f.bytes.len(),
            max_collection: 0,
            ..Limits::default()
        };
        let p = ClientControlPacket::decode(f.name, &f.bytes, f.version, limits).unwrap();
        assert_eq!(p.encode(f.version, limits).unwrap(), f.bytes);
        let raw = p.packet(f.version, limits).unwrap();
        assert_eq!(raw.id, f.id);
        assert_eq!(raw.data, f.bytes);
        assert_eq!(
            f.version
                .packet_id(State::Play, Direction::Serverbound, f.name)
                .unwrap(),
            f.id
        );
        for end in 0..f.bytes.len() {
            assert!(
                ClientControlPacket::decode(f.name, &f.bytes[..end], f.version, limits).is_err()
            );
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(
            ClientControlPacket::decode(f.name, &trailing, f.version, Limits::default()).is_err()
        );
        let small = Limits {
            max_packet: f.bytes.len() - 1,
            ..limits
        };
        assert!(ClientControlPacket::decode(f.name, &f.bytes, f.version, small).is_err());
        assert!(p.encode(f.version, small).is_err());
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
            State::Play,
        ] {
            assert!(
                DecodedPacket::decode(state, f.name, &f.bytes, f.version, limits)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
#[test]
fn fixture_fields_preserve_semantics_without_game_state_normalization() {
    for f in fixtures() {
        let p =
            ClientControlPacket::decode(f.name, &f.bytes, f.version, Limits::default()).unwrap();
        match p {
            ClientControlPacket::Boat(p) => {
                let n: u8 = f.case.parse().unwrap();
                assert_eq!(p.left_paddle, n & 1 != 0);
                assert_eq!(p.right_paddle, n & 2 != 0);
            }
            ClientControlPacket::SpectateUuid(p) => assert_eq!(
                p.target_uuid,
                if f.case == "zero" {
                    [0; 16]
                } else {
                    core::array::from_fn(|i| i as u8)
                }
            ),
            ClientControlPacket::SpectateEntity(p) => {
                assert_eq!(p.entity_id, f.case.parse::<i32>().unwrap())
            }
            ClientControlPacket::SpectatorAction(p) => assert_eq!(
                p.entity_id,
                if f.case == "absent" {
                    None
                } else {
                    Some(f.case.parse().unwrap())
                }
            ),
            ClientControlPacket::PickInventory(p) => {
                assert_eq!(p.slot, f.case.parse::<i32>().unwrap())
            }
            ClientControlPacket::Difficulty(p) => {
                assert_eq!(p.difficulty, f.case.parse::<i32>().unwrap())
            }
            ClientControlPacket::GameMode(p) => assert_eq!(p.mode, f.case.parse::<i32>().unwrap()),
            ClientControlPacket::Bundle(p) if f.case == "clear" => {
                assert_eq!(p.slot_id, i32::MIN);
                assert_eq!(p.selected_index, -1);
            }
            ClientControlPacket::SlotState(p) if f.case == "bounds" => {
                assert_eq!(p.slot_id, i32::MIN);
                assert_eq!(p.window_id, i32::MAX);
                assert!(p.enabled);
            }
            ClientControlPacket::PickBlock(p) if f.case == "bounds" => {
                assert_eq!(
                    (p.position.x, p.position.y, p.position.z),
                    (-33554432, -2048, 33554431)
                );
                assert!(p.include_data);
            }
            _ => {}
        }
    }
}
#[test]
fn all_version_boundaries_are_catalog_gated() {
    for &version in Version::ALL {
        let p = version.protocol();
        let cases = [
            ("spectate", true, vec![0; 16]),
            ("spectate_entity", p == 775, vec![0]),
            ("spectator_action", p == 776, vec![0]),
            ("pick_item", p < 769, vec![0]),
            ("pick_item_from_block", p >= 769, vec![0; 9]),
            ("pick_item_from_entity", p >= 769, vec![0, 0]),
            ("select_bundle_item", p >= 768, vec![0, 0]),
            ("set_slot_state", p >= 765, vec![0, 0, 0]),
            ("change_gamemode", p >= 771, vec![0]),
        ];
        for (name, present, body) in cases {
            let result = ClientControlPacket::decode(name, &body, version, Limits::default());
            assert_eq!(result.is_ok(), present, "{name} {p}");
        }
    }
    assert!(Spectate {
        target_uuid: [0; 16]
    }
    .encode(v(776), Limits::default())
    .is_ok());
    assert!(SpectateEntity { entity_id: 0 }
        .encode(v(774), Limits::default())
        .is_err());
    assert!(SpectatorAction { entity_id: None }
        .encode(v(775), Limits::default())
        .is_err());
    assert!(PickItem { slot: 0 }
        .encode(v(769), Limits::default())
        .is_err());
}
#[test]
fn difficulty_width_and_optional_spectator_integer_boundaries() {
    let value = SetDifficulty { difficulty: 128 };
    assert_eq!(value.encode(v(770), Limits::default()).unwrap(), [128]);
    assert_eq!(value.encode(v(771), Limits::default()).unwrap(), [128, 1]);
    for id in [-1, 256, i32::MIN, i32::MAX] {
        assert!(SetDifficulty { difficulty: id }
            .encode(v(770), Limits::default())
            .is_err());
        let b = SetDifficulty { difficulty: id }
            .encode(v(771), Limits::default())
            .unwrap();
        assert_eq!(
            SetDifficulty::decode(&b, v(771), Limits::default())
                .unwrap()
                .difficulty,
            id
        );
    }
    assert!(SpectatorAction {
        entity_id: Some(-1)
    }
    .encode(v(776), Limits::default())
    .is_err());
    assert_eq!(
        SpectatorAction::decode(&[0], v(776), Limits::default())
            .unwrap()
            .entity_id,
        None
    );
    // Signed overflow is Java-style wrapping, not saturating or unsigned conversion.
    let b = SpectatorAction {
        entity_id: Some(i32::MAX),
    }
    .encode(v(776), Limits::default())
    .unwrap();
    assert_eq!(b, [128, 128, 128, 128, 8]);
    assert_eq!(
        SpectatorAction::decode(&b, v(776), Limits::default())
            .unwrap()
            .entity_id,
        Some(i32::MAX)
    );
    assert!(SetDifficulty::decode(&[255], v(771), Limits::default()).is_err());
}
#[test]
fn malformed_flags_bundle_indices_and_varints_fail_without_partial_io() {
    for &version in Version::ALL {
        assert!(SteerBoat::decode(&[0, 2], version, Limits::default()).is_err());
        assert!(LockDifficulty::decode(&[255], version, Limits::default()).is_err());
        let mut r = Reader::new(&[0, 2], Limits::default());
        assert!(SteerBoat::read(&mut r, version).is_err());
        assert_eq!(r.position(), 0);
        let mut w = Writer::new();
        w.raw(&[4, 5]);
        assert!(SteerBoat {
            left_paddle: true,
            right_paddle: true
        }
        .write(
            &mut w,
            version,
            Limits {
                max_packet: 1,
                ..Limits::default()
            }
        )
        .is_err());
        assert_eq!(w.as_slice(), [4, 5]);
        if version.protocol() >= 768 {
            for index in [-2, i32::MIN] {
                assert!(SelectBundleItem {
                    slot_id: 0,
                    selected_index: index
                }
                .encode(version, Limits::default())
                .is_err());
                let mut w = Writer::new();
                w.var_i32(0);
                w.var_i32(index);
                assert!(matches!(
                    SelectBundleItem::decode(w.as_slice(), version, Limits::default()),
                    Err(Error::Invalid("bundle selected index"))
                ));
            }
            assert!(SelectBundleItem::decode(
                &[128, 128, 128, 128, 16, 0],
                version,
                Limits::default()
            )
            .is_err());
        }
    }
}
#[test]
fn fixture_mutations_never_panic_and_successes_are_semantically_stable() {
    for f in fixtures() {
        for i in 0..f.bytes.len() {
            for mask in [1, 128, 255] {
                let mut b = f.bytes.clone();
                b[i] ^= mask;
                if let Ok(p) = ClientControlPacket::decode(f.name, &b, f.version, Limits::default())
                {
                    let canonical = p.encode(f.version, Limits::default()).unwrap();
                    assert_eq!(
                        ClientControlPacket::decode(
                            f.name,
                            &canonical,
                            f.version,
                            Limits::default()
                        )
                        .unwrap(),
                        p
                    );
                }
            }
        }
    }
}
