use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{
        inventory,
        typed::{ChatPacket, DecodedPacket},
    },
    version::{Direction, State},
    Limits, Version,
};
use std::io::{Cursor, Read, Write};
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
    let io = Memory {
        input: Cursor::new(bytes),
        output: vec![],
    };
    let mut c = Connection::new(io, v, Limits::default());
    c.start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    c
}
#[test]
fn unsupported_inventory_preserves_entire_raw_packet() {
    let v = Version::V1_21_5;
    let mut w = Writer::new();
    w.var_i32(0);
    w.var_i32(0);
    w.i16(0);
    w.var_i32(1);
    w.var_i32(1);
    w.var_i32(1);
    w.var_i32(0);
    w.var_i32(inventory::component_id(v, "food").unwrap());
    w.raw(&[1, 2, 3, 4, 5]);
    let packet = RawPacket::new(
        v.packet_id(State::Play, Direction::Clientbound, "set_slot")
            .unwrap(),
        w.into_inner(),
    );
    let expected = packet.clone();
    let mut c = play_connection(packet);
    match c.next_typed_event().unwrap() {
        TypedEvent::Raw {
            packet,
            unsupported: Some(_),
            name: Some("set_slot"),
            ..
        } => assert_eq!(packet, expected),
        other => panic!("unexpected {other:?}"),
    }
}
#[test]
fn unknown_packet_preserves_entire_raw_packet() {
    let packet = RawPacket::new(999, vec![1, 2, 3]);
    let expected = packet.clone();
    let mut c = play_connection(packet);
    match c.next_typed_event().unwrap() {
        TypedEvent::Raw {
            packet,
            name: None,
            unsupported: None,
            ..
        } => assert_eq!(packet, expected),
        other => panic!("unexpected {other:?}"),
    }
}
#[test]
fn malformed_known_packet_does_not_silently_fallback() {
    let v = Version::V1_21_5;
    let p = RawPacket::new(
        v.packet_id(State::Play, Direction::Clientbound, "update_health")
            .unwrap(),
        vec![],
    );
    assert!(play_connection(p).next_typed_event().is_err());
}
#[test]
fn typed_system_chat_dispatch() {
    let v = Version::V1_21_5;
    let body = vec![8, 0, 2, b'H', b'i', 0];
    let p = RawPacket::new(
        v.packet_id(State::Play, Direction::Clientbound, "system_chat")
            .unwrap(),
        body,
    );
    assert!(matches!(
        play_connection(p).next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Chat(ChatPacket::System(_)))
    ));
}
