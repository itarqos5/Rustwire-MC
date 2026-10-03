use rustwire_mc::{
    packet::{entity_control::EntityControlPacket, typed::DecodedPacket},
    version::State,
    Error, Limits, Version,
};

#[test]
fn every_golden_dispatches_only_in_play() {
    let mut count = 0;
    for line in include_str!("fixtures/entity-control.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        let version = Version::from_protocol(fields[0].parse().unwrap()).unwrap();
        let name = fields[1];
        let bytes: Vec<_> = fields[4]
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let Some(DecodedPacket::EntityControl(value)) =
            DecodedPacket::decode(State::Play, name, &bytes, version, Limits::default()).unwrap()
        else {
            panic!("wrong typed family for {version} {name}")
        };
        assert_eq!(value.encode(version, Limits::default()).unwrap(), bytes);
        assert_eq!(
            value.packet(version, Limits::default()).unwrap().id,
            fields[2].parse::<i32>().unwrap()
        );
        assert!(matches!(
            (name, value),
            ("set_passengers", EntityControlPacket::Passengers(_))
                | ("attach_entity", EntityControlPacket::Attach(_))
                | ("entity_head_rotation", EntityControlPacket::HeadRotation(_))
                | ("camera", EntityControlPacket::Camera(_))
                | ("animation", EntityControlPacket::Animation(_))
                | ("damage_event", EntityControlPacket::Damage(_))
                | ("hurt_animation", EntityControlPacket::HurtAnimation(_))
        ));
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(
                DecodedPacket::decode(state, name, &bytes, version, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        for length in 0..bytes.len() {
            assert!(DecodedPacket::decode(
                State::Play,
                name,
                &bytes[..length],
                version,
                Limits::default()
            )
            .is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            DecodedPacket::decode(State::Play, name, &trailing, version, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
        count += 1;
    }
    assert_eq!(count, 224);
}

#[test]
fn raw_fallback_remains_for_unimplemented_entity_packets() {
    for &version in Version::ALL {
        // Other entity-targeted packets remain outside this typed slice.
        for name in ["collect", "face_player", "future_entity_control"] {
            assert!(
                DecodedPacket::decode(State::Play, name, &[255], version, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        assert!(matches!(
            DecodedPacket::decode(
                State::Play,
                "damage_event",
                &[0, 0, 0, 0, 2],
                version,
                Limits::default()
            ),
            Err(Error::Invalid("boolean"))
        ));
        // A raw animation byte has no unimplemented nested layout.
        assert!(matches!(
            DecodedPacket::decode(
                State::Play,
                "animation",
                &[0, 255],
                version,
                Limits::default()
            )
            .unwrap(),
            Some(DecodedPacket::EntityControl(
                EntityControlPacket::Animation(_)
            ))
        ));
    }
}
