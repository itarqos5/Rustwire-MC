use rustwire_mc::{packet::typed::DecodedPacket, version::State, Error, Limits, Version};
#[test]
fn world_state_dispatches_every_independent_clientbound_golden() {
    let mut count = 0;
    for line in include_str!("fixtures/world-state.tsv")
        .lines()
        .filter(|s| !s.starts_with('#') && !s.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        if fields[1] == "player_loaded" {
            continue;
        }
        let version = Version::from_protocol(fields[0].parse().unwrap()).unwrap();
        let bytes: Vec<u8> = fields[3]
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
            .collect();
        let Some(DecodedPacket::WorldState(value)) =
            DecodedPacket::decode(State::Play, fields[1], &bytes, version, Limits::default())
                .unwrap()
        else {
            panic!("wrong typed family")
        };
        assert_eq!(value.encode(version, Limits::default()).unwrap(), bytes);
        assert_eq!(
            value.packet(version, Limits::default()).unwrap().id,
            fields[2].parse::<i32>().unwrap()
        );
        assert!(DecodedPacket::decode(
            State::Configuration,
            fields[1],
            &bytes,
            version,
            Limits::default()
        )
        .unwrap()
        .is_none());
        for length in 0..bytes.len() {
            assert!(DecodedPacket::decode(
                State::Play,
                fields[1],
                &bytes[..length],
                version,
                Limits::default()
            )
            .is_err());
        }
        count += 1;
    }
    assert_eq!(count, 154);
}
#[test]
fn unknown_game_reasons_preserve_the_unsupported_boundary() {
    for &version in Version::ALL {
        let bytes = [255, 0, 0, 0, 0];
        assert!(matches!(
            DecodedPacket::decode(
                State::Play,
                "game_state_change",
                &bytes,
                version,
                Limits::default()
            ),
            Err(Error::Unsupported("game-event reason"))
        ));
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(matches!(
            DecodedPacket::decode(
                State::Play,
                "game_state_change",
                &trailing,
                version,
                Limits::default()
            ),
            Err(Error::Invalid("trailing bytes"))
        ));
    }
}
