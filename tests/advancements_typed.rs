use rustwire_mc::{
    codec::Writer,
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{advancements::*, typed::DecodedPacket},
    version::{Direction, State},
    Error, Limits, Version,
};
use std::io::{Cursor, Read, Write};

#[test]
fn every_fixture_has_exact_packet_id_and_state_direction_dispatch() {
    let mut count = 0;
    for line in include_str!("fixtures/advancements.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let fields: Vec<_> = line.split('\t').collect();
        let v = Version::from_protocol(fields[0].parse().unwrap()).unwrap();
        let name = fields[1];
        let direction = if fields[2] == "toClient" {
            Direction::Clientbound
        } else {
            Direction::Serverbound
        };
        let id = fields[3].parse::<i32>().unwrap();
        let bytes: Vec<_> = fields[5]
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let packet = if direction == Direction::Serverbound {
            assert!(
                DecodedPacket::decode(State::Play, name, &bytes, v, Limits::default())
                    .unwrap()
                    .is_none()
            );
            AdvancementTab::decode(&bytes, v, Limits::default())
                .unwrap()
                .packet(v, Limits::default())
                .unwrap()
        } else {
            let Some(DecodedPacket::Advancement(value)) =
                DecodedPacket::decode(State::Play, name, &bytes, v, Limits::default()).unwrap()
            else {
                panic!("wrong family")
            };
            assert!(matches!(
                (&value, name),
                (AdvancementPacket::Update(_), "advancements")
                    | (AdvancementPacket::SelectTab(_), "select_advancement_tab")
            ));
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
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(matches!(
                DecodedPacket::decode(State::Play, name, &trailing, v, Limits::default()),
                Err(Error::Invalid("trailing bytes"))
            ));
            value.packet(v, Limits::default()).unwrap()
        };
        assert_eq!(packet.id, id);
        assert_eq!(packet.data, bytes);
        assert_eq!(v.packet(State::Play, direction, id).unwrap().name, name);
        assert_eq!(v.packet_id(State::Play, direction, name).unwrap(), id);
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
            assert!(v.packet_id(state, direction, name).is_err());
        }
        let wrong = if direction == Direction::Clientbound {
            Direction::Serverbound
        } else {
            Direction::Clientbound
        };
        assert!(v.packet_id(State::Play, wrong, name).is_err());
        count += 1;
    }
    assert_eq!(count, 84);
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
fn connection(v: Version, packet: RawPacket) -> Connection<Memory> {
    let codec = FrameCodec::default();
    let mut login = Writer::new();
    login.raw(&[0; 16]);
    login.string("Rustwire", 16).unwrap();
    login.var_i32(0);
    if (766..=767).contains(&v.protocol()) {
        login.bool(false);
    }
    if v.protocol() >= 776 {
        login.raw(&[0; 16]);
    }
    let mut bytes = codec
        .encode(&RawPacket::new(2, login.into_inner()))
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
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    c
}
#[test]
fn unsupported_icon_retains_full_raw_packet_but_known_malformed_icon_errors() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        // Original partial body: one definition, no parent, display, two NBT
        // strings, then a component icon whose unframed payload is unknown.
        let mut bytes = vec![0, 1, 1, b'a', 0, 1, 8, 0, 1, b'T', 8, 0, 1, b'D'];
        if v.protocol() < 775 {
            bytes.extend([1, 2, 1, 0]);
        } else {
            bytes.extend([2, 1, 1, 0]);
        }
        bytes.extend([255, 255, 255, 255, 7, 9, 8, 7, 6]);
        let id = v
            .packet_id(State::Play, Direction::Clientbound, "advancements")
            .unwrap();
        let raw = RawPacket::new(id, bytes.clone());
        assert!(
            matches!(connection(v,raw.clone()).next_typed_event().unwrap(), TypedEvent::Raw { packet, name: Some("advancements"), unsupported: Some(_), .. } if packet == raw)
        );
        // Known custom_data with an invalid tag kind is malformed, not a gap.
        bytes.truncate(18);
        bytes.extend([0, 255]);
        assert!(matches!(
            connection(v, RawPacket::new(id, bytes)).next_typed_event(),
            Err(Error::Invalid(_))
        ));
    }
}
