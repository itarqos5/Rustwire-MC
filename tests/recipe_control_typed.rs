use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::typed::DecodedPacket,
    version::{Direction, State},
    Error, Limits, Version,
};
use std::io::{Cursor, Read, Write};
#[path = "support/recipe_control_fixtures.rs"]
mod support;
#[test]
fn clientbound_dispatch_is_direction_and_state_scoped() {
    for f in support::fixtures() {
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
        let result =
            DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, Limits::default());
        if f.direction == "toServer" {
            assert!(result.unwrap().is_none());
        } else if f.case == "unknown-action" {
            assert!(matches!(
                result,
                Err(Error::Unsupported("unlock recipes action"))
            ));
        } else {
            let Some(DecodedPacket::RecipeControl(packet)) = result.unwrap() else {
                panic!("wrong typed family")
            };
            assert_eq!(
                packet.packet(f.version, Limits::default()).unwrap(),
                RawPacket::new(f.id, f.bytes.clone())
            );
        }
        if f.direction == "toClient" {
            for end in 0..f.bytes.len() {
                assert!(matches!(
                    DecodedPacket::decode(
                        State::Play,
                        f.name,
                        &f.bytes[..end],
                        f.version,
                        Limits::default()
                    ),
                    Err(Error::Invalid(_) | Error::Eof)
                ));
            }
        }
    }
}
#[test]
fn modern_declaration_add_and_display_bodies_are_explicitly_unimplemented() {
    for &v in Version::ALL {
        let names = if v.protocol() < 768 {
            vec![]
        } else {
            vec![
                "declare_recipes",
                "recipe_book_add",
                "craft_recipe_response",
            ]
        };
        for name in names {
            v.packet_id(State::Play, Direction::Clientbound, name)
                .unwrap();
            // Unsupported whole packets remain raw; this is not validation of
            // their arbitrary bodies or a claim that the supplied bytes are valid.
            assert!(
                DecodedPacket::decode(State::Play, name, &[255], v, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        let missing = if v.protocol() < 768 {
            "recipe_book_settings"
        } else {
            "unlock_recipes"
        };
        assert!(matches!(
            DecodedPacket::decode(State::Play, missing, &[], v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
    }
}
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
fn connection_preserves_unknown_enum_raw_and_rejects_malformed_known_bodies() {
    for f in support::fixtures()
        .into_iter()
        .filter(|f| f.case == "unknown-action")
    {
        let original = RawPacket::new(f.id, f.bytes.clone());
        assert!(
            matches!(connection(f.version, original.clone()).next_typed_event().unwrap(), TypedEvent::Raw { name: Some("unlock_recipes"), packet, unsupported: Some("unlock recipes action"), .. } if packet == original)
        );
        let mut truncated = f.bytes.clone();
        truncated.pop();
        assert!(connection(f.version, RawPacket::new(f.id, truncated))
            .next_typed_event()
            .is_err());
        let mut trailing = f.bytes;
        trailing.push(0);
        assert!(connection(f.version, RawPacket::new(f.id, trailing))
            .next_typed_event()
            .is_err());
    }
}
#[test]
fn connection_retains_unimplemented_modern_display_packets_without_claiming_validation() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 768) {
        for name in [
            "craft_recipe_response",
            "recipe_book_add",
            "declare_recipes",
        ] {
            let original = RawPacket::new(
                v.packet_id(State::Play, Direction::Clientbound, name)
                    .unwrap(),
                vec![255],
            );
            assert!(
                matches!(connection(v, original.clone()).next_typed_event().unwrap(), TypedEvent::Raw { name: Some(actual), packet, unsupported: None, .. } if actual == name && packet == original)
            );
        }
        let id = v
            .packet_id(State::Play, Direction::Clientbound, "recipe_book_remove")
            .unwrap();
        assert!(connection(v, RawPacket::new(id, vec![1]))
            .next_typed_event()
            .is_err());
    }
}
