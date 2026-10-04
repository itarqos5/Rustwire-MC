use rustwire_mc::{
    packet::{
        entity::{Angle, EntityPacket, SpawnExperienceOrb, SpawnPlayer},
        typed::DecodedPacket,
    },
    version::{Direction, State},
    Error, Limits, Version,
};

const ORB: &str = "spawn_entity_experience_orb";
const PLAYER: &str = "named_entity_spawn";

struct Fixture {
    version: Version,
    name: &'static str,
    id: i32,
    case: &'static str,
    bytes: Vec<u8>,
}
fn fixtures() -> Vec<Fixture> {
    include_str!("fixtures/legacy-spawn.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            Fixture {
                version: Version::from_protocol(fields[0].parse().unwrap()).unwrap(),
                name: fields[1],
                id: fields[2].parse().unwrap(),
                case: fields[3],
                bytes: fields[4]
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}
fn decode(f: &Fixture, bytes: &[u8], limits: Limits) -> rustwire_mc::Result<EntityPacket> {
    match f.name {
        ORB => {
            SpawnExperienceOrb::decode(bytes, f.version, limits).map(EntityPacket::ExperienceOrb)
        }
        PLAYER => SpawnPlayer::decode(bytes, f.version, limits).map(EntityPacket::PlayerSpawn),
        _ => unreachable!(),
    }
}
fn body(p: &EntityPacket, version: Version, limits: Limits) -> rustwire_mc::Result<Vec<u8>> {
    match p {
        EntityPacket::ExperienceOrb(p) => p.encode(version, limits),
        EntityPacket::PlayerSpawn(p) => p.encode(version, limits),
        _ => unreachable!(),
    }
}

#[test]
fn independent_goldens_direct_family_and_typed_dispatch() {
    let fixtures = fixtures();
    assert_eq!(fixtures.len(), 62);
    for f in fixtures {
        let limits = Limits::default();
        let direct = decode(&f, &f.bytes, limits).unwrap();
        assert_eq!(body(&direct, f.version, limits).unwrap(), f.bytes);
        assert_eq!(direct.encode(f.version, limits).unwrap().id, f.id);
        let family = EntityPacket::decode(f.name, &f.bytes, f.version, limits).unwrap();
        assert_eq!(family.encode(f.version, limits).unwrap().data, f.bytes);
        let Some(DecodedPacket::Entity(typed)) =
            DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, limits).unwrap()
        else {
            panic!("wrong typed family for {} {}", f.version, f.name);
        };
        let raw = typed.encode(f.version, limits).unwrap();
        assert_eq!(raw.id, f.id);
        assert_eq!(raw.data, f.bytes);
        assert_eq!(
            f.version
                .packet_id(State::Play, Direction::Clientbound, f.name)
                .unwrap(),
            f.id
        );
    }
}

#[test]
fn fields_preserve_uuid_angles_signed_values_and_coordinate_bits() {
    for f in fixtures() {
        let expected_id = match f.case {
            "ordinary" => 300,
            "zero" => 0,
            "signed_min" => i32::MIN,
            "signed_max" => i32::MAX,
            "negative_id" | "raw_bits" => -1,
            "id_127" => 127,
            "id_128" | "raw_bits_negative" => 128,
            _ => unreachable!(),
        };
        let expected_position = match f.case {
            "zero" => [0; 3],
            "raw_bits" => [0x8000000000000000, 0x7ff8000000000123, 0xfff0000000000000],
            "signed_min" | "raw_bits_negative" => [0x7ff0000000000000, 0xfff8000000000456, 1],
            _ => [-1.25f64, 64.5, 30_000_000.0].map(f64::to_bits),
        };
        match decode(&f, &f.bytes, Limits::default()).unwrap() {
            EntityPacket::ExperienceOrb(p) => {
                assert_eq!(p.entity_id, expected_id);
                assert_eq!(p.position.map(f64::to_bits), expected_position);
                assert_eq!(
                    p.value,
                    match f.case {
                        "ordinary" => 7,
                        "zero" => 0,
                        "signed_min" => i16::MIN,
                        "signed_max" => i16::MAX,
                        "negative_id" => -1,
                        "raw_bits" => -7,
                        "id_127" => 128,
                        "id_128" => 127,
                        _ => unreachable!(),
                    }
                );
            }
            EntityPacket::PlayerSpawn(p) => {
                assert_eq!(p.entity_id, expected_id);
                assert_eq!(p.position.map(f64::to_bits), expected_position);
                let (uuid, yaw, pitch) = match f.case {
                    "ordinary" => (std::array::from_fn(|i| i as u8), 255, 128),
                    "zero" => ([0; 16], 0, 0),
                    "signed_min" => (std::array::from_fn(|i| 15 - i as u8), 127, 0),
                    "signed_max" => ([255; 16], 128, 255),
                    "raw_bits" => (std::array::from_fn(|i| i as u8), 1, 254),
                    "raw_bits_negative" => (std::array::from_fn(|i| 15 - i as u8), 254, 1),
                    _ => unreachable!(),
                };
                assert_eq!(p.uuid, uuid);
                assert_eq!((p.yaw, p.pitch), (Angle(yaw), Angle(pitch)));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn every_prefix_trailing_byte_and_packet_budget_fails_closed() {
    let mut prefixes = 0;
    for f in fixtures() {
        let limits = Limits::default();
        for n in 0..f.bytes.len() {
            assert!(decode(&f, &f.bytes[..n], limits).is_err());
            assert!(EntityPacket::decode(f.name, &f.bytes[..n], f.version, limits).is_err());
            assert!(
                DecodedPacket::decode(State::Play, f.name, &f.bytes[..n], f.version, limits)
                    .is_err()
            );
            prefixes += 1;
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode(&f, &trailing, limits),
            Err(Error::Invalid("trailing bytes"))
        ));
        assert!(matches!(
            DecodedPacket::decode(State::Play, f.name, &trailing, f.version, limits),
            Err(Error::Invalid("trailing bytes"))
        ));
        let exact = Limits {
            max_packet: f.bytes.len(),
            max_collection: 0,
            ..limits
        };
        let value = decode(&f, &f.bytes, exact).unwrap();
        assert_eq!(body(&value, f.version, exact).unwrap(), f.bytes);
        assert_eq!(value.encode(f.version, exact).unwrap().data, f.bytes);
        let small = Limits {
            max_packet: f.bytes.len() - 1,
            ..limits
        };
        assert!(matches!(decode(&f, &f.bytes, small), Err(Error::Limit(_))));
        assert!(matches!(
            body(&value, f.version, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            value.encode(f.version, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, small),
            Err(Error::Limit(_))
        ));
        let empty = Limits {
            max_packet: 0,
            ..limits
        };
        assert!(body(&value, f.version, empty).is_err());
    }
    assert_eq!(prefixes, 1910);
}

#[test]
fn exact_presence_boundaries_and_non_play_states() {
    let limits = Limits::default();
    let fixtures = fixtures();
    for &version in Version::ALL {
        for name in [ORB, PLAYER] {
            let f = fixtures
                .iter()
                .find(|f| f.name == name && f.case == "ordinary")
                .unwrap();
            let value = decode(f, &f.bytes, limits).unwrap();
            let present = version.protocol() <= if name == ORB { 769 } else { 763 };
            let direct = match name {
                ORB => SpawnExperienceOrb::decode(&f.bytes, version, limits)
                    .map(EntityPacket::ExperienceOrb),
                _ => SpawnPlayer::decode(&f.bytes, version, limits).map(EntityPacket::PlayerSpawn),
            };
            assert_eq!(direct.is_ok(), present, "direct {version} {name}");
            assert_eq!(
                body(&value, version, limits).is_ok(),
                present,
                "encode {version} {name}"
            );
            assert_eq!(value.encode(version, limits).is_ok(), present);
            assert_eq!(
                EntityPacket::decode(name, &f.bytes, version, limits).is_ok(),
                present
            );
            assert_eq!(
                DecodedPacket::decode(State::Play, name, &f.bytes, version, limits).is_ok(),
                present
            );
            for state in [
                State::Handshake,
                State::Status,
                State::Login,
                State::Configuration,
            ] {
                assert!(
                    DecodedPacket::decode(state, name, &f.bytes, version, limits)
                        .unwrap()
                        .is_none()
                );
            }
        }
        assert!(
            DecodedPacket::decode(State::Play, "future_spawn_packet", &[255], version, limits)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            EntityPacket::decode("future_spawn_packet", &[255], version, limits),
            Err(Error::Unsupported(_))
        ));
    }
}

#[test]
fn every_player_angle_byte_is_lossless_and_in_correct_order() {
    let version = Version::V1_20;
    let limits = Limits::default();
    for yaw in 0..=255u8 {
        for pitch in [0, 127, 128, 255, 255 - yaw] {
            let mut bytes = vec![0; 41]; // ID + UUID + three f64s
            bytes.extend([yaw, pitch]);
            let value = SpawnPlayer::decode(&bytes, version, limits).unwrap();
            assert_eq!((value.yaw, value.pitch), (Angle(yaw), Angle(pitch)));
            assert_eq!(value.encode(version, limits).unwrap(), bytes);
        }
    }
}

#[test]
fn malformed_and_overlong_entity_varints_follow_shared_reader() {
    for f in fixtures().iter().filter(|f| f.case == "zero") {
        for invalid in [
            &[0x80; 5][..],
            &[0xff, 0xff, 0xff, 0xff, 0x10][..],
            &[0x80; 6][..],
        ] {
            let mut bytes = invalid.to_vec();
            bytes.extend(&f.bytes[1..]);
            assert!(decode(f, &bytes, Limits::default()).is_err());
        }
        // The shared reader accepts a non-minimal zero; re-encoding canonicalizes it.
        let mut overlong = vec![0x80, 0];
        overlong.extend(&f.bytes[1..]);
        let value = decode(f, &overlong, Limits::default()).unwrap();
        assert_eq!(body(&value, f.version, Limits::default()).unwrap(), f.bytes);
    }
}
