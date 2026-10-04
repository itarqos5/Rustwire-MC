use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::typed::DecodedPacket,
    version::{Direction, State},
    Limits, Version,
};
use std::io::{Cursor, Read, Write};
struct Memory {
    input: Cursor<Vec<u8>>,
    output: Vec<u8>,
}
impl Read for Memory {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(bytes)
    }
}
impl Write for Memory {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn connection(v: Version, packet: RawPacket) -> Connection<Memory> {
    let codec = FrameCodec::default();
    let mut success = Writer::new();
    success.raw(&[0; 16]);
    success.string("Rustwire", 16).unwrap();
    success.var_i32(0);
    if (766..=767).contains(&v.protocol()) {
        success.bool(false);
    }
    if v.protocol() >= 776 {
        success.raw(&[0; 16]);
    }
    let mut bytes = codec
        .encode(&RawPacket::new(2, success.into_inner()))
        .unwrap();
    if v.protocol() >= 764 {
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
    }
    bytes.extend(codec.encode(&packet).unwrap());
    let mut c = Connection::new(
        Memory {
            input: Cursor::new(bytes),
            output: vec![],
        },
        v,
        Limits::default(),
    );
    c.start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    if v.protocol() >= 764 {
        assert!(matches!(
            c.next_typed_event().unwrap(),
            TypedEvent::Control(Event::Ready)
        ));
    }
    c
}
#[test]
fn connection_keeps_unknown_unframed_display_or_component_body_intact() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 768) {
        for (body, reason) in [
            (vec![0, 5, 99, 88], "recipe display kind"),
            (vec![0, 3, 127, 99, 88], "slot display kind"),
            (
                if v.protocol() >= 775 {
                    vec![0, 3, 5, 1, 1, 1, 0, 255, 255, 255, 255, 7, 99, 88]
                } else {
                    vec![0, 3, 3, 1, 1, 1, 0, 255, 255, 255, 255, 7, 99, 88]
                },
                "unknown component ID",
            ),
        ] {
            let raw = RawPacket::new(
                v.packet_id(State::Play, Direction::Clientbound, "craft_recipe_response")
                    .unwrap(),
                body,
            );
            let event = connection(v, raw.clone()).next_typed_event().unwrap();
            match event {
                TypedEvent::Raw {
                    packet,
                    unsupported: Some(actual),
                    ..
                } => {
                    assert_eq!(packet, raw);
                    if reason != "unknown component ID" {
                        assert_eq!(actual, reason);
                    }
                }
                _ => panic!("not raw fallback: {event:?}"),
            }
        }
        for body in [vec![0], vec![0, 3, 0], vec![0, 3, 0, 0, 0, 0]] {
            let raw = RawPacket::new(
                v.packet_id(State::Play, Direction::Clientbound, "craft_recipe_response")
                    .unwrap(),
                body,
            );
            assert!(connection(v, raw).next_typed_event().is_err());
        }
    }
}
#[test]
fn modern_declarations_are_still_outside_this_slice() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 768) {
        assert!(DecodedPacket::decode(
            State::Play,
            "declare_recipes",
            &[255],
            v,
            Limits::default()
        )
        .unwrap()
        .is_none());
    }
}
