use rustwire_mc::{
    packet::{
        chat::{signed::*, state::MessageSignature, LastSeenUpdate},
        player::ChatSession,
    },
    Error, Limits, Version,
};
fn unhex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
fn fixture(name: &str) -> Vec<u8> {
    let line = include_str!("fixtures/chat-signing.txt")
        .lines()
        .find(|l| l.starts_with(&format!("{name}=")))
        .unwrap();
    unhex(line.split_once('=').unwrap().1)
}
fn data<'a>(
    content: &'a str,
    timestamp_ms: i64,
    last_seen: &'a [MessageSignature],
) -> SigningData<'a> {
    SigningData {
        sender: unhex("00112233445566778899aabbccddeeff")
            .try_into()
            .unwrap(),
        session_id: unhex("ffeeddccbbaa99887766554433221100")
            .try_into()
            .unwrap(),
        index: 7,
        content,
        timestamp_ms,
        salt: -17,
        last_seen,
    }
}
fn ack(v: Version) -> LastSeenUpdate {
    LastSeenUpdate {
        offset: 3,
        acknowledged: [1, 0, 0],
        checksum: (v.protocol() >= 770).then_some(33),
    }
}
#[test]
fn canonical_input_matches_all14_official_release_oracles() {
    assert_eq!(
        data("", 0, &[]).encode(Limits::default()).unwrap(),
        fixture("empty")
    );
    assert_eq!(
        data("café 😀", -1, &[[0xa5; 256]])
            .encode(Limits::default())
            .unwrap(),
        fixture("unicode_negative_millis")
    );
    assert_eq!(
        data("test", 1999, &[]).encode(Limits::default()).unwrap(),
        fixture("positive_millis")
    );
    assert_eq!(
        data("test", 1000, &[]).encode(Limits::default()).unwrap(),
        fixture("positive_millis")
    );
}
#[test]
fn signing_data_limits_precede_external_provider() {
    let mut d = data("x", 0, &[]);
    d.index = u32::MAX;
    assert!(d
        .sign_with(Limits::default(), |_| panic!("must not reach provider"))
        .is_err());
    assert!(data("x", 0, &[[0; 256]; 21])
        .encode(Limits::default())
        .is_err());
    assert!(data("x", 0, &[])
        .encode(Limits {
            max_packet: 64,
            ..Limits::default()
        })
        .is_err());
    assert_eq!(
        data("x", 0, &[])
            .encode(Limits {
                max_packet: 65,
                ..Limits::default()
            })
            .unwrap()
            .len(),
        65
    );
    assert!(data("😀", 0, &[])
        .encode(Limits {
            max_string_chars: 1,
            ..Limits::default()
        })
        .is_err());
    assert!(data("", 0, &[[0; 256]])
        .encode(Limits {
            max_collection: 0,
            ..Limits::default()
        })
        .is_err());
    assert!(data(&"x".repeat(32768), 0, &[])
        .encode(Limits::default())
        .is_err());
}
#[test]
fn provider_receives_canonical_input_and_errors_propagate_without_state() {
    let d = data("", 0, &[]);
    let mut called = 0;
    let result = d
        .sign_with(Limits::default(), |bytes| {
            called += 1;
            assert_eq!(bytes, fixture("empty"));
            Ok([0x71; 256])
        })
        .unwrap();
    assert_eq!(called, 1);
    assert_eq!(result, [0x71; 256]);
    assert!(matches!(
        d.sign_with(Limits::default(), |_| Err(Error::Auth(
            "test provider failure"
        ))),
        Err(Error::Auth(_))
    ));
    assert_eq!(d.index, 7);
}
#[test]
fn signed_message_golden_bodies_and_ids_all_families() {
    let ids = [5, 5, 5, 6, 6, 7, 7, 7, 8, 8, 8, 8, 9, 9];
    for (i, &v) in Version::ALL.iter().enumerate() {
        let p = SignedChatMessage {
            message: "x".into(),
            timestamp_ms: -1,
            salt: 2,
            signature: Box::new([0xa5; 256]),
            last_seen: ack(v),
        };
        let mut bytes = vec![1, b'x'];
        bytes.extend((-1i64).to_be_bytes());
        bytes.extend(2i64.to_be_bytes());
        bytes.push(1);
        bytes.extend([0xa5; 256]);
        bytes.extend([3, 1, 0, 0]);
        if v.protocol() >= 770 {
            bytes.push(33)
        };
        let encoded = p.encode(v, Limits::default()).unwrap();
        assert_eq!(encoded.id, ids[i]);
        assert_eq!(encoded.data, bytes);
        assert!(p
            .encode(
                v,
                Limits {
                    max_packet: bytes.len() - 1,
                    ..Limits::default()
                }
            )
            .is_err());
        assert_eq!(
            p.encode(
                v,
                Limits {
                    max_packet: bytes.len(),
                    ..Limits::default()
                }
            )
            .unwrap()
            .data,
            bytes
        );
    }
}
#[test]
fn signed_command_boundary_and_argument_wire_sizes() {
    let ids = [4, 4, 4, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 8];
    for (i, &v) in Version::ALL.iter().enumerate() {
        let p = SignedChatCommand {
            command: "say x".into(),
            timestamp_ms: 0,
            salt: 0,
            arguments: vec![ArgumentSignature {
                name: "message".into(),
                signature: Box::new([0x77; 256]),
            }],
            last_seen: ack(v),
        };
        let mut bytes = vec![5, b's', b'a', b'y', b' ', b'x'];
        bytes.extend([0; 16]);
        bytes.extend([1, 7]);
        bytes.extend(b"message");
        bytes.extend([0x77; 256]);
        bytes.extend([3, 1, 0, 0]);
        if v.protocol() >= 770 {
            bytes.push(33)
        };
        let encoded = p.encode(v, Limits::default()).unwrap();
        assert_eq!(encoded.id, ids[i]);
        assert_eq!(encoded.data, bytes);
        assert!(p
            .encode(
                v,
                Limits {
                    max_packet: bytes.len() - 1,
                    ..Limits::default()
                }
            )
            .is_err());
        assert_eq!(
            p.encode(
                v,
                Limits {
                    max_packet: bytes.len(),
                    ..Limits::default()
                }
            )
            .unwrap()
            .data,
            bytes
        );
    }
}
#[test]
fn command_and_acknowledgement_invalid_inputs_fail_closed() {
    let v = Version::V1_21_5;
    let mut p = SignedChatCommand {
        command: "say x".into(),
        timestamp_ms: 0,
        salt: 0,
        arguments: vec![],
        last_seen: ack(v),
    };
    p.command = "/say x".into();
    assert!(p.encode(v, Limits::default()).is_err());
    p.command = "say x".into();
    p.arguments = vec![
        ArgumentSignature {
            name: "x".into(),
            signature: Box::new([0; 256])
        };
        2
    ];
    assert!(p.encode(v, Limits::default()).is_err());
    p.arguments.truncate(1);
    p.arguments[0].name = "a".repeat(17);
    assert!(p.encode(v, Limits::default()).is_err());
    p.arguments = (0..9)
        .map(|i| ArgumentSignature {
            name: i.to_string(),
            signature: Box::new([0; 256]),
        })
        .collect();
    assert!(p.encode(v, Limits::default()).is_err());
    p.arguments.clear();
    p.last_seen.acknowledged[2] = 16;
    assert!(p.encode(v, Limits::default()).is_err());
    p.last_seen = ack(v);
    p.last_seen.checksum = None;
    assert!(p.encode(v, Limits::default()).is_err());
    p.last_seen = ack(v);
    assert!(p.encode(Version::V1_20, Limits::default()).is_err());
    let message = SignedChatMessage {
        message: "a".repeat(257),
        timestamp_ms: 0,
        salt: 0,
        signature: Box::new([0; 256]),
        last_seen: ack(v),
    };
    assert!(message.encode(v, Limits::default()).is_err());
}
#[test]
fn session_envelope_golden_limits_and_all_packet_ids() {
    let ids = [6, 6, 6, 7, 7, 8, 8, 8, 9, 9, 9, 9, 10, 10];
    let s = ChatSession {
        session_id: [0x11; 16],
        expires_at: 123,
        public_key: vec![0x22; 3],
        key_signature: vec![0x33; 2],
    };
    let mut bytes = vec![0x11; 16];
    bytes.extend(123i64.to_be_bytes());
    bytes.extend([3, 0x22, 0x22, 0x22, 2, 0x33, 0x33]);
    for (i, &v) in Version::ALL.iter().enumerate() {
        let p = session_update(v, &s, Limits::default()).unwrap();
        assert_eq!(p.id, ids[i]);
        assert_eq!(p.data, bytes);
        assert!(session_update(
            v,
            &s,
            Limits {
                max_packet: bytes.len() - 1,
                ..Limits::default()
            }
        )
        .is_err());
        assert_eq!(
            session_update(
                v,
                &s,
                Limits {
                    max_packet: bytes.len(),
                    ..Limits::default()
                }
            )
            .unwrap()
            .data,
            bytes
        );
    }
    let mut invalid = s.clone();
    invalid.public_key = vec![0; 513];
    assert!(session_update(Version::V1_20, &invalid, Limits::default()).is_err());
    invalid.public_key.clear();
    assert!(session_update(Version::V1_20, &invalid, Limits::default()).is_err());
    invalid = s;
    invalid.key_signature = vec![0; 4097];
    assert!(session_update(Version::V1_20, &invalid, Limits::default()).is_err());
}
