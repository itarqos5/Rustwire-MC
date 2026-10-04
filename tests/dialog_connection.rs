use rustwire_mc::{
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{dialog::DialogPacket, typed::DecodedPacket},
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
fn connect(packets: Vec<RawPacket>) -> (Connection<Memory>, Rc<RefCell<Vec<u8>>>) {
    let v = Version::V1_21_6;
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
        Limits::default(),
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
        Version::V1_21_6
            .packet_id(state, Direction::Clientbound, name)
            .unwrap(),
        body,
    )
}
#[test]
fn config_and_play_dialogs_do_not_execute_or_send_actions() {
    let (mut c, out) = connect(vec![
        packet(State::Configuration, "show_dialog", vec![10, 0]),
        packet(State::Configuration, "clear_dialog", vec![]),
        packet(State::Configuration, "finish_configuration", vec![]),
        packet(State::Play, "show_dialog", vec![0, 10, 0]),
        packet(State::Play, "show_dialog", vec![1]),
        packet(State::Play, "clear_dialog", vec![]),
    ]);
    let before = out.borrow().len();
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Dialog(DialogPacket::Show(_)))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Dialog(DialogPacket::Clear(_)))
    ));
    assert_eq!(out.borrow().len(), before);
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    let before = out.borrow().len();
    for _ in 0..2 {
        assert!(matches!(
            c.next_typed_event().unwrap(),
            TypedEvent::Decoded(DecodedPacket::Dialog(DialogPacket::Show(_)))
        ));
    }
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Decoded(DecodedPacket::Dialog(DialogPacket::Clear(_)))
    ));
    assert_eq!(out.borrow().len(), before);
}
#[test]
fn malformed_dialogs_error_and_unrelated_unframed_unsupported_packets_keep_raw_bytes() {
    for state in [State::Configuration, State::Play] {
        for (name, body) in [("clear_dialog", vec![1]), ("show_dialog", vec![0])] {
            let mut packets = vec![];
            if state == State::Play {
                packets.push(packet(State::Configuration, "finish_configuration", vec![]));
            }
            packets.push(packet(state, name, body));
            let (mut c, out) = connect(packets);
            if state == State::Play {
                c.next_typed_event().unwrap();
            }
            let before = out.borrow().len();
            assert!(c.next_typed_event().is_err());
            assert_eq!(out.borrow().len(), before);
        }
    }
    // Existing unframed item-component fallback must retain the entire packet.
    let raw = packet(
        State::Play,
        "set_slot",
        vec![0, 0, 0, 0, 1, 1, 1, 0, 255, 255, 255, 255, 7, 9, 8, 7],
    );
    let (mut c, _) = connect(vec![
        packet(State::Configuration, "finish_configuration", vec![]),
        raw.clone(),
    ]);
    c.next_typed_event().unwrap();
    assert!(
        matches!(c.next_typed_event().unwrap(),TypedEvent::Raw {packet,unsupported:Some(_),name:Some("set_slot"),..} if packet==raw)
    );
}
