use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, Tag},
    packet::{dialog::*, entity_metadata::holders::RegistryHolder, typed::DecodedPacket},
    version::{Direction, State},
    Error, Limits, Version,
};

struct Fixture<'a> {
    v: Version,
    state: State,
    direction: Direction,
    name: &'a str,
    case: &'a str,
    id: i32,
    body: Vec<u8>,
}
fn fixtures() -> Vec<Fixture<'static>> {
    include_str!("fixtures/dialog.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            assert_eq!(f.len(), 7);
            assert_eq!(f[6].len() % 2, 0);
            Fixture {
                v: Version::from_protocol(f[0].parse().unwrap()).unwrap(),
                state: match f[1] {
                    "configuration" => State::Configuration,
                    "play" => State::Play,
                    _ => unreachable!(),
                },
                direction: match f[2] {
                    "toClient" => Direction::Clientbound,
                    "toServer" => Direction::Serverbound,
                    _ => unreachable!(),
                },
                name: f[3],
                case: f[4],
                id: f[5].parse().unwrap(),
                body: f[6]
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}
fn empty() -> Nbt {
    Nbt::anonymous(Tag::Compound(vec![]))
}
fn opaque() -> Nbt {
    Nbt::anonymous(Tag::Compound(vec![(
        "type".into(),
        Tag::String("x".into()),
    )]))
}

// Wire goldens were encoded by the independent Python stdlib fixture builder.
macro_rules! body_checks {
    ($ty:ty, $f:expr, $value:expr) => {{
        let f = $f;
        let value: $ty = $value;
        let limits = Limits::default();
        assert_eq!(<$ty>::decode(&f.body, f.v, f.state, limits).unwrap(), value);
        assert_eq!(value.encode(f.v, f.state, limits).unwrap(), f.body);
        let packet = value.packet(f.v, f.state, limits).unwrap();
        assert_eq!((packet.id, packet.data), (f.id, f.body.clone()));
        let exact = Limits {
            max_packet: f.body.len(),
            ..limits
        };
        assert_eq!(value.encode(f.v, f.state, exact).unwrap(), f.body);
        assert_eq!(<$ty>::decode(&f.body, f.v, f.state, exact).unwrap(), value);
        let mut stream = vec![0xff];
        stream.extend_from_slice(&f.body);
        stream.push(0xaa);
        let mut r = Reader::new(&stream, exact);
        r.u8().unwrap();
        assert_eq!(<$ty>::read(&mut r, f.v, f.state).unwrap(), value);
        assert_eq!(r.remaining(), [0xaa]);
        assert!(<$ty>::decode(&stream[1..], f.v, f.state, limits).is_err());
        let mut w = Writer::new();
        w.u8(0xbb);
        value.write(&mut w, f.v, f.state, exact).unwrap();
        assert_eq!(&w.as_slice()[1..], f.body);
        if !f.body.is_empty() {
            let smaller = Limits {
                max_packet: f.body.len() - 1,
                ..limits
            };
            assert!(value.encode(f.v, f.state, smaller).is_err());
            assert!(<$ty>::decode(&f.body, f.v, f.state, smaller).is_err());
            let before = w.as_slice().to_vec();
            assert!(value.write(&mut w, f.v, f.state, smaller).is_err());
            assert_eq!(w.as_slice(), before);
        }
        for end in 0..f.body.len() {
            assert!(
                <$ty>::decode(&f.body[..end], f.v, f.state, limits).is_err(),
                "{} {} {} {end}",
                f.v.protocol(),
                f.name,
                f.case
            );
            let truncated = [&[0xff], &f.body[..end]].concat();
            let mut r = Reader::new(&truncated, limits);
            r.u8().unwrap();
            assert!(<$ty>::read(&mut r, f.v, f.state).is_err());
            assert_eq!(r.position(), 1);
        }
    }};
}

#[test]
fn independent_fixtures_all_states_versions_prefixes_trailing_and_transactional_budgets() {
    let rows = fixtures();
    assert_eq!(rows.len(), 102);
    for f in &rows {
        assert_eq!(f.v.packet_id(f.state, f.direction, f.name).unwrap(), f.id);
        match f.name {
            "clear_dialog" => body_checks!(ClearDialog, f, ClearDialog),
            "show_dialog" => body_checks!(
                ShowDialog,
                f,
                ShowDialog {
                    dialog: match f.case {
                        "empty_compound" => RegistryHolder::Inline(empty()),
                        "opaque" => RegistryHolder::Inline(opaque()),
                        "first_reference" => RegistryHolder::RegistryId(0),
                        "reference_127" => RegistryHolder::RegistryId(127),
                        "maximum_reference" => RegistryHolder::RegistryId(i32::MAX - 1),
                        _ => unreachable!(),
                    }
                }
            ),
            "custom_click_action" => body_checks!(
                CustomClickAction,
                f,
                CustomClickAction {
                    id: "x:y".into(),
                    payload: match f.case {
                        "absent" => None,
                        "empty_compound" => Some(empty()),
                        "byte" => Some(Nbt::anonymous(Tag::Byte(127))),
                        "string" => Some(Nbt::anonymous(Tag::String("hi".into()))),
                        _ => unreachable!(),
                    }
                }
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn typed_clientbound_dispatch_is_state_explicit_and_excludes_custom_clicks() {
    for f in fixtures() {
        let decoded =
            DecodedPacket::decode(f.state, f.name, &f.body, f.v, Limits::default()).unwrap();
        match f.name {
            "clear_dialog" => assert!(matches!(
                decoded,
                Some(DecodedPacket::Dialog(DialogPacket::Clear(_)))
            )),
            "show_dialog" => assert!(matches!(
                decoded,
                Some(DecodedPacket::Dialog(DialogPacket::Show(_)))
            )),
            "custom_click_action" => assert!(decoded.is_none()),
            _ => unreachable!(),
        }
        for state in [State::Handshake, State::Status, State::Login] {
            assert!(
                DecodedPacket::decode(state, f.name, &f.body, f.v, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
    }
    let limits = Limits::default();
    let v = Version::V1_21_6;
    // Distinct layouts deliberately fail if they are routed through the other state.
    assert!(ShowDialog::decode(&[10, 0], v, State::Play, limits).is_err());
    assert!(ShowDialog::decode(&[0, 10, 0], v, State::Configuration, limits).is_err());
    let value = ShowDialog {
        dialog: RegistryHolder::Inline(empty()),
    };
    assert_eq!(
        value.encode(v, State::Configuration, limits).unwrap(),
        [10, 0]
    );
    assert_eq!(value.encode(v, State::Play, limits).unwrap(), [0, 10, 0]);
    assert!(ShowDialog {
        dialog: RegistryHolder::RegistryId(0)
    }
    .encode(v, State::Configuration, limits)
    .is_err());
}

#[test]
fn exact_version_and_state_gates_cover_all_entry_points() {
    let limits = Limits::default();
    for &v in Version::ALL {
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
            State::Play,
        ] {
            let supported =
                v.protocol() >= 771 && matches!(state, State::Configuration | State::Play);
            let clear = ClearDialog;
            assert_eq!(clear.encode(v, state, limits).is_ok(), supported);
            assert_eq!(clear.packet(v, state, limits).is_ok(), supported);
            assert_eq!(
                ClearDialog::decode(&[], v, state, limits).is_ok(),
                supported
            );
            let show = ShowDialog {
                dialog: RegistryHolder::Inline(empty()),
            };
            assert_eq!(show.encode(v, state, limits).is_ok(), supported);
            assert_eq!(show.packet(v, state, limits).is_ok(), supported);
            let body: &[u8] = if state == State::Play {
                &[0, 10, 0]
            } else {
                &[10, 0]
            };
            assert_eq!(
                ShowDialog::decode(body, v, state, limits).is_ok(),
                supported
            );
            let click = CustomClickAction {
                id: "a".into(),
                payload: None,
            };
            assert_eq!(click.encode(v, state, limits).is_ok(), supported);
            assert_eq!(click.packet(v, state, limits).is_ok(), supported);
            assert_eq!(
                CustomClickAction::decode(&[1, b'a', 1, 0], v, state, limits).is_ok(),
                supported
            );
        }
    }
}

#[test]
fn malformed_required_nbt_holders_and_empty_bodies_are_rejected() {
    let v = Version::V1_21_6;
    let l = Limits::default();
    assert!(ClearDialog::decode(&[0], v, State::Play, l).is_err());
    for bytes in [vec![], vec![0], vec![13], vec![10, 1, 0, 1, b'a']] {
        assert!(ShowDialog::decode(&bytes, v, State::Configuration, l).is_err());
    }
    for bytes in [
        vec![],
        vec![0],
        vec![0, 0],
        vec![0, 13],
        vec![255, 255, 255, 255, 15],
        vec![128, 128, 128, 128, 16],
        vec![128; 6],
    ] {
        assert!(ShowDialog::decode(&bytes, v, State::Play, l).is_err());
    }
    for id in [-1, i32::MIN, i32::MAX] {
        assert!(ShowDialog {
            dialog: RegistryHolder::RegistryId(id)
        }
        .encode(v, State::Play, l)
        .is_err());
    }
    let value = ShowDialog::decode(&[0x81, 0], v, State::Play, l).unwrap();
    assert_eq!(value.encode(v, State::Play, l).unwrap(), [1]);
}

#[test]
fn custom_click_absence_requires_length_framed_end_without_boolean() {
    let v = Version::V1_21_6;
    let l = Limits::default();
    for state in [State::Configuration, State::Play] {
        let value = CustomClickAction::decode(&[1, b'a', 1, 0], v, state, l).unwrap();
        assert_eq!(value.payload, None);
        assert_eq!(value.encode(v, state, l).unwrap(), [1, b'a', 1, 0]);
        // Zero length, stale schema boolean option, missing sentinel, bad tag,
        // malformed length and trailing bytes inside or after the sub-buffer.
        for bad in [
            vec![1, b'a', 0],
            vec![1, b'a', 1],
            vec![1, b'a', 1, 10, 0],
            vec![1, b'a', 1, 13],
            vec![1, b'a', 2, 0, 0],
            vec![1, b'a', 1, 0, 0],
            vec![1, b'a', 255, 255, 255, 255, 15],
            vec![1, b'a', 128, 128, 128, 128, 16],
        ] {
            assert!(
                CustomClickAction::decode(&bad, v, state, l).is_err(),
                "{bad:?}"
            );
        }
        let byte = CustomClickAction::decode(&[1, b'a', 2, 1, 0], v, state, l).unwrap();
        assert_eq!(byte.payload, Some(Nbt::anonymous(Tag::Byte(0))));
        let noncanonical = CustomClickAction::decode(&[1, b'a', 0x81, 0, 0], v, state, l).unwrap();
        assert_eq!(noncanonical.encode(v, state, l).unwrap(), [1, b'a', 1, 0]);
    }
}

#[test]
fn custom_click_exact_payload_cap_and_whole_packet_budget() {
    let v = Version::V26_2;
    let l = Limits::default();
    // Root ByteArray header is five bytes; 65,531 elements reach exactly 65,536.
    let value = CustomClickAction {
        id: "a".into(),
        payload: Some(Nbt::anonymous(Tag::ByteArray(vec![7; 65531]))),
    };
    let mut wire = vec![1, b'a', 0x80, 0x80, 4, 7, 0, 0, 0xff, 0xfb];
    wire.extend(vec![7; 65531]);
    for state in [State::Configuration, State::Play] {
        assert_eq!(value.encode(v, state, l).unwrap(), wire);
        assert_eq!(
            CustomClickAction::decode(&wire, v, state, l).unwrap(),
            value
        );
        let exact = Limits {
            max_packet: wire.len(),
            ..l
        };
        assert_eq!(value.encode(v, state, exact).unwrap(), wire);
        let small = Limits {
            max_packet: wire.len() - 1,
            ..l
        };
        assert!(value.encode(v, state, small).is_err());
        assert!(CustomClickAction::decode(&wire, v, state, small).is_err());
        let too_large = CustomClickAction {
            id: "a".into(),
            payload: Some(Nbt::anonymous(Tag::ByteArray(vec![7; 65532]))),
        };
        assert!(too_large.encode(v, state, l).is_err());
        // Cap is checked before requesting or allocating the claimed body.
        assert!(matches!(
            CustomClickAction::decode(&[1, b'a', 0x81, 0x80, 4], v, state, l),
            Err(Error::Limit(_))
        ));
    }
}

fn nested_compounds(count: usize, empty_leaf: bool) -> Nbt {
    let mut tag = if empty_leaf {
        Tag::Compound(vec![])
    } else {
        Tag::Byte(1)
    };
    for _ in 0..count {
        tag = Tag::Compound(vec![("a".into(), tag)]);
    }
    Nbt::anonymous(tag)
}
fn nested_wire(count: usize, empty_leaf: bool) -> Vec<u8> {
    let mut bytes = vec![10];
    for _ in 1..count {
        bytes.extend([10, 0, 1, b'a']);
    }
    if empty_leaf {
        bytes.extend([10, 0, 1, b'a', 0]);
    } else {
        bytes.extend([1, 0, 1, b'a', 1]);
    }
    bytes.extend(vec![0; count]);
    bytes
}

#[test]
fn payload_depth_is_conservative_root_zero_policy_not_jvm_equivalence() {
    let v = Version::V1_21_6;
    let l = Limits::default();
    assert_eq!(CustomClickAction::MAX_PAYLOAD_DEPTH, 15);
    for empty_leaf in [false, true] {
        let allowed = CustomClickAction {
            id: "a".into(),
            payload: Some(nested_compounds(15, empty_leaf)),
        };
        let mut wire = vec![1, b'a'];
        let payload = nested_wire(15, empty_leaf);
        wire.push(payload.len() as u8);
        wire.extend(payload);
        assert_eq!(allowed.encode(v, State::Play, l).unwrap(), wire);
        assert_eq!(
            CustomClickAction::decode(&wire, v, State::Play, l).unwrap(),
            allowed
        );
        let rejected = CustomClickAction {
            id: "a".into(),
            payload: Some(nested_compounds(16, empty_leaf)),
        };
        assert!(rejected.encode(v, State::Play, l).is_err());
        let mut bad = vec![1, b'a'];
        let payload = nested_wire(16, empty_leaf);
        bad.push(payload.len() as u8);
        bad.extend(payload);
        assert!(CustomClickAction::decode(&bad, v, State::Play, l).is_err());
        let smaller = Limits {
            max_nbt_depth: 14,
            ..l
        };
        assert!(allowed.encode(v, State::Play, smaller).is_err());
        assert!(CustomClickAction::decode(&wire, v, State::Play, smaller).is_err());
    }
}

#[test]
fn nbt_nodes_collections_strings_and_aggregate_packet_budgets_are_shared() {
    let v = Version::V1_21_6;
    let l = Limits::default();
    let nbt = Nbt::anonymous(Tag::Compound(vec![
        ("a".into(), Tag::ByteArray(vec![1, 2])),
        ("b".into(), Tag::ByteArray(vec![3, 4])),
    ]));
    let raw = [
        10, 7, 0, 1, b'a', 0, 0, 0, 2, 1, 2, 7, 0, 1, b'b', 0, 0, 0, 2, 3, 4, 0,
    ];
    let show = ShowDialog {
        dialog: RegistryHolder::Inline(nbt.clone()),
    };
    let click = CustomClickAction {
        id: "a".into(),
        payload: Some(nbt),
    };
    let mut click_wire = vec![1, b'a', raw.len() as u8];
    click_wire.extend(raw);
    for limit in [
        Limits {
            max_nbt_nodes: 6,
            ..l
        },
        Limits {
            max_collection: 1,
            ..l
        },
        Limits {
            max_string_chars: 0,
            ..l
        },
        Limits {
            max_nbt_depth: 0,
            ..l
        },
    ] {
        assert!(show.encode(v, State::Configuration, limit).is_err());
        assert!(ShowDialog::decode(&raw, v, State::Configuration, limit).is_err());
        assert!(click.encode(v, State::Play, limit).is_err());
        assert!(CustomClickAction::decode(&click_wire, v, State::Play, limit).is_err());
    }
    let exact_nodes = Limits {
        max_nbt_nodes: 7,
        ..l
    };
    assert_eq!(
        show.encode(v, State::Configuration, exact_nodes).unwrap(),
        raw
    );
    assert_eq!(
        click.encode(v, State::Play, exact_nodes).unwrap(),
        click_wire
    );
    // Each field individually fits, but their aggregate exceeds the body limit.
    let aggregate = Limits {
        max_packet: raw.len() + 2,
        ..l
    };
    assert!(click.encode(v, State::Play, aggregate).is_err());
    assert!(CustomClickAction::decode(&click_wire, v, State::Play, aggregate).is_err());
    let no_nodes = Limits {
        max_nbt_nodes: 0,
        ..l
    };
    assert_eq!(
        CustomClickAction {
            id: "".into(),
            payload: None
        }
        .encode(v, State::Play, no_nodes)
        .unwrap(),
        [0, 1, 0]
    );
    assert!(ShowDialog::decode(&[10, 0], v, State::Configuration, no_nodes).is_err());
}

#[test]
fn identifiers_reuse_version_rules_without_normalizing_or_executing() {
    for protocol in 771..=776 {
        let v = Version::from_protocol(protocol).unwrap();
        let l = Limits::default();
        for id in [
            "",
            "a",
            ":a",
            "minecraft:a",
            "a/b",
            "..:x",
            "https://example.invalid",
        ] {
            let value = CustomClickAction {
                id: id.into(),
                payload: None,
            };
            let mut raw = vec![id.len() as u8];
            raw.extend(id.as_bytes());
            raw.extend([1, 0]);
            let allowed = protocol < 775 || id != "..:x";
            assert_eq!(value.encode(v, State::Play, l).is_ok(), allowed);
            assert_eq!(
                CustomClickAction::decode(&raw, v, State::Play, l).is_ok(),
                allowed
            );
            if allowed {
                assert_eq!(value.encode(v, State::Play, l).unwrap(), raw);
            }
        }
        for id in ["A:x", "a:X", "a:b:c", "https://Example.invalid"] {
            assert!(CustomClickAction {
                id: id.into(),
                payload: None
            }
            .encode(v, State::Play, l)
            .is_err());
        }
        let id = "a".repeat(32768);
        assert!(CustomClickAction { id, payload: None }
            .encode(v, State::Play, l)
            .is_err());
        assert!(CustomClickAction::decode(&[1, 0xff, 1, 0], v, State::Play, l).is_err());
    }
}
