use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::typed::DecodedPacket,
    version::{Direction, State},
    Error, Limits, Version,
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
#[path = "support/recipe_property_fixtures.rs"]
mod support;
#[test]
fn connection_decodes_all_modern_declaration_fixtures() {
    for f in support::fixtures() {
        let raw = RawPacket::new(f.packet_id, f.bytes.clone());
        match connection(f.version, raw).next_typed_event().unwrap() {
            TypedEvent::Decoded(DecodedPacket::ModernRecipes(value)) => assert_eq!(
                value.encode(f.version, Limits::default()).unwrap(),
                f.bytes,
                "{} {}",
                f.version.protocol(),
                f.case
            ),
            event => panic!("wrong event: {event:?}"),
        }
    }
}
#[test]
fn unknown_nested_layout_keeps_entire_original_packet_and_known_malformed_errors() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 768) {
        let id = v
            .packet_id(State::Play, Direction::Clientbound, "declare_recipes")
            .unwrap();
        // Empty property list; one explicit-empty input; an unknown display.
        let raw = RawPacket::new(id, vec![0, 1, 1, 127, 99, 88]);
        assert!(
            matches!(connection(v,raw.clone()).next_typed_event().unwrap(),TypedEvent::Raw { name:Some("declare_recipes"),packet,unsupported:Some("slot display kind"),.. } if packet==raw)
        );
        let kind = if v.protocol() >= 775 { 5 } else { 3 };
        let raw = RawPacket::new(
            id,
            vec![0, 1, 1, kind, 1, 1, 1, 0, 255, 255, 255, 255, 7, 99, 88],
        );
        assert!(
            matches!(connection(v,raw.clone()).next_typed_event().unwrap(),TypedEvent::Raw { packet,unsupported:Some("unknown item component ID"),.. } if packet==raw)
        );
        for malformed in [
            vec![255],
            vec![0],
            vec![0, 1, 1],
            vec![0, 0, 0],
            vec![1, 1, 255, 0, 0],
        ] {
            assert!(matches!(
                connection(v, RawPacket::new(id, malformed)).next_typed_event(),
                Err(Error::Eof | Error::Invalid(_))
            ));
        }
    }
}
