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
fn connection(v: Version, body: Vec<u8>) -> Connection<Memory> {
    let frame = FrameCodec::default();
    let mut success = Writer::new();
    success.raw(&[0; 16]);
    success.string("Rustwire", 16).unwrap();
    success.var_i32(0);
    if v.protocol() == 776 {
        success.raw(&[0; 16]);
    }
    let mut input = frame
        .encode(&RawPacket::new(2, success.into_inner()))
        .unwrap();
    input.extend(
        frame
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
    input.extend(
        frame
            .encode(&RawPacket::new(
                v.packet_id(State::Play, Direction::Clientbound, "tracked_waypoint")
                    .unwrap(),
                body,
            ))
            .unwrap(),
    );
    let mut c = Connection::new(
        Memory {
            input: Cursor::new(input),
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
fn connection_dispatches_known_waypoints_and_preserves_whole_unknown_payloads() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 771) {
        let known = vec![1, 0, 0, 0, 0, 0];
        assert!(matches!(
            connection(v, known).next_typed_event().unwrap(),
            TypedEvent::Decoded(DecodedPacket::Waypoint(_))
        ));
        let unknown = vec![0, 0, 0, 0, 0, 127, 99, 88];
        let raw = RawPacket::new(
            v.packet_id(State::Play, Direction::Clientbound, "tracked_waypoint")
                .unwrap(),
            unknown.clone(),
        );
        assert!(
            matches!(connection(v,unknown).next_typed_event().unwrap(),TypedEvent::Raw{packet,name:Some("tracked_waypoint"),unsupported:Some("waypoint location kind"),..} if packet==raw)
        );
        assert!(matches!(
            connection(v, vec![3]).next_typed_event(),
            Err(Error::Invalid("waypoint operation"))
        ));
        for body in [vec![1], vec![0, 0, 0, 0, 2, 0], vec![0, 0, 0, 0, 0, 3]] {
            assert!(connection(v, body).next_typed_event().is_err());
        }
    }
}
