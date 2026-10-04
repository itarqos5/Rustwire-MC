use rustwire_mc::{
    codec::{BlockPosition, Reader, Writer},
    packet::{
        editing::*,
        inventory::{ComponentPatch, ItemData, ItemStack, Slot},
        typed::DecodedPacket,
    },
    version::{Direction, State},
    Error, Limits, Version,
};
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
struct Fixture {
    v: Version,
    name: &'static str,
    id: i32,
    case: &'static str,
    body: Vec<u8>,
}
fn fixtures() -> Vec<Fixture> {
    include_str!("fixtures/editing.tsv")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let f: Vec<_> = line.split('\t').collect();
            Fixture {
                v: v(f[0].parse().unwrap()),
                name: f[1],
                id: f[2].parse().unwrap(),
                case: f[3],
                body: (0..f[4].len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&f[4][i..i + 2], 16).unwrap())
                    .collect(),
            }
        })
        .collect()
}
#[test]
fn independent_fixtures_packet_ids_prefixes_and_exact_budgets() {
    let rows = fixtures();
    assert_eq!(rows.len(), 217);
    for f in rows {
        let limits = Limits {
            max_packet: f.body.len(),
            ..Limits::default()
        };
        let p = EditingPacket::decode(f.name, &f.body, f.v, limits).unwrap();
        assert_eq!(
            p.encode(f.v, limits).unwrap(),
            f.body,
            "{} {}",
            f.name,
            f.case
        );
        let raw = p.packet(f.v, limits).unwrap();
        assert_eq!(raw.id, f.id);
        assert_eq!(raw.data, f.body);
        assert_eq!(
            f.v.packet_id(State::Play, Direction::Serverbound, f.name)
                .unwrap(),
            f.id
        );
        for end in 0..f.body.len() {
            assert!(
                EditingPacket::decode(f.name, &f.body[..end], f.v, limits).is_err(),
                "{} {} at {end}",
                f.name,
                f.case
            );
        }
        let mut extra = f.body.clone();
        extra.push(0);
        assert!(EditingPacket::decode(f.name, &extra, f.v, Limits::default()).is_err());
        let small = Limits {
            max_packet: f.body.len() - 1,
            ..limits
        };
        assert!(p.encode(f.v, small).is_err());
        assert!(EditingPacket::decode(f.name, &f.body, f.v, small).is_err());
        // These helpers serialize serverbound intent and do not enter the clientbound dispatcher.
        for state in [State::Login, State::Configuration, State::Play] {
            assert!(DecodedPacket::decode(state, f.name, &f.body, f.v, limits)
                .unwrap()
                .is_none());
        }
    }
}
#[test]
fn fixtures_have_independently_expected_semantics() {
    for f in fixtures() {
        let packet = EditingPacket::decode(f.name, &f.body, f.v, Limits::default()).unwrap();
        match (f.name, f.case, packet) {
            ("update_sign", "unicode", EditingPacket::Sign(p)) => {
                assert_eq!(
                    p.position,
                    BlockPosition {
                        x: -33554432,
                        y: -2048,
                        z: 33554431
                    }
                );
                assert!(!p.front);
                assert_eq!(p.lines, ["hello", "😀", "nul\0", ""]);
            }
            ("edit_book", "signed", EditingPacket::Book(p)) => {
                assert_eq!(p.slot, 40);
                assert_eq!(p.pages, ["a", "😀", ""]);
                assert_eq!(p.title.as_deref(), Some("Title"));
            }
            ("set_beacon_effect", "present-negative", EditingPacket::Beacon(p)) => {
                assert_eq!(p.primary, Some(0));
                assert_eq!(p.secondary, Some(-1));
            }
            ("query_entity_nbt", "bounds", EditingPacket::EntityQuery(p)) => {
                assert_eq!(p.transaction_id, i32::MIN);
                assert_eq!(p.entity_id, i32::MAX);
            }
            ("set_creative_slot", "empty", EditingPacket::Creative(p)) => {
                assert_eq!(p.slot, -1);
                assert!(matches!(
                    p.item,
                    CreativeItem::Unframed(Slot::Empty)
                        | CreativeItem::Framed(UntrustedSlot::Empty)
                ));
            }
            ("set_creative_slot", "plain", EditingPacket::Creative(p)) => {
                assert_eq!(p.slot, i16::MIN);
                let (id, count) = match p.item {
                    CreativeItem::Unframed(Slot::Item(p)) => (p.item_id, p.count),
                    CreativeItem::Framed(UntrustedSlot::Item(p)) => (p.item_id, p.count),
                    _ => panic!(),
                };
                assert_eq!(id, 5);
                assert_eq!(count, if f.v.protocol() < 766 { 64 } else { 300 });
            }
            ("set_creative_slot", "opaque-framed", EditingPacket::Creative(p)) => {
                let CreativeItem::Framed(UntrustedSlot::Item(item)) = p.item else {
                    panic!()
                };
                assert_eq!(
                    item.added[0],
                    FramedComponent {
                        type_id: i32::MAX,
                        data: vec![0, 255, 1]
                    }
                );
                assert_eq!(
                    item.added[1],
                    FramedComponent {
                        type_id: 3,
                        data: vec![]
                    }
                );
                assert_eq!(item.removed, [4, 4]);
            }
            _ => {}
        }
    }
}
#[test]
fn book_limits_change_at_768_and_utf16_limits_are_exact() {
    for &version in Version::ALL {
        let (pages, chars, title) = if version.protocol() < 768 {
            (200, 8192, 128)
        } else {
            (100, 1024, 32)
        };
        for (field, limit) in [(0, pages), (1, chars), (2, title)] {
            let mut book = EditBook {
                slot: i32::MIN,
                pages: vec![],
                title: None,
            };
            match field {
                0 => book.pages = vec![String::new(); limit],
                1 => book.pages = vec!["😀".repeat(limit / 2)],
                _ => book.title = Some("😀".repeat(limit / 2)),
            };
            let bytes = book.encode(version, Limits::default()).unwrap();
            assert_eq!(
                EditBook::decode(&bytes, version, Limits::default()).unwrap(),
                book
            );
            match field {
                0 => book.pages.push(String::new()),
                1 => book.pages[0].push('x'),
                _ => book.title.as_mut().unwrap().push('x'),
            };
            assert!(book.encode(version, Limits::default()).is_err());
        }
        let sign = UpdateSign {
            position: BlockPosition { x: 0, y: 0, z: 0 },
            front: true,
            lines: [
                "😀".repeat(192),
                String::new(),
                String::new(),
                String::new(),
            ],
        };
        let b = sign.encode(version, Limits::default()).unwrap();
        assert_eq!(
            UpdateSign::decode(&b, version, Limits::default()).unwrap(),
            sign
        );
        let mut invalid = sign;
        invalid.lines[0].push('x');
        assert!(invalid.encode(version, Limits::default()).is_err());
        // Read-side explicit limit checks are independent of the encoder.
        let mut w = Writer::new();
        w.var_i32(0);
        w.var_i32((pages + 1) as i32);
        for _ in 0..=pages {
            w.u8(0);
        }
        w.bool(false);
        assert!(EditBook::decode(w.as_slice(), version, Limits::default()).is_err());
        let mut w = Writer::new();
        w.var_i32(0);
        w.var_i32(1);
        w.string(&"x".repeat(chars + 1), 32767).unwrap();
        w.bool(false);
        assert!(EditBook::decode(w.as_slice(), version, Limits::default()).is_err());
    }
}
#[test]
fn framing_versions_aggregate_collections_and_opaque_component_bytes() {
    let legacy = SetCreativeSlot {
        slot: -1,
        item: CreativeItem::Unframed(Slot::Item(ItemStack {
            item_id: 5,
            count: 300,
            data: ItemData::Components(ComponentPatch::default()),
        })),
    };
    assert!(legacy.encode(v(769), Limits::default()).is_ok());
    assert!(legacy.encode(v(770), Limits::default()).is_err());
    let framed = SetCreativeSlot {
        slot: -1,
        item: CreativeItem::Framed(UntrustedSlot::Item(UntrustedItemStack {
            item_id: 5,
            count: 300,
            added: vec![FramedComponent {
                type_id: 3,
                data: vec![255; 1000],
            }],
            removed: vec![2],
        })),
    };
    assert!(framed.encode(v(769), Limits::default()).is_err());
    let enough = Limits {
        max_collection: 3,
        ..Limits::default()
    };
    let bytes = framed.encode(v(770), enough).unwrap();
    assert_eq!(
        SetCreativeSlot::decode(&bytes, v(770), enough).unwrap(),
        framed
    );
    // Opaque byte length consumes the packet byte budget, not 1000 collection nodes.
    let small = Limits {
        max_collection: 2,
        ..enough
    };
    assert!(framed.encode(v(770), small).is_err());
    assert!(SetCreativeSlot::decode(&bytes, v(770), small).is_err());
    let book = EditBook {
        slot: 0,
        pages: vec!["a".into(), "b".into()],
        title: None,
    };
    let bytes = book.encode(v(776), Limits::default()).unwrap();
    let limits = Limits {
        max_collection: 1,
        ..Limits::default()
    };
    assert!(book.encode(v(776), limits).is_err());
    assert!(EditBook::decode(&bytes, v(776), limits).is_err());
}
#[test]
fn malformed_known_fields_fail_and_reads_writes_are_transactional() {
    for &version in Version::ALL {
        assert!(SetBeaconEffect::decode(&[2, 0], version, Limits::default()).is_err());
        assert!(NameItem::decode(&[1, 255], version, Limits::default()).is_err());
        let mut sign = vec![0; 13];
        sign[8] = 2;
        assert!(UpdateSign::decode(&sign, version, Limits::default()).is_err());
        assert!(
            EditBook::decode(&[0, 255, 255, 255, 255, 15], version, Limits::default()).is_err()
        );
        let mut reader = Reader::new(&[0, 0, 0], Limits::default());
        assert!(UpdateSign::read(&mut reader, version).is_err());
        assert_eq!(reader.position(), 0);
        let value = NameItem {
            name: "too much".into(),
        };
        let mut writer = Writer::new();
        writer.raw(&[9, 8, 7]);
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
        assert_eq!(writer.as_slice(), [9, 8, 7]);
    }
    for body in [
        vec![0, 0, 255, 255, 255, 255, 15],
        vec![0, 0, 1, 255, 255, 255, 255, 15],
        vec![0, 0, 1, 1, 255, 255, 255, 255, 7, 0],
        vec![0, 0, 1, 1, 1, 0, 3, 255, 255, 255, 255, 7],
    ] {
        assert!(SetCreativeSlot::decode(&body, v(776), Limits::default()).is_err());
    }
    // Unknown unframed component lengths are not guessed in earlier slots.
    assert!(matches!(
        SetCreativeSlot::decode(
            &[0, 0, 1, 1, 1, 0, 255, 255, 255, 255, 7],
            v(769),
            Limits::default()
        ),
        Err(Error::Unsupported(_))
    ));
}
#[test]
fn bounded_fixture_mutations_never_panic_and_successes_are_stable() {
    for f in fixtures() {
        for offset in 0..f.body.len() {
            for mask in [1, 128, 255] {
                let mut bytes = f.body.clone();
                bytes[offset] ^= mask;
                if let Ok(p) = EditingPacket::decode(f.name, &bytes, f.v, Limits::default()) {
                    let canonical = p.encode(f.v, Limits::default()).unwrap();
                    assert_eq!(
                        EditingPacket::decode(f.name, &canonical, f.v, Limits::default()).unwrap(),
                        p
                    );
                }
            }
        }
    }
}

#[test]
fn present_empty_titles_and_tighter_caller_string_limits_are_preserved() {
    for &version in Version::ALL {
        let empty = EditBook {
            slot: 0,
            pages: vec![],
            title: Some(String::new()),
        };
        let bytes = empty.encode(version, Limits::default()).unwrap();
        assert_eq!(bytes, [0, 0, 1, 0]);
        assert_eq!(
            EditBook::decode(&bytes, version, Limits::default()).unwrap(),
            empty
        );
        let title_limit = if version.protocol() < 768 { 128 } else { 32 };
        let mut body = Writer::new();
        body.var_i32(0);
        body.var_i32(0);
        body.bool(true);
        body.string(&"x".repeat(title_limit + 1), 32767).unwrap();
        assert!(EditBook::decode(body.as_slice(), version, Limits::default()).is_err());
        let value = NameItem {
            name: "😀".into()
        };
        let enough = Limits {
            max_string_chars: 2,
            ..Limits::default()
        };
        let bytes = value.encode(version, enough).unwrap();
        assert_eq!(NameItem::decode(&bytes, version, enough).unwrap(), value);
        let small = Limits {
            max_string_chars: 1,
            ..enough
        };
        assert!(value.encode(version, small).is_err());
        assert!(NameItem::decode(&bytes, version, small).is_err());
    }
}
