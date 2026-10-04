use rustwire_mc::{
    codec::{Reader, Writer},
    packet::{common::CommonPacket, server_metadata::ResetChat, typed::DecodedPacket},
    version::{Direction, State},
    Limits, Version,
};

#[test]
fn reset_chat_exact_version_state_id_and_empty_body_matrix() {
    let limits = Limits {
        max_packet: 0,
        ..Limits::default()
    };
    let mut count = 0;
    for row in include_str!("fixtures/reset-chat.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let f: Vec<_> = row.split('\t').collect();
        assert_eq!(f.len(), 3);
        let v = Version::from_protocol(f[0].parse().unwrap()).unwrap();
        let id = f[1].parse::<i32>().unwrap();
        assert!(f[2].is_empty());
        assert_eq!(ResetChat::decode(&[], v, limits).unwrap(), ResetChat);
        assert_eq!(ResetChat.packet(v, limits).unwrap().id, id);
        assert!(ResetChat.packet(v, limits).unwrap().data.is_empty());
        assert_eq!(
            v.packet(State::Configuration, Direction::Clientbound, id)
                .unwrap()
                .name,
            "reset_chat"
        );
        assert!(matches!(
            DecodedPacket::decode(State::Configuration, "reset_chat", &[], v, limits).unwrap(),
            Some(DecodedPacket::Common(CommonPacket::ResetChat(ResetChat)))
        ));
        for state in [State::Handshake, State::Status, State::Login, State::Play] {
            assert!(v
                .packet_id(state, Direction::Clientbound, "reset_chat")
                .is_err());
            assert!(DecodedPacket::decode(state, "reset_chat", &[], v, limits)
                .unwrap()
                .is_none());
        }
        assert!(v
            .packet_id(State::Configuration, Direction::Serverbound, "reset_chat")
            .is_err());
        count += 1;
    }
    assert_eq!(count, 11);
    for &v in Version::ALL.iter().filter(|v| v.protocol() < 766) {
        assert!(ResetChat::decode(&[], v, limits).is_err());
        assert!(ResetChat.encode(v, limits).is_err());
        assert!(ResetChat.packet(v, limits).is_err());
        assert!(v
            .packet_id(State::Configuration, Direction::Clientbound, "reset_chat")
            .is_err());
    }
}

#[test]
fn reset_chat_trailing_bytes_and_transactional_helpers() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        for byte in 0..=255 {
            assert!(ResetChat::decode(&[byte], v, Limits::default()).is_err());
            assert!(ResetChat::decode(
                &[byte],
                v,
                Limits {
                    max_packet: 0,
                    ..Limits::default()
                }
            )
            .is_err());
        }
        let mut r = Reader::new(&[7], Limits::default());
        assert_eq!(ResetChat::read(&mut r, v).unwrap(), ResetChat);
        assert_eq!(r.position(), 0); // a body reader must leave the next body untouched
        let mut w = Writer::new();
        w.u8(7);
        ResetChat.write(&mut w, v, Limits::default()).unwrap();
        assert_eq!(w.as_slice(), &[7]);
    }
    let mut r = Reader::new(&[7], Limits::default());
    assert!(ResetChat::read(&mut r, Version::V1_20,).is_err());
    assert_eq!(r.position(), 0);
    let mut w = Writer::new();
    w.u8(7);
    assert!(ResetChat
        .write(&mut w, Version::V1_20, Limits::default())
        .is_err());
    assert_eq!(w.as_slice(), &[7]);
}
