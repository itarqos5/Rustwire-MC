use rustwire_mc::{
    codec::Writer,
    packet::{
        self,
        custom_payload::{CustomPayload, CustomPayloadRef},
        typed::DecodedPacket,
    },
    version::{Direction, State},
    Limits, Version,
};
fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn value(case: &str) -> CustomPayload {
    let (channel, data) = match case {
        "empty" => ("rustwire:test".into(), vec![]),
        "binary" => ("rustwire:opaque".into(), vec![0, 255, 128, 10]),
        "implicit_namespace" => ("brand".into(), vec![0, 128]),
        "empty_namespace" => (":x".into(), vec![1, 2]),
        "empty_path" => ("minecraft:".into(), vec![]),
        "long_prefix" => ("x".repeat(128), (0..128).collect()),
        _ => panic!("fixture case"),
    };
    CustomPayload { channel, data }
}
#[test]
fn independently_encoded_full_matrix_and_zero_copy_views() {
    let limits = Limits::default();
    let mut count = 0;
    for line in include_str!("fixtures/custom-payload.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 6);
        let version = Version::from_protocol(f[0].parse().unwrap()).unwrap();
        let state = if f[1] == "play" {
            State::Play
        } else {
            State::Configuration
        };
        let direction = if f[2] == "toClient" {
            Direction::Clientbound
        } else {
            Direction::Serverbound
        };
        let id: i32 = f[4].parse().unwrap();
        let body = bytes(f[5]);
        let expected = value(f[3]);
        let view = CustomPayloadRef::decode(&body, version, state, direction, limits).unwrap();
        assert_eq!(view.to_owned(), expected);
        assert_eq!(
            view.data.as_ptr(),
            body[body.len() - expected.data.len()..].as_ptr()
        );
        let channel_offset = if expected.channel.len() < 128 { 1 } else { 2 };
        assert_eq!(view.channel.as_ptr(), body[channel_offset..].as_ptr());
        assert_eq!(
            view.encode(version, state, direction, limits).unwrap(),
            body
        );
        assert_eq!(
            CustomPayload::decode(&body, version, state, direction, limits).unwrap(),
            expected
        );
        assert_eq!(
            expected
                .packet(version, state, direction, limits)
                .unwrap()
                .id,
            id
        );
        assert_eq!(
            expected
                .packet(version, state, direction, limits)
                .unwrap()
                .data,
            body
        );
        if direction == Direction::Clientbound {
            assert!(
                matches!(DecodedPacket::decode(state, "custom_payload", &body, version, limits).unwrap(), Some(DecodedPacket::CustomPayload(p)) if p == expected)
            );
        } else {
            assert_eq!(
                packet::custom_payload(version, state, &expected.channel, &expected.data)
                    .unwrap()
                    .data,
                body
            );
        }
        // There is no inner payload length. Only a truncated channel is invalid;
        // every remaining prefix is a valid shorter opaque payload.
        let header = body.len() - expected.data.len();
        for end in 0..header {
            assert!(
                CustomPayloadRef::decode(&body[..end], version, state, direction, limits).is_err()
            );
        }
        for end in header..body.len() {
            assert_eq!(
                CustomPayloadRef::decode(&body[..end], version, state, direction, limits)
                    .unwrap()
                    .data,
                &expected.data[..end - header]
            );
        }
        let mut extended = body.clone();
        extended.push(255);
        assert_eq!(
            CustomPayload::decode(&extended, version, state, direction, limits)
                .unwrap()
                .data
                .last(),
            Some(&255)
        );
        count += 1;
    }
    assert_eq!(count, 324);
}
#[test]
fn exact_direction_caps_and_whole_body_limits() {
    let v = Version::V26_2;
    let state = State::Play;
    for (direction, size) in [
        (Direction::Clientbound, CustomPayload::MAX_CLIENTBOUND_BYTES),
        (Direction::Serverbound, CustomPayload::MAX_SERVERBOUND_BYTES),
    ] {
        let mut p = CustomPayload {
            channel: "x".into(),
            data: vec![0xff; size],
        };
        let exact = Limits {
            max_packet: size + 2,
            ..Limits::default()
        };
        let body = p.encode(v, state, direction, exact).unwrap();
        assert_eq!(body.len(), size + 2);
        assert_eq!(
            CustomPayloadRef::decode(&body, v, state, direction, exact)
                .unwrap()
                .data
                .len(),
            size
        );
        let tight = Limits {
            max_packet: size + 1,
            ..exact
        };
        assert!(p.encode(v, state, direction, tight).is_err());
        assert!(CustomPayloadRef::decode(&body, v, state, direction, tight).is_err());
        p.data.push(0);
        assert!(p.encode(v, state, direction, Limits::default()).is_err());
        let mut extra = body;
        extra.push(0);
        assert!(CustomPayloadRef::decode(&extra, v, state, direction, Limits::default()).is_err());
        let mut w = Writer::new();
        w.u8(7);
        assert!(p
            .write(&mut w, v, state, direction, Limits::default())
            .is_err());
        assert_eq!(w.as_slice(), &[7]);
    }
}
#[test]
fn identifier_version_boundaries_and_string_limits() {
    let state = State::Play;
    let dir = Direction::Clientbound;
    let limits = Limits::default();
    for &v in Version::ALL {
        for channel in ["", ":", "minecraft:", "brand", ":brand", "a:b/c_.-9"] {
            let p = CustomPayloadRef { channel, data: &[] };
            let body = p.encode(v, state, dir, limits).unwrap();
            assert_eq!(
                CustomPayloadRef::decode(&body, v, state, dir, limits).unwrap(),
                p
            );
        }
        for channel in ["UPPER:x", "a:b:c", "a:b c", "a/namespace:x", "a:é"] {
            let p = CustomPayloadRef { channel, data: &[] };
            assert!(p.encode(v, state, dir, limits).is_err());
            let mut w = Writer::new();
            w.string(channel, 32767).unwrap();
            assert!(CustomPayloadRef::decode(w.as_slice(), v, state, dir, limits).is_err());
        }
        let p = CustomPayloadRef {
            channel: "..:x",
            data: &[],
        };
        assert_eq!(p.encode(v, state, dir, limits).is_ok(), v.protocol() < 775);
        assert_eq!(
            CustomPayloadRef::decode(b"\x04..:x", v, state, dir, limits).is_ok(),
            v.protocol() < 775
        );
    }
    let channel = "x".repeat(32767);
    let p = CustomPayloadRef {
        channel: &channel,
        data: &[],
    };
    let body = p.encode(Version::V26_2, state, dir, limits).unwrap();
    assert_eq!(
        CustomPayloadRef::decode(&body, Version::V26_2, state, dir, limits).unwrap(),
        p
    );
    let tight = Limits {
        max_string_chars: 32766,
        ..limits
    };
    assert!(p.encode(Version::V26_2, state, dir, tight).is_err());
    assert!(CustomPayloadRef::decode(&body, Version::V26_2, state, dir, tight).is_err());
    assert!(CustomPayloadRef {
        channel: &"x".repeat(32768),
        data: &[]
    }
    .encode(Version::V26_2, state, dir, limits)
    .is_err());
}
#[test]
fn malformed_header_and_state_gates() {
    let p = CustomPayloadRef {
        channel: "x",
        data: &[],
    };
    let limits = Limits::default();
    for &v in Version::ALL {
        for state in [State::Handshake, State::Status, State::Login] {
            for dir in [Direction::Clientbound, Direction::Serverbound] {
                assert!(p.encode(v, state, dir, limits).is_err());
                assert!(p.packet(v, state, dir, limits).is_err());
                assert!(CustomPayloadRef::decode(b"\x01x", v, state, dir, limits).is_err());
            }
        }
    }
    assert!(p
        .encode(
            Version::V1_20,
            State::Configuration,
            Direction::Clientbound,
            limits
        )
        .is_err());
    assert!(CustomPayloadRef::decode(
        b"\x01x",
        Version::V1_20,
        State::Configuration,
        Direction::Clientbound,
        limits
    )
    .is_err());
    for body in [
        &b"\xff\xff\xff\xff\x0f"[..],
        &b"\x80\x80\x80\x80\x80\x00"[..],
        &b"\x01\xff"[..],
        &b"\x02x"[..],
    ] {
        assert!(CustomPayloadRef::decode(
            body,
            Version::V26_2,
            State::Play,
            Direction::Clientbound,
            limits
        )
        .is_err());
    }
    // Canonical encoding of a permitted overlong string-length VarInt.
    let parsed = CustomPayloadRef::decode(
        b"\x81\x00x",
        Version::V26_2,
        State::Play,
        Direction::Clientbound,
        limits,
    )
    .unwrap();
    assert_eq!(
        parsed
            .encode(Version::V26_2, State::Play, Direction::Clientbound, limits)
            .unwrap(),
        b"\x01x"
    );
}

#[test]
fn output_budgets_precede_channel_syntax_scans() {
    use rustwire_mc::Error;
    let value = CustomPayloadRef {
        channel: "UPPER",
        data: &[],
    };
    let v = Version::V26_2;
    let state = State::Play;
    for limits in [
        Limits {
            max_packet: 0,
            ..Limits::default()
        },
        Limits {
            max_string_chars: 1,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            value.encode(v, state, Direction::Clientbound, limits),
            Err(Error::Limit(_))
        ));
        let mut w = Writer::new();
        w.u8(7);
        assert!(matches!(
            value.write(&mut w, v, state, Direction::Clientbound, limits),
            Err(Error::Limit(_))
        ));
        assert_eq!(w.as_slice(), &[7]);
    }
    let payload = vec![0; CustomPayload::MAX_SERVERBOUND_BYTES + 1];
    let oversized = CustomPayloadRef {
        channel: "UPPER",
        data: &payload,
    };
    assert!(matches!(
        oversized.encode(v, state, Direction::Serverbound, Limits::default()),
        Err(Error::Limit(_))
    ));
    assert!(matches!(
        value.encode(v, state, Direction::Clientbound, Limits::default()),
        Err(Error::Invalid(_))
    ));
}
