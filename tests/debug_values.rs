use rustwire_mc::{
    codec::{BlockPosition, Reader, Writer},
    packet::{debug_values::*, typed::DecodedPacket},
    version::{Direction, State},
    Error, Limits, Version,
};
const V: Version = Version::V26_2;
fn pos(x: i32) -> BlockPosition {
    BlockPosition { x, y: -2, z: 3 }
}
fn hex(s: &str) -> Vec<u8> {
    assert_eq!(s.len() % 2, 0);
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
fn node(x: i32, path_type: i32) -> DebugPathNode {
    DebugPathNode {
        x,
        y: i32::MIN,
        z: i32::MAX,
        walked_distance: -0.0,
        cost_malus: f32::INFINITY,
        closed: true,
        path_type: DebugPathType::new(path_type, V).unwrap(),
        f: -3.5,
    }
}
fn path() -> DebugPath {
    DebugPath {
        reached: true,
        next_node_index: -1234,
        target: pos(-3),
        nodes: vec![],
        target_nodes: vec![],
        open_set: vec![],
        closed_set: vec![],
        max_node_distance: f32::NEG_INFINITY,
    }
}
fn brain() -> DebugBrain {
    DebugBrain {
        name: "".into(),
        profession: "".into(),
        xp: i32::MIN,
        health: -0.0,
        max_health: f32::INFINITY,
        inventory: "".into(),
        wants_golem: false,
        anger_level: i32::MAX,
        activities: vec![],
        behaviors: vec![],
        memories: vec![],
        gossips: vec![],
        pois: vec![],
        potential_pois: vec![],
    }
}
fn event(value: DebugValue) -> DebugEvent {
    DebugEvent { value }
}
fn roundtrip(p: DebugEvent) -> Vec<u8> {
    let b = p.encode(V, Limits::default()).unwrap();
    assert_eq!(
        DebugEvent::decode(&b, V, Limits::default())
            .unwrap()
            .encode(V, Limits::default())
            .unwrap(),
        b
    );
    b
}
fn limited(p: DebugEvent, max_collection: usize) {
    let bytes = p.encode(V, Limits::default()).unwrap();
    let limits = Limits {
        max_collection,
        ..Limits::default()
    };
    assert!(matches!(
        DebugEvent::decode(&bytes, V, limits),
        Err(Error::Limit(_))
    ));
    let mut r = Reader::new(&bytes, limits);
    assert!(matches!(DebugEvent::read(&mut r, V), Err(Error::Limit(_))));
    assert_eq!(r.position(), 0);
    let mut w = Writer::new();
    w.u8(99);
    assert!(matches!(p.write(&mut w, V, limits), Err(Error::Limit(_))));
    assert_eq!(w.as_slice(), &[99]);
}
#[test]
fn goal_events_have_list_prefix_and_unit_updates_keep_presence() {
    let goals = event(DebugValue::GoalSelectors(vec![
        DebugGoal {
            priority: 7,
            running: true,
            name: "a".into(),
        },
        DebugGoal {
            priority: 0,
            running: false,
            name: "bc".into(),
        },
    ]));
    assert_eq!(roundtrip(goals), hex("0402070101610000026263"));
    assert_eq!(roundtrip(event(DebugValue::VillageSections)), vec![10]);
    for (update, tail) in [
        (
            DebugUpdate::Removed(DebugValueKind::VillageSections),
            [10, 0],
        ),
        (DebugUpdate::Value(DebugValue::VillageSections), [10, 1]),
    ] {
        let packet = DebugChunkValue {
            position: DebugChunkPosition { x: -2, z: 3 },
            update,
        };
        let mut expected = hex("00000003fffffffe");
        expected.extend(tail);
        assert_eq!(packet.encode(V, Limits::default()).unwrap(), expected);
        assert_eq!(
            DebugChunkValue::decode(&expected, V, Limits::default()).unwrap(),
            packet
        );
    }
}
#[test]
fn packed_chunk_full_signed_coordinates_and_position_range_validation() {
    for x in [i32::MIN, -2, 0, i32::MAX] {
        for z in [i32::MIN, -3, 0, i32::MAX] {
            let p = DebugChunkValue {
                position: DebugChunkPosition { x, z },
                update: DebugUpdate::Value(DebugValue::VillageSections),
            };
            let b = p.encode(V, Limits::default()).unwrap();
            assert_eq!(&b[..4], &z.to_be_bytes());
            assert_eq!(&b[4..8], &x.to_be_bytes());
            assert_eq!(
                DebugChunkValue::decode(&b, V, Limits::default()).unwrap(),
                p
            );
        }
    }
    let p = DebugBlockValue {
        position: pos(i32::MAX),
        update: DebugUpdate::Value(DebugValue::VillageSections),
    };
    let mut w = Writer::new();
    w.u8(42);
    assert!(matches!(
        p.write(&mut w, V, Limits::default()),
        Err(Error::Invalid(_))
    ));
    assert_eq!(w.as_slice(), &[42]);
}
#[test]
fn path_type_boundary_and_decoder_valid_empty_targets() {
    for protocol in 773..=776 {
        let v = Version::from_protocol(protocol).unwrap();
        for id in 0..=25 {
            assert_eq!(DebugPathType::new(id, v).unwrap().id(), id as u8);
        }
        assert_eq!(DebugPathType::new(26, v).is_ok(), protocol >= 775);
        assert!(matches!(
            DebugPathType::new(27, v),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(DebugPathType::new(-1, v), Err(Error::Invalid(_))));
    }
    let mut p = path();
    p.nodes.push(node(1, 26));
    let packet = event(DebugValue::EntityPaths(Box::new(p)));
    let bytes = packet.encode(V, Limits::default()).unwrap();
    for protocol in 773..=776 {
        let v = Version::from_protocol(protocol).unwrap();
        if protocol >= 775 {
            assert_eq!(packet.encode(v, Limits::default()).unwrap(), bytes);
        } else {
            assert!(matches!(
                packet.encode(v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
            let mut r = Reader::new(&bytes, Limits::default());
            assert!(matches!(
                DebugEvent::read(&mut r, v),
                Err(Error::Unsupported(_))
            ));
            assert_eq!(r.position(), 0);
        }
    }
    let empty = roundtrip(event(DebugValue::EntityPaths(Box::new(path()))));
    assert_eq!(empty.len(), 22);
}
#[test]
fn path_wire_list_order_and_duplicates_are_not_normalized() {
    let mut p = path();
    p.nodes = vec![node(11, 0)];
    p.target_nodes = vec![node(22, 25), node(22, 25)];
    p.open_set = vec![node(33, 1), node(34, 2), node(35, 3)];
    p.closed_set = vec![node(44, 4), node(45, 5), node(46, 6), node(47, 7)];
    let b = roundtrip(event(DebugValue::EntityPaths(Box::new(p))));
    // kind + reached + fixed i32 index + packed target, then four list counts.
    let mut i = 14;
    for (count, first_x) in [(1, 11i32), (2, 22), (3, 33), (4, 44)] {
        assert_eq!(b[i], count as u8);
        i += 1;
        assert_eq!(&b[i..i + 4], &first_x.to_be_bytes());
        i += count * 26;
    }
    assert_eq!(i + 4, b.len());
}
#[test]
fn aggregate_lists_charge_siblings_nested_pieces_and_empty_strings() {
    let mut p = path();
    p.nodes = vec![node(1, 0); 2];
    p.target_nodes = vec![node(2, 1); 2];
    p.open_set = vec![node(3, 2); 2];
    p.closed_set = vec![node(4, 3); 2];
    limited(event(DebugValue::EntityPaths(Box::new(p))), 7);
    let piece = DebugStructurePiece {
        min: pos(1),
        max: pos(0),
        start: true,
    };
    let structure = DebugStructure {
        min: pos(2),
        max: pos(-2),
        pieces: vec![piece; 2],
    };
    limited(event(DebugValue::Structures(vec![structure; 2])), 5);
    let mut b = brain();
    b.activities = vec!["".into(); 2];
    b.behaviors = vec!["".into(); 2];
    b.memories = vec!["".into(); 2];
    b.gossips = vec!["".into(); 2];
    b.pois = vec![pos(1)];
    b.potential_pois = vec![pos(1)];
    limited(event(DebugValue::Brains(Box::new(b))), 9);
    limited(
        event(DebugValue::GoalSelectors(vec![
            DebugGoal {
                priority: 0,
                running: false,
                name: "".into()
            };
            2
        ])),
        1,
    );
    limited(event(DebugValue::Raids(vec![pos(0); 2])), 1);
}
#[test]
fn goal_and_brain_string_utf16_boundaries() {
    for name in ["a".repeat(255), "😀".repeat(127) + "a"] {
        let bytes = roundtrip(event(DebugValue::GoalSelectors(vec![DebugGoal {
            priority: -1,
            running: false,
            name,
        }])));
        assert!(DebugEvent::decode(
            &bytes,
            V,
            Limits {
                max_string_chars: 254,
                ..Limits::default()
            }
        )
        .is_err());
    }
    for name in ["a".repeat(256), "😀".repeat(128)] {
        let p = event(DebugValue::GoalSelectors(vec![DebugGoal {
            priority: 0,
            running: false,
            name: name.clone(),
        }]));
        assert!(matches!(
            p.encode(V, Limits::default()),
            Err(Error::Limit(_))
        ));
        // Form invalid UTF-16 lengths independently of the typed encoder.
        let mut w = Writer::new();
        w.raw(&[4, 1, 0, 0]);
        w.var_i32(name.len() as i32);
        w.raw(name.as_bytes());
        assert!(matches!(
            DebugEvent::decode(w.as_slice(), V, Limits::default()),
            Err(Error::Limit(_))
        ));
    }
    let mut b = brain();
    b.name = "a".repeat(32767);
    b.profession = "😀".repeat(16383) + "a";
    let bytes = roundtrip(event(DebugValue::Brains(Box::new(b.clone()))));
    assert!(bytes.len() > 32767);
    b.inventory = "a".repeat(32768);
    assert!(matches!(
        event(DebugValue::Brains(Box::new(b))).encode(V, Limits::default()),
        Err(Error::Limit(_))
    ));
}
#[test]
fn floats_signed_scalars_registry_ids_and_wire_lists_are_lossless() {
    let mut b = brain();
    b.health = f32::from_bits(0x7fc01234);
    b.max_health = f32::from_bits(0xff801234);
    b.pois = vec![pos(-3), pos(2), pos(-3)];
    b.potential_pois = vec![pos(2), pos(2)];
    let bytes = roundtrip(event(DebugValue::Brains(Box::new(b.clone()))));
    let DebugValue::Brains(decoded) = DebugEvent::decode(&bytes, V, Limits::default())
        .unwrap()
        .value
    else {
        panic!()
    };
    assert_eq!(decoded.health.to_bits(), b.health.to_bits());
    assert_eq!(decoded.max_health.to_bits(), b.max_health.to_bits());
    assert_eq!(decoded.pois, b.pois);
    assert_eq!(decoded.potential_pois, b.potential_pois);
    assert_eq!(decoded.xp, i32::MIN);
    assert_eq!(decoded.anger_level, i32::MAX);
    let bytes = roundtrip(event(DebugValue::GameEvents(DebugGameEvent {
        game_event_id: 128,
        x: f64::from_bits(0x7ff812345678abcd),
        y: -0.0,
        z: f64::NEG_INFINITY,
    })));
    assert_eq!(&bytes[..3], &[15, 128, 1]);
    assert_eq!(bytes.len(), 27);
    assert_eq!(&bytes[3..11], &0x7ff812345678abcdu64.to_be_bytes());
    assert_eq!(&bytes[11..19], &0x8000000000000000u64.to_be_bytes());
    let hive = roundtrip(event(DebugValue::BeeHives(DebugHive {
        block_id: 0,
        occupant_count: -1,
        honey_level: i32::MIN,
        sedated: true,
    })));
    assert_eq!(&hive[..2], &[7, 0]);
    assert_eq!(hive.len(), 13);
    let poi = roundtrip(event(DebugValue::Pois(DebugPoi {
        position: pos(1),
        poi_type_id: 0,
        free_ticket_count: -1,
    })));
    assert_eq!(poi[9], 0);
    assert_eq!(poi.len(), 15);
}
#[test]
fn unknown_kinds_are_unsupported_but_malformed_values_remain_errors() {
    for bytes in [vec![0], vec![0, 0], vec![16], vec![127], vec![6, 3]] {
        assert!(matches!(
            DebugEvent::decode(&bytes, V, Limits::default()),
            Err(Error::Unsupported(_))
        ));
        let mut r = Reader::new(&bytes, Limits::default());
        assert!(DebugEvent::read(&mut r, V).is_err());
        assert_eq!(r.position(), 0);
    }
    for kind in [0, 16] {
        for presence in [0, 1] {
            assert!(matches!(
                DebugEntityValue::decode(&[1, kind, presence], V, Limits::default()),
                Err(Error::Unsupported(_))
            ));
        }
    }
    let cases = [
        vec![1, 2],                       // hive optional bool
        vec![3, 2],                       // attack-target optional bool
        vec![4, 1, 0, 2, 0],              // goal running bool
        vec![7, 0, 0, 0, 2],              // hive sedated bool
        vec![9, 48],                      // known fixed orientation table
        vec![9, 255, 255, 255, 255, 15],  // negative orientation
        vec![4, 1, 0, 0, 1, 0xff],        // UTF-8
        vec![11, 255, 255, 255, 255, 15], // negative list count
        vec![15, 255, 255, 255, 255, 15], // negative registry
        vec![7, 255, 255, 255, 255, 15],
        vec![128, 128, 128, 128, 16], // VarInt overflow
    ];
    for bytes in cases {
        assert!(
            matches!(
                DebugEvent::decode(&bytes, V, Limits::default()),
                Err(Error::Invalid(_))
            ),
            "{bytes:?}"
        );
        assert!(matches!(
            DecodedPacket::decode(State::Play, "debug_event", &bytes, V, Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
    assert!(matches!(
        DebugEntityValue::decode(&[1, 10, 2], V, Limits::default()),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        DebugEvent::decode(&[15, 1], V, Limits::default()),
        Err(Error::Eof)
    ));
}
#[test]
fn hostile_declared_counts_and_required_suffixes_fail_before_vec_allocation() {
    let unbounded = Limits {
        max_collection: usize::MAX,
        max_packet: usize::MAX,
        ..Limits::default()
    };
    for kind in [4, 11, 12] {
        let bytes = [kind, 255, 255, 255, 255, 7];
        assert!(matches!(
            DebugEvent::decode(&bytes, V, unbounded),
            Err(Error::Eof)
        ));
        assert!(matches!(
            DebugEvent::decode(&bytes, V, Limits::default()),
            Err(Error::Limit(_))
        ));
    }
    // No target/open/closed counts or terminal max-distance behind the node list.
    let mut bytes = vec![5, 0];
    bytes.extend([0; 12]);
    bytes.push(1);
    bytes.extend([0; 26]);
    assert!(matches!(
        DebugEvent::decode(&bytes, V, unbounded),
        Err(Error::Eof)
    ));
    // A structure's pieces cannot consume the minimum envelope of its sibling.
    let mut bytes = vec![12, 2];
    bytes.extend([0; 16]);
    bytes.push(1);
    bytes.extend([0; 17]);
    assert!(matches!(
        DebugEvent::decode(&bytes, V, unbounded),
        Err(Error::Eof)
    ));
}
#[test]
fn invalid_encode_fields_roll_back_and_old_versions_are_unsupported() {
    for p in [
        event(DebugValue::RedstoneWireOrientations(48)),
        event(DebugValue::BeeHives(DebugHive {
            block_id: u32::MAX,
            occupant_count: 0,
            honey_level: 0,
            sedated: false,
        })),
        event(DebugValue::Pois(DebugPoi {
            position: pos(0),
            poi_type_id: u32::MAX,
            free_ticket_count: 0,
        })),
        event(DebugValue::GameEvents(DebugGameEvent {
            game_event_id: u32::MAX,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        })),
        event(DebugValue::NeighborUpdates(pos(i32::MAX))),
    ] {
        let mut w = Writer::new();
        w.u8(88);
        assert!(matches!(
            p.write(&mut w, V, Limits::default()),
            Err(Error::Invalid(_))
        ));
        assert_eq!(w.as_slice(), &[88]);
    }
    for protocol in 763..=772 {
        let v = Version::from_protocol(protocol).unwrap();
        let mut r = Reader::new(&[10], Limits::default());
        assert!(matches!(
            DebugEvent::read(&mut r, v),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(r.position(), 0);
        assert!(matches!(
            event(DebugValue::VillageSections).encode(v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
    }
    assert!(DecodedPacket::decode(
        State::Configuration,
        "debug_event",
        &[10],
        V,
        Limits::default()
    )
    .unwrap()
    .is_none());
    assert!(
        DecodedPacket::decode(State::Login, "debug_event", &[10], V, Limits::default())
            .unwrap()
            .is_none()
    );
}
#[test]
fn zero_resource_budgets_and_bounded_output() {
    let limits = Limits {
        max_collection: 0,
        max_string_chars: 0,
        ..Limits::default()
    };
    assert_eq!(
        event(DebugValue::VillageSections)
            .encode(V, limits)
            .unwrap(),
        [10]
    );
    assert_eq!(
        event(DebugValue::GoalSelectors(vec![]))
            .encode(V, limits)
            .unwrap(),
        [4, 0]
    );
    assert!(event(DebugValue::Brains(Box::new(brain())))
        .encode(V, limits)
        .is_ok());
    let zero = Limits {
        max_packet: 0,
        ..limits
    };
    assert!(matches!(
        event(DebugValue::VillageSections).encode(V, zero),
        Err(Error::Limit(_))
    ));
    let mut b = brain();
    b.name = "a".repeat(20);
    b.profession = "b".repeat(20);
    b.inventory = "c".repeat(20);
    let p = event(DebugValue::Brains(Box::new(b)));
    let bytes = p.encode(V, Limits::default()).unwrap();
    for n in [0, 19, 40, 60, bytes.len() - 1] {
        let small = Limits {
            max_packet: n,
            ..Limits::default()
        };
        assert!(matches!(p.encode(V, small), Err(Error::Limit(_))));
        assert!(matches!(
            DebugEvent::decode(&bytes, V, small),
            Err(Error::Limit(_))
        ));
    }
}

macro_rules! fixture {
    ($ty:ty, $variant:ident, $v:expr, $bytes:expr, $id:expr, $prefixes:expr) => {{
        let v = $v;
        let bytes: &[u8] = $bytes;
        let limits = Limits::default();
        let p = <$ty>::decode(bytes, v, limits).unwrap();
        assert_eq!(p.encode(v, limits).unwrap(), bytes);
        assert_eq!(p.packet(v, limits).unwrap().id, $id);
        assert_eq!(p.packet(v, limits).unwrap().data, bytes);
        for end in 0..bytes.len() {
            assert!(<$ty>::decode(&bytes[..end], v, limits).is_err());
            let mut r = Reader::new(&bytes[..end], limits);
            assert!(<$ty>::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
            let mut prefixed = vec![42];
            prefixed.extend(&bytes[..end]);
            let mut r = Reader::new(&prefixed, limits);
            r.u8().unwrap();
            assert!(<$ty>::read(&mut r, v).is_err());
            assert_eq!(r.position(), 1);
            $prefixes += 1;
        }
        let mut extra = bytes.to_vec();
        extra.push(0xa5);
        assert!(matches!(
            <$ty>::decode(&extra, v, limits),
            Err(Error::Invalid(_))
        ));
        let mut r = Reader::new(&extra, limits);
        assert_eq!(
            <$ty>::read(&mut r, v).unwrap().encode(v, limits).unwrap(),
            bytes
        );
        assert_eq!(r.remaining(), &[0xa5]);
        let mut prefixed = vec![42];
        prefixed.extend(bytes);
        prefixed.push(43);
        let mut r = Reader::new(&prefixed, limits);
        r.u8().unwrap();
        assert_eq!(
            <$ty>::read(&mut r, v).unwrap().encode(v, limits).unwrap(),
            bytes
        );
        assert_eq!(r.remaining(), &[43]);
        let exact = Limits {
            max_packet: bytes.len(),
            ..limits
        };
        assert_eq!(p.encode(v, exact).unwrap(), bytes);
        assert_eq!(
            <$ty>::decode(bytes, v, exact)
                .unwrap()
                .encode(v, exact)
                .unwrap(),
            bytes
        );
        let small = Limits {
            max_packet: bytes.len() - 1,
            ..limits
        };
        assert!(matches!(p.encode(v, small), Err(Error::Limit(_))));
        assert!(matches!(
            <$ty>::decode(bytes, v, small),
            Err(Error::Limit(_))
        ));
        let mut w = Writer::new();
        w.u8(42);
        assert!(p.write(&mut w, v, small).is_err());
        assert_eq!(w.as_slice(), &[42]);
        p.write(&mut w, v, limits).unwrap();
        assert_eq!(&w.as_slice()[1..], bytes);
        DebugValuePacket::$variant(p)
    }};
}
#[test]
fn independently_encoded_fixture_matrix_all_prefixes_trailing_bytes_ids_and_atomic_io() {
    let mut total = 0;
    let mut prefixes = 0;
    let mut inventory = std::collections::BTreeSet::new();
    for line in include_str!("fixtures/debug-values.tsv")
        .lines()
        .filter(|s| !s.starts_with('#') && !s.is_empty())
    {
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 5);
        let v = Version::from_protocol(f[0].parse().unwrap()).unwrap();
        let bytes = hex(f[4]);
        let id: i32 = f[3].parse().unwrap();
        assert_eq!(
            v.packet_id(State::Play, Direction::Clientbound, f[1])
                .unwrap(),
            id
        );
        let p = match f[1] {
            "debug_block_value" => fixture!(DebugBlockValue, Block, v, &bytes, id, prefixes),
            "debug_chunk_value" => fixture!(DebugChunkValue, Chunk, v, &bytes, id, prefixes),
            "debug_entity_value" => fixture!(DebugEntityValue, Entity, v, &bytes, id, prefixes),
            "debug_event" => fixture!(DebugEvent, Event, v, &bytes, id, prefixes),
            _ => panic!("unexpected fixture packet"),
        };
        match &p {
            DebugValuePacket::Block(v) => {
                assert_eq!(v.position, BlockPosition { x: -2, y: 64, z: 3 })
            }
            DebugValuePacket::Chunk(v) => {
                assert_eq!(v.position, DebugChunkPosition { x: -2, z: 3 })
            }
            DebugValuePacket::Entity(v) => assert_eq!(v.entity_id, -1),
            DebugValuePacket::Event(_) => {}
        }
        let parts: Vec<_> = f[2].split('_').collect();
        let expected_kind: i32 = parts[1].parse().unwrap();
        let (kind, removed) = match &p {
            DebugValuePacket::Event(v) => (v.value.kind() as i32, false),
            _ => {
                let update = match &p {
                    DebugValuePacket::Block(v) => &v.update,
                    DebugValuePacket::Chunk(v) => &v.update,
                    DebugValuePacket::Entity(v) => &v.update,
                    _ => unreachable!(),
                };
                match update {
                    DebugUpdate::Removed(k) => (*k as i32, true),
                    DebugUpdate::Value(v) => (v.kind() as i32, false),
                }
            }
        };
        let actual = match &p {
            DebugValuePacket::Event(v) => Some(&v.value),
            _ => {
                let update = match &p {
                    DebugValuePacket::Block(v) => &v.update,
                    DebugValuePacket::Chunk(v) => &v.update,
                    DebugValuePacket::Entity(v) => &v.update,
                    _ => unreachable!(),
                };
                match update {
                    DebugUpdate::Value(v) => Some(v),
                    DebugUpdate::Removed(_) => None,
                }
            }
        };
        if let Some(actual) = actual {
            let label = parts[2..].join("_");
            let expected = fixture_value(expected_kind, &label);
            assert_eq!(
                event(actual.clone()).encode(v, Limits::default()).unwrap(),
                event(expected).encode(v, Limits::default()).unwrap(),
                "{}",
                f[2]
            );
            if label == "rich" {
                let aggregate = match expected_kind {
                    2 => Some(9),
                    5 => Some(10),
                    12 => Some(4),
                    _ => None,
                };
                if let Some(aggregate) = aggregate {
                    let event = event(actual.clone());
                    let body = event.encode(v, Limits::default()).unwrap();
                    let exact = Limits {
                        max_collection: aggregate,
                        ..Limits::default()
                    };
                    assert!(DebugEvent::decode(&body, v, exact).is_ok());
                    assert_eq!(event.encode(v, exact).unwrap(), body);
                    limited(event, aggregate - 1);
                }
            }
        }
        assert_eq!(kind, expected_kind);
        assert_eq!(removed, parts[0] == "removed");
        let typed = DecodedPacket::decode(State::Play, f[1], &bytes, v, Limits::default()).unwrap();
        assert!(matches!(typed, Some(DecodedPacket::DebugValue(_))));
        inventory.insert((v.protocol(), f[1].to_string(), kind, removed));
        total += 1;
    }
    for protocol in 773..=776 {
        for name in [
            "debug_event",
            "debug_block_value",
            "debug_chunk_value",
            "debug_entity_value",
        ] {
            for kind in 1..=15 {
                assert!(inventory.contains(&(protocol, name.into(), kind, false)));
                if name != "debug_event" {
                    assert!(inventory.contains(&(protocol, name.into(), kind, true)));
                }
            }
        }
    }
    assert_eq!(total, 588);
    assert_eq!(prefixes, 23780);
    eprintln!("Verified {total} independent debug-value rows and {prefixes} strict body prefixes");
}

#[test]
fn every_nested_boolean_is_strict_and_remains_transactional() {
    let mut bad_brain = roundtrip(event(DebugValue::Brains(Box::new(brain()))));
    bad_brain[16] = 2;
    let mut p = path();
    p.nodes.push(node(1, 0));
    let mut bad_node = roundtrip(event(DebugValue::EntityPaths(Box::new(p))));
    bad_node[35] = 2;
    let mut bad_path = roundtrip(event(DebugValue::EntityPaths(Box::new(path()))));
    bad_path[1] = 2;
    let mut bad_piece = vec![12, 1];
    bad_piece.extend([0; 16]);
    bad_piece.push(1);
    bad_piece.extend([0; 16]);
    bad_piece.push(2);
    for bytes in [
        bad_brain,
        bad_node,
        bad_path,
        bad_piece,
        vec![1, 0, 2],
        vec![3, 0, 2],
    ] {
        assert!(
            matches!(
                DebugEvent::decode(&bytes, V, Limits::default()),
                Err(Error::Invalid(_))
            ),
            "{bytes:?}"
        );
        let mut r = Reader::new(&bytes, Limits::default());
        assert!(matches!(
            DebugEvent::read(&mut r, V),
            Err(Error::Invalid(_))
        ));
        assert_eq!(r.position(), 0);
    }
}
#[test]
fn direct_registry_extremes_and_signed_diagnostics_do_not_become_enums() {
    for id in [0, 127, 128, 16383, 16384, i32::MAX as u32] {
        roundtrip(event(DebugValue::BeeHives(DebugHive {
            block_id: id,
            occupant_count: -1,
            honey_level: i32::MIN,
            sedated: false,
        })));
        roundtrip(event(DebugValue::Pois(DebugPoi {
            position: pos(0),
            poi_type_id: id,
            free_ticket_count: i32::MIN,
        })));
        roundtrip(event(DebugValue::GameEvents(DebugGameEvent {
            game_event_id: id,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        })));
    }
    for n in [i32::MIN, -1, 0, i32::MAX] {
        roundtrip(event(DebugValue::Bees(DebugBee {
            hive: None,
            flower: None,
            travel_ticks: n,
            blacklisted_hives: vec![],
        })));
        roundtrip(event(DebugValue::Breezes(DebugBreeze {
            attack_target: Some(n),
            jump_target: None,
        })));
        roundtrip(event(DebugValue::GameEventListeners { listener_radius: n }));
        let p = DebugEntityValue {
            entity_id: n,
            update: DebugUpdate::Removed(DebugValueKind::VillageSections),
        };
        let bytes = p.encode(V, Limits::default()).unwrap();
        assert_eq!(
            DebugEntityValue::decode(&bytes, V, Limits::default())
                .unwrap()
                .entity_id,
            n
        );
    }
    assert!(std::mem::size_of::<DebugValue>() <= 80);
}

// Semantic expectations transcribed from the independent Python fixture inputs,
// not from the Rust reader. This catches symmetric reader/writer field swaps.
fn fixture_value(kind: i32, label: &str) -> DebugValue {
    let a = BlockPosition { x: -1, y: 64, z: 2 };
    let b = BlockPosition { x: -2, y: 70, z: 3 };
    let zero = BlockPosition { x: 0, y: 0, z: 0 };
    let mut value = match kind {
        1 => DebugValue::Bees(DebugBee {
            hive: Some(a),
            flower: None,
            travel_ticks: -7,
            blacklisted_hives: vec![a, b],
        }),
        2 => DebugValue::Brains(Box::new(DebugBrain {
            name: "Villager".into(),
            profession: "Farmer test".into(),
            xp: -1,
            health: f32::from_bits(0x40600000),
            max_health: f32::from_bits(0x41a00000),
            inventory: "slot 0: wheat".into(),
            wants_golem: true,
            anger_level: i32::MIN,
            activities: vec!["a".into(), "".into()],
            behaviors: vec!["look 👀".into()],
            memories: vec!["m1".into(), "m2".into()],
            gossips: vec!["g".into()],
            pois: vec![a, a],
            potential_pois: vec![b],
        })),
        3 => DebugValue::Breezes(DebugBreeze {
            attack_target: Some(-3),
            jump_target: Some(b),
        }),
        4 => DebugValue::GoalSelectors(vec![
            DebugGoal {
                priority: 7,
                running: true,
                name: "a".into(),
            },
            DebugGoal {
                priority: 0,
                running: false,
                name: "bc".into(),
            },
        ]),
        5 => {
            let n = |i: i32| DebugPathNode {
                x: 100 + i,
                y: -200 - i,
                z: 300 + i,
                walked_distance: f32::from_bits(0x3fc00000),
                cost_malus: f32::from_bits(0xbe800000),
                closed: i % 2 == 1,
                path_type: DebugPathType::new(i, V).unwrap(),
                f: f32::from_bits(0x3e000000),
            };
            let first = n(2);
            let mut duplicate = n(3);
            duplicate.x = first.x;
            duplicate.y = first.y;
            duplicate.z = first.z;
            DebugValue::EntityPaths(Box::new(DebugPath {
                reached: false,
                next_node_index: -123,
                target: a,
                nodes: vec![n(1)],
                target_nodes: vec![first, duplicate],
                open_set: vec![n(4), n(5), n(6)],
                closed_set: vec![n(7), n(8), n(9), n(10)],
                max_node_distance: f32::from_bits(0x3f000000),
            }))
        }
        6 => DebugValue::EntityBlockIntersections(DebugIntersection::InAir),
        7 => DebugValue::BeeHives(DebugHive {
            block_id: 128,
            occupant_count: -2,
            honey_level: 7,
            sedated: true,
        }),
        8 => DebugValue::Pois(DebugPoi {
            position: a,
            poi_type_id: 300,
            free_ticket_count: -1,
        }),
        9 => DebugValue::RedstoneWireOrientations(47),
        10 => DebugValue::VillageSections,
        11 => DebugValue::Raids(vec![a, b, a]),
        12 => DebugValue::Structures(vec![
            DebugStructure {
                min: a,
                max: b,
                pieces: vec![
                    DebugStructurePiece {
                        min: b,
                        max: a,
                        start: true,
                    },
                    DebugStructurePiece {
                        min: a,
                        max: b,
                        start: false,
                    },
                ],
            },
            DebugStructure {
                min: BlockPosition {
                    x: -33554432,
                    y: -2048,
                    z: -33554432,
                },
                max: BlockPosition {
                    x: 33554431,
                    y: 2047,
                    z: 33554431,
                },
                pieces: vec![],
            },
        ]),
        13 => DebugValue::GameEventListeners {
            listener_radius: -1,
        },
        14 => DebugValue::NeighborUpdates(a),
        15 => DebugValue::GameEvents(DebugGameEvent {
            game_event_id: 128,
            x: f64::from_bits(0x3ff4000000000000),
            y: f64::from_bits(0xc004000000000000),
            z: f64::from_bits(0x400e000000000000),
        }),
        _ => panic!("unsupported fixture kind"),
    };
    match label {
        "rich" => {}
        "empty" => match &mut value {
            DebugValue::Bees(v) => {
                v.hive = None;
                v.flower = None;
                v.travel_ticks = 0;
                v.blacklisted_hives.clear();
            }
            DebugValue::Brains(v) => {
                **v = brain();
                v.xp = 0;
                v.health = 0.0;
                v.max_health = 0.0;
                v.anger_level = 0;
            }
            DebugValue::Breezes(v) => {
                v.attack_target = None;
                v.jump_target = None;
            }
            DebugValue::GoalSelectors(v) => v.clear(),
            DebugValue::EntityPaths(v) => {
                **v = path();
                v.reached = false;
                v.next_node_index = 0;
                v.target = zero;
                v.max_node_distance = 0.0;
            }
            DebugValue::Raids(v) => v.clear(),
            DebugValue::Structures(v) => v.clear(),
            _ => panic!("unexpected empty fixture"),
        },
        "float_bits" => match &mut value {
            DebugValue::Brains(v) => {
                v.health = f32::from_bits(0x7fc01234);
                v.max_health = -0.0;
            }
            DebugValue::EntityPaths(v) => {
                v.nodes[0].walked_distance = -0.0;
                v.nodes[0].cost_malus = f32::from_bits(0x7fc01234);
                v.nodes[0].f = f32::INFINITY;
                v.max_node_distance = f32::NEG_INFINITY;
            }
            DebugValue::GameEvents(v) => {
                v.x = -0.0;
                v.y = f64::INFINITY;
                v.z = f64::from_bits(0x7ff8000000001234);
            }
            _ => panic!("unexpected float fixture"),
        },
        "path_type_26" => {
            let DebugValue::EntityPaths(v) = &mut value else {
                panic!()
            };
            v.nodes[0].x = i32::MIN;
            v.nodes[0].y = i32::MAX;
            v.nodes[0].z = -1;
            v.nodes[0].path_type = DebugPathType::new(26, V).unwrap();
        }
        _ => panic!("unknown fixture label {label}"),
    }
    value
}
