use rustwire_mc::{
    codec::{BlockPosition, Reader, Writer},
    nbt::{Nbt, Tag},
    packet::{chat::ChatComponent, game_test::*, typed::DecodedPacket},
    version::{Direction, State},
    Limits, Version,
};
fn pos(x: i32, y: i32, z: i32) -> BlockPosition {
    BlockPosition { x, y, z }
}
fn text(s: &str) -> ChatComponent {
    ChatComponent::Nbt(Nbt::anonymous(Tag::String(s.into())))
}
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(), 16).unwrap())
        .collect()
}
fn expected(name: &str, case: &str) -> GameTestPacket {
    match name {
        "game_test_highlight_pos" => {
            let (a, b) = match case {
                "zero" => (pos(0, 0, 0), pos(0, 0, 0)),
                "extremes" => (
                    pos(-33554432, -2048, 33554431),
                    pos(33554431, 2047, -33554432),
                ),
                "mixed" => (pos(4, 5, 6), pos(-4, -5, -6)),
                _ => panic!("case"),
            };
            GameTestPacket::Highlight(GameTestHighlightPos {
                absolute_position: a,
                relative_position: b,
            })
        }
        "test_instance_block_status" => {
            let (s, size) = match case {
                "no_size" => ("", None),
                "small" => ("Ready", Some([1, 2, 3])),
                "wide" => ("Done", Some([i32::MIN, 0, i32::MAX])),
                "zero_size" => ("0", Some([0, 0, 0])),
                _ => panic!("case"),
            };
            GameTestPacket::Status(TestInstanceBlockStatus {
                status: text(s),
                size,
            })
        }
        "set_test_block" => {
            let mode = case.strip_prefix("mode_").unwrap().parse::<i32>().unwrap();
            GameTestPacket::SetBlock(SetTestBlock {
                position: pos(mode - 2, 64 + mode, 2 - mode),
                mode: TestBlockMode::from_id(mode).unwrap(),
                message: ["", "hello", "line\nerror", "ok😀"][mode as usize].into(),
            })
        }
        "test_instance_block_action" => {
            if case == "extreme_sizes" {
                return GameTestPacket::Action(TestInstanceBlockAction {
                    position: pos(-33554432, 2047, 33554431),
                    action: TestInstanceActionKind::Run,
                    data: TestInstanceData {
                        test: Some("".into()),
                        size: [i32::MIN, i32::MAX, -1],
                        rotation: StructureRotation::Counterclockwise90,
                        ignore_entities: false,
                        status: TestInstanceRunStatus::Finished,
                        error_message: Some(text("")),
                    },
                });
            }
            let a = case
                .strip_prefix("action_")
                .unwrap()
                .parse::<i32>()
                .unwrap();
            GameTestPacket::Action(TestInstanceBlockAction {
                position: pos(a, 63, -a),
                action: TestInstanceActionKind::from_id(a).unwrap(),
                data: TestInstanceData {
                    test: (a % 2 != 0).then(|| "rustwire:test".into()),
                    size: [a, -a, 128 + a],
                    rotation: StructureRotation::from_id(a % 4).unwrap(),
                    ignore_entities: a % 2 == 0,
                    status: TestInstanceRunStatus::from_id(a % 3).unwrap(),
                    error_message: (a >= 3).then(|| text("failure")),
                },
            })
        }
        _ => panic!("name"),
    }
}
macro_rules! verify {
    ($ty:ty,$p:expr,$v:expr,$b:expr,$id:expr) => {{
        let p: $ty = $p;
        let v = $v;
        let bytes: &[u8] = $b;
        let limits = Limits::default();
        assert_eq!(<$ty>::decode(bytes, v, limits).unwrap(), p);
        assert_eq!(p.encode(v, limits).unwrap(), bytes);
        assert_eq!(p.packet(v, limits).unwrap().id, $id);
        assert_eq!(p.packet(v, limits).unwrap().data, bytes);
        for end in 0..bytes.len() {
            assert!(<$ty>::decode(&bytes[..end], v, limits).is_err());
            let mut r = Reader::new(&bytes[..end], limits);
            assert!(<$ty>::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0x55);
        assert!(<$ty>::decode(&trailing, v, limits).is_err());
        let mut r = Reader::new(&trailing, limits);
        assert_eq!(<$ty>::read(&mut r, v).unwrap(), p);
        assert_eq!(r.remaining(), &[0x55]);
        let exact = Limits {
            max_packet: bytes.len(),
            ..limits
        };
        assert_eq!(p.encode(v, exact).unwrap(), bytes);
        assert_eq!(<$ty>::decode(bytes, v, exact).unwrap(), p);
        let tight = Limits {
            max_packet: bytes.len() - 1,
            ..limits
        };
        assert!(p.encode(v, tight).is_err());
        assert!(<$ty>::decode(bytes, v, tight).is_err());
        let mut w = Writer::new();
        w.u8(7);
        assert!(p.write(&mut w, v, tight).is_err());
        assert_eq!(w.as_slice(), &[7]);
        p.write(&mut w, v, limits).unwrap();
        assert_eq!(&w.as_slice()[1..], bytes);
    }};
}
#[test]
fn independent_fixtures_values_ids_truncations_and_transactional_helpers() {
    let mut rows = 0;
    let mut prefixes = 0;
    for line in include_str!("fixtures/game-test.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 6);
        let v = Version::from_protocol(f[0].parse().unwrap()).unwrap();
        let body = hex(f[5]);
        let id = f[4].parse::<i32>().unwrap();
        let direction = if f[2] == "toClient" {
            Direction::Clientbound
        } else {
            Direction::Serverbound
        };
        let p = expected(f[1], f[3]);
        assert_eq!(v.packet_id(State::Play, direction, f[1]).unwrap(), id);
        assert_eq!(
            GameTestPacket::decode(f[1], &body, v, Limits::default()).unwrap(),
            p
        );
        assert_eq!(p.encode(v, Limits::default()).unwrap(), body);
        assert_eq!(p.packet(v, Limits::default()).unwrap().id, id);
        match &p {
            GameTestPacket::Highlight(p) => verify!(GameTestHighlightPos, *p, v, &body, id),
            GameTestPacket::Status(p) => verify!(TestInstanceBlockStatus, p.clone(), v, &body, id),
            GameTestPacket::SetBlock(p) => verify!(SetTestBlock, p.clone(), v, &body, id),
            GameTestPacket::Action(p) => verify!(TestInstanceBlockAction, p.clone(), v, &body, id),
        }
        let typed = DecodedPacket::decode(State::Play, f[1], &body, v, Limits::default()).unwrap();
        if direction == Direction::Clientbound {
            assert!(matches!(typed,Some(DecodedPacket::GameTest(value)) if value==p));
        } else {
            assert!(typed.is_none());
        }
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(v.packet_id(state, direction, f[1]).is_err());
            assert!(
                DecodedPacket::decode(state, f[1], &body, v, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        rows += 1;
        prefixes += body.len();
    }
    assert_eq!(rows, 124);
    assert_eq!(prefixes, 2775);
}
#[test]
fn every_family_introduction_and_raw_fallback_boundary() {
    for &v in Version::ALL {
        for (name, case, minimum) in [
            ("game_test_highlight_pos", "zero", 773),
            ("test_instance_block_status", "no_size", 770),
            ("set_test_block", "mode_0", 770),
            ("test_instance_block_action", "action_0", 770),
        ] {
            let p = expected(name, case);
            let bytes = p.encode(Version::V26_2, Limits::default()).unwrap();
            assert_eq!(
                p.encode(v, Limits::default()).is_ok(),
                v.protocol() >= minimum
            );
            assert_eq!(
                GameTestPacket::decode(name, &bytes, v, Limits::default()).is_ok(),
                v.protocol() >= minimum
            );
        }
    }
    assert!(GameTestPacket::decode("unknown", &[], Version::V26_2, Limits::default()).is_err());
    assert!(DecodedPacket::decode(
        State::Play,
        "unknown",
        &[],
        Version::V26_2,
        Limits::default()
    )
    .unwrap()
    .is_none());
}
fn base_action() -> TestInstanceBlockAction {
    TestInstanceBlockAction {
        position: pos(0, 0, 0),
        action: TestInstanceActionKind::Init,
        data: TestInstanceData {
            test: None,
            size: [0, 0, 0],
            rotation: StructureRotation::None,
            ignore_entities: true,
            status: TestInstanceRunStatus::Cleared,
            error_message: None,
        },
    }
}
#[test]
fn invalid_enums_booleans_and_absent_components_are_rejected() {
    let v = Version::V26_2;
    let limits = Limits::default();
    let base = base_action().encode(v, limits).unwrap();
    assert_eq!(base.len(), 17);
    for (offset, value) in [(8, 7), (9, 2), (13, 4), (14, 2), (15, 3), (16, 2)] {
        let mut bad = base.clone();
        bad[offset] = value;
        assert!(TestInstanceBlockAction::decode(&bad, v, limits).is_err());
        let mut r = Reader::new(&bad, limits);
        assert!(TestInstanceBlockAction::read(&mut r, v).is_err());
        assert_eq!(r.position(), 0);
    }
    let mut bad = base;
    bad[16] = 1;
    bad.push(0);
    assert!(TestInstanceBlockAction::decode(&bad, v, limits).is_err());
    assert!(TestInstanceBlockStatus::decode(&[0, 0], v, limits).is_err());
    assert!(TestInstanceBlockStatus::decode(&[8, 0, 0, 2], v, limits).is_err());
    let mut bad = vec![0; 8];
    bad.extend([4, 0]);
    assert!(SetTestBlock::decode(&bad, v, limits).is_err());
    for id in [-1, i32::MIN, 4, i32::MAX] {
        assert!(TestBlockMode::from_id(id).is_err());
    }
    assert!(TestInstanceActionKind::from_id(7).is_err());
    assert!(TestInstanceRunStatus::from_id(3).is_err());
}
#[test]
fn signed_varint_sizes_and_versioned_identifier_rules_are_preserved() {
    let mut p = base_action();
    p.data.size = [i32::MIN, i32::MAX, -1];
    let bytes = p.encode(Version::V26_2, Limits::default()).unwrap();
    assert_eq!(bytes.len(), 29);
    assert_eq!(
        TestInstanceBlockAction::decode(&bytes, Version::V26_2, Limits::default()).unwrap(),
        p
    );
    p.data.test = Some("..:x".into());
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 770) {
        assert_eq!(p.encode(v, Limits::default()).is_ok(), v.protocol() < 775);
    }
    for key in ["a:b:c", "Upper:path", "a:é"] {
        p.data.test = Some(key.into());
        assert!(p.encode(Version::V26_2, Limits::default()).is_err());
        let mut w = Writer::new();
        w.i64(0);
        w.var_i32(0);
        w.bool(true);
        w.string(key, 32767).unwrap();
        w.raw(&[0; 7]);
        assert!(
            TestInstanceBlockAction::decode(w.as_slice(), Version::V26_2, Limits::default())
                .is_err()
        );
    }
    p.data.test = Some("".into());
    assert!(p.encode(Version::V26_2, Limits::default()).is_ok());
}
#[test]
fn text_nbt_fixed_position_budgets_and_representation_fail_atomically() {
    let v = Version::V26_2;
    let limits = Limits::default();
    let mut p = SetTestBlock {
        position: pos(0, 0, 0),
        mode: TestBlockMode::Log,
        message: "ok😀".into(),
    };
    let exact = Limits {
        max_string_chars: 4,
        ..limits
    };
    let b = p.encode(v, exact).unwrap();
    assert_eq!(SetTestBlock::decode(&b, v, exact).unwrap(), p);
    let tight = Limits {
        max_string_chars: 3,
        ..limits
    };
    assert!(p.encode(v, tight).is_err());
    assert!(SetTestBlock::decode(&b, v, tight).is_err());
    p.message = "x".repeat(32768);
    let mut w = Writer::new();
    w.u8(9);
    assert!(p.write(&mut w, v, limits).is_err());
    assert_eq!(w.as_slice(), &[9]);
    let mut status = TestInstanceBlockStatus {
        status: text("Ready"),
        size: Some([1, 2, 3]),
    };
    let b = status.encode(v, limits).unwrap();
    for small in [
        Limits {
            max_nbt_nodes: 0,
            ..limits
        },
        Limits {
            max_string_chars: 0,
            ..limits
        },
    ] {
        assert!(status.write(&mut w, v, small).is_err());
        assert_eq!(w.as_slice(), &[9]);
        assert!(TestInstanceBlockStatus::decode(&b, v, small).is_err());
    }
    status.status = ChatComponent::Json("{}".into());
    assert!(status.write(&mut w, v, limits).is_err());
    assert_eq!(w.as_slice(), &[9]);
    let mut action = base_action();
    action.data.error_message = Some(ChatComponent::Json("{}".into()));
    assert!(action.write(&mut w, v, limits).is_err());
    assert_eq!(w.as_slice(), &[9]);
    let invalid = GameTestHighlightPos {
        absolute_position: pos(33554432, 0, 0),
        relative_position: pos(0, 0, 0),
    };
    assert!(invalid.write(&mut w, v, limits).is_err());
    assert_eq!(w.as_slice(), &[9]);
    let fixed = GameTestHighlightPos {
        absolute_position: pos(0, 0, 0),
        relative_position: pos(0, 0, 0),
    };
    assert!(fixed
        .encode(
            v,
            Limits {
                max_collection: 0,
                ..limits
            }
        )
        .is_ok());
}
