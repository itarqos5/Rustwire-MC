use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, NbtString, Tag},
    packet::{chat::ChatComponent, hud::*, interact::Hand},
    Error, Limits, Version,
};

fn fixtures() -> Vec<(Version, &'static str, i32, Vec<u8>)> {
    include_str!("fixtures/hud.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let parts: Vec<_> = line.split('\t').collect();
            let bytes = parts[3]
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            (
                Version::from_protocol(parts[0].parse().unwrap()).unwrap(),
                parts[1],
                parts[2].parse().unwrap(),
                bytes,
            )
        })
        .collect()
}
fn text(version: Version, s: &str) -> ChatComponent {
    if version.protocol() < 765 {
        ChatComponent::Json(format!("\"{s}\""))
    } else {
        ChatComponent::Nbt(Nbt::anonymous(Tag::String(s.into())))
    }
}
fn expected(version: Version, name: &str) -> HudPacket {
    match name {
        "clear_titles" => HudPacket::ClearTitles(ClearTitles { reset: true }),
        "action_bar" => HudPacket::ActionBar(ActionBar {
            text: text(version, "a"),
        }),
        "set_title_text" => HudPacket::Title(SetTitleText {
            text: text(version, "t"),
        }),
        "set_title_subtitle" => HudPacket::Subtitle(SetTitleSubtitle {
            text: text(version, "s"),
        }),
        "set_title_time" => HudPacket::TitleTime(SetTitleTime {
            fade_in: -1,
            stay: 70,
            fade_out: 20,
        }),
        "open_book" => HudPacket::OpenBook(OpenBook { hand_id: 1 }),
        "experience" => HudPacket::Experience(Experience {
            bar: 0.5,
            level: 10,
            total: 300,
        }),
        "enter_combat_event" => HudPacket::EnterCombat(EnterCombat),
        "end_combat_event" => HudPacket::EndCombat(EndCombat { duration: 300 }),
        "death_combat_event" => HudPacket::DeathCombat(DeathCombat {
            player_id: 300,
            message: text(version, "d"),
        }),
        _ => panic!("unrecognized fixture"),
    }
}

#[test]
fn independent_hud_fixtures_decode_encode_and_ids_all_families() {
    let rows = fixtures();
    assert_eq!(rows.len(), 140);
    for (v, name, id, bytes) in rows {
        let value = expected(v, name);
        assert_eq!(
            HudPacket::decode(name, &bytes, v, Limits::default()).unwrap(),
            value
        );
        assert_eq!(value.name(), name);
        assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
        let packet = value.packet(v, Limits::default()).unwrap();
        assert_eq!(packet.id, id);
        assert_eq!(packet.data, bytes);
    }
}

#[test]
fn every_fixture_rejects_every_strict_truncation_and_trailing_bytes() {
    for (v, name, _, bytes) in fixtures() {
        for end in 0..bytes.len() {
            assert!(
                HudPacket::decode(name, &bytes[..end], v, Limits::default()).is_err(),
                "{v} {name} {end}"
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(matches!(
            HudPacket::decode(name, &trailing, v, Limits::default()),
            Err(Error::Invalid("trailing bytes"))
        ));
    }
}

#[test]
fn packet_byte_budgets_cover_empty_and_nonempty_bodies_both_ways() {
    for (v, name, _, bytes) in fixtures() {
        let value = expected(v, name);
        for max_packet in 0..bytes.len() {
            let limits = Limits {
                max_packet,
                ..Limits::default()
            };
            assert!(matches!(
                HudPacket::decode(name, &bytes, v, limits),
                Err(Error::Limit(_))
            ));
            assert!(matches!(value.encode(v, limits), Err(Error::Limit(_))));
        }
        let limits = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        assert_eq!(value.encode(v, limits).unwrap(), bytes);
        assert_eq!(HudPacket::decode(name, &bytes, v, limits).unwrap(), value);
    }
}

#[test]
fn stream_reads_and_writes_are_transactional_for_every_type() {
    macro_rules! check {
        ($ty:ty, $v:expr, $bytes:expr) => {{
            let v = $v;
            let bytes = $bytes;
            let value = <$ty>::decode(&bytes, v, Limits::default()).unwrap();
            let mut stream = vec![99];
            stream.extend_from_slice(&bytes);
            stream.push(42);
            let mut r = Reader::new(&stream, Limits::default());
            r.u8().unwrap();
            assert_eq!(<$ty>::read(&mut r, v).unwrap(), value);
            assert_eq!(r.remaining(), &[42]);
            let mut w = Writer::new();
            w.u8(99);
            value.write(&mut w, v, Limits::default()).unwrap();
            assert_eq!(w.as_slice(), &stream[..stream.len() - 1]);
            for end in 0..bytes.len() {
                let mut truncated = vec![99];
                truncated.extend_from_slice(&bytes[..end]);
                let mut r = Reader::new(&truncated, Limits::default());
                r.u8().unwrap();
                assert!(<$ty>::read(&mut r, v).is_err());
                assert_eq!(r.position(), 1);
            }
            for max_packet in 0..bytes.len() {
                let limits = Limits {
                    max_packet,
                    ..Limits::default()
                };
                let mut r = Reader::new(&bytes, limits);
                assert!(<$ty>::read(&mut r, v).is_err());
                assert_eq!(r.position(), 0);
                let mut w = Writer::new();
                w.u8(99);
                assert!(value.write(&mut w, v, limits).is_err());
                assert_eq!(w.as_slice(), &[99]);
            }
        }};
    }
    for (v, name, _, bytes) in fixtures() {
        match name {
            "clear_titles" => check!(ClearTitles, v, bytes),
            "action_bar" => check!(ActionBar, v, bytes),
            "set_title_text" => check!(SetTitleText, v, bytes),
            "set_title_subtitle" => check!(SetTitleSubtitle, v, bytes),
            "set_title_time" => check!(SetTitleTime, v, bytes),
            "open_book" => check!(OpenBook, v, bytes),
            "experience" => check!(Experience, v, bytes),
            "enter_combat_event" => check!(EnterCombat, v, bytes),
            "end_combat_event" => check!(EndCombat, v, bytes),
            "death_combat_event" => check!(DeathCombat, v, bytes),
            _ => unreachable!(),
        }
    }
}

#[test]
fn text_boundary_is_json_through_764_anonymous_nbt_from_765() {
    for name in [
        "action_bar",
        "set_title_text",
        "set_title_subtitle",
        "death_combat_event",
    ] {
        let before = expected(Version::V1_20_2, name);
        let after = expected(Version::V1_20_3, name);
        assert!(before.encode(Version::V1_20_3, Limits::default()).is_err());
        assert!(after.encode(Version::V1_20_2, Limits::default()).is_err());
        let old_bytes = before.encode(Version::V1_20_2, Limits::default()).unwrap();
        let new_bytes = after.encode(Version::V1_20_3, Limits::default()).unwrap();
        assert!(HudPacket::decode(name, &old_bytes, Version::V1_20_3, Limits::default()).is_err());
        assert!(HudPacket::decode(name, &new_bytes, Version::V1_20_2, Limits::default()).is_err());
    }
}

#[test]
fn signed_varint_scalar_extremes_are_preserved() {
    for &v in Version::ALL {
        for (bytes, number) in [
            (&[0][..], 0),
            (&[0xff, 0xff, 0xff, 0xff, 0x0f][..], -1),
            (&[0x80, 0x80, 0x80, 0x80, 0x08][..], i32::MIN),
            (&[0xff, 0xff, 0xff, 0xff, 0x07][..], i32::MAX),
        ] {
            let hand = OpenBook::decode(bytes, v, Limits::default()).unwrap();
            assert_eq!(hand.hand_id, number);
            assert_eq!(hand.encode(v, Limits::default()).unwrap(), bytes);
            let end = EndCombat::decode(bytes, v, Limits::default()).unwrap();
            assert_eq!(end.duration, number);
            assert_eq!(end.encode(v, Limits::default()).unwrap(), bytes);
            let mut death = bytes.to_vec();
            death.extend(if v.protocol() < 765 {
                &[3, b'"', b'x', b'"'][..]
            } else {
                &[8, 0, 1, b'x'][..]
            });
            let value = DeathCombat::decode(&death, v, Limits::default()).unwrap();
            assert_eq!(value.player_id, number);
            assert_eq!(value.encode(v, Limits::default()).unwrap(), death);
            let mut experience = vec![0, 0, 0, 0];
            experience.extend(bytes);
            experience.extend(bytes);
            let value = Experience::decode(&experience, v, Limits::default()).unwrap();
            assert_eq!((value.level, value.total), (number, number));
            assert_eq!(value.encode(v, Limits::default()).unwrap(), experience);
        }
        let bytes = [
            0x80, 0, 0, 0, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ];
        let times = SetTitleTime {
            fade_in: i32::MIN,
            stay: i32::MAX,
            fade_out: -1,
        };
        assert_eq!(
            SetTitleTime::decode(&bytes, v, Limits::default()).unwrap(),
            times
        );
        assert_eq!(times.encode(v, Limits::default()).unwrap(), bytes);
    }
}

#[test]
fn experience_retains_special_float_bits_without_gameplay_clamping() {
    for &v in Version::ALL {
        for (bytes, bits) in [
            ([0, 0, 0, 0], 0),
            ([0x80, 0, 0, 0], 0x8000_0000),
            ([0x7f, 0x80, 0, 0], 0x7f80_0000),
            ([0xff, 0x80, 0, 0], 0xff80_0000),
            ([0x7f, 0xc0, 0x12, 0x34], 0x7fc0_1234),
            ([0x7f, 0x80, 0, 1], 0x7f80_0001),
            ([0x40, 0, 0, 0], 0x4000_0000),
            ([0xbf, 0x80, 0, 0], 0xbf80_0000),
        ] {
            let wire = [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0];
            let value = Experience::decode(&wire, v, Limits::default()).unwrap();
            assert_eq!(value.bar.to_bits(), bits);
            assert_eq!(value.encode(v, Limits::default()).unwrap(), wire);
        }
    }
}

#[test]
fn known_book_hands_are_optional_views_of_raw_ids() {
    for hand in [Hand::Main, Hand::Off] {
        assert_eq!(OpenBook::for_hand(hand).known_hand(), Some(hand));
    }
    for &v in Version::ALL {
        for hand_id in [-1, 2, i32::MIN, i32::MAX] {
            assert_eq!(OpenBook { hand_id }.known_hand(), None);
        }
        assert_eq!(
            OpenBook::decode(&[2], v, Limits::default())
                .unwrap()
                .known_hand(),
            None
        );
        assert_eq!(
            ClearTitles::decode(&[0], v, Limits::default()).unwrap(),
            ClearTitles { reset: false }
        );
        assert!(matches!(
            ClearTitles::decode(&[2], v, Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
}

#[test]
fn malformed_varints_strings_and_nbt_are_errors_not_unsupported() {
    for &v in Version::ALL {
        for bytes in [vec![0x80; 6], vec![0xff, 0xff, 0xff, 0xff, 0x10]] {
            for name in ["open_book", "end_combat_event", "death_combat_event"] {
                let result = HudPacket::decode(name, &bytes, v, Limits::default());
                assert!(result.is_err());
                assert!(!matches!(result, Err(Error::Unsupported(_))));
            }
            let mut experience = vec![0, 0, 0, 0];
            experience.extend(&bytes);
            experience.push(0);
            assert!(Experience::decode(&experience, v, Limits::default()).is_err());
            experience = vec![0, 0, 0, 0, 0];
            experience.extend(&bytes);
            assert!(Experience::decode(&experience, v, Limits::default()).is_err());
        }
        let malformed = if v.protocol() < 765 {
            vec![
                vec![1, 0xff],
                vec![0xff, 0xff, 0xff, 0xff, 0x0f],
                vec![0x80; 6],
            ]
        } else {
            vec![
                vec![0],
                vec![13],
                vec![8, 0, 1, 0xff],
                vec![7, 0xff, 0xff, 0xff, 0xff],
            ]
        };
        for bytes in malformed {
            for name in [
                "action_bar",
                "set_title_text",
                "set_title_subtitle",
                "death_combat_event",
            ] {
                let mut wire = if name == "death_combat_event" {
                    vec![0]
                } else {
                    vec![]
                };
                wire.extend(&bytes);
                let result = HudPacket::decode(name, &wire, v, Limits::default());
                assert!(result.is_err(), "{name} {v}");
                assert!(!matches!(result, Err(Error::Unsupported(_))));
            }
        }
    }
}

#[test]
fn json_is_retained_verbatim_without_semantic_validation() {
    for v in [Version::V1_20, Version::V1_20_2] {
        let wire = b"\x04nope";
        let value = ActionBar {
            text: ChatComponent::Json("nope".into()),
        };
        assert_eq!(
            ActionBar::decode(wire, v, Limits::default()).unwrap(),
            value
        );
        assert_eq!(value.encode(v, Limits::default()).unwrap(), wire);
    }
}

#[test]
fn text_limits_apply_to_every_component_packet() {
    for (v, name, _, bytes) in fixtures().into_iter().filter(|(_, name, _, _)| {
        matches!(
            *name,
            "action_bar" | "set_title_text" | "set_title_subtitle" | "death_combat_event"
        )
    }) {
        let needed_chars = if v.protocol() < 765 { 3 } else { 1 };
        for max_string_chars in 0..needed_chars {
            let limits = Limits {
                max_string_chars,
                ..Limits::default()
            };
            assert!(HudPacket::decode(name, &bytes, v, limits).is_err());
            assert!(expected(v, name).encode(v, limits).is_err());
        }
        let limits = Limits {
            max_string_chars: needed_chars,
            ..Limits::default()
        };
        assert!(HudPacket::decode(name, &bytes, v, limits).is_ok());
        assert!(expected(v, name).encode(v, limits).is_ok());
        if v.protocol() >= 765 {
            let limits = Limits {
                max_nbt_nodes: 0,
                ..Limits::default()
            };
            assert!(HudPacket::decode(name, &bytes, v, limits).is_err());
            assert!(expected(v, name).encode(v, limits).is_err());
        }
    }
}

#[test]
fn nbt_node_collection_depth_and_modified_utf8_budgets() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 765) {
        // Compound preserves duplicate keys and the complete anonymous-root shape.
        let bytes = [10, 8, 0, 1, b'a', 0, 1, b'b', 8, 0, 1, b'a', 0, 1, b'c', 0];
        let value = ActionBar {
            text: ChatComponent::Nbt(Nbt::anonymous(Tag::Compound(vec![
                ("a".into(), Tag::String("b".into())),
                ("a".into(), Tag::String("c".into())),
            ]))),
        };
        assert_eq!(
            ActionBar::decode(&bytes, v, Limits::default()).unwrap(),
            value
        );
        assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
        for limits in [
            Limits {
                max_nbt_nodes: 2,
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
            assert!(ActionBar::decode(&bytes, v, limits).is_err());
            assert!(value.encode(v, limits).is_err());
        }
        let limits = Limits {
            max_nbt_nodes: 3,
            max_collection: 2,
            max_nbt_depth: 1,
            ..Limits::default()
        };
        assert!(ActionBar::decode(&bytes, v, limits).is_ok());
        assert!(value.encode(v, limits).is_ok());
        // The root node and every array element consume the shared NBT budget.
        let bytes = [7, 0, 0, 0, 2, 0xff, 0x7f];
        let value = ActionBar {
            text: ChatComponent::Nbt(Nbt::anonymous(Tag::ByteArray(vec![-1, 127]))),
        };
        let limits = Limits {
            max_nbt_nodes: 2,
            ..Limits::default()
        };
        assert!(ActionBar::decode(&bytes, v, limits).is_err());
        assert!(value.encode(v, limits).is_err());
        let limits = Limits {
            max_nbt_nodes: 3,
            ..Limits::default()
        };
        assert_eq!(ActionBar::decode(&bytes, v, limits).unwrap(), value);
        assert_eq!(value.encode(v, limits).unwrap(), bytes);
        let bytes = [8, 0, 5, 0xc0, 0x80, 0xed, 0xa0, 0x80];
        let value = ActionBar {
            text: ChatComponent::Nbt(Nbt::anonymous(Tag::String(NbtString(vec![0, 0xd800])))),
        };
        assert_eq!(
            ActionBar::decode(&bytes, v, Limits::default()).unwrap(),
            value
        );
        assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
        let limits = Limits {
            max_string_chars: 1,
            ..Limits::default()
        };
        assert!(ActionBar::decode(&bytes, v, limits).is_err());
        assert!(value.encode(v, limits).is_err());
    }
}

#[test]
fn death_message_byte_budget_accounts_for_player_id_prefix() {
    for &v in Version::ALL {
        let value = DeathCombat {
            player_id: i32::MIN,
            message: text(v, "a"),
        };
        let bytes = value.encode(v, Limits::default()).unwrap();
        assert_eq!(bytes.len(), 9);
        let limits = Limits {
            max_packet: 8,
            ..Limits::default()
        };
        assert!(value.encode(v, limits).is_err());
        assert!(DeathCombat::decode(&bytes, v, limits).is_err());
        let mut w = Writer::new();
        w.u8(42);
        assert!(value.write(&mut w, v, limits).is_err());
        assert_eq!(w.as_slice(), &[42]);
    }
}

#[test]
fn unrecognized_family_name_is_unsupported() {
    assert!(matches!(
        HudPacket::decode("future_hud", &[1, 2, 3], Version::V1_20, Limits::default()),
        Err(Error::Unsupported("typed HUD packet"))
    ));
}

#[test]
fn json_utf16_and_hard_component_length_limits_are_enforced() {
    for v in [Version::V1_20, Version::V1_20_2] {
        let wire = [4, 0xf0, 0x9f, 0x98, 0x80];
        let value = ActionBar {
            text: ChatComponent::Json("😀".into()),
        };
        let limits = Limits {
            max_string_chars: 1,
            ..Limits::default()
        };
        assert!(ActionBar::decode(&wire, v, limits).is_err());
        assert!(value.encode(v, limits).is_err());
        let limits = Limits {
            max_string_chars: 2,
            ..Limits::default()
        };
        assert_eq!(ActionBar::decode(&wire, v, limits).unwrap(), value);
        assert_eq!(value.encode(v, limits).unwrap(), wire);
        let limits = Limits {
            max_string_chars: 262_145,
            ..Limits::default()
        };
        // Independently encoded VarInt length 262144 = 80 80 10.
        let mut wire = vec![0x80, 0x80, 0x10];
        wire.resize(3 + 262_144, b'a');
        let value = ActionBar {
            text: ChatComponent::Json("a".repeat(262_144)),
        };
        assert_eq!(ActionBar::decode(&wire, v, limits).unwrap(), value);
        assert_eq!(value.encode(v, limits).unwrap(), wire);
        wire[0] = 0x81;
        wire.push(b'a');
        let value = ActionBar {
            text: ChatComponent::Json("a".repeat(262_145)),
        };
        assert!(ActionBar::decode(&wire, v, limits).is_err());
        assert!(value.encode(v, limits).is_err());
    }
}

#[test]
fn malformed_components_and_wrong_representations_leave_streams_unchanged() {
    for &v in Version::ALL {
        let malformed = if v.protocol() < 765 {
            vec![1, 0xff]
        } else {
            vec![10, 13]
        };
        let mut r = Reader::new(&malformed, Limits::default());
        assert!(ActionBar::read(&mut r, v).is_err());
        assert_eq!(r.position(), 0);
        let value = ActionBar {
            text: if v.protocol() < 765 {
                ChatComponent::Nbt(Nbt::anonymous(Tag::String("x".into())))
            } else {
                ChatComponent::Json("\"x\"".into())
            },
        };
        let mut w = Writer::new();
        w.u8(42);
        assert!(value.write(&mut w, v, Limits::default()).is_err());
        assert_eq!(w.as_slice(), &[42]);
    }
}
