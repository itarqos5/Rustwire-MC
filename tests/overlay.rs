use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, Tag, TagType},
    packet::{chat::ChatComponent, overlay::*},
    version::{Direction, State},
    Error, Limits, Version,
};

const UUID: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

// Independent wire fixtures checked by tools/paper/OverlayOracle.java against
// prepared release APIs for all 14 protocol families. No library encode helper.
fn text_bytes(version: Version, c: u8) -> Vec<u8> {
    if version.protocol() < 765 {
        let mut bytes = b"\x0c{\"text\":\"x\"}".to_vec();
        bytes[10] = c;
        bytes
    } else {
        vec![8, 0, 1, c]
    }
}
fn text(version: Version, c: char) -> ChatComponent {
    if version.protocol() < 765 {
        ChatComponent::Json(format!("{{\"text\":\"{c}\"}}"))
    } else {
        ChatComponent::Nbt(Nbt::anonymous(Tag::String(c.to_string().into())))
    }
}
fn boss_fixture(version: Version, action: u8) -> Vec<u8> {
    let mut bytes = UUID.to_vec();
    bytes.push(action);
    if action == 0 || action == 3 {
        bytes.extend(text_bytes(version, b'x'));
    }
    if action == 0 || action == 2 {
        bytes.extend([0x3f, 0xc0, 0, 0]); // 1.5, deliberately outside 0–1.
    }
    if action == 0 || action == 4 {
        bytes.extend([6, 4]);
    }
    if action == 0 || action == 5 {
        bytes.push(7);
    }
    bytes
}
fn expected_boss(version: Version, action: u8) -> BossBar {
    BossBar {
        uuid: UUID,
        action: match action {
            0 => BossBarAction::Add {
                title: text(version, 'x'),
                health: 1.5,
                color: BossBarColor::White,
                division: BossBarDivision::Twenty,
                flags: BossBarFlags(7),
            },
            1 => BossBarAction::Remove,
            2 => BossBarAction::UpdateHealth(1.5),
            3 => BossBarAction::UpdateTitle(text(version, 'x')),
            4 => BossBarAction::UpdateStyle {
                color: BossBarColor::White,
                division: BossBarDivision::Twenty,
            },
            5 => BossBarAction::UpdateFlags(BossBarFlags(7)),
            _ => unreachable!(),
        },
    }
}

#[test]
fn independent_boss_fixtures_every_action_every_protocol() {
    for &version in Version::ALL {
        for action in 0..=5 {
            let fixture = boss_fixture(version, action);
            let expected = expected_boss(version, action);
            let decoded = BossBar::decode(&fixture, version, Limits::default()).unwrap();
            assert_eq!(decoded, expected);
            assert_eq!(decoded.action.id(), action as i32);
            assert_eq!(
                expected.encode(version, Limits::default()).unwrap(),
                fixture
            );
        }
    }
}

#[test]
fn independent_header_footer_fixtures_every_protocol() {
    for &version in Version::ALL {
        let fixture = [text_bytes(version, b'x'), text_bytes(version, b'y')].concat();
        let expected = PlayerListHeaderFooter {
            header: text(version, 'x'),
            footer: text(version, 'y'),
        };
        assert_eq!(
            PlayerListHeaderFooter::decode(&fixture, version, Limits::default()).unwrap(),
            expected
        );
        assert_eq!(
            expected.encode(version, Limits::default()).unwrap(),
            fixture
        );
    }
}

#[test]
fn rejects_every_truncation_and_trailing_byte() {
    for &version in Version::ALL {
        for action in 0..=5 {
            let mut bytes = boss_fixture(version, action);
            for end in 0..bytes.len() {
                assert!(
                    BossBar::decode(&bytes[..end], version, Limits::default()).is_err(),
                    "{} action {action} end {end}",
                    version.protocol()
                );
            }
            bytes.push(0);
            assert!(BossBar::decode(&bytes, version, Limits::default()).is_err());
        }
        let mut bytes = [text_bytes(version, b'x'), text_bytes(version, b'y')].concat();
        for end in 0..bytes.len() {
            assert!(
                PlayerListHeaderFooter::decode(&bytes[..end], version, Limits::default()).is_err()
            );
        }
        bytes.push(0);
        assert!(PlayerListHeaderFooter::decode(&bytes, version, Limits::default()).is_err());
    }
}

#[test]
fn reads_one_body_and_writes_atomically() {
    for &version in Version::ALL {
        let boss = expected_boss(version, 0);
        let header = PlayerListHeaderFooter {
            header: text(version, 'x'),
            footer: text(version, 'y'),
        };
        let mut w = Writer::new();
        w.u8(0xaa);
        boss.write(&mut w, version, Limits::default()).unwrap();
        header.write(&mut w, version, Limits::default()).unwrap();
        let mut r = Reader::new(w.as_slice(), Limits::default());
        assert_eq!(r.u8().unwrap(), 0xaa);
        assert_eq!(BossBar::read(&mut r, version).unwrap(), boss);
        assert_eq!(
            PlayerListHeaderFooter::read(&mut r, version).unwrap(),
            header
        );
        r.finish().unwrap();
        let before = w.as_slice().to_vec();
        let tiny = Limits {
            max_packet: 1,
            ..Limits::default()
        };
        assert!(boss.write(&mut w, version, tiny).is_err());
        assert_eq!(w.as_slice(), before);
        assert!(header.write(&mut w, version, tiny).is_err());
        assert_eq!(w.as_slice(), before);
    }
}

#[test]
fn helpers_use_correct_clientbound_play_ids_every_protocol() {
    let boss_ids = [
        0x0b, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x0a, 0x09, 0x09, 0x09, 0x09, 0x09, 0x09, 0x09,
    ];
    let list_ids = [
        0x65, 0x68, 0x6a, 0x6d, 0x6d, 0x74, 0x74, 0x73, 0x73, 0x73, 0x78, 0x78, 0x7a, 0x7a,
    ];
    for (index, &version) in Version::ALL.iter().enumerate() {
        let boss = expected_boss(version, 1)
            .packet(version, Limits::default())
            .unwrap();
        assert_eq!(
            boss.id,
            version
                .packet_id(State::Play, Direction::Clientbound, "boss_bar")
                .unwrap()
        );
        assert_eq!(boss.id, boss_ids[index]);
        assert_eq!(boss.data, boss_fixture(version, 1));
        let list = PlayerListHeaderFooter {
            header: text(version, 'x'),
            footer: text(version, 'y'),
        }
        .packet(version, Limits::default())
        .unwrap();
        assert_eq!(
            list.id,
            version
                .packet_id(State::Play, Direction::Clientbound, "playerlist_header")
                .unwrap()
        );
        assert_eq!(list.id, list_ids[index]);
        assert_eq!(
            list.data,
            [text_bytes(version, b'x'), text_bytes(version, b'y')].concat()
        );
    }
}

#[test]
fn all_colors_and_divisions_have_closed_wire_ids() {
    for &version in Version::ALL {
        for color in 0..=6 {
            for division in 0..=4 {
                let bytes = [UUID.as_slice(), &[4, color, division]].concat();
                let decoded = BossBar::decode(&bytes, version, Limits::default()).unwrap();
                assert_eq!(
                    decoded.action,
                    BossBarAction::UpdateStyle {
                        color: BossBarColor::from_id(color as i32).unwrap(),
                        division: BossBarDivision::from_id(division as i32).unwrap()
                    }
                );
                assert_eq!(decoded.encode(version, Limits::default()).unwrap(), bytes);
            }
        }
        for id in [-1, 7, 128, i32::MAX] {
            assert!(BossBarColor::from_id(id).is_err());
            let mut w = Writer::new();
            w.raw(&UUID);
            w.var_i32(4);
            w.var_i32(id);
            w.var_i32(0);
            assert!(matches!(
                BossBar::decode(w.as_slice(), version, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        for id in [-1, 5, 128, i32::MAX] {
            assert!(BossBarDivision::from_id(id).is_err());
            let mut w = Writer::new();
            w.raw(&UUID);
            w.var_i32(4);
            w.var_i32(0);
            w.var_i32(id);
            assert!(matches!(
                BossBar::decode(w.as_slice(), version, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        for id in [-1, 6, 128, i32::MAX] {
            let mut w = Writer::new();
            w.raw(&UUID);
            w.var_i32(id);
            assert!(matches!(
                BossBar::decode(w.as_slice(), version, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
    }
}

#[test]
fn preserves_all_flag_bits_and_ieee754_health_bits() {
    for &version in Version::ALL {
        for flags in 0..=255 {
            let bytes = [UUID.as_slice(), &[5, flags]].concat();
            let decoded = BossBar::decode(&bytes, version, Limits::default()).unwrap();
            assert_eq!(
                decoded.action,
                BossBarAction::UpdateFlags(BossBarFlags(flags))
            );
            assert_eq!(decoded.encode(version, Limits::default()).unwrap(), bytes);
            let flags = BossBarFlags(flags);
            assert_eq!(flags.darken_sky(), flags.0 & 1 != 0);
            assert_eq!(flags.play_boss_music(), flags.0 & 2 != 0);
            assert_eq!(flags.create_fog(), flags.0 & 4 != 0);
        }
        for bits in [
            0_u32,
            0x8000_0000,
            0x3fc0_0000,
            0xbf80_0000,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_1234,
            0x7fa0_0001,
        ] {
            for action in [0, 2] {
                let mut bytes = boss_fixture(version, action);
                let offset = 17
                    + if action == 0 {
                        text_bytes(version, b'x').len()
                    } else {
                        0
                    };
                bytes[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
                let decoded = BossBar::decode(&bytes, version, Limits::default()).unwrap();
                let health = match decoded.action {
                    BossBarAction::Add { health, .. } | BossBarAction::UpdateHealth(health) => {
                        health
                    }
                    _ => unreachable!(),
                };
                assert_eq!(health.to_bits(), bits);
                assert_eq!(decoded.encode(version, Limits::default()).unwrap(), bytes);
            }
        }
    }
}

#[test]
fn exact_packet_budgets_and_bounded_stream_reads() {
    for &version in Version::ALL {
        for action in 0..=5 {
            let bytes = boss_fixture(version, action);
            let exact = Limits {
                max_packet: bytes.len(),
                ..Limits::default()
            };
            assert!(BossBar::decode(&bytes, version, exact).is_ok());
            assert!(expected_boss(version, action)
                .encode(version, exact)
                .is_ok());
            let short = Limits {
                max_packet: bytes.len() - 1,
                ..exact
            };
            assert!(BossBar::decode(&bytes, version, short).is_err());
            assert!(expected_boss(version, action)
                .encode(version, short)
                .is_err());
            let mut r = Reader::new(&bytes, short);
            assert!(BossBar::read(&mut r, version).is_err());
            assert_eq!(r.position(), 0);
            let repeated = [bytes.as_slice(), bytes.as_slice()].concat();
            let mut r = Reader::new(&repeated, exact);
            assert!(BossBar::read(&mut r, version).is_ok());
            assert_eq!(r.position(), bytes.len());
        }
        let bytes = [text_bytes(version, b'x'), text_bytes(version, b'y')].concat();
        let value = PlayerListHeaderFooter::decode(&bytes, version, Limits::default()).unwrap();
        let exact = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        assert!(value.encode(version, exact).is_ok());
        let short = Limits {
            max_packet: bytes.len() - 1,
            ..exact
        };
        assert!(PlayerListHeaderFooter::decode(&bytes, version, short).is_err());
        assert!(value.encode(version, short).is_err());
        assert!(PlayerListHeaderFooter::read(&mut Reader::new(&bytes, short), version).is_err());
    }
}

#[test]
fn header_footer_share_nbt_node_budget() {
    for &version in Version::ALL.iter().filter(|v| v.protocol() >= 765) {
        let bytes = [text_bytes(version, b'x'), text_bytes(version, b'y')].concat();
        let value = PlayerListHeaderFooter {
            header: text(version, 'x'),
            footer: text(version, 'y'),
        };
        let limits = Limits {
            max_nbt_nodes: 2,
            ..Limits::default()
        };
        assert!(PlayerListHeaderFooter::decode(&bytes, version, limits).is_ok());
        assert!(value.encode(version, limits).is_ok());
        let limits = Limits {
            max_nbt_nodes: 1,
            ..limits
        };
        assert!(matches!(
            PlayerListHeaderFooter::decode(&bytes, version, limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            value.encode(version, limits),
            Err(Error::Limit(_))
        ));
        let nested = ChatComponent::Nbt(Nbt::anonymous(Tag::Compound(vec![(
            "text".into(),
            Tag::String("x".into()),
        )])));
        let value = PlayerListHeaderFooter {
            header: nested.clone(),
            footer: nested,
        };
        let bytes = value.encode(version, Limits::default()).unwrap();
        let limits = Limits {
            max_nbt_nodes: 4,
            ..Limits::default()
        };
        assert!(PlayerListHeaderFooter::decode(&bytes, version, limits).is_ok());
        assert!(value.encode(version, limits).is_ok());
        let limits = Limits {
            max_nbt_nodes: 3,
            ..limits
        };
        assert!(PlayerListHeaderFooter::decode(&bytes, version, limits).is_err());
        assert!(value.encode(version, limits).is_err());
    }
}

#[test]
fn nbt_depth_collection_and_array_nodes_are_bounded() {
    let version = Version::V1_20_3;
    let value = PlayerListHeaderFooter {
        header: ChatComponent::Nbt(Nbt::anonymous(Tag::List {
            element_type: TagType::String,
            elements: vec![Tag::String("x".into()), Tag::String("y".into())],
        })),
        footer: ChatComponent::Nbt(Nbt::anonymous(Tag::ByteArray(vec![1, 2, 3]))),
    };
    let bytes = value.encode(version, Limits::default()).unwrap();
    for limits in [
        Limits {
            max_nbt_nodes: 6,
            ..Limits::default()
        },
        Limits {
            max_collection: 1,
            ..Limits::default()
        },
        Limits {
            max_nbt_depth: 0,
            ..Limits::default()
        },
    ] {
        assert!(PlayerListHeaderFooter::decode(&bytes, version, limits).is_err());
        assert!(value.encode(version, limits).is_err());
    }
    let limits = Limits {
        max_nbt_nodes: 7,
        ..Limits::default()
    };
    assert!(PlayerListHeaderFooter::decode(&bytes, version, limits).is_ok());
    assert!(value.encode(version, limits).is_ok());
}

#[test]
fn component_representation_boundary_and_absent_nbt_rejected() {
    for &version in Version::ALL {
        let wrong = if version.protocol() < 765 {
            text(Version::V1_20_3, 'x')
        } else {
            text(Version::V1_20, 'x')
        };
        let boss = BossBar {
            uuid: UUID,
            action: BossBarAction::UpdateTitle(wrong.clone()),
        };
        assert!(boss.encode(version, Limits::default()).is_err());
        for value in [
            PlayerListHeaderFooter {
                header: wrong.clone(),
                footer: text(version, 'x'),
            },
            PlayerListHeaderFooter {
                header: text(version, 'x'),
                footer: wrong,
            },
        ] {
            let mut w = Writer::new();
            w.u8(77);
            assert!(value.write(&mut w, version, Limits::default()).is_err());
            assert_eq!(w.as_slice(), [77]);
        }
        if version.protocol() >= 765 {
            assert!(BossBar::decode(
                &[UUID.as_slice(), &[3, 0]].concat(),
                version,
                Limits::default()
            )
            .is_err());
            assert!(PlayerListHeaderFooter::decode(&[0, 0], version, Limits::default()).is_err());
            assert!(PlayerListHeaderFooter::decode(
                &[text_bytes(version, b'x'), vec![0]].concat(),
                version,
                Limits::default()
            )
            .is_err());
        }
    }
}

#[test]
fn malformed_varints_json_and_nbt_fail_closed() {
    for &version in Version::ALL {
        for varint in [&[0x80; 6][..], &[0xff, 0xff, 0xff, 0xff, 0x10][..]] {
            assert!(BossBar::decode(
                &[UUID.as_slice(), varint].concat(),
                version,
                Limits::default()
            )
            .is_err());
            assert!(BossBar::decode(
                &[UUID.as_slice(), &[4], varint, &[0]].concat(),
                version,
                Limits::default()
            )
            .is_err());
        }
        let invalid_components = if version.protocol() < 765 {
            vec![
                vec![1, 0xff],
                vec![0xff, 0xff, 0xff, 0xff, 0x7],
                vec![0xff, 0xff, 0xff, 0xff, 0x0f],
            ]
        } else {
            vec![
                vec![13],
                vec![8, 0, 1, 0xff],
                vec![9, 8, 0xff, 0xff, 0xff, 0xff],
            ]
        };
        for bad in invalid_components {
            assert!(BossBar::decode(
                &[UUID.as_slice(), &[3], bad.as_slice()].concat(),
                version,
                Limits::default()
            )
            .is_err());
            assert!(PlayerListHeaderFooter::decode(
                &[bad, text_bytes(version, b'x')].concat(),
                version,
                Limits::default()
            )
            .is_err());
        }
    }
}

#[test]
fn json_is_lossless_and_character_limit_counts_utf16() {
    for version in [Version::V1_20, Version::V1_20_2] {
        // The envelope intentionally does not parse JSON or render components.
        let value = PlayerListHeaderFooter {
            header: ChatComponent::Json("😀".into()),
            footer: ChatComponent::Json("".into()),
        };
        let limits = Limits {
            max_string_chars: 2,
            ..Limits::default()
        };
        let bytes = value.encode(version, limits).unwrap();
        assert_eq!(
            PlayerListHeaderFooter::decode(&bytes, version, limits).unwrap(),
            value
        );
        let limits = Limits {
            max_string_chars: 1,
            ..limits
        };
        assert!(value.encode(version, limits).is_err());
        assert!(PlayerListHeaderFooter::decode(&bytes, version, limits).is_err());
        let body = BossBar {
            uuid: UUID,
            action: BossBarAction::UpdateTitle(ChatComponent::Json("x".repeat(262_145))),
        };
        assert!(body
            .encode(
                version,
                Limits {
                    max_string_chars: usize::MAX / 3,
                    ..Limits::default()
                }
            )
            .is_err());
    }
}

#[test]
fn single_byte_mutations_never_panic_and_accepted_bodies_reencode() {
    let limits = Limits {
        max_packet: 256,
        max_string_chars: 32,
        max_collection: 32,
        max_nbt_nodes: 32,
        max_nbt_depth: 8,
        ..Limits::default()
    };
    for &version in Version::ALL {
        for action in 0..=5 {
            let original = boss_fixture(version, action);
            for index in 0..original.len() {
                for mask in [1, 0x80, 0xff] {
                    let mut bytes = original.clone();
                    bytes[index] ^= mask;
                    if let Ok(value) = BossBar::decode(&bytes, version, limits) {
                        let encoded = value.encode(version, limits).unwrap();
                        assert!(BossBar::decode(&encoded, version, limits).is_ok());
                    }
                }
            }
        }
        let original = [text_bytes(version, b'x'), text_bytes(version, b'y')].concat();
        for index in 0..original.len() {
            for mask in [1, 0x80, 0xff] {
                let mut bytes = original.clone();
                bytes[index] ^= mask;
                if let Ok(value) = PlayerListHeaderFooter::decode(&bytes, version, limits) {
                    let encoded = value.encode(version, limits).unwrap();
                    assert!(PlayerListHeaderFooter::decode(&encoded, version, limits).is_ok());
                }
            }
        }
    }
}
