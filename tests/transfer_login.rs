use rustwire_mc::{
    codec::{Reader, Writer},
    connection::{Connection, Event},
    frame::{FrameCodec, RawPacket},
    packet::{self, HandshakeIntent},
    version::{Direction, State},
    Error, Limits, Version,
};
use std::io::{Cursor, Read, Write};
#[derive(Default)]
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
        self.output.extend(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn expected_handshake(v: Version, intent: u8) -> Vec<u8> {
    // Every supported protocol is two VarInt bytes; this is independent of Writer.
    vec![
        (v.protocol() as u8 & 127) | 128,
        (v.protocol() >> 7) as u8,
        1,
        b'a',
        0x63,
        0xdd,
        intent,
    ]
}
fn start_bytes(v: Version, intent: u8) -> Vec<u8> {
    let mut bytes = vec![8, 0];
    bytes.extend(expected_handshake(v, intent));
    bytes.extend([26, 0, 8]);
    bytes.extend(b"Rustwire");
    bytes.extend([7; 16]);
    bytes
}
#[test]
fn ordinary_handshakes_remain_byte_identical_in_all_families() {
    for &v in Version::ALL {
        for (state, intent, id) in [
            (State::Status, HandshakeIntent::Status, 1),
            (State::Login, HandshakeIntent::Login, 2),
        ] {
            let expected = RawPacket::new(0, expected_handshake(v, id));
            assert_eq!(packet::handshake(v, "a", 25565, state).unwrap(), expected);
            assert_eq!(
                packet::handshake_with_intent(v, "a", 25565, intent).unwrap(),
                expected
            );
        }
        for state in [State::Handshake, State::Configuration, State::Play] {
            assert!(matches!(
                packet::handshake(v, "a", 25565, state),
                Err(Error::State(_))
            ));
        }
    }
}
#[test]
fn transfer_intent_exact_boundary_host_limit_and_port_width() {
    for &v in Version::ALL {
        let result = packet::handshake_with_intent(v, "a", 25565, HandshakeIntent::Transfer);
        if v.protocol() < 766 {
            assert!(matches!(result, Err(Error::Unsupported(_))));
            continue;
        }
        assert_eq!(result.unwrap(), RawPacket::new(0, expected_handshake(v, 3)));
        let host = "😀".repeat(127) + "a";
        let packet =
            packet::handshake_with_intent(v, &host, u16::MAX, HandshakeIntent::Transfer).unwrap();
        let mut r = Reader::new(&packet.data, Limits::default());
        assert_eq!(r.var_i32().unwrap(), v.protocol());
        assert_eq!(r.string(255).unwrap(), host);
        assert_eq!(r.u16().unwrap(), u16::MAX);
        assert_eq!(r.var_i32().unwrap(), 3);
        r.finish().unwrap();
        assert!(
            packet::handshake_with_intent(v, &"😀".repeat(128), 0, HandshakeIntent::Transfer)
                .is_err()
        );
    }
}
#[test]
fn explicit_transfer_login_sends_only_two_expected_frames() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        let mut c = Connection::new(Memory::default(), v, Limits::default());
        c.start_transfer_login("a", 25565, "Rustwire", [7; 16])
            .unwrap();
        assert_eq!(c.state(), State::Login);
        assert!(matches!(
            c.start_transfer_login("other", 1, "Rustwire", [7; 16]),
            Err(Error::State(_))
        ));
        assert!(matches!(
            c.start_login("other", 1, "Rustwire", [7; 16]),
            Err(Error::State(_))
        ));
        assert_eq!(c.into_inner().output, start_bytes(v, 3));
        let mut ordinary = Connection::new(Memory::default(), v, Limits::default());
        ordinary
            .start_login("a", 25565, "Rustwire", [7; 16])
            .unwrap();
        assert_eq!(ordinary.into_inner().output, start_bytes(v, 2));
    }
}
#[test]
fn transfer_preflight_failures_leave_handshake_and_output_untouched() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() < 766) {
        let mut c = Connection::new(Memory::default(), v, Limits::default());
        assert!(matches!(
            c.start_transfer_login("a", 25565, "Rustwire", [7; 16]),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(c.state(), State::Handshake);
        assert!(c.into_inner().output.is_empty());
    }
    for (host, user, limits) in [
        ("a".repeat(256), "Rustwire", Limits::default()),
        ("a".into(), "not valid", Limits::default()),
        (
            "a".into(),
            "Rustwire",
            Limits {
                max_packet: 16,
                ..Limits::default()
            },
        ),
        (
            "a".into(),
            "Rustwire",
            Limits {
                max_frame: 16,
                ..Limits::default()
            },
        ),
        (
            "a".into(),
            "Rustwire",
            Limits {
                max_frame: 1,
                ..Limits::default()
            },
        ),
    ] {
        let mut c = Connection::new(Memory::default(), Version::V26_2, limits);
        assert!(c.start_transfer_login(&host, 25565, user, [7; 16]).is_err());
        assert_eq!(c.state(), State::Handshake);
        assert!(c.into_inner().output.is_empty());
    }
}
#[test]
fn transfer_intent_does_not_override_server_authentication_requests() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        for authenticate in [false, true] {
            let mut body = Writer::new();
            body.string("", 20).unwrap();
            body.bytes(&[1, 2, 3]).unwrap();
            body.bytes(&[4, 5]).unwrap();
            body.bool(authenticate);
            let id = v
                .packet_id(State::Login, Direction::Clientbound, "encryption_begin")
                .unwrap();
            let input = FrameCodec::default()
                .encode(&RawPacket::new(id, body.into_inner()))
                .unwrap();
            let mut c = Connection::new(
                Memory {
                    input: Cursor::new(input),
                    ..Memory::default()
                },
                v,
                Limits::default(),
            );
            c.start_transfer_login("a", 25565, "Rustwire", [7; 16])
                .unwrap();
            let Event::EncryptionRequested(request) = c.next_event().unwrap() else {
                panic!()
            };
            assert_eq!(request.should_authenticate, authenticate);
            assert_eq!(request.public_key, [1, 2, 3]);
            assert_eq!(request.verify_token, [4, 5]);
            assert_eq!(c.state(), State::Login);
            assert_eq!(c.into_inner().output, start_bytes(v, 3));
        }
    }
}
#[test]
fn transfer_login_enters_existing_configuration_flow() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        let mut body = Writer::new();
        body.raw(&[7; 16]);
        body.string("Rustwire", 16).unwrap();
        body.var_i32(0);
        if (766..=767).contains(&v.protocol()) {
            body.bool(false)
        }
        if v.protocol() >= 776 {
            body.raw(&[8; 16]);
        }
        let frame = FrameCodec::default();
        let mut input = frame.encode(&RawPacket::new(2, body.into_inner())).unwrap();
        let finish = v
            .packet_id(
                State::Configuration,
                Direction::Clientbound,
                "finish_configuration",
            )
            .unwrap();
        input.extend(frame.encode(&RawPacket::new(finish, [])).unwrap());
        let mut c = Connection::new(
            Memory {
                input: Cursor::new(input),
                ..Memory::default()
            },
            v,
            Limits::default(),
        );
        c.start_transfer_login("a", 25565, "Rustwire", [7; 16])
            .unwrap();
        assert!(matches!(c.next_event().unwrap(), Event::LoginSuccess(_)));
        assert_eq!(c.state(), State::Configuration);
        assert!(matches!(c.next_event().unwrap(), Event::Ready));
        assert_eq!(c.state(), State::Play);
        let mut expected = start_bytes(v, 3);
        expected.extend([1, 3]);
        expected.extend([
            1,
            v.packet_id(
                State::Configuration,
                Direction::Serverbound,
                "finish_configuration",
            )
            .unwrap() as u8,
        ]);
        assert_eq!(c.into_inner().output, expected);
    }
}
#[test]
fn partial_transfer_write_poisoning_prevents_unsafe_retry() {
    struct Broken {
        bytes: usize,
        calls: usize,
    }
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Ok(0)
        }
    }
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.calls += 1;
            if self.bytes == 0 {
                self.bytes = 1;
                Ok(1)
            } else {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut c = Connection::new(
        Broken { bytes: 0, calls: 0 },
        Version::V26_2,
        Limits::default(),
    );
    assert!(matches!(
        c.start_transfer_login("a", 25565, "Rustwire", [7; 16]),
        Err(Error::Io(_))
    ));
    assert!(matches!(
        c.send(&RawPacket::new(0, [])),
        Err(Error::State(_))
    ));
    assert!(matches!(
        c.start_transfer_login("a", 25565, "Rustwire", [7; 16]),
        Err(Error::State(_))
    ));
    let b = c.into_inner();
    assert_eq!(b.bytes, 1);
    assert_eq!(b.calls, 2);
}
