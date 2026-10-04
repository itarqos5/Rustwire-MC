use rustwire_mc::{
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::typed::DecodedPacket,
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
fn debug_samples_are_delivered_without_automatic_subscriptions_or_replies() {
    let (mut c, out) = connect(vec![
        packet(State::Configuration, "finish_configuration", vec![]),
        packet(State::Play, "debug_sample", vec![0, 0]),
    ]);
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    let before = out.borrow().len();
    assert!(
        matches!(c.next_typed_event().unwrap(), TypedEvent::Decoded(DecodedPacket::DebugSample(p)) if p.samples.is_empty())
    );
    assert_eq!(out.borrow().len(), before);
}
