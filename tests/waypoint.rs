use rustwire_mc::{
    codec::{Reader, Writer},
    packet::{typed::DecodedPacket, waypoint::*},
    version::{Direction, State},
    Error, Limits, Version,
};
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn fixtures() -> Vec<(Version, i32, &'static str, Vec<u8>)> {
    include_str!("fixtures/waypoint.tsv")
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
fn base() -> TrackedWaypoint {
    TrackedWaypoint {
        operation: WaypointOperation::Untrack,
        identity: WaypointIdentity::Name("A Name 😀".into()),
        icon: WaypointIcon {
            style: "minecraft:default".into(),
            color: Some([0, 128, 255]),
        },
        location: WaypointLocation::Position([i32::MIN, 0, i32::MAX]),
    }
}
#[test]
fn independent_waypoint_fixtures_all_prefixes_exact_limits_and_meanings() {
    let rows = fixtures();
    assert_eq!(rows.len(), 306);
    for (version, id, case, bytes) in rows {
        let limits = Limits {
            max_packet: bytes.len(),
            max_collection: 0,
            ..Limits::default()
        };
        let p = TrackedWaypoint::decode(&bytes, version, limits).unwrap();
        assert_eq!(p.encode(version, limits).unwrap(), bytes);
        assert_eq!(p.packet(version, limits).unwrap().id, id);
        assert_eq!(
            version
                .packet_id(State::Play, Direction::Clientbound, "tracked_waypoint")
                .unwrap(),
            id
        );
        if let Some(bits) = case.strip_prefix("azimuth-") {
            let WaypointLocation::Azimuth(value) = p.location else {
                panic!()
            };
            assert_eq!(value.to_bits(), bits.parse::<u32>().unwrap());
        } else {
            let parts: Vec<usize> = case.split('-').map(|n| n.parse().unwrap()).collect();
            assert_eq!(p.operation as usize, parts[0]);
            match &p.identity {
                WaypointIdentity::Uuid(uuid) => {
                    assert_eq!(parts[1], 1);
                    assert_eq!(*uuid, core::array::from_fn(|i| i as u8));
                }
                WaypointIdentity::Name(name) => {
                    assert_eq!(parts[1], 0);
                    assert_eq!(name, "A Name 😀");
                }
            }
            assert_eq!(
                p.icon.color,
                if parts[2] == 1 {
                    Some([0, 128, 255])
                } else {
                    None
                }
            );
            assert_eq!(p.icon.style, "minecraft:default");
            match p.location {
                WaypointLocation::Empty => assert_eq!(parts[3], 0),
                WaypointLocation::Position(xyz) => {
                    assert_eq!(parts[3], 1);
                    assert_eq!(xyz, [i32::MIN, 0, i32::MAX]);
                }
                WaypointLocation::Chunk { x, z } => {
                    assert_eq!(parts[3], 2);
                    assert_eq!((x, z), (-1, 300));
                }
                WaypointLocation::Azimuth(a) => {
                    assert_eq!(parts[3], 3);
                    assert_eq!(a.to_bits(), 0x80000000);
                }
            }
        }
        for end in 0..bytes.len() {
            assert!(TrackedWaypoint::decode(&bytes[..end], version, limits).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(TrackedWaypoint::decode(&trailing, version, Limits::default()).is_err());
        let small = Limits {
            max_packet: bytes.len() - 1,
            ..limits
        };
        assert!(p.encode(version, small).is_err());
        assert!(TrackedWaypoint::decode(&bytes, version, small).is_err());
        let Some(DecodedPacket::Waypoint(q)) =
            DecodedPacket::decode(State::Play, "tracked_waypoint", &bytes, version, limits)
                .unwrap()
        else {
            panic!()
        };
        assert_eq!(q.encode(version, limits).unwrap(), bytes);
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(
                DecodedPacket::decode(state, "tracked_waypoint", &bytes, version, limits)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
#[test]
fn removal_still_has_full_body_and_old_versions_fail_closed() {
    for &version in Version::ALL {
        let value = base();
        if version.protocol() < 771 {
            assert!(value.encode(version, Limits::default()).is_err());
            assert!(TrackedWaypoint::decode(&[], version, Limits::default()).is_err());
            continue;
        }
        let bytes = value.encode(version, Limits::default()).unwrap();
        assert_eq!(
            TrackedWaypoint::decode(&bytes, version, Limits::default()).unwrap(),
            value
        );
        assert!(bytes.len() > 20);
        assert!(TrackedWaypoint::decode(&[1], version, Limits::default()).is_err());
    }
}
#[test]
fn malformed_known_fields_unknown_payloads_and_transactional_io() {
    for &version in Version::ALL.iter().filter(|v| v.protocol() >= 771) {
        // Empty named identity/style are legal identifiers/strings; flag offsets are fixed here.
        let canonical = [0, 0, 0, 0, 0, 0];
        assert!(TrackedWaypoint::decode(&canonical, version, Limits::default()).is_ok());
        for at in [1, 4] {
            let mut b = canonical;
            b[at] = 2;
            assert!(matches!(
                TrackedWaypoint::decode(&b, version, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        let mut bad = canonical;
        bad[0] = 3;
        assert!(matches!(
            TrackedWaypoint::decode(&bad, version, Limits::default()),
            Err(Error::Invalid("waypoint operation"))
        ));
        let unknown = [0, 0, 0, 0, 0, 127, 9, 8];
        assert!(matches!(
            TrackedWaypoint::decode(&unknown, version, Limits::default()),
            Err(Error::Unsupported("waypoint location kind"))
        ));
        let negative = [0, 0, 0, 0, 0, 255, 255, 255, 255, 15];
        assert!(matches!(
            TrackedWaypoint::decode(&negative, version, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let mut r = Reader::new(&unknown, Limits::default());
        assert!(TrackedWaypoint::read(&mut r, version).is_err());
        assert_eq!(r.position(), 0);
        let mut w = Writer::new();
        w.raw(&[7, 8]);
        assert!(base()
            .write(
                &mut w,
                version,
                Limits {
                    max_packet: 1,
                    ..Limits::default()
                }
            )
            .is_err());
        assert_eq!(w.as_slice(), [7, 8]);
        let mut value = base();
        value.icon.style = "BAD:Style".into();
        assert!(value.encode(version, Limits::default()).is_err());
        let invalid_style = [0, 0, 0, 1, b'A', 0, 0];
        assert!(TrackedWaypoint::decode(&invalid_style, version, Limits::default()).is_err());
    }
}
#[test]
fn identity_strings_obey_utf16_limits_without_resource_key_restrictions() {
    let version = v(776);
    let value = TrackedWaypoint {
        identity: WaypointIdentity::Name("😀".into()),
        icon: WaypointIcon {
            style: "a".into(),
            color: None,
        },
        location: WaypointLocation::Empty,
        ..base()
    };
    let enough = Limits {
        max_string_chars: 2,
        ..Limits::default()
    };
    let b = value.encode(version, enough).unwrap();
    assert_eq!(TrackedWaypoint::decode(&b, version, enough).unwrap(), value);
    let small = Limits {
        max_string_chars: 1,
        ..enough
    };
    assert!(value.encode(version, small).is_err());
    assert!(TrackedWaypoint::decode(&b, version, small).is_err());
    let mut large = value;
    large.identity = WaypointIdentity::Name("x".repeat(32767));
    assert!(large.encode(version, Limits::default()).is_ok());
    if let WaypointIdentity::Name(name) = &mut large.identity {
        name.push('x');
    }
    assert!(large.encode(version, Limits::default()).is_err());
}
#[test]
fn bounded_mutations_preserve_successful_float_bits() {
    for (version, _, _, body) in fixtures() {
        for i in 0..body.len() {
            let mut b = body.clone();
            b[i] ^= 255;
            if let Ok(value) = TrackedWaypoint::decode(&b, version, Limits::default()) {
                let wire = value.encode(version, Limits::default()).unwrap();
                assert_eq!(
                    TrackedWaypoint::decode(&wire, version, Limits::default())
                        .unwrap()
                        .encode(version, Limits::default())
                        .unwrap(),
                    wire
                );
            }
        }
    }
}
