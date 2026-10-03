use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{hud::HudPacket, typed::DecodedPacket},
    version::{Direction, State},
    Error, Limits, Version,
};
use std::io::{Cursor, Read, Write};

#[test]
fn dispatch_covers_every_fixture_only_in_play_and_with_clientbound_ids() {
    let mut count = 0;
    for line in include_str!("fixtures/hud.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let parts: Vec<_> = line.split('\t').collect();
        let v = Version::from_protocol(parts[0].parse().unwrap()).unwrap();
        let name = parts[1];
        let bytes: Vec<u8> = parts[3]
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let Some(DecodedPacket::Hud(value)) =
            DecodedPacket::decode(State::Play, name, &bytes, v, Limits::default()).unwrap()
        else {
            panic!("missing HUD dispatch")
        };
        assert_eq!(value.name(), name);
        let packet = value.packet(v, Limits::default()).unwrap();
        assert_eq!(packet.data, bytes);
        assert_eq!(packet.id, parts[2].parse::<i32>().unwrap());
        assert_eq!(
            v.packet_id(State::Play, Direction::Clientbound, name)
                .unwrap(),
            packet.id
        );
        assert!(v
            .packet_id(State::Play, Direction::Serverbound, name)
            .is_err());
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(
                DecodedPacket::decode(state, name, &bytes, v, Limits::default())
                    .unwrap()
                    .is_none()
            );
            assert!(v.packet_id(state, Direction::Clientbound, name).is_err());
        }
        for length in 0..bytes.len() {
            assert!(DecodedPacket::decode(
                State::Play,
                name,
                &bytes[..length],
                v,
                Limits::default()
            )
            .is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            DecodedPacket::decode(State::Play, name, &trailing, v, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
        count += 1;
    }
    assert_eq!(count, 140);
}

#[test]
fn unknown_names_and_unknown_raw_scalars_have_different_boundaries() {
    for &v in Version::ALL {
        assert!(
            DecodedPacket::decode(State::Play, "future_hud", &[1, 2, 3], v, Limits::default())
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            HudPacket::decode("future_hud", &[1, 2, 3], v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
        let Some(DecodedPacket::Hud(HudPacket::OpenBook(book))) =
            DecodedPacket::decode(State::Play, "open_book", &[2], v, Limits::default()).unwrap()
        else {
            panic!("raw scalar must remain typed")
        };
        assert_eq!(book.hand_id, 2);
        assert!(book.known_hand().is_none());
    }
}

struct Memory {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
}
impl Read for Memory {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(b)
    }
}
impl Write for Memory {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.output.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn play_connection(packet: RawPacket) -> Connection<Memory> {
    let v = Version::V1_21_5;
    let codec = FrameCodec::default();
    let mut success = Writer::new();
    success.raw(&[0; 16]);
    success.string("Rustwire", 16).unwrap();
    success.var_i32(0);
    let mut bytes = codec
        .encode(&RawPacket::new(2, success.into_inner()))
        .unwrap();
    bytes.extend(
        codec
            .encode(&RawPacket::new(
                v.packet_id(
                    State::Configuration,
                    Direction::Clientbound,
                    "finish_configuration",
                )
                .unwrap(),
                vec![],
            ))
            .unwrap(),
    );
    bytes.extend(codec.encode(&packet).unwrap());
    let mut connection = Connection::new(
        Memory {
            input: Cursor::new(bytes),
            output: vec![],
        },
        v,
        Limits::default(),
    );
    connection
        .start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(
        connection.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    assert!(matches!(
        connection.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    connection
}

#[test]
fn connection_keeps_unknown_ids_raw_unknown_hands_typed_and_malformed_known_errors() {
    let v = Version::V1_21_5;
    let raw = RawPacket::new(999, vec![1, 2, 3]);
    assert!(
        matches!(play_connection(raw.clone()).next_typed_event().unwrap(), TypedEvent::Raw { packet, name: None, unsupported: None, .. } if packet == raw)
    );
    let book_id = v
        .packet_id(State::Play, Direction::Clientbound, "open_book")
        .unwrap();
    assert!(
        matches!(play_connection(RawPacket::new(book_id, vec![2])).next_typed_event().unwrap(), TypedEvent::Decoded(DecodedPacket::Hud(HudPacket::OpenBook(book))) if book.hand_id == 2)
    );
    for (name, bytes) in [
        ("open_book", vec![]),
        ("clear_titles", vec![2]),
        ("action_bar", vec![0]),
        ("enter_combat_event", vec![0]),
        ("death_combat_event", vec![0, 0]),
    ] {
        let id = v
            .packet_id(State::Play, Direction::Clientbound, name)
            .unwrap();
        let result = play_connection(RawPacket::new(id, bytes)).next_typed_event();
        assert!(result.is_err(), "{name}");
        assert!(!matches!(result, Err(Error::Unsupported(_))), "{name}");
    }
}
