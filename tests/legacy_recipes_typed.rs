use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::typed::DecodedPacket,
    version::{Direction, State},
    Error, Limits, Version,
};
use std::io::{Cursor, Read, Write};
#[path = "support/legacy_recipe_fixtures.rs"]
mod support;
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
fn every_fixture_is_clientbound_play_only_and_unknown_preserves_the_whole_packet() {
    for f in support::fixtures() {
        assert_eq!(f.direction, "toClient");
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(
                DecodedPacket::decode(state, f.name, &f.bytes, f.version, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        for serverbound in ["craft_recipe_request", "displayed_recipe", "recipe_book"] {
            assert!(DecodedPacket::decode(
                State::Play,
                serverbound,
                &f.bytes,
                f.version,
                Limits::default()
            )
            .unwrap()
            .is_none());
        }
        let raw = RawPacket::new(f.id, f.bytes.clone());
        if f.case.starts_with("unsupported-") {
            assert!(matches!(
                DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, Limits::default()),
                Err(Error::Unsupported("legacy recipe serializer"))
            ));
            assert!(
                matches!(connection(f.version, raw.clone()).next_typed_event().unwrap(), TypedEvent::Raw { name: Some("declare_recipes"), packet, unsupported: Some("legacy recipe serializer"), .. } if packet == raw)
            );
        } else {
            let Some(DecodedPacket::LegacyRecipes(packet)) =
                DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, Limits::default())
                    .unwrap()
            else {
                panic!()
            };
            assert_eq!(packet.packet(f.version, Limits::default()).unwrap(), raw);
            assert!(matches!(
                connection(f.version, raw).next_typed_event().unwrap(),
                TypedEvent::Decoded(DecodedPacket::LegacyRecipes(_))
            ));
            let mut bytes = f.bytes.clone();
            bytes.pop();
            assert!(connection(f.version, RawPacket::new(f.id, bytes))
                .next_typed_event()
                .is_err());
            let mut bytes = f.bytes;
            bytes.push(0);
            assert!(connection(f.version, RawPacket::new(f.id, bytes))
                .next_typed_event()
                .is_err());
        }
    }
}
#[test]
fn modern_declarations_remain_unimplemented_raw_without_body_validation() {
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
        let raw = RawPacket::new(
            v.packet_id(State::Play, Direction::Clientbound, "declare_recipes")
                .unwrap(),
            vec![255],
        );
        assert!(
            matches!(connection(v, raw.clone()).next_typed_event().unwrap(), TypedEvent::Raw { packet, name: Some("declare_recipes"), unsupported: None, .. } if packet == raw)
        );
    }
}
#[test]
fn unknown_component_falls_back_whole_packet_but_invalid_known_slot_does_not() {
    for p in 763..=767 {
        let v = Version::from_protocol(p).unwrap();
        let id = v
            .packet_id(State::Play, Direction::Clientbound, "declare_recipes")
            .unwrap();
        let mut w = Writer::new();
        w.var_i32(1);
        if p < 766 {
            w.string("minecraft:stonecutting", 32767).unwrap();
            w.string("rustwire:r", 32767).unwrap();
        } else {
            w.string("rustwire:r", 32767).unwrap();
            w.var_i32(19);
        }
        w.string("", 32767).unwrap();
        w.var_i32(0); // empty ingredient
        let head = w.as_slice().to_vec();
        if p >= 766 {
            w.var_i32(1);
            w.var_i32(1);
            w.var_i32(1);
            w.var_i32(0);
            w.var_i32(999);
            w.raw(&[255, 0]);
            let raw = RawPacket::new(id, w.into_inner());
            assert!(
                matches!(connection(v, raw.clone()).next_typed_event().unwrap(), TypedEvent::Raw { packet, unsupported: Some("unknown item component ID"), .. } if packet == raw)
            );
        }
        let mut bad = head.clone();
        bad.push(if p <= 765 { 2 } else { 255 });
        if p >= 766 {
            bad.extend_from_slice(&[255, 255, 255, 15]);
        }
        assert!(matches!(
            connection(v, RawPacket::new(id, bad)).next_typed_event(),
            Err(Error::Invalid(_))
        ));
        if p >= 766 {
            let mut w = Writer::new();
            w.raw(&head);
            w.var_i32(1);
            w.var_i32(1);
            w.var_i32(1);
            w.var_i32(0);
            w.var_i32(4);
            w.u8(2); // invalid unbreakable boolean
            assert!(matches!(
                connection(v, RawPacket::new(id, w.into_inner())).next_typed_event(),
                Err(Error::Invalid("boolean"))
            ));
        }
    }
}
