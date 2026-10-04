use rustwire_mc::{
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{debug_values::DebugValuePacket, typed::DecodedPacket},
    version::{Direction, State},
    Error, Limits, Version,
};
use std::{
    cell::RefCell,
    io::{Cursor, Read, Write},
    rc::Rc,
};
struct Memory {
    input: Cursor<Vec<u8>>,
    output: Rc<RefCell<Vec<u8>>>,
}
impl Read for Memory {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(out)
    }
}
impl Write for Memory {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.output.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn connect(packets: Vec<RawPacket>, limits: Limits) -> (Connection<Memory>, Rc<RefCell<Vec<u8>>>) {
    let v = Version::V1_21_9;
    let frame = FrameCodec::default();
    let mut success = vec![0; 16];
    success.extend(b"\x08Rustwire\0");
    let mut bytes = frame.encode(&RawPacket::new(2, success)).unwrap();
    for packet in packets {
        bytes.extend(frame.encode(&packet).unwrap());
    }
    let output = Rc::new(RefCell::new(vec![]));
    let mut c = Connection::new(
        Memory {
            input: Cursor::new(bytes),
            output: output.clone(),
        },
        v,
        limits,
    );
    c.start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    (c, output)
}
fn packet(state: State, name: &str, body: Vec<u8>) -> RawPacket {
    RawPacket::new(
        Version::V1_21_9
            .packet_id(state, Direction::Clientbound, name)
            .unwrap(),
        body,
    )
}
#[test]
fn all_debug_envelopes_deliver_without_subscriptions_replies_or_world_changes() {
    let packets = [
        ("debug_event", vec![10]),
        ("debug_entity_value", vec![1, 10, 1]),
        ("debug_block_value", vec![0, 0, 0, 0, 0, 0, 0, 0, 10, 0]),
        (
            "debug_chunk_value",
            vec![0, 0, 0, 3, 255, 255, 255, 254, 10, 1],
        ),
    ];
    for (name, bytes) in packets {
        let (mut c, out) = connect(
            vec![
                packet(State::Configuration, "finish_configuration", vec![]),
                packet(State::Play, name, bytes),
            ],
            Limits::default(),
        );
        assert!(matches!(
            c.next_typed_event().unwrap(),
            TypedEvent::Control(Event::Ready)
        ));
        let before = out.borrow().len();
        let TypedEvent::Decoded(DecodedPacket::DebugValue(value)) = c.next_typed_event().unwrap()
        else {
            panic!()
        };
        assert!(matches!(
            (name, *value),
            ("debug_event", DebugValuePacket::Event(_))
                | ("debug_entity_value", DebugValuePacket::Entity(_))
                | ("debug_block_value", DebugValuePacket::Block(_))
                | ("debug_chunk_value", DebugValuePacket::Chunk(_))
        ));
        assert_eq!(out.borrow().len(), before);
        assert_eq!(c.state(), State::Play);
    }
}
#[test]
fn unknown_value_kinds_preserve_whole_raw_body_without_replies() {
    let mut newer_path = vec![5, 0];
    newer_path.extend([0; 12]);
    newer_path.push(1);
    newer_path.extend([0; 21]);
    newer_path.push(26);
    newer_path.extend([0; 11]);
    for (name, body) in [
        ("debug_event", newer_path),
        ("debug_event", vec![0]),
        ("debug_event", vec![16, 99, 88]),
        ("debug_entity_value", vec![127, 0, 0]),
        ("debug_block_value", vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 99]),
    ] {
        let raw = packet(State::Play, name, body);
        let (mut c, out) = connect(
            vec![
                packet(State::Configuration, "finish_configuration", vec![]),
                raw.clone(),
            ],
            Limits::default(),
        );
        assert!(matches!(
            c.next_typed_event().unwrap(),
            TypedEvent::Control(Event::Ready)
        ));
        let before = out.borrow().len();
        assert!(
            matches!(c.next_typed_event().unwrap(), TypedEvent::Raw { packet, unsupported: Some(_), .. } if packet == raw)
        );
        assert_eq!(out.borrow().len(), before);
    }
}
#[test]
fn malformed_truncated_trailing_and_over_budget_values_remain_connection_errors() {
    for (name, body, limits, error) in [
        ("debug_event", vec![1, 2], Limits::default(), "invalid"),
        ("debug_event", vec![15, 1], Limits::default(), "eof"),
        ("debug_event", vec![10, 0], Limits::default(), "invalid"),
        (
            "debug_entity_value",
            vec![1, 10, 2],
            Limits::default(),
            "invalid",
        ),
        (
            "debug_event",
            vec![4, 2, 0, 0, 0, 0, 0, 0],
            Limits {
                max_collection: 1,
                ..Limits::default()
            },
            "limit",
        ),
    ] {
        let (mut c, out) = connect(
            vec![
                packet(State::Configuration, "finish_configuration", vec![]),
                packet(State::Play, name, body),
                packet(State::Play, "debug_event", vec![10]),
            ],
            limits,
        );
        assert!(matches!(
            c.next_typed_event().unwrap(),
            TypedEvent::Control(Event::Ready)
        ));
        let before = out.borrow().len();
        match error {
            "invalid" => assert!(matches!(c.next_typed_event(), Err(Error::Invalid(_)))),
            "eof" => assert!(matches!(c.next_typed_event(), Err(Error::Eof))),
            "limit" => assert!(matches!(c.next_typed_event(), Err(Error::Limit(_)))),
            _ => panic!(),
        }
        // Selected-body errors consume exactly one complete frame. The caller
        // retains control over connection policy; a later frame is not swallowed.
        // This does not change the recommendation to end malformed connections.
        assert!(
            matches!(c.next_typed_event().unwrap(), TypedEvent::Decoded(DecodedPacket::DebugValue(value)) if matches!(*value, DebugValuePacket::Event(_)))
        );
        assert_eq!(out.borrow().len(), before);
    }
}
