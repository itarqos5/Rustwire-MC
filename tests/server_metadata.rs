use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, Tag},
    packet::{
        chat::{state::SignatureCache, ChatComponent, PreviousMessage},
        common::CommonPacket,
        server_metadata::*,
        typed::{ChatPacket, DecodedPacket},
    },
    version::{Direction, State},
    Error, Limits, Version,
};

struct Fixture<'a> {
    version: Version,
    state: State,
    direction: Direction,
    name: &'a str,
    case: &'a str,
    id: i32,
    bytes: Vec<u8>,
}
fn fixtures() -> Vec<Fixture<'static>> {
    include_str!("fixtures/server-metadata.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), 7);
            assert_eq!(f[6].len() % 2, 0);
            Fixture {
                version: Version::from_protocol(f[0].parse().unwrap()).unwrap(),
                state: match f[1] {
                    "configuration" => State::Configuration,
                    "play" => State::Play,
                    _ => panic!("unexpected fixture state"),
                },
                direction: match f[2] {
                    "toClient" => Direction::Clientbound,
                    "toServer" => Direction::Serverbound,
                    _ => panic!("unexpected fixture direction"),
                },
                name: f[3],
                case: f[4],
                id: f[5].parse().unwrap(),
                bytes: f[6]
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(), 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}
fn text(version: Version) -> ChatComponent {
    if version.protocol() < 765 {
        ChatComponent::Json("{\"text\":\"x\"}".into())
    } else {
        ChatComponent::Nbt(Nbt::anonymous(Tag::String("x".into())))
    }
}
fn ping_id(case: &str) -> i64 {
    match case {
        "positive" => 0x0102030405060708,
        "negative" => -1,
        "minimum" => i64::MIN,
        "maximum" => i64::MAX,
        "zero" => 0,
        _ => unreachable!(),
    }
}

// Expected bytes are independently written by a Python stdlib fixture encoder,
// never by the Rust codec being tested. Fields below independently assert meaning.
macro_rules! check_body {
    ($ty:ty, $f:expr, $expected:expr) => {{
        let f = $f;
        let expected: $ty = $expected;
        let limits = Limits::default();
        assert_eq!(
            <$ty>::decode(&f.bytes, f.version, limits).unwrap(),
            expected
        );
        assert_eq!(expected.encode(f.version, limits).unwrap(), f.bytes);
        let exact = Limits {
            max_packet: f.bytes.len(),
            ..limits
        };
        assert_eq!(expected.encode(f.version, exact).unwrap(), f.bytes);
        assert_eq!(<$ty>::decode(&f.bytes, f.version, exact).unwrap(), expected);
        let too_small = Limits {
            max_packet: f.bytes.len() - 1,
            ..limits
        };
        assert!(expected.encode(f.version, too_small).is_err());
        assert!(<$ty>::decode(&f.bytes, f.version, too_small).is_err());
        let mut stream = f.bytes.clone();
        stream.push(0xaa);
        let mut r = Reader::new(&stream, exact);
        assert_eq!(<$ty>::read(&mut r, f.version).unwrap(), expected);
        assert_eq!(r.remaining(), [0xaa]);
        assert!(<$ty>::decode(&stream, f.version, limits).is_err());
        let mut w = Writer::new();
        w.u8(0xbb);
        assert!(expected.write(&mut w, f.version, too_small).is_err());
        assert_eq!(w.as_slice(), [0xbb]);
        expected.write(&mut w, f.version, exact).unwrap();
        assert_eq!(&w.as_slice()[1..], &f.bytes);
        for end in 0..f.bytes.len() {
            assert!(
                <$ty>::decode(&f.bytes[..end], f.version, limits).is_err(),
                "{} {} {} prefix {end}",
                f.version.protocol(),
                f.name,
                f.case
            );
            let mut r = Reader::new(&f.bytes[..end], limits);
            assert!(<$ty>::read(&mut r, f.version).is_err());
            assert_eq!(r.position(), 0);
        }
        expected
    }};
}

#[test]
fn independent_fixtures_all_protocols_prefixes_trailing_and_transactional_budgets() {
    let rows = fixtures();
    assert_eq!(rows.len(), 310);
    for f in &rows {
        assert_eq!(
            f.version.packet_id(f.state, f.direction, f.name).unwrap(),
            f.id
        );
        let packet = match f.name {
            "server_data" => check_body!(
                ServerData,
                f,
                ServerData {
                    motd: text(f.version),
                    icon: match f.case {
                        "absent" => None,
                        "empty" => Some(vec![]),
                        "bytes" => Some(vec![0x89, 0, 0xff]),
                        _ => unreachable!(),
                    },
                    enforces_secure_chat: (f.version.protocol() < 766).then_some(true),
                }
            )
            .packet(f.version, Limits::default())
            .unwrap(),
            "custom_report_details" => check_body!(
                CustomReportDetails,
                f,
                CustomReportDetails {
                    details: if f.case == "empty" {
                        vec![]
                    } else {
                        vec![
                            ReportDetail {
                                key: "a".into(),
                                value: "x".into(),
                            },
                            ReportDetail {
                                key: "a".into(),
                                value: "y".into(),
                            },
                        ]
                    },
                }
            )
            .packet(f.version, f.state, Limits::default())
            .unwrap(),
            "chat_suggestions" => check_body!(
                ChatSuggestions,
                f,
                match f.case {
                    "add" => ChatSuggestions {
                        action: ChatSuggestionsAction::Add,
                        entries: vec!["".into(), "hi".into(), "😀".into()]
                    },
                    "remove" => ChatSuggestions {
                        action: ChatSuggestionsAction::Remove,
                        entries: vec![]
                    },
                    "set" => ChatSuggestions {
                        action: ChatSuggestionsAction::Set,
                        entries: vec!["/hello".into()]
                    },
                    _ => unreachable!(),
                }
            )
            .packet(f.version, Limits::default())
            .unwrap(),
            "hide_message" => check_body!(
                HideMessage,
                f,
                HideMessage {
                    signature: match f.case {
                        "signature" =>
                            PreviousMessage::Signature(Box::new(std::array::from_fn(|i| i as u8))),
                        "first" => PreviousMessage::Cached(0),
                        "last" => PreviousMessage::Cached(127),
                        "unresolved" => PreviousMessage::Cached(i32::MAX as u32 - 1),
                        _ => unreachable!(),
                    },
                }
            )
            .packet(f.version, Limits::default())
            .unwrap(),
            "ping_response" => check_body!(
                PingResponse,
                f,
                PingResponse {
                    id: ping_id(f.case)
                }
            )
            .packet(f.version, Limits::default())
            .unwrap(),
            "ping_request" => check_body!(
                PingRequest,
                f,
                PingRequest {
                    id: ping_id(f.case)
                }
            )
            .packet(f.version, Limits::default())
            .unwrap(),
            _ => unreachable!(),
        };
        assert_eq!(packet.id, f.id);
        assert_eq!(packet.data, f.bytes);
    }
}

#[test]
fn typed_state_and_direction_dispatch_for_every_fixture() {
    for f in fixtures() {
        let decoded =
            DecodedPacket::decode(f.state, f.name, &f.bytes, f.version, Limits::default()).unwrap();
        match f.name {
            "server_data" => assert!(matches!(decoded, Some(DecodedPacket::ServerData(_)))),
            "custom_report_details" => assert!(matches!(
                decoded,
                Some(DecodedPacket::Common(CommonPacket::CustomReportDetails(_)))
            )),
            "chat_suggestions" => assert!(matches!(
                decoded,
                Some(DecodedPacket::Chat(ChatPacket::Suggestions(_)))
            )),
            "hide_message" => assert!(matches!(
                decoded,
                Some(DecodedPacket::Chat(ChatPacket::Hide(_)))
            )),
            "ping_response" => assert!(matches!(decoded, Some(DecodedPacket::PingResponse(_)))),
            "ping_request" => assert!(decoded.is_none()), // outbound has no clientbound typed dispatch
            _ => unreachable!(),
        }
        for state in [State::Handshake, State::Status, State::Login] {
            assert!(
                DecodedPacket::decode(state, f.name, &f.bytes, f.version, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        if f.name != "custom_report_details" {
            assert!(DecodedPacket::decode(
                State::Configuration,
                f.name,
                &f.bytes,
                f.version,
                Limits::default()
            )
            .unwrap()
            .is_none());
        }
        if f.direction == Direction::Clientbound {
            assert!(
                DecodedPacket::decode(f.state, f.name, &[], f.version, Limits::default()).is_err()
            );
        }
    }
}

#[test]
fn exact_version_boundaries_and_representation_mismatch() {
    let limits = Limits::default();
    for &v in Version::ALL {
        let mut data = ServerData {
            motd: text(v),
            icon: None,
            enforces_secure_chat: (v.protocol() < 766).then_some(false),
        };
        let encoded = data.encode(v, limits).unwrap();
        assert_eq!(ServerData::decode(&encoded, v, limits).unwrap(), data);
        data.enforces_secure_chat = if v.protocol() < 766 {
            None
        } else {
            Some(false)
        };
        assert!(matches!(data.encode(v, limits), Err(Error::Invalid(_))));
        data.enforces_secure_chat = (v.protocol() < 766).then_some(false);
        data.motd = text(if v.protocol() < 765 {
            Version::V1_20_3
        } else {
            Version::V1_20
        });
        assert!(matches!(data.encode(v, limits), Err(Error::Invalid(_))));
        let report = CustomReportDetails { details: vec![] };
        assert_eq!(report.encode(v, limits).is_ok(), v.protocol() >= 767);
        assert_eq!(
            CustomReportDetails::decode(&[0], v, limits).is_ok(),
            v.protocol() >= 767
        );
        for state in [State::Configuration, State::Play] {
            assert_eq!(report.packet(v, state, limits).is_ok(), v.protocol() >= 767);
        }
        for state in [State::Handshake, State::Status, State::Login] {
            assert!(report.packet(v, state, limits).is_err());
        }
        assert_eq!(
            PingRequest { id: 0 }.packet(v, limits).is_ok(),
            v.protocol() >= 764
        );
        assert_eq!(
            PingResponse { id: 0 }.packet(v, limits).is_ok(),
            v.protocol() >= 764
        );
        assert_eq!(
            PingRequest::decode(&[0; 8], v, limits).is_ok(),
            v.protocol() >= 764
        );
        assert_eq!(
            PingResponse::decode(&[0; 8], v, limits).is_ok(),
            v.protocol() >= 764
        );
    }
}

#[test]
fn malformed_enums_booleans_lengths_utf8_and_varints() {
    let v = Version::V1_21;
    let limits = Limits::default();
    for bad in [
        vec![3, 0],
        vec![0xff, 0xff, 0xff, 0xff, 0x0f, 0],
        vec![0, 1, 1, 0xff],
        vec![0, 0xff, 0xff, 0xff, 0xff, 0x0f],
        vec![0, 0x80, 0x80, 0x80, 0x80, 0x10],
        vec![0, 0x80, 0x80, 0x80, 0x80, 0x80, 0],
    ] {
        assert!(ChatSuggestions::decode(&bad, v, limits).is_err());
    }
    for bad in [
        vec![0],
        vec![0xff, 0xff, 0xff, 0xff, 0x0f],
        vec![0x80, 0x80, 0x80, 0x80, 0x10],
    ] {
        assert!(HideMessage::decode(&bad, v, limits).is_err());
    }
    for bad in [
        vec![1],
        vec![1, 0],
        vec![1, 1, 0xff, 0],
        vec![0xff, 0xff, 0xff, 0xff, 0x0f],
        vec![33],
    ] {
        assert!(CustomReportDetails::decode(&bad, v, limits).is_err());
    }
    for &v in Version::ALL {
        let mut bytes = if v.protocol() < 765 {
            b"\x0c{\"text\":\"x\"}".to_vec()
        } else {
            vec![8, 0, 1, b'x']
        };
        bytes.push(2); // invalid icon boolean
        if v.protocol() < 766 {
            bytes.push(0);
        }
        assert!(ServerData::decode(&bytes, v, limits).is_err());
        if v.protocol() < 766 {
            *bytes.last_mut().unwrap() = 2;
            let n = bytes.len();
            bytes[n - 2] = 0;
            assert!(ServerData::decode(&bytes, v, limits).is_err());
        }
    }
    assert!(ServerData::decode(&[0, 0], v, limits).is_err()); // absent required NBT
    assert!(
        ServerData::decode(&[8, 0, 1, b'x', 1, 0xff, 0xff, 0xff, 0xff, 0x0f], v, limits).is_err()
    );
    assert!(ServerData::decode(&[8, 0, 1, b'x', 1, 9], v, limits).is_err());
}

#[test]
fn report_count_key_value_limits_utf16_and_duplicate_preservation() {
    let v = Version::V1_21;
    let limits = Limits::default();
    let mut value = CustomReportDetails {
        details: vec![
            ReportDetail {
                key: "😀".repeat(64),
                value: "v".repeat(4096)
            };
            32
        ],
    };
    let bytes = value.encode(v, limits).unwrap();
    assert_eq!(
        CustomReportDetails::decode(&bytes, v, limits).unwrap(),
        value
    );
    value.details.push(value.details[0].clone());
    assert!(value.encode(v, limits).is_err());
    value.details.pop();
    value.details[0].key.push('a');
    assert!(value.encode(v, limits).is_err());
    value.details[0].key.pop();
    value.details[0].value.push('v');
    assert!(value.encode(v, limits).is_err());
    // Independently malformed key and value strings beyond the wire maxima.
    let mut key = vec![1, 0x81, 1];
    key.extend([b'k'; 129]);
    key.push(0);
    assert!(CustomReportDetails::decode(&key, v, limits).is_err());
    let mut val = vec![1, 0, 0x81, 0x20];
    val.extend([b'v'; 4097]);
    assert!(CustomReportDetails::decode(&val, v, limits).is_err());
    let small = Limits {
        max_string_chars: 1,
        ..limits
    };
    assert!(CustomReportDetails::decode(&[1, 2, b'a', b'b', 0], v, small).is_err());
}

#[test]
fn collection_packet_and_nbt_budgets_are_enforced_before_unbounded_work() {
    let v = Version::V1_21;
    let limits = Limits::default();
    let suggestions = ChatSuggestions {
        action: ChatSuggestionsAction::Set,
        entries: vec!["abc".into(), "def".into()],
    };
    let bytes = [2, 2, 3, b'a', b'b', b'c', 3, b'd', b'e', b'f'];
    let report = CustomReportDetails {
        details: vec![ReportDetail {
            key: "abc".into(),
            value: "def".into(),
        }],
    };
    // Each string fits separately, but the complete body does not.
    let small = Limits {
        max_packet: 8,
        ..limits
    };
    assert!(suggestions.encode(v, small).is_err());
    assert!(ChatSuggestions::decode(&bytes, v, small).is_err());
    assert!(report.encode(v, small).is_err());
    assert!(
        CustomReportDetails::decode(&[1, 3, b'a', b'b', b'c', 3, b'd', b'e', b'f'], v, small)
            .is_err()
    );
    let collection = Limits {
        max_collection: 1,
        ..limits
    };
    assert!(suggestions.encode(v, collection).is_err());
    assert!(ChatSuggestions::decode(&bytes, v, collection).is_err());
    assert!(CustomReportDetails::decode(&[2, 0, 0, 0, 0], v, collection).is_err());
    let icon = ServerData {
        motd: text(v),
        icon: Some(vec![1, 2]),
        enforces_secure_chat: None,
    };
    assert!(icon.encode(v, collection).is_err());
    assert!(ServerData::decode(&[8, 0, 1, b'x', 1, 2, 1, 2], v, collection).is_err());
    let no_nodes = Limits {
        max_nbt_nodes: 0,
        ..limits
    };
    assert!(icon.encode(v, no_nodes).is_err());
    assert!(ServerData::decode(&[8, 0, 1, b'x', 0], v, no_nodes).is_err());
    let nested = ServerData {
        motd: ChatComponent::Nbt(Nbt::anonymous(Tag::Compound(vec![(
            "a".into(),
            Tag::Byte(1),
        )]))),
        icon: None,
        enforces_secure_chat: None,
    };
    let one_node = Limits {
        max_nbt_nodes: 1,
        ..limits
    };
    assert!(nested.encode(v, one_node).is_err());
    assert!(ServerData::decode(&[10, 1, 0, 1, b'a', 1, 0, 0], v, one_node).is_err());
    let no_depth = Limits {
        max_nbt_depth: 0,
        ..limits
    };
    assert!(nested.encode(v, no_depth).is_err());
    assert!(ServerData::decode(&[10, 1, 0, 1, b'a', 1, 0, 0], v, no_depth).is_err());
    // Impossible huge counts fail against remaining bytes before reserving a Vec.
    assert!(ChatSuggestions::decode(&[0, 0xff, 0xff, 0x3f], v, limits).is_err());
    assert!(CustomReportDetails::decode(&[32, 0, 0], v, limits).is_err());
}

#[test]
fn packed_signatures_are_bounded_but_do_not_imply_cache_presence_or_trust() {
    let v = Version::V1_21;
    let limits = Limits::default();
    for index in [i32::MAX as u32, u32::MAX] {
        assert!(HideMessage {
            signature: PreviousMessage::Cached(index)
        }
        .encode(v, limits)
        .is_err());
    }
    let first = HideMessage::decode(&[1], v, limits).unwrap();
    let mut cache = SignatureCache::default();
    assert!(cache
        .resolve(std::slice::from_ref(&first.signature))
        .is_err());
    cache.push(&[], Some(&[3; 256])).unwrap();
    assert_eq!(cache.resolve(&[first.signature]).unwrap(), vec![[3; 256]]);
    assert_eq!(cache.get(0).unwrap(), &[3; 256]);
    // An out-of-cache reference is retained by the envelope, then rejected by
    // explicit cache resolution. No automatic ignore/acknowledge action occurs.
    let out = HideMessage::decode(&[0x81, 1], v, limits).unwrap();
    assert_eq!(out.signature, PreviousMessage::Cached(128));
    assert!(cache.resolve(&[out.signature]).is_err());
    assert_eq!(cache.get(0).unwrap(), &[3; 256]);
}

#[test]
fn overlong_varints_follow_shared_reader_canonicalization() {
    let v = Version::V1_21;
    let limits = Limits::default();
    let value = HideMessage::decode(&[0x81, 0], v, limits).unwrap();
    assert_eq!(value.encode(v, limits).unwrap(), [1]);
    let value = ChatSuggestions::decode(&[0x82, 0, 0x80, 0], v, limits).unwrap();
    assert_eq!(value.encode(v, limits).unwrap(), [2, 0]);
}
