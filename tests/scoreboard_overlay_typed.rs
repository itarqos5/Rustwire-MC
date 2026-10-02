use rustwire_mc::{
    packet::{
        overlay::BossBarAction,
        scoreboard::{ScoreAction, ScoreboardPacket},
        typed::{DecodedPacket, OverlayPacket},
    },
    version::{Direction, State},
    Limits, Version,
};
fn fixtures(version: Version) -> Vec<(&'static str, Vec<u8>)> {
    let score = if version.protocol() < 765 {
        vec![1, b'a', 0, 1, b'b', 3]
    } else {
        vec![1, b'a', 1, b'b', 3, 0, 0]
    };
    let mut boss = vec![0; 16];
    boss.push(1);
    // Independent JSON strings through 764, anonymous NBT strings from 765.
    let header = if version.protocol() < 765 {
        vec![4, b'"', b'h', b'i', b'"', 4, b'"', b'o', b'k', b'"']
    } else {
        vec![8, 0, 2, b'h', b'i', 8, 0, 2, b'o', b'k']
    };
    let mut values = vec![
        ("scoreboard_objective", vec![3, b'o', b'b', b'j', 1]),
        ("scoreboard_display_objective", vec![1, 3, b'o', b'b', b'j']),
        ("scoreboard_score", score),
        ("teams", vec![1, b't', 1]),
        ("boss_bar", boss),
        ("playerlist_header", header),
    ];
    if version.protocol() >= 765 {
        values.push(("reset_score", vec![1, b'a', 0]));
    }
    values
}
#[test]
fn all_available_scoreboard_and_overlay_names_dispatch_by_family() {
    for &v in Version::ALL {
        for (name, bytes) in fixtures(v) {
            v.packet_id(State::Play, Direction::Clientbound, name)
                .unwrap();
            let packet = DecodedPacket::decode(State::Play, name, &bytes, v, Limits::default())
                .unwrap()
                .unwrap();
            match packet {
                DecodedPacket::Scoreboard(packet) => {
                    assert!(!matches!(name, "boss_bar" | "playerlist_header"));
                    assert_eq!(packet.encode(v, Limits::default()).unwrap(), bytes);
                    if let ScoreboardPacket::Score(score) = *packet {
                        assert!(matches!(score.action, ScoreAction::Set { value: 3, .. }));
                    }
                }
                DecodedPacket::Overlay(OverlayPacket::BossBar(packet)) => {
                    assert_eq!(name, "boss_bar");
                    assert_eq!(packet.action, BossBarAction::Remove);
                    assert_eq!(packet.encode(v, Limits::default()).unwrap(), bytes);
                }
                DecodedPacket::Overlay(OverlayPacket::PlayerList(packet)) => {
                    assert_eq!(name, "playerlist_header");
                    assert_eq!(packet.encode(v, Limits::default()).unwrap(), bytes);
                }
                _ => panic!("wrong typed family for {v} {name}"),
            }
            assert!(DecodedPacket::decode(
                State::Configuration,
                name,
                &bytes,
                v,
                Limits::default()
            )
            .unwrap()
            .is_none());
        }
    }
}
#[test]
fn typed_scoreboard_and_overlay_errors_stay_errors() {
    for &v in Version::ALL {
        for (name, bytes) in fixtures(v) {
            for end in 0..bytes.len() {
                assert!(
                    DecodedPacket::decode(State::Play, name, &bytes[..end], v, Limits::default())
                        .is_err(),
                    "{v} {name} prefix {end}"
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(
                DecodedPacket::decode(State::Play, name, &trailing, v, Limits::default()).is_err()
            );
        }
    }
}
