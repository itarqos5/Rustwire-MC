use rustwire_mc::{
    packet::typed::DecodedPacket,
    version::{Direction, State},
    Limits, Version,
};

#[test]
fn map_and_statistics_dispatch_and_packet_ids_all_families() {
    for &v in Version::ALL {
        for (name, bytes) in [
            ("map", vec![0, 0, 0, 1, 0, 0]),
            ("statistics", vec![1, 8, 2, 3]),
        ] {
            let decoded = DecodedPacket::decode(State::Play, name, &bytes, v, Limits::default())
                .unwrap()
                .unwrap();
            let packet = match decoded {
                DecodedPacket::Map(value) => {
                    assert_eq!(name, "map");
                    assert_eq!(value.decorations, Some(vec![]));
                    value.packet(v, Limits::default()).unwrap()
                }
                DecodedPacket::Statistics(value) => {
                    assert_eq!(name, "statistics");
                    assert_eq!(value.entries[0].value, 3);
                    value.packet(v, Limits::default()).unwrap()
                }
                _ => panic!("wrong dispatch for {name}"),
            };
            assert_eq!(packet.data, bytes);
            assert_eq!(
                packet.id,
                v.packet_id(State::Play, Direction::Clientbound, name)
                    .unwrap()
            );
            for state in [
                State::Configuration,
                State::Login,
                State::Status,
                State::Handshake,
            ] {
                assert!(
                    DecodedPacket::decode(state, name, &bytes, v, Limits::default())
                        .unwrap()
                        .is_none()
                );
            }
        }
    }
}

#[test]
fn malformed_typed_map_and_statistics_remain_errors() {
    for &v in Version::ALL {
        for (name, bytes) in [
            ("map", vec![0, 0, 0, 1, 0, 0]),
            ("statistics", vec![1, 8, 2, 3]),
        ] {
            for end in 0..bytes.len() {
                assert!(DecodedPacket::decode(
                    State::Play,
                    name,
                    &bytes[..end],
                    v,
                    Limits::default()
                )
                .is_err());
            }
            let mut trailing = bytes;
            trailing.push(0);
            assert!(
                DecodedPacket::decode(State::Play, name, &trailing, v, Limits::default()).is_err()
            );
        }
    }
}
