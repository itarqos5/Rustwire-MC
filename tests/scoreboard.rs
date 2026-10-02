use rustwire_mc::{
    nbt::{Nbt, Tag},
    packet::{chat::ChatComponent, scoreboard::*},
    version::{Direction, State},
    Limits, Version,
};

// Independent synthetic wire fixtures: no Rustwire writer or encoder is used.
fn vint(b: &mut Vec<u8>, n: i32) {
    let mut n = n as u32;
    loop {
        let value = (n & 127) as u8;
        n >>= 7;
        b.push(value | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
fn string(b: &mut Vec<u8>, s: &str) {
    vint(b, s.len() as i32);
    b.extend(s.as_bytes());
}
fn component(b: &mut Vec<u8>, v: Version, text: &str) {
    if v.protocol() < 765 {
        string(b, &format!("{{\"text\":\"{text}\"}}"));
    } else {
        b.push(8);
        b.extend((text.len() as u16).to_be_bytes());
        b.extend(text.as_bytes());
    }
}
fn format(b: &mut Vec<u8>, v: Version, id: Option<u8>) {
    b.push(u8::from(id.is_some()));
    if let Some(id) = id {
        b.push(id);
        match id {
            1 => b.extend([10, 0]),
            2 => component(b, v, "fixed"),
            _ => (),
        }
    }
}
fn objective(v: Version, mode: u8, format_id: Option<u8>) -> Vec<u8> {
    let mut b = Vec::new();
    string(&mut b, "objective");
    b.push(mode);
    if matches!(mode, 0 | 2) {
        component(&mut b, v, "title");
        b.push(1);
        if v.protocol() >= 765 {
            format(&mut b, v, format_id);
        }
    }
    b
}
fn score(v: Version, remove: bool, value: i32, format_id: Option<u8>) -> Vec<u8> {
    let mut b = Vec::new();
    string(&mut b, "owner");
    if v.protocol() < 765 {
        b.push(u8::from(remove));
    }
    string(&mut b, "objective");
    if !remove {
        vint(&mut b, value);
        if v.protocol() >= 765 {
            b.push(1);
            component(&mut b, v, "display");
            format(&mut b, v, format_id);
        }
    }
    b
}
fn team(v: Version, mode: u8) -> Vec<u8> {
    let mut b = Vec::new();
    string(&mut b, "team");
    b.push(mode);
    if matches!(mode, 0 | 2) {
        component(&mut b, v, "title");
        if v.protocol() == 776 {
            component(&mut b, v, "prefix");
            component(&mut b, v, "suffix");
            b.extend([2, 3, 1, 15, 255]);
        } else {
            b.push(255);
            if v.protocol() < 770 {
                string(&mut b, "hideForOtherTeams");
                string(&mut b, "pushOwnTeam");
            } else {
                b.extend([2, 3]);
            }
            b.push(21);
            component(&mut b, v, "prefix");
            component(&mut b, v, "suffix");
        }
    }
    if matches!(mode, 0 | 3 | 4) {
        b.push(3);
        string(&mut b, "A");
        string(&mut b, "A");
        string(&mut b, "");
    }
    b
}
fn fixtures(v: Version) -> Vec<(&'static str, Vec<u8>)> {
    let mut out = vec![];
    for action in [0, 1, 2, 3, 128, 255] {
        for fmt in [None, Some(0), Some(1), Some(2)] {
            out.push(("scoreboard_objective", objective(v, action, fmt)));
        }
    }
    for value in [i32::MIN, -1, 0, i32::MAX] {
        for fmt in [None, Some(0), Some(1), Some(2)] {
            out.push(("scoreboard_score", score(v, false, value, fmt)));
        }
    }
    if v.protocol() < 765 {
        out.push(("scoreboard_score", score(v, true, 0, None)));
    } else {
        for objective in [None, Some(""), Some("objective")] {
            let mut b = vec![];
            string(&mut b, "owner");
            b.push(u8::from(objective.is_some()));
            if let Some(s) = objective {
                string(&mut b, s);
            }
            out.push(("reset_score", b));
        }
    }
    for mode in [0, 1, 2, 3, 4, 5, 128, 255] {
        out.push(("teams", team(v, mode)));
    }
    for slot in [-128, -1, 0, 18, 19, 127] {
        let mut b = vec![];
        if v.protocol() == 763 {
            b.push(slot as u8);
        } else {
            vint(&mut b, slot);
        }
        string(&mut b, "");
        out.push(("scoreboard_display_objective", b));
    }
    out
}
#[test]
fn independent_goldens_all_actions_formats_and_fourteen_releases() {
    for &v in Version::ALL {
        for (name, b) in fixtures(v) {
            let p = ScoreboardPacket::decode(name, &b, v, Limits::default()).unwrap();
            assert_eq!(p.encode(v, Limits::default()).unwrap(), b, "{v:?} {name}");
            assert_eq!(p.packet(v, Limits::default()).unwrap().data, b);
            for len in 0..b.len() {
                assert!(
                    ScoreboardPacket::decode(name, &b[..len], v, Limits::default()).is_err(),
                    "{v:?} {name} prefix{len}"
                );
            }
            let mut trailing = b.clone();
            trailing.push(0);
            assert!(ScoreboardPacket::decode(name, &trailing, v, Limits::default()).is_err());
            let limited = Limits {
                max_packet: b.len() - 1,
                ..Limits::default()
            };
            assert!(ScoreboardPacket::decode(name, &b, v, limited).is_err());
            assert!(p.encode(v, limited).is_err());
        }
    }
}
#[test]
fn pinned_packet_ids_are_clientbound_play() {
    // Independent catalog values transcribed from pinned release schemas.
    let rows = [
        [0x58, 0x51, 0x5b, -1, 0x5a],
        [0x5a, 0x53, 0x5d, -1, 0x5c],
        [0x5c, 0x55, 0x5f, 0x42, 0x5e],
        [0x5e, 0x57, 0x61, 0x44, 0x60],
        [0x5e, 0x57, 0x61, 0x44, 0x60],
        [0x64, 0x5c, 0x68, 0x49, 0x67],
        [0x64, 0x5c, 0x68, 0x49, 0x67],
        [0x63, 0x5b, 0x67, 0x48, 0x66],
        [0x63, 0x5b, 0x67, 0x48, 0x66],
        [0x63, 0x5b, 0x67, 0x48, 0x66],
        [0x68, 0x60, 0x6c, 0x4d, 0x6b],
        [0x68, 0x60, 0x6c, 0x4d, 0x6b],
        [0x6a, 0x62, 0x6e, 0x4f, 0x6d],
        [0x6a, 0x62, 0x6e, 0x4f, 0x6d],
    ];
    let names = [
        "scoreboard_objective",
        "scoreboard_display_objective",
        "scoreboard_score",
        "reset_score",
        "teams",
    ];
    for (&v, row) in Version::ALL.iter().zip(rows) {
        for (name, id) in names.into_iter().zip(row) {
            if id < 0 {
                assert!(v
                    .packet_id(State::Play, Direction::Clientbound, name)
                    .is_err());
                continue;
            }
            assert_eq!(
                v.packet_id(State::Play, Direction::Clientbound, name)
                    .unwrap(),
                id
            );
            assert!(v
                .packet_id(State::Configuration, Direction::Clientbound, name)
                .is_err());
            assert!(v
                .packet_id(State::Play, Direction::Serverbound, name)
                .is_err());
        }
    }
}
#[test]
fn unknown_java_domains_and_version_specific_shapes_are_preserved() {
    for &v in Version::ALL {
        let p = Teams::decode(&team(v, 0), v, Limits::default()).unwrap();
        let TeamAction::Create {
            mut parameters,
            members,
        } = p.action
        else {
            panic!()
        };
        assert_eq!(parameters.flags, 255);
        assert_eq!(members, ["A", "A", ""]);
        assert_eq!(parameters.visibility, TeamVisibility::HideForOtherTeams);
        assert_eq!(parameters.collision_rule, TeamCollisionRule::PushOwnTeam);
        if v.protocol() < 770 {
            parameters.visibility = TeamVisibility::OtherName("custom".into());
            parameters.collision_rule = TeamCollisionRule::OtherName("".into());
        } else {
            parameters.visibility = TeamVisibility::OtherId(i32::MIN);
            parameters.collision_rule = TeamCollisionRule::OtherId(i32::MAX);
        }
        if v.protocol() == 776 {
            parameters.color = TeamColor::Color(Some(i32::MIN));
        }
        let p = Teams {
            name: "team".into(),
            action: TeamAction::Update(parameters.clone()),
        };
        let b = p.encode(v, Limits::default()).unwrap();
        assert_eq!(Teams::decode(&b, v, Limits::default()).unwrap(), p);
        parameters.color = if v.protocol() == 776 {
            TeamColor::Formatting(21)
        } else {
            TeamColor::Color(None)
        };
        assert!(Teams {
            name: "team".into(),
            action: TeamAction::Update(parameters)
        }
        .encode(v, Limits::default())
        .is_err());
        if v.protocol() > 763 {
            let p = DisplayObjective {
                slot: i32::MIN,
                objective: "".into(),
            };
            assert_eq!(
                DisplayObjective::decode(
                    &p.encode(v, Limits::default()).unwrap(),
                    v,
                    Limits::default()
                )
                .unwrap(),
                p
            );
        }
    }
}
#[test]
fn genuine_malformed_values_and_unsupported_release_features_reject() {
    for &v in Version::ALL {
        let mut b = vec![];
        string(&mut b, "o");
        b.push(0);
        component(&mut b, v, "x");
        b.push(2);
        if v.protocol() >= 765 {
            b.push(0);
        }
        assert!(ScoreboardObjective::decode(&b, v, Limits::default()).is_err());
        if v.protocol() < 765 {
            assert!(ResetScore {
                owner: "o".into(),
                objective: None
            }
            .encode(v, Limits::default())
            .is_err());
            assert!(ResetScore::decode(&[1, b'o', 0], v, Limits::default()).is_err());
            for action in [-1, 2, i32::MAX] {
                let mut b = vec![];
                string(&mut b, "o");
                vint(&mut b, action);
                string(&mut b, "o");
                b.push(0);
                assert!(ScoreboardScore::decode(&b, v, Limits::default()).is_err());
            }
        } else {
            assert!(
                ScoreboardObjective::decode(&objective(v, 0, Some(3)), v, Limits::default())
                    .is_err()
            );
            assert!(ScoreboardScore {
                owner: "o".into(),
                objective: "o".into(),
                action: ScoreAction::Remove
            }
            .encode(v, Limits::default())
            .is_err());
            let mut b = vec![];
            string(&mut b, "o");
            b.push(2);
            assert!(ResetScore::decode(&b, v, Limits::default()).is_err());
        }
        assert!(ScoreboardObjective {
            name: "o".into(),
            action: ObjectiveAction::Other(0)
        }
        .encode(v, Limits::default())
        .is_err());
        assert!(Teams {
            name: "t".into(),
            action: TeamAction::Other(4)
        }
        .encode(v, Limits::default())
        .is_err());
        let mut bad_count = vec![1, b't', 3];
        vint(&mut bad_count, -1);
        assert!(Teams::decode(&bad_count, v, Limits::default()).is_err());
        let p = Teams::decode(&team(v, 2), v, Limits::default()).unwrap();
        let TeamAction::Update(mut parameters) = p.action else {
            panic!()
        };
        if v.protocol() < 776 {
            parameters.color = TeamColor::Formatting(22);
            assert!(Teams {
                name: "t".into(),
                action: TeamAction::Update(parameters.clone())
            }
            .encode(v, Limits::default())
            .is_err());
        }
    }
}
#[test]
fn styled_formats_require_compound_and_share_nbt_node_budgets() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 765) {
        let p =
            ScoreboardObjective::decode(&objective(v, 0, Some(1)), v, Limits::default()).unwrap();
        let limit = Limits {
            max_nbt_nodes: 1,
            ..Limits::default()
        };
        assert!(p.encode(v, limit).is_err());
        assert!(ScoreboardObjective::decode(&objective(v, 0, Some(1)), v, limit).is_err());
        let ObjectiveAction::Create(mut props) = p.action else {
            panic!()
        };
        props.number_format = Some(NumberFormat::Styled(Nbt::anonymous(Tag::String(
            "wrong".into(),
        ))));
        assert!(ScoreboardObjective {
            name: "o".into(),
            action: ObjectiveAction::Create(props)
        }
        .encode(v, Limits::default())
        .is_err());
        let mut b = objective(v, 0, Some(1));
        b.truncate(b.len() - 2);
        b.extend([8, 0, 1, b'x']);
        assert!(ScoreboardObjective::decode(&b, v, Limits::default()).is_err());
        let limit = Limits {
            max_nbt_nodes: 2,
            ..Limits::default()
        };
        let p = Teams::decode(&team(v, 0), v, Limits::default()).unwrap();
        assert!(p.encode(v, limit).is_err());
        assert!(Teams::decode(&team(v, 0), v, limit).is_err());
        let mut b = objective(v, 0, None);
        b["objective".len() + 2] = 0;
        assert!(ScoreboardObjective::decode(&b, v, Limits::default()).is_err());
    }
}
#[test]
fn string_utf16_and_collection_limits_are_symmetric() {
    for &v in Version::ALL {
        for s in ["x".repeat(41), "x".repeat(32767), "😀".repeat(16383)] {
            let p = ScoreboardObjective {
                name: s,
                action: ObjectiveAction::Remove,
            };
            assert_eq!(
                ScoreboardObjective::decode(
                    &p.encode(v, Limits::default()).unwrap(),
                    v,
                    Limits::default()
                )
                .unwrap(),
                p
            );
        }
        let p = ScoreboardObjective {
            name: "x".repeat(32768),
            action: ObjectiveAction::Remove,
        };
        assert!(p.encode(v, Limits::default()).is_err());
        let mut b = vec![];
        string(&mut b, &p.name);
        b.push(1);
        assert!(ScoreboardObjective::decode(&b, v, Limits::default()).is_err());
        let b = team(v, 0);
        let p = Teams::decode(&b, v, Limits::default()).unwrap();
        let limit = Limits {
            max_collection: 2,
            ..Limits::default()
        };
        assert!(p.encode(v, limit).is_err());
        assert!(Teams::decode(&b, v, limit).is_err());
        if v.protocol() < 770 {
            let TeamAction::Create { mut parameters, .. } = p.action else {
                panic!()
            };
            parameters.visibility = TeamVisibility::OtherName("x".repeat(41));
            assert!(Teams {
                name: "t".into(),
                action: TeamAction::Update(parameters)
            }
            .encode(v, Limits::default())
            .is_err());
        }
    }
}
#[test]
fn component_mismatch_and_unsupported_number_format_reject() {
    for &v in Version::ALL {
        let component = if v.protocol() < 765 {
            ChatComponent::Nbt(Nbt::anonymous(Tag::String("x".into())))
        } else {
            ChatComponent::Json("\"x\"".into())
        };
        let p = ScoreboardObjective {
            name: "o".into(),
            action: ObjectiveAction::Create(ObjectiveProperties {
                display_name: component,
                render_type: ObjectiveRenderType::Integer,
                number_format: None,
            }),
        };
        assert!(p.encode(v, Limits::default()).is_err());
        if v.protocol() < 765 {
            let p = ScoreboardScore {
                owner: "o".into(),
                objective: "o".into(),
                action: ScoreAction::Set {
                    value: 1,
                    display_name: None,
                    number_format: Some(NumberFormat::Blank),
                },
            };
            assert!(p.encode(v, Limits::default()).is_err());
        }
    }
}
#[test]
fn mutation_smoke_under_tight_budgets_never_panics() {
    let limits = Limits {
        max_packet: 4096,
        max_collection: 128,
        max_nbt_nodes: 128,
        max_nbt_depth: 8,
        ..Limits::default()
    };
    for &v in Version::ALL {
        for (name, b) in fixtures(v) {
            for i in 0..b.len() {
                for byte in [0, 127, 128, 255] {
                    let mut mutated = b.clone();
                    mutated[i] = byte;
                    if let Ok(p) = ScoreboardPacket::decode(name, &mutated, v, limits) {
                        let canonical = p.encode(v, limits).unwrap();
                        assert_eq!(
                            ScoreboardPacket::decode(name, &canonical, v, limits).unwrap(),
                            p
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn body_read_and_write_are_transactional_and_bounded() {
    use rustwire_mc::codec::{Reader, Writer};
    for &v in Version::ALL {
        let bytes = team(v, 0);
        let mut input = bytes.clone();
        input.push(99);
        let mut r = Reader::new(&input, Limits::default());
        let p = Teams::read(&mut r, v).unwrap();
        assert_eq!(r.u8().unwrap(), 99);
        let mut w = Writer::new();
        w.u8(42);
        p.write(&mut w, v, Limits::default()).unwrap();
        assert_eq!(&w.as_slice()[1..], &bytes);
        let limited = Limits {
            max_packet: bytes.len() - 1,
            ..Limits::default()
        };
        let before = w.as_slice().to_vec();
        assert!(p.write(&mut w, v, limited).is_err());
        assert_eq!(w.as_slice(), before);
        let mut r = Reader::new(&input, limited);
        assert!(Teams::read(&mut r, v).is_err());
        assert_eq!(r.position(), 0);
        let mut r = Reader::new(&bytes[..bytes.len() - 1], Limits::default());
        assert!(Teams::read(&mut r, v).is_err());
        assert_eq!(r.position(), 0);
    }
}

#[test]
fn named_rule_limits_and_disjoint_unknown_variants_are_checked() {
    for &v in Version::ALL {
        let p = Teams::decode(&team(v, 2), v, Limits::default()).unwrap();
        let TeamAction::Update(mut parameters) = p.action else {
            panic!()
        };
        parameters.visibility = if v.protocol() < 770 {
            TeamVisibility::OtherName("always".into())
        } else {
            TeamVisibility::OtherId(0)
        };
        assert!(Teams {
            name: "t".into(),
            action: TeamAction::Update(parameters)
        }
        .encode(v, Limits::default())
        .is_err());
        if v.protocol() < 770 {
            let mut b = vec![1, b't', 2];
            component(&mut b, v, "x");
            b.push(0);
            string(&mut b, &"x".repeat(41));
            string(&mut b, "always");
            b.push(21);
            component(&mut b, v, "");
            component(&mut b, v, "");
            assert!(Teams::decode(&b, v, Limits::default()).is_err());
        }
    }
    assert!(DisplayObjective {
        slot: 128,
        objective: "".into()
    }
    .encode(Version::V1_20, Limits::default())
    .is_err());
    assert!(DisplayObjective {
        slot: -129,
        objective: "".into()
    }
    .encode(Version::V1_20, Limits::default())
    .is_err());
}

#[test]
fn complete_team_color_domains_and_absent_score_options() {
    for &v in Version::ALL {
        let p = Teams::decode(&team(v, 2), v, Limits::default()).unwrap();
        let TeamAction::Update(parameters) = p.action else {
            panic!()
        };
        let colors = if v.protocol() == 776 {
            std::iter::once(TeamColor::Color(None))
                .chain((0..16).map(|n| TeamColor::Color(Some(n))))
                .collect::<Vec<_>>()
        } else {
            (0..22).map(TeamColor::Formatting).collect()
        };
        for color in colors {
            let mut parameters = parameters.clone();
            parameters.color = color;
            let p = Teams {
                name: "t".into(),
                action: TeamAction::Update(parameters),
            };
            assert_eq!(
                Teams::decode(
                    &p.encode(v, Limits::default()).unwrap(),
                    v,
                    Limits::default()
                )
                .unwrap(),
                p
            );
        }
        if v.protocol() >= 765 {
            let b = [1, b'o', 1, b'o', 0, 0, 0];
            let p = ScoreboardScore::decode(&b, v, Limits::default()).unwrap();
            assert_eq!(
                p.action,
                ScoreAction::Set {
                    value: 0,
                    display_name: None,
                    number_format: None
                }
            );
            assert_eq!(p.encode(v, Limits::default()).unwrap(), b);
        }
        let b = [1, b't', 3, 0];
        let p = Teams::decode(&b, v, Limits::default()).unwrap();
        assert_eq!(p.action, TeamAction::AddMembers(vec![]));
        assert_eq!(p.encode(v, Limits::default()).unwrap(), b);
    }
}
