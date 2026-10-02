use rustwire_mc::{
    codec::{Reader, Writer},
    connection::{Connection, Event},
    frame::{FrameCodec, RawPacket},
    packet,
    version::{Direction, State},
    Limits, Version,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};
fn packet_id(v: Version, s: State, d: Direction, n: &str) -> i32 {
    v.packet_id(s, d, n).unwrap()
}
fn login_success(v: Version) -> RawPacket {
    let mut w = Writer::new();
    w.raw(&[1; 16]);
    w.string("Rustwire", 16).unwrap();
    w.var_i32(0);
    if (766..=767).contains(&v.protocol()) {
        w.bool(false);
    }
    if v.protocol() >= 776 {
        w.raw(&[2; 16]);
    }
    RawPacket::new(2, w.into_inner())
}
#[test]
fn local_mock_login_all_release_families() {
    for &version in Version::ALL {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut c = Connection::new(stream, version, Limits::default());
            let handshake = c.receive().unwrap();
            assert_eq!(handshake.id, 0);
            let mut r = Reader::new(&handshake.data, Limits::default());
            assert_eq!(r.var_i32().unwrap(), version.protocol());
            assert_eq!(r.string(255).unwrap(), "localhost");
            assert_eq!(r.u16().unwrap(), address.port());
            assert_eq!(r.var_i32().unwrap(), 2);
            r.finish().unwrap();
            let start = c.receive().unwrap();
            assert_eq!(start.id, 0);
            let mut r = Reader::new(&start.data, Limits::default());
            assert_eq!(r.string(16).unwrap(), "Rustwire");
            if version.protocol() == 763 {
                assert!(r.bool().unwrap());
            }
            assert_eq!(r.uuid().unwrap(), [1; 16]);
            r.finish().unwrap();
            #[cfg(feature = "compression")]
            {
                let mut w = Writer::new();
                w.var_i32(64);
                c.send(&RawPacket::new(3, w.into_inner())).unwrap();
                c.set_compression(Some(64)).unwrap();
            }
            c.send(&login_success(version)).unwrap();
            if version.has_configuration() {
                assert_eq!(c.receive().unwrap().id, 3);
                if version.protocol() >= 766 {
                    let mut w = Writer::new();
                    w.var_i32(0);
                    c.send(&RawPacket::new(
                        packet_id(
                            version,
                            State::Configuration,
                            Direction::Clientbound,
                            "select_known_packs",
                        ),
                        w.into_inner(),
                    ))
                    .unwrap();
                    let reply = c.receive().unwrap();
                    assert_eq!(
                        reply.id,
                        packet_id(
                            version,
                            State::Configuration,
                            Direction::Serverbound,
                            "select_known_packs"
                        )
                    );
                    assert_eq!(reply.data, vec![0]);
                }
                c.send(&RawPacket::new(
                    packet_id(
                        version,
                        State::Configuration,
                        Direction::Clientbound,
                        "finish_configuration",
                    ),
                    [],
                ))
                .unwrap();
                let reply = c.receive().unwrap();
                assert_eq!(
                    reply.id,
                    packet_id(
                        version,
                        State::Configuration,
                        Direction::Serverbound,
                        "finish_configuration"
                    )
                );
            }
            c.send(&RawPacket::new(
                packet_id(version, State::Play, Direction::Clientbound, "keep_alive"),
                123i64.to_be_bytes(),
            ))
            .unwrap();
            let response = c.receive().unwrap();
            assert_eq!(
                response.id,
                packet_id(version, State::Play, Direction::Serverbound, "keep_alive")
            );
            assert_eq!(response.data, 123i64.to_be_bytes());
        });
        let mut client =
            Connection::connect(address, version, Duration::from_secs(3), Limits::default())
                .unwrap();
        client
            .start_login("localhost", address.port(), "Rustwire", [1; 16])
            .unwrap();
        let mut ready = false;
        for _ in 0..10 {
            match client.next_event().unwrap() {
                Event::LoginSuccess(p) => {
                    assert_eq!(p.username, "Rustwire");
                    if !version.has_configuration() {
                        ready = true;
                    }
                }
                Event::Ready => ready = true,
                Event::KnownPacks(_) => client.select_known_packs(&[]).unwrap(),
                Event::KeepAlive(123) => {
                    assert!(ready);
                    break;
                }
                Event::Compression(Some(64)) => {}
                other => panic!("unexpected event {other:?}"),
            }
        }
        assert!(ready);
        server.join().unwrap();
    }
}
#[test]
fn local_status_and_nonce() {
    let v = Version::V26_2;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        let mut c = Connection::new(s, v, Limits::default());
        assert_eq!(c.receive().unwrap().id, 0);
        assert_eq!(c.receive().unwrap(), RawPacket::new(0, []));
        let mut w = Writer::new();
        w.string("{\"version\":{\"protocol\":776}}", 32767).unwrap();
        c.send(&RawPacket::new(0, w.into_inner())).unwrap();
        let ping = c.receive().unwrap();
        assert_eq!(ping.id, 1);
        c.send(&ping).unwrap();
    });
    let mut c = Connection::connect(addr, v, Duration::from_secs(3), Limits::default()).unwrap();
    let response = c.status("localhost", addr.port(), -123).unwrap();
    assert!(response.json.contains("776"));
    assert!(c
        .start_login("localhost", addr.port(), "Rustwire", [0; 16])
        .is_err());
    server.join().unwrap();
}
#[derive(Debug)]
struct Fragmented {
    input: std::io::Cursor<Vec<u8>>,
    output: Vec<u8>,
}
impl Read for Fragmented {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        if b.is_empty() {
            return Ok(0);
        }
        self.input.read(&mut b[..1])
    }
}
impl Write for Fragmented {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if b.is_empty() {
            return Ok(0);
        }
        self.output.push(b[0]);
        Ok(1)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[test]
fn fragmented_transport_and_failure_poisoning() {
    let p = RawPacket::new(43, vec![1; 300]);
    let bytes = FrameCodec::default().encode(&p).unwrap();
    let io = Fragmented {
        input: std::io::Cursor::new(bytes),
        output: vec![],
    };
    let mut c = Connection::new(io, Version::V1_21, Limits::default());
    assert_eq!(c.receive().unwrap(), p);
    c.send(&p).unwrap();
    assert!(c.receive().is_err());
    assert!(c.send(&p).is_err());
    assert!(c.receive().is_err());
}
#[test]
fn login_validation_is_transactional() {
    let io = Fragmented {
        input: std::io::Cursor::new(vec![]),
        output: vec![],
    };
    let mut c = Connection::new(io, Version::V1_21, Limits::default());
    assert!(c
        .start_login("localhost", 25565, "bad name", [0; 16])
        .is_err());
    assert!(c.into_inner().output.is_empty());
}
#[test]
fn movement_and_position_fixtures() {
    for &v in Version::ALL {
        let mut w = Writer::new();
        if v.protocol() >= 768 {
            w.var_i32(123);
        }
        w.f64(1.5);
        w.f64(-64.);
        w.f64(2.5);
        if v.protocol() >= 768 {
            w.f64(0.);
            w.f64(1.);
            w.f64(2.);
        }
        w.f32(90.);
        w.f32(10.);
        if v.protocol() >= 768 {
            w.i32(3);
        } else {
            w.u8(3);
            w.var_i32(123);
        }
        let pos = packet::PositionSync::decode(w.as_slice(), v, Limits::default()).unwrap();
        assert_eq!(pos.teleport_id, 123);
        assert_eq!(pos.y, -64.);
        assert_eq!(pos.relative_flags, 3);
        assert_eq!(pos.acknowledgement(v).unwrap().data, vec![123]);
        assert!(packet::player_position(v, f64::NAN, 0., 0., false, false).is_err());
    }
}
#[test]
fn public_packet_decoders_enforce_byte_budget() {
    let mut w = Writer::new();
    w.raw(&[0; 16]);
    w.string("x", 16).unwrap();
    w.var_i32(0);
    assert!(packet::LoginSuccess::decode(
        w.as_slice(),
        Version::V1_20,
        Limits {
            max_packet: 1,
            ..Limits::default()
        }
    )
    .is_err());
}
#[cfg(feature = "compression")]
#[test]
fn negative_threshold_disables_compression() {
    let mut w = Writer::new();
    w.var_i32(-1);
    let input = FrameCodec::default()
        .encode(&RawPacket::new(3, w.into_inner()))
        .unwrap();
    let io = Fragmented {
        input: std::io::Cursor::new(input),
        output: vec![],
    };
    let mut c = Connection::new(io, Version::V1_21, Limits::default());
    c.start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(c.next_event().unwrap(), Event::Compression(None)));
}
fn join_game_fixture(v: Version) -> Vec<u8> {
    let mut w = Writer::new();
    w.i32(42);
    w.bool(false);
    if v.protocol() == 763 {
        w.u8(1);
        w.u8(255);
    }
    w.var_i32(1);
    w.string("minecraft:overworld", 32767).unwrap();
    if v.protocol() == 763 {
        w.raw(&[10, 0, 0, 0]);
        w.string("minecraft:overworld", 32767).unwrap();
        w.string("minecraft:overworld", 32767).unwrap();
        w.i64(123);
    }
    w.var_i32(10);
    w.var_i32(8);
    w.var_i32(8);
    w.bool(false);
    w.bool(true);
    if v.protocol() >= 764 {
        w.bool(false);
        if v.protocol() < 766 {
            w.string("minecraft:overworld", 32767).unwrap();
        } else {
            w.var_i32(0);
        }
        w.string("minecraft:overworld", 32767).unwrap();
        w.i64(123);
        w.u8(1);
        w.u8(255);
    }
    w.bool(false);
    w.bool(true);
    w.bool(false);
    w.var_i32(0);
    if v.protocol() >= 768 {
        w.var_i32(63);
    }
    if v.protocol() >= 776 {
        w.bool(false);
    }
    if v.protocol() >= 766 {
        w.bool(false);
    }
    w.into_inner()
}
#[test]
fn join_game_metadata_all_release_families() {
    for &v in Version::ALL {
        let bytes = join_game_fixture(v);
        let join = packet::JoinGame::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(join.entity_id, 42);
        assert_eq!(join.spawn.dimension_name, "minecraft:overworld");
        assert_eq!(join.spawn.previous_game_mode, -1);
        assert_eq!(join.dimension_codec.is_some(), v.protocol() == 763);
        assert_eq!(join.online_mode.is_some(), v.protocol() == 776);
        assert_eq!(join.spawn.sea_level.is_some(), v.protocol() >= 768);
        for n in 0..bytes.len() {
            assert!(packet::JoinGame::decode(&bytes[..n], v, Limits::default()).is_err());
        }
    }
}
#[test]
fn legacy_settings_wait_for_join_game() {
    let v = Version::V1_20;
    let codec = FrameCodec::default();
    let mut input = codec.encode(&login_success(v)).unwrap();
    input.extend(
        codec
            .encode(&RawPacket::new(
                packet_id(v, State::Play, Direction::Clientbound, "login"),
                join_game_fixture(v),
            ))
            .unwrap(),
    );
    let io = Fragmented {
        input: std::io::Cursor::new(input),
        output: vec![],
    };
    let mut c = Connection::new(io, v, Limits::default());
    c.start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(c.next_event().unwrap(), Event::LoginSuccess(_)));
    assert!(c.send_settings(&packet::ClientSettings::default()).is_err());
    assert!(matches!(c.next_event().unwrap(), Event::Joined(_)));
    c.send_settings(&packet::ClientSettings::default()).unwrap();
}
