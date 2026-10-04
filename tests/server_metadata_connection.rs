use rustwire_mc::{
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{
        common::CommonPacket,
        typed::{ChatPacket, DecodedPacket},
    },
    version::{Direction, State},
    Limits, Version,
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
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(bytes)
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
fn connection(play_packets: &[(&str, Vec<u8>)]) -> (Connection<Memory>, Rc<RefCell<Vec<u8>>>) {
    let v = Version::V1_21_5;
    let frame = FrameCodec::default();
    // Independently authored login-success body, with no profile properties.
    let mut success = vec![0; 16];
    success.extend_from_slice(b"\x08Rustwire\x00");
    let mut stream = frame.encode(&RawPacket::new(2, success)).unwrap();
    let mut append = |state, name, bytes| {
        let id = v.packet_id(state, Direction::Clientbound, name).unwrap();
        stream.extend(frame.encode(&RawPacket::new(id, bytes)).unwrap());
    };
    append(
        State::Configuration,
        "custom_report_details",
        vec![1, 1, b'a', 1, b'b'],
    );
    append(State::Configuration, "finish_configuration", vec![]);
    for (name, bytes) in play_packets {
        append(State::Play, name, bytes.clone());
    }
    let output = Rc::new(RefCell::new(vec![]));
    let io = Memory {
        input: Cursor::new(stream),
        output: output.clone(),
    };
    let mut connection = Connection::new(io, v, Limits::default());
    connection
        .start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(
        connection.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    let before_report = output.borrow().len();
    assert!(matches!(
        connection.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Common(CommonPacket::CustomReportDetails(_)))
    ));
    assert_eq!(output.borrow().len(), before_report);
    assert!(matches!(
        connection.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    (connection, output)
}

#[test]
fn metadata_routes_without_automatic_replies_reports_or_chat_actions() {
    let (mut c, output) = connection(&[
        ("server_data", vec![8, 0, 1, b'x', 0]),
        ("custom_report_details", vec![0]),
        ("chat_suggestions", vec![0, 0]),
        ("hide_message", vec![1]),
        ("ping_response", vec![0xff; 8]),
    ]);
    let before = output.borrow().len();
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::ServerData(_))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Common(CommonPacket::CustomReportDetails(_)))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Chat(ChatPacket::Suggestions(_)))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Chat(ChatPacket::Hide(_)))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::PingResponse(_))
    ));
    assert_eq!(output.borrow().len(), before);
}

#[test]
fn malformed_metadata_remains_an_error_without_a_reply() {
    for name in [
        "server_data",
        "custom_report_details",
        "chat_suggestions",
        "hide_message",
        "ping_response",
    ] {
        let (mut c, output) = connection(&[(name, vec![])]);
        let before = output.borrow().len();
        assert!(c.next_typed_event().is_err(), "{name}");
        assert_eq!(output.borrow().len(), before);
    }
}
