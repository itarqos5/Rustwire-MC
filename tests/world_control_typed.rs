use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::typed::DecodedPacket,
    version::{Direction, State},
    Error, Limits, Version,
};
use std::io::{Cursor, Read, Write};
#[path = "support/world_control_fixtures.rs"]
mod support;

#[test]
fn named_dispatch_ids_state_boundaries_and_every_fixture_prefix() {
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
        if f.case != "raw-anchors" {
            let Some(DecodedPacket::WorldControl(p)) =
                DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, Limits::default())
                    .unwrap()
            else {
                panic!("wrong family");
            };
            let packet = p.packet(f.version, Limits::default()).unwrap();
            assert_eq!(packet.id, f.id);
            assert_eq!(packet.data, f.bytes);
        } else {
            assert!(matches!(
                DecodedPacket::decode(State::Play, f.name, &f.bytes, f.version, Limits::default()),
                Err(Error::Unsupported("face-player anchor"))
            ));
        }
        for end in 0..f.bytes.len() {
            assert!(matches!(
                DecodedPacket::decode(
                    State::Play,
                    f.name,
                    &f.bytes[..end],
                    f.version,
                    Limits::default()
                ),
                Err(Error::Eof | Error::Invalid(_))
            ));
        }
        let mut trailing = f.bytes.clone();
        trailing.push(0);
        assert!(matches!(
            DecodedPacket::decode(State::Play, f.name, &trailing, f.version, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
    }
}

#[test]
fn named_dispatch_rejects_missing_version_packets() {
    for &v in Version::ALL {
        for (name, minimum) in [
            ("player_rotation", 768),
            ("set_projectile_power", 766),
            ("set_ticking_state", 765),
            ("step_tick", 765),
        ] {
            if v.protocol() < minimum {
                assert!(matches!(
                    DecodedPacket::decode(State::Play, name, &[], v, Limits::default()),
                    Err(Error::Unsupported(_))
                ));
            }
        }
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
fn play_connection(packet: RawPacket) -> Connection<Memory> {
    let version = Version::V1_21_5;
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
                version
                    .packet_id(
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
    let mut c = Connection::new(
        Memory {
            input: Cursor::new(bytes),
            output: vec![],
        },
        version,
        Limits::default(),
    );
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
fn unsupported_anchors_preserve_raw_but_malformed_bodies_remain_errors() {
    let f = support::fixtures()
        .into_iter()
        .find(|f| f.version == Version::V1_21_5 && f.case == "raw-anchors")
        .unwrap();
    let original = RawPacket::new(f.id, f.bytes.clone());
    let mut c = play_connection(original.clone());
    assert!(
        matches!(c.next_typed_event().unwrap(), TypedEvent::Raw { name: Some("face_player"), packet, unsupported: Some("face-player anchor"), .. } if packet == original)
    );
    let mut truncated = f.bytes.clone();
    truncated.pop();
    assert!(play_connection(RawPacket::new(f.id, truncated))
        .next_typed_event()
        .is_err());
    let mut trailing = f.bytes;
    trailing.push(0);
    assert!(play_connection(RawPacket::new(f.id, trailing))
        .next_typed_event()
        .is_err());
}

#[test]
fn malformed_query_root_never_becomes_raw_fallback() {
    let v = Version::V1_21_5;
    let id = v
        .packet_id(State::Play, Direction::Clientbound, "nbt_query_response")
        .unwrap();
    for bytes in [vec![0, 1, 0x80], vec![0, 1], vec![0, 1, 0x80, 0]] {
        assert!(play_connection(RawPacket::new(id, bytes))
            .next_typed_event()
            .is_err());
    }
}
