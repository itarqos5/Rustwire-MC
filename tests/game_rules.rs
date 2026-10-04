use rustwire_mc::{
    codec::{Reader, Writer},
    packet::{game_rules::*, typed::DecodedPacket},
    version::{Direction, State},
    Limits, Version,
};
fn v(n: i32) -> Version {
    Version::from_protocol(n).unwrap()
}
#[test]
fn independent_rule_fixtures_have_exact_ids_fields_prefixes_and_limits() {
    let mut count = 0;
    for line in include_str!("fixtures/game-rules.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let f: Vec<_> = line.split('\t').collect();
        let version = v(f[0].parse().unwrap());
        let direction = if f[1] == "toClient" {
            Direction::Clientbound
        } else {
            Direction::Serverbound
        };
        let name = f[2];
        let ident: i32 = f[3].parse().unwrap();
        let bytes: Vec<u8> = (0..f[5].len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&f[5][i..i + 2], 16).unwrap())
            .collect();
        let limits = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        let check = |input: &[u8], limits: Limits| -> rustwire_mc::Result<Vec<u8>> {
            match name {
                "game_rule_values" => {
                    GameRuleValues::decode(input, version, limits)?.encode(version, limits)
                }
                "set_game_rule" => {
                    SetGameRules::decode(input, version, limits)?.encode(version, limits)
                }
                _ => LowDiskSpaceWarning::decode(input, version, limits)?.encode(version, limits),
            }
        };
        assert_eq!(check(&bytes, limits).unwrap(), bytes);
        assert_eq!(
            version.packet_id(State::Play, direction, name).unwrap(),
            ident
        );
        let raw = match name {
            "game_rule_values" => GameRuleValues::decode(&bytes, version, limits)
                .unwrap()
                .packet(version, limits)
                .unwrap(),
            "set_game_rule" => SetGameRules::decode(&bytes, version, limits)
                .unwrap()
                .packet(version, limits)
                .unwrap(),
            _ => LowDiskSpaceWarning.packet(version, limits).unwrap(),
        };
        assert_eq!(raw.id, ident);
        assert_eq!(raw.data, bytes);
        if f[4] == "ordered-duplicates" {
            let rules = GameRuleValues::decode(&bytes, version, limits)
                .unwrap()
                .rules;
            assert_eq!(rules.len(), 5);
            assert_eq!(rules[0].key, rules[2].key);
            assert_eq!(rules[0].value, "true");
            assert_eq!(rules[2].value, "false");
            assert_eq!(rules[1].value, "not a parsed value 😀");
            assert!(rules[3].key.is_empty());
            assert_eq!(rules[4].value, "\0");
        }
        for end in 0..bytes.len() {
            assert!(check(&bytes[..end], limits).is_err());
        }
        let mut tail = bytes.clone();
        tail.push(0);
        assert!(check(&tail, Limits::default()).is_err());
        if let Some(max_packet) = bytes.len().checked_sub(1) {
            let small = Limits {
                max_packet,
                ..limits
            };
            assert!(check(&bytes, small).is_err());
            let encoded = match name {
                "game_rule_values" => GameRuleValues::decode(&bytes, version, limits)
                    .unwrap()
                    .encode(version, small),
                "set_game_rule" => SetGameRules::decode(&bytes, version, limits)
                    .unwrap()
                    .encode(version, small),
                _ => LowDiskSpaceWarning.encode(version, small),
            };
            assert!(encoded.is_err());
        }
        for state in [State::Handshake, State::Login, State::Configuration] {
            assert!(DecodedPacket::decode(state, name, &bytes, version, limits)
                .unwrap()
                .is_none());
        }
        let typed = DecodedPacket::decode(State::Play, name, &bytes, version, limits).unwrap();
        match name {
            "game_rule_values" => assert!(matches!(typed, Some(DecodedPacket::GameRules(_)))),
            "low_disk_space_warning" => {
                assert!(matches!(typed, Some(DecodedPacket::LowDiskSpaceWarning)))
            }
            _ => assert!(typed.is_none()),
        };
        count += 1;
    }
    assert_eq!(count, 14);
}
#[test]
fn version_gates_empty_bodies_and_collection_limits() {
    for &version in Version::ALL {
        let available = version.protocol() >= 775;
        assert_eq!(
            GameRuleValues::decode(&[0], version, Limits::default()).is_ok(),
            available
        );
        assert_eq!(
            SetGameRules { rules: vec![] }
                .encode(version, Limits::default())
                .is_ok(),
            available
        );
        assert_eq!(
            LowDiskSpaceWarning::decode(
                &[],
                version,
                Limits {
                    max_packet: 0,
                    ..Limits::default()
                }
            )
            .is_ok(),
            available
        );
    }
    let rules = vec![
        GameRuleEntry {
            key: "a".into(),
            value: "b".into()
        };
        2
    ];
    let value = GameRuleValues { rules };
    let bytes = value.encode(v(776), Limits::default()).unwrap();
    let limits = Limits {
        max_collection: 1,
        ..Limits::default()
    };
    assert!(value.encode(v(776), limits).is_err());
    assert!(GameRuleValues::decode(&bytes, v(776), limits).is_err());
    let large = Limits {
        max_collection: usize::MAX,
        max_packet: usize::MAX,
        ..Limits::default()
    };
    assert!(GameRuleValues::decode(&[255, 255, 255, 255, 7], v(776), large).is_err());
    assert!(GameRuleValues::decode(&[255, 255, 255, 255, 15], v(776), large).is_err());
}
#[test]
fn string_resource_domains_budgets_and_transactional_io() {
    let version = v(775);
    let value = GameRuleValues {
        rules: vec![GameRuleEntry {
            key: "a".into(),
            value: "😀".into(),
        }],
    };
    let enough = Limits {
        max_string_chars: 2,
        ..Limits::default()
    };
    let bytes = value.encode(version, enough).unwrap();
    assert_eq!(
        GameRuleValues::decode(&bytes, version, enough).unwrap(),
        value
    );
    let small = Limits {
        max_string_chars: 1,
        ..enough
    };
    assert!(value.encode(version, small).is_err());
    assert!(GameRuleValues::decode(&bytes, version, small).is_err());
    let mut bad = value.clone();
    bad.rules[0].key = "BAD:KEY".into();
    assert!(bad.encode(version, Limits::default()).is_err());
    assert!(GameRuleValues::decode(&[1, 1, b'A', 0], version, Limits::default()).is_err());
    assert!(GameRuleValues::decode(&[1, 0, 1, 255], version, Limits::default()).is_err());
    let mut reader = Reader::new(&[1, 0], Limits::default());
    assert!(GameRuleValues::read(&mut reader, version).is_err());
    assert_eq!(reader.position(), 0);
    let mut writer = Writer::new();
    writer.raw(&[1, 2]);
    assert!(value
        .write(
            &mut writer,
            version,
            Limits {
                max_packet: 1,
                ..Limits::default()
            }
        )
        .is_err());
    assert_eq!(writer.as_slice(), [1, 2]);
    let mut large = value;
    large.rules[0].value = "x".repeat(32767);
    assert!(large.encode(version, Limits::default()).is_ok());
    large.rules[0].value.push('x');
    assert!(large.encode(version, Limits::default()).is_err());
}

#[test]
fn bounded_fixture_mutations_never_panic_and_successful_values_stay_stable() {
    for line in include_str!("fixtures/game-rules.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let fields: Vec<_> = line.split('\t').collect();
        if fields[2] == "low_disk_space_warning" {
            continue;
        }
        let version = v(fields[0].parse().unwrap());
        let bytes: Vec<u8> = (0..fields[5].len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&fields[5][i..i + 2], 16).unwrap())
            .collect();
        for offset in 0..bytes.len() {
            let mut changed = bytes.clone();
            changed[offset] ^= 255;
            if let Ok(value) = GameRuleValues::decode(&changed, version, Limits::default()) {
                let canonical = value.encode(version, Limits::default()).unwrap();
                assert_eq!(
                    GameRuleValues::decode(&canonical, version, Limits::default()).unwrap(),
                    value
                );
            }
        }
    }
}
