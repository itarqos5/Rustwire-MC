use rustwire_mc::{
    codec::{BlockPosition, Reader, Writer},
    frame::{FrameCodec, RawPacket},
    Limits, Version,
};
#[test]
fn varint_boundary_roundtrips() {
    for n in [0, 1, 127, 128, 255, 2_097_151, i32::MAX, i32::MIN, -1] {
        let mut w = Writer::new();
        w.var_i32(n);
        let mut r = Reader::new(w.as_slice(), Limits::default());
        assert_eq!(r.var_i32().unwrap(), n);
        r.finish().unwrap();
    }
}
#[test]
fn varlong_boundary_roundtrips() {
    for n in [0, 127, 128, i64::MAX, i64::MIN, -1] {
        let mut w = Writer::new();
        w.var_i64(n);
        let mut r = Reader::new(w.as_slice(), Limits::default());
        assert_eq!(r.var_i64().unwrap(), n);
        r.finish().unwrap();
    }
}
#[test]
fn negative_varints_are_not_zigzag() {
    let mut w = Writer::new();
    w.var_i32(-1);
    assert_eq!(w.as_slice(), &[255, 255, 255, 255, 15]);
}
#[test]
fn malformed_varints_fail() {
    for b in [
        &[255, 255, 255, 255, 16][..],
        &[128, 128, 128, 128, 128, 0][..],
    ] {
        assert!(Reader::new(b, Limits::default()).var_i32().is_err());
    }
}
#[test]
fn incomplete_input_does_not_panic() {
    for n in 0..5 {
        assert!(Reader::new(&[128; 5][..n], Limits::default())
            .var_i32()
            .is_err());
    }
}
#[test]
fn strings_count_java_utf16_units() {
    let mut w = Writer::new();
    assert!(w.string("🦀", 1).is_err());
    w.string("🦀", 2).unwrap();
    assert_eq!(
        Reader::new(w.as_slice(), Limits::default())
            .string(2)
            .unwrap(),
        "🦀"
    );
    assert!(Reader::new(w.as_slice(), Limits::default())
        .string(1)
        .is_err());
}
#[test]
fn strict_bool() {
    assert!(Reader::new(&[2], Limits::default()).bool().is_err());
}
#[test]
fn block_positions() {
    for p in [
        BlockPosition {
            x: -33_554_432,
            y: -2048,
            z: 33_554_431,
        },
        BlockPosition {
            x: 42,
            y: -64,
            z: -12,
        },
    ] {
        assert_eq!(BlockPosition::unpack(p.pack().unwrap()), p);
    }
}
#[test]
fn every_release_has_catalog() {
    for v in Version::ALL {
        assert!(!v.catalog().is_empty());
        for name in v.releases() {
            assert_eq!(name.parse::<Version>().unwrap(), *v);
        }
        let mut seen = std::collections::HashSet::new();
        for p in v.catalog() {
            assert!(seen.insert((p.state, p.direction, p.id)));
        }
    }
    assert_eq!("26.2".parse::<Version>().unwrap().protocol(), 776);
    assert!(Version::from_protocol(777).is_err());
}
#[test]
fn uncompressed_roundtrip() {
    let c = FrameCodec::default();
    let p = RawPacket::new(99, vec![1, 2, 3, 4]);
    let encoded = c.encode(&p).unwrap();
    let mut input = encoded.as_slice();
    assert_eq!(c.decode(&mut input).unwrap(), Some(p));
    assert!(input.is_empty());
}
#[test]
fn every_fragment_is_transactional() {
    let c = FrameCodec::default();
    let bytes = c.encode(&RawPacket::new(42, vec![5; 300])).unwrap();
    for n in 0..bytes.len() {
        let original = &bytes[..n];
        let mut input = original;
        assert!(c.decode(&mut input).unwrap().is_none());
        assert_eq!(input, original);
    }
}
#[test]
fn rejects_oversized_lengths_before_payload() {
    let c = FrameCodec::new(Limits {
        max_frame: 16,
        ..Limits::default()
    });
    assert!(c.decode(&mut &[17][..]).is_err());
    assert!(c.decode(&mut &[128, 128, 128][..]).is_err());
}
#[test]
fn malformed_fuzz_style_no_panics() {
    let c = FrameCodec::new(Limits {
        max_frame: 4096,
        max_packet: 4096,
        ..Limits::default()
    });
    let mut seed = 3u64;
    for n in 0..4096 {
        let mut bytes = vec![0; n % 128];
        for b in &mut bytes {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *b = seed as u8;
        }
        let _ = c.decode(&mut bytes.as_slice());
        let _ = Reader::new(&bytes, Limits::default()).var_i32();
    }
}
#[cfg(feature = "compression")]
#[test]
fn compression_threshold_boundaries() {
    for threshold in [0, 1, 10, 256] {
        let mut c = FrameCodec::default();
        c.set_compression(Some(threshold)).unwrap();
        for n in [0, 1, 8, 9, 10, 255, 256, 4096] {
            let p = RawPacket::new(1, vec![42; n]);
            let bytes = c.encode(&p).unwrap();
            assert_eq!(c.decode(&mut bytes.as_slice()).unwrap(), Some(p));
        }
    }
}
#[cfg(feature = "compression")]
#[test]
fn rejects_compression_bombs_and_trailing_streams() {
    use std::io::Write;
    let mut c = FrameCodec::new(Limits {
        max_packet: 128,
        ..Limits::default()
    });
    c.set_compression(Some(1)).unwrap();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(&[1; 4096]).unwrap();
    let compressed = z.finish().unwrap();
    let mut w = Writer::new();
    w.var_i32(10);
    w.raw(&compressed);
    assert!(c.decode_body(w.as_slice()).is_err());
    let mut valid = c.encode(&RawPacket::new(1, vec![5; 20])).unwrap();
    valid.push(0);
    let mut input = valid.as_slice();
    assert!(c.decode(&mut input).unwrap().is_some());
    assert_eq!(input, &[0]);
    let mut body = Writer::new();
    body.var_i32(4096);
    body.raw(&compressed);
    assert!(c.decode_body(body.as_slice()).is_err());
}
#[cfg(feature = "compression")]
#[test]
fn compression_threshold_may_exceed_packet_budget() {
    let mut c = FrameCodec::new(Limits {
        max_packet: 32,
        ..Limits::default()
    });
    c.set_compression(Some(4096)).unwrap();
    let p = RawPacket::new(1, vec![2; 8]);
    let encoded = c.encode(&p).unwrap();
    assert_eq!(c.decode(&mut encoded.as_slice()).unwrap(), Some(p));
    c.set_compression(None).unwrap();
    assert_eq!(c.compression_threshold(), None);
}
#[test]
fn indexed_catalog_lookup_matches_every_generated_entry() {
    use rustwire_mc::version::{Direction, State};
    for &v in Version::ALL {
        for p in v.catalog() {
            assert_eq!(v.packet(p.state, p.direction, p.id), Some(p));
            assert_eq!(v.packet_id(p.state, p.direction, p.name).unwrap(), p.id);
        }
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
            State::Play,
        ] {
            for direction in [Direction::Clientbound, Direction::Serverbound] {
                let packets = v.packets(state, direction);
                assert!(packets.windows(2).all(|pair| pair[0].id < pair[1].id));
                assert!(packets
                    .iter()
                    .all(|p| p.state == state && p.direction == direction));
                assert!(v.packet(state, direction, -1).is_none());
                assert!(v.packet(state, direction, i32::MAX).is_none());
            }
        }
    }
}

#[test]
fn protocol_776_serverbound_tail_matches_official_registration() {
    // Independently counted from official client and Paper 26.2 GameProtocols,
    // not inferred by roundtripping the pinned third-party schema.
    use rustwire_mc::{
        packet::interact::{self, Hand, UseItem},
        version::{Direction, State},
    };
    let v = Version::V26_2;
    assert_eq!(v.packets(State::Play, Direction::Serverbound).len(), 69);
    for (id, name) in [
        (0x3e, "spectator_action"),
        (0x3f, "arm_animation"),
        (0x40, "spectate"),
        (0x41, "test_instance_block_action"),
        (0x42, "block_place"),
        (0x43, "use_item"),
        (0x44, "custom_click_action"),
    ] {
        assert_eq!(
            v.packet_id(State::Play, Direction::Serverbound, name)
                .unwrap(),
            id
        );
        assert_eq!(
            v.packet(State::Play, Direction::Serverbound, id)
                .unwrap()
                .name,
            name
        );
    }
    let packet = UseItem {
        hand: Hand::Off,
        sequence: 0,
        rotation: Some([0., -90.]),
    }
    .encode(v)
    .unwrap();
    assert_eq!(
        FrameCodec::default().encode(&packet).unwrap(),
        [0x0b, 0x43, 1, 0, 0, 0, 0, 0, 0xc2, 0xb4, 0, 0]
    );
    assert_eq!(
        interact::swing_arm(v, Hand::Off).unwrap(),
        RawPacket::new(0x3f, [1])
    );
    assert_eq!(
        Version::V26_1
            .packet_id(State::Play, Direction::Serverbound, "use_item")
            .unwrap(),
        0x43
    );
}
