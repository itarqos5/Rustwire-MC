use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, Tag},
    packet::{
        advancements::*,
        chat::ChatComponent,
        inventory::{Component, ComponentPatch, ComponentValue, ItemData, ItemStack, Slot},
    },
    Error, Limits, Version,
};

fn fixtures() -> Vec<(Version, &'static str, &'static str, Vec<u8>)> {
    include_str!("fixtures/advancements.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            let bytes = fields[5]
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            (
                Version::from_protocol(fields[0].parse().unwrap()).unwrap(),
                fields[1],
                fields[4],
                bytes,
            )
        })
        .collect()
}
fn full(v: Version) -> Vec<u8> {
    fixtures()
        .into_iter()
        .find(|(version, _, case, _)| *version == v && *case == "full")
        .unwrap()
        .3
}
fn text(v: Version, s: &str) -> ChatComponent {
    if v.protocol() < 765 {
        ChatComponent::Json(format!("\"{s}\""))
    } else {
        ChatComponent::Nbt(Nbt::anonymous(Tag::String(s.into())))
    }
}
fn empty(v: Version) -> Advancements {
    Advancements {
        reset: false,
        added: vec![],
        removed: vec![],
        progress: vec![],
        show_advancements: (v.protocol() >= 770).then_some(false),
    }
}
fn expected(v: Version) -> Advancements {
    let nbt = Nbt {
        name: (v.protocol() == 763).then(|| "".into()),
        root: Tag::Compound(vec![("k".into(), Tag::String("v".into()))]),
    };
    let item = ItemStack {
        item_id: 300,
        count: if v.protocol() >= 775 { 0 } else { 3 },
        data: if v.protocol() <= 765 {
            ItemData::Legacy(Some(nbt))
        } else {
            ItemData::Components(ComponentPatch {
                added: vec![Component {
                    name: "custom_data",
                    value: ComponentValue::Nbt(nbt),
                }],
                removed: vec![],
            })
        },
    };
    Advancements {
        reset: true,
        added: vec![
            Advancement {
                id: "a:root".into(),
                parent_id: Some("a:p".into()),
                display: Some(AdvancementDisplay {
                    title: text(v, "T"),
                    description: text(v, "D"),
                    icon: if v.protocol() >= 775 {
                        AdvancementIcon::Template(item)
                    } else {
                        AdvancementIcon::Slot(Slot::Item(item))
                    },
                    frame: AdvancementFrame::Challenge,
                    flags: 0x8000_0007,
                    background_texture: Some("a:bg".into()),
                    x: 1.5,
                    y: -2.0,
                }),
                criteria: (v.protocol() == 763)
                    .then(|| vec!["First Criterion!?".into(), "b".into(), "c".into()]),
                requirements: vec![
                    vec!["First Criterion!?".into()],
                    vec!["b".into(), "c".into()],
                    vec![],
                ],
                sends_telemetry_data: true,
            },
            Advancement {
                id: "a:child".into(),
                parent_id: None,
                display: None,
                criteria: (v.protocol() == 763).then(Vec::new),
                requirements: vec![],
                sends_telemetry_data: false,
            },
        ],
        removed: vec!["a:old".into(), "a:old".into()],
        progress: vec![
            AdvancementProgress {
                id: "a:root".into(),
                criteria: vec![
                    CriterionProgress {
                        criterion: "First Criterion!?".into(),
                        achieved_at: Some(300),
                    },
                    CriterionProgress {
                        criterion: "b".into(),
                        achieved_at: None,
                    },
                    CriterionProgress {
                        criterion: "First Criterion!?".into(),
                        achieved_at: Some(-1),
                    },
                ],
            },
            AdvancementProgress {
                id: "a:child".into(),
                criteria: vec![],
            },
        ],
        show_advancements: (v.protocol() >= 770).then_some(true),
    }
}
fn decode(name: &str, bytes: &[u8], v: Version, limits: Limits) -> rustwire_mc::Result<Vec<u8>> {
    if name == "advancement_tab" {
        AdvancementTab::decode(bytes, v, limits)?.encode(v, limits)
    } else {
        AdvancementPacket::decode(name, bytes, v, limits)?.encode(v, limits)
    }
}

#[test]
fn original_fixtures_match_independent_expected_values_all_fourteen_families() {
    for (v, name, case, bytes) in fixtures() {
        match case {
            "full" => assert_eq!(
                Advancements::decode(&bytes, v, Limits::default()).unwrap(),
                expected(v)
            ),
            "empty" => assert_eq!(
                Advancements::decode(&bytes, v, Limits::default()).unwrap(),
                empty(v)
            ),
            "selected" => assert_eq!(
                SelectAdvancementTab::decode(&bytes, v, Limits::default())
                    .unwrap()
                    .id
                    .as_deref(),
                Some("a:root")
            ),
            "clear" => assert_eq!(
                SelectAdvancementTab::decode(&bytes, v, Limits::default()).unwrap(),
                SelectAdvancementTab::default()
            ),
            "opened" => assert_eq!(
                AdvancementTab::decode(&bytes, v, Limits::default()).unwrap(),
                AdvancementTab::Opened("a:root".into())
            ),
            "closed" => assert_eq!(
                AdvancementTab::decode(&bytes, v, Limits::default()).unwrap(),
                AdvancementTab::Closed
            ),
            _ => panic!("unknown fixture case"),
        }
        assert_eq!(
            decode(name, &bytes, v, Limits::default()).unwrap(),
            bytes,
            "{v} {case}"
        );
    }
    assert_eq!(fixtures().len(), 84);
}

#[test]
fn every_fixture_truncation_and_trailing_byte_is_an_error() {
    for (v, name, case, bytes) in fixtures() {
        for end in 0..bytes.len() {
            assert!(
                decode(name, &bytes[..end], v, Limits::default()).is_err(),
                "{v} {case} {end}"
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(
            decode(name, &trailing, v, Limits::default()).is_err(),
            "{v} {case}"
        );
    }
}

#[test]
fn exact_packet_budgets_and_stream_transactions() {
    for &v in Version::ALL {
        let bytes = full(v);
        let value = expected(v);
        for max_packet in 0..bytes.len() {
            let limits = Limits {
                max_packet,
                ..Limits::default()
            };
            assert!(Advancements::decode(&bytes, v, limits).is_err());
            assert!(value.encode(v, limits).is_err());
            let mut r = Reader::new(&bytes, limits);
            assert!(Advancements::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
            let mut w = Writer::new();
            w.u8(42);
            assert!(value.write(&mut w, v, limits).is_err());
            assert_eq!(w.as_slice(), [42]);
        }
        let exact = Limits {
            max_packet: bytes.len(),
            ..Limits::default()
        };
        assert_eq!(value.encode(v, exact).unwrap(), bytes);
        assert_eq!(Advancements::decode(&bytes, v, exact).unwrap(), value);
        let combined = [&[77], bytes.as_slice(), &[88]].concat();
        let mut r = Reader::new(&combined, exact);
        assert_eq!(r.u8().unwrap(), 77);
        assert_eq!(Advancements::read(&mut r, v).unwrap(), value);
        assert_eq!(r.remaining(), [88]);
        let mut r = Reader::new(&combined[..combined.len() - 2], exact);
        r.u8().unwrap();
        assert!(Advancements::read(&mut r, v).is_err());
        assert_eq!(r.position(), 1);
        let mut w = Writer::new();
        w.u8(77);
        value.write(&mut w, v, exact).unwrap();
        assert_eq!(w.as_slice(), [&[77], bytes.as_slice()].concat());
    }
}

#[test]
fn packet_collection_budget_covers_every_nested_list_and_icon() {
    for &v in Version::ALL {
        // Added definitions (2), icon (1), groups (3), requirement names (3),
        // removed IDs (2), progress entries (2), progress criteria (3), plus
        // 763 criteria names (3) or component-patch entries (1 at 766+).
        let total = 16
            + if v.protocol() == 763 {
                3
            } else if v.protocol() >= 766 {
                1
            } else {
                0
            };
        let bytes = full(v);
        let value = expected(v);
        for max_collection in 0..total {
            let limits = Limits {
                max_collection,
                ..Limits::default()
            };
            assert!(
                matches!(
                    Advancements::decode(&bytes, v, limits),
                    Err(Error::Limit(_))
                ),
                "{v} {max_collection}"
            );
            assert!(
                matches!(value.encode(v, limits), Err(Error::Limit(_))),
                "{v} {max_collection}"
            );
        }
        let limits = Limits {
            max_collection: total,
            ..Limits::default()
        };
        assert_eq!(Advancements::decode(&bytes, v, limits).unwrap(), value);
        assert_eq!(value.encode(v, limits).unwrap(), bytes);
    }
}

#[test]
fn nbt_budget_is_shared_across_text_and_icon_in_both_directions() {
    for &v in Version::ALL {
        let bytes = full(v);
        let value = expected(v);
        let total = if v.protocol() < 765 { 2 } else { 4 };
        for max_nbt_nodes in 0..total {
            let limits = Limits {
                max_nbt_nodes,
                ..Limits::default()
            };
            assert!(
                Advancements::decode(&bytes, v, limits).is_err(),
                "{v} {max_nbt_nodes}"
            );
            assert!(value.encode(v, limits).is_err());
        }
        let exact = Limits {
            max_nbt_nodes: total,
            ..Limits::default()
        };
        assert_eq!(Advancements::decode(&bytes, v, exact).unwrap(), value);
        assert_eq!(value.encode(v, exact).unwrap(), bytes);
        let depth = Limits {
            max_nbt_depth: 0,
            ..Limits::default()
        };
        assert!(Advancements::decode(&bytes, v, depth).is_err());
        assert!(value.encode(v, depth).is_err());
    }
}

#[test]
fn all_changed_version_shapes_are_explicit() {
    for (a, b) in [(763, 764), (764, 765), (765, 766), (769, 770)] {
        let a = Version::from_protocol(a).unwrap();
        let b = Version::from_protocol(b).unwrap();
        assert!(
            Advancements::decode(&full(a), b, Limits::default()).is_err(),
            "{a} -> {b}"
        );
        assert!(
            Advancements::decode(&full(b), a, Limits::default()).is_err(),
            "{b} -> {a}"
        );
        assert!(expected(a).encode(b, Limits::default()).is_err());
        assert!(expected(b).encode(a, Limits::default()).is_err());
    }
    // At 775 two valid VarInts swap meanings. Both byte strings can parse in
    // the other release; exact Version selection must determine their meaning.
    let old = Version::from_protocol(774).unwrap();
    let new = Version::from_protocol(775).unwrap();
    let as_new = Advancements::decode(&full(old), new, Limits::default()).unwrap();
    assert!(matches!(&as_new.added[0].display.as_ref().unwrap().icon,
        AdvancementIcon::Template(item) if item.item_id == 3 && item.count == 300));
    let as_old = Advancements::decode(&full(new), old, Limits::default()).unwrap();
    assert!(matches!(&as_old.added[0].display.as_ref().unwrap().icon,
        AdvancementIcon::Slot(Slot::Item(item)) if item.item_id == 0 && item.count == 300));
    assert!(expected(old).encode(new, Limits::default()).is_err());
    assert!(expected(new).encode(old, Limits::default()).is_err());
    // The 766 signed-byte count and 767 VarInt count agree for small positive
    // counts; the independently assembled extended-count fixture below does not.
    let mut b = full(Version::V1_21);
    b.splice(23..24, [0x80, 1]);
    let value = Advancements::decode(&b, Version::V1_21, Limits::default()).unwrap();
    assert!(value.encode(Version::V1_20_5, Limits::default()).is_err());
    assert!(Advancements::decode(&b, Version::V1_20_5, Limits::default()).is_err());
}

#[test]
fn malformed_booleans_frames_actions_text_and_identifiers_fail() {
    for &v in Version::ALL {
        for offset in [0, 9, 14] {
            let mut bytes = full(v);
            bytes[offset] = 2;
            assert!(Advancements::decode(&bytes, v, Limits::default()).is_err());
        }
        let frame = if matches!(v.protocol(), 764 | 765) {
            36
        } else {
            38
        };
        for bad in [3, 127, 255] {
            let mut bytes = full(v);
            bytes[frame] = bad;
            assert!(
                Advancements::decode(&bytes, v, Limits::default()).is_err(),
                "{v} frame {bad}"
            );
        }
        for bytes in [
            vec![2],
            vec![255, 255, 255, 255, 15],
            vec![255, 255, 255, 255, 16],
            vec![1, 0],
        ] {
            assert!(AdvancementTab::decode(&bytes, v, Limits::default()).is_err());
        }
        assert!(SelectAdvancementTab::decode(&[2], v, Limits::default()).is_err());
        for bytes in [
            vec![1, 1, b'A'],
            vec![1, 1, 255],
            vec![1, 3, b'a', b':', b':'],
        ] {
            assert!(SelectAdvancementTab::decode(&bytes, v, Limits::default()).is_err());
        }
        let mut bytes = full(v);
        bytes[3] = b'A';
        assert!(Advancements::decode(&bytes, v, Limits::default()).is_err());
        let mut bytes = full(v);
        bytes[15] = if v.protocol() < 765 { 255 } else { 0 };
        assert!(Advancements::decode(&bytes, v, Limits::default()).is_err());
        let mut value = expected(v);
        value.added[0].display.as_mut().unwrap().flags &= !1;
        assert!(matches!(
            value.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        value = expected(v);
        value.added[0].display.as_mut().unwrap().background_texture = None;
        assert!(matches!(
            value.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        value = expected(v);
        value.added[0].criteria = if v.protocol() == 763 {
            None
        } else {
            Some(vec![])
        };
        assert!(matches!(
            value.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        value = expected(v);
        value.show_advancements = if v.protocol() >= 770 {
            None
        } else {
            Some(false)
        };
        assert!(matches!(
            value.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
}

#[test]
fn negative_overflowing_and_impossible_counts_reject_before_allocation() {
    for &v in Version::ALL {
        let mut requirement_prefix = vec![0, 1, 1, b'a', 0, 0];
        if v.protocol() == 763 {
            requirement_prefix.push(0);
        }
        for bad in [
            &[255, 255, 255, 255, 15][..],
            &[255, 255, 255, 255, 7],
            &[255, 255, 255, 255, 16],
        ] {
            for prefix in [
                vec![0],
                requirement_prefix.clone(),
                [requirement_prefix.as_slice(), &[1]].concat(),
                vec![0, 0],
                vec![0, 0, 0],
                vec![0, 0, 0, 1, 1, b'a'],
            ] {
                let bytes = [prefix.as_slice(), bad].concat();
                assert!(Advancements::decode(
                    &bytes,
                    v,
                    Limits {
                        max_collection: usize::MAX,
                        ..Limits::default()
                    }
                )
                .is_err());
            }
            if v.protocol() == 763 {
                let bytes = [&[0, 1, 1, b'a', 0, 0][..], bad].concat();
                assert!(Advancements::decode(
                    &bytes,
                    v,
                    Limits {
                        max_collection: usize::MAX,
                        ..Limits::default()
                    }
                )
                .is_err());
            }
        }
    }
}

#[test]
fn opaque_values_and_optional_variants_roundtrip_without_state_inference() {
    for &v in Version::ALL {
        for frame in [
            AdvancementFrame::Task,
            AdvancementFrame::Challenge,
            AdvancementFrame::Goal,
        ] {
            let mut value = expected(v);
            let display = value.added[0].display.as_mut().unwrap();
            display.frame = frame;
            display.flags = 0xffff_fffe;
            display.background_texture = None;
            display.x = f32::from_bits(0x7fc0_0123);
            display.y = f32::NEG_INFINITY;
            if v.protocol() < 775 {
                display.icon = AdvancementIcon::Slot(Slot::Empty);
            }
            let bytes = value.encode(v, Limits::default()).unwrap();
            let got = Advancements::decode(&bytes, v, Limits::default()).unwrap();
            let got_display = got.added[0].display.as_ref().unwrap();
            assert_eq!(got_display.x.to_bits(), 0x7fc0_0123);
            assert_eq!(got_display.y, f32::NEG_INFINITY);
            assert_eq!(got_display.flags, 0xffff_fffe);
            assert_eq!(got_display.frame, frame);
            assert_eq!(got.encode(v, Limits::default()).unwrap(), bytes);
        }
        for timestamp in [i64::MIN, -1, 0, 1, i64::MAX] {
            let mut value = empty(v);
            value.progress.push(AdvancementProgress {
                id: "a:x".into(),
                criteria: vec![CriterionProgress {
                    criterion: "not an identifier!?".into(),
                    achieved_at: Some(timestamp),
                }],
            });
            let bytes = value.encode(v, Limits::default()).unwrap();
            assert_eq!(
                Advancements::decode(&bytes, v, Limits::default()).unwrap(),
                value
            );
        }
    }
}

#[test]
fn identifier_boundary_and_string_limits_are_shared() {
    for &v in Version::ALL {
        for id in ["", ":path", "plain/path", "minecraft:path"] {
            let value = SelectAdvancementTab {
                id: Some(id.into()),
            };
            let bytes = value.encode(v, Limits::default()).unwrap();
            assert_eq!(
                SelectAdvancementTab::decode(&bytes, v, Limits::default()).unwrap(),
                value
            );
        }
        let value = SelectAdvancementTab {
            id: Some("..:x".into()),
        };
        let bytes = [1, 4, b'.', b'.', b':', b'x'];
        assert_eq!(
            value.encode(v, Limits::default()).is_ok(),
            v.protocol() < 775
        );
        assert_eq!(
            SelectAdvancementTab::decode(&bytes, v, Limits::default()).is_ok(),
            v.protocol() < 775
        );
        let limits = Limits {
            max_string_chars: 16,
            ..Limits::default()
        };
        assert!(expected(v).encode(v, limits).is_err());
        assert!(Advancements::decode(&full(v), v, limits).is_err());
    }
}

#[test]
fn unsupported_icon_never_skips_payload_or_mutates_stream() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        let mut bytes = full(v);
        // Replace custom_data's known ID with a deliberately unknown component.
        bytes.splice(28..29, [255, 255, 255, 255, 7]);
        assert!(matches!(
            Advancements::decode(&bytes, v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
        let prefixed = [&[42], bytes.as_slice()].concat();
        let mut r = Reader::new(&prefixed, Limits::default());
        r.u8().unwrap();
        assert!(matches!(
            Advancements::read(&mut r, v),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(r.position(), 1);
        // Unsupported is intentional even with arbitrary remainder: components
        // have no length prefix, so subsequent fields cannot safely be scanned.
        bytes.truncate(33);
        assert!(matches!(
            Advancements::decode(&bytes, v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
    }
}

#[test]
fn malformed_known_icon_is_not_unsupported() {
    for &v in Version::ALL {
        let mut bytes = full(v);
        let root = if v.protocol() <= 765 { 27 } else { 29 };
        bytes[root] = 255;
        assert!(matches!(
            Advancements::decode(&bytes, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        if v.protocol() >= 766 {
            let mut bytes = full(v);
            bytes[29] = 0;
            assert!(matches!(
                Advancements::decode(&bytes, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
    }
}

#[test]
fn nested_icon_items_obey_shared_element_and_depth_budgets() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        let child = ItemStack {
            item_id: 1,
            count: 1,
            data: ItemData::Components(ComponentPatch::default()),
        };
        let parent = ItemStack {
            item_id: 2,
            count: 1,
            data: ItemData::Components(ComponentPatch {
                added: vec![Component {
                    name: "bundle_contents",
                    value: if v.protocol() < 775 {
                        ComponentValue::Items(vec![Slot::Item(child)])
                    } else {
                        ComponentValue::ItemTemplates(vec![child])
                    },
                }],
                removed: vec![],
            }),
        };
        let mut value = expected(v);
        value.added[0].display.as_mut().unwrap().icon = if v.protocol() < 775 {
            AdvancementIcon::Slot(Slot::Item(parent))
        } else {
            AdvancementIcon::Template(parent)
        };
        let bytes = value.encode(v, Limits::default()).unwrap();
        assert_eq!(
            Advancements::decode(&bytes, v, Limits::default()).unwrap(),
            value
        );
        for limits in [
            Limits {
                max_nbt_depth: 0,
                ..Limits::default()
            },
            Limits {
                max_collection: 18,
                ..Limits::default()
            },
        ] {
            assert!(matches!(value.encode(v, limits), Err(Error::Limit(_))));
            assert!(matches!(
                Advancements::decode(&bytes, v, limits),
                Err(Error::Limit(_))
            ));
        }
        let exact = Limits {
            max_nbt_depth: 1,
            max_collection: 19,
            ..Limits::default()
        };
        assert_eq!(value.encode(v, exact).unwrap(), bytes);
        assert_eq!(Advancements::decode(&bytes, v, exact).unwrap(), value);
    }
}

#[test]
fn tab_streams_and_byte_limits_are_transactional() {
    for &v in Version::ALL {
        let mut r = Reader::new(&[9, 0, 8], Limits::default());
        r.u8().unwrap();
        assert_eq!(
            SelectAdvancementTab::read(&mut r, v).unwrap(),
            SelectAdvancementTab::default()
        );
        assert_eq!(r.remaining(), [8]);
        let mut r = Reader::new(&[9, 1, 8], Limits::default());
        r.u8().unwrap();
        assert_eq!(
            AdvancementTab::read(&mut r, v).unwrap(),
            AdvancementTab::Closed
        );
        assert_eq!(r.remaining(), [8]);
        for max_packet in 0..8 {
            let limits = Limits {
                max_packet,
                ..Limits::default()
            };
            let mut w = Writer::new();
            w.u8(9);
            assert!(SelectAdvancementTab {
                id: Some("a:root".into())
            }
            .write(&mut w, v, limits)
            .is_err());
            assert_eq!(w.as_slice(), [9]);
            assert!(AdvancementTab::Opened("a:root".into())
                .write(&mut w, v, limits)
                .is_err());
            assert_eq!(w.as_slice(), [9]);
        }
        let mut w = Writer::new();
        SelectAdvancementTab::default()
            .write(&mut w, v, Limits::default())
            .unwrap();
        AdvancementTab::Closed
            .write(&mut w, v, Limits::default())
            .unwrap();
        assert_eq!(w.as_slice(), [0, 1]);
        for bytes in [&[1, 9][..], &[2]] {
            let mut r = Reader::new(bytes, Limits::default());
            assert!(SelectAdvancementTab::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
        }
        for bytes in [&[0, 9][..], &[2]] {
            let mut r = Reader::new(bytes, Limits::default());
            assert!(AdvancementTab::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
        }
    }
}

#[test]
fn remaining_presence_and_telemetry_booleans_are_strict() {
    for &v in Version::ALL {
        let suffix: &[u8] = if v.protocol() >= 770 { &[0] } else { &[] };
        let mut telemetry = vec![0, 1, 1, b'a', 0, 0];
        if v.protocol() == 763 {
            telemetry.push(0);
        }
        telemetry.extend([0, 2, 0, 0]);
        telemetry.extend(suffix);
        assert!(matches!(
            Advancements::decode(&telemetry, v, Limits::default()),
            Err(Error::Invalid("boolean"))
        ));
        let progress = [&[0, 0, 0, 1, 1, b'a', 1, 1, b'A', 2][..], suffix].concat();
        assert!(matches!(
            Advancements::decode(&progress, v, Limits::default()),
            Err(Error::Invalid("boolean"))
        ));
        if v.protocol() >= 770 {
            assert!(matches!(
                Advancements::decode(&[0, 0, 0, 0, 2], v, Limits::default()),
                Err(Error::Invalid("boolean"))
            ));
        }
    }
}
