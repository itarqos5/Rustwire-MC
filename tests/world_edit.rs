use rustwire_mc::{
    codec::{BlockPosition, Reader, Writer},
    packet::{typed::DecodedPacket, world_edit::*},
    version::{Direction, State},
    Error, Limits, Version,
};
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
struct Fixture {
    version: Version,
    name: &'static str,
    id: i32,
    case: &'static str,
    body: Vec<u8>,
}
fn fixtures() -> Vec<Fixture> {
    include_str!("fixtures/world-edit.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
        .map(|s| {
            let f: Vec<_> = s.split('\t').collect();
            Fixture {
                version: v(f[0].parse().unwrap()),
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
fn independent_world_edit_fixtures_all_prefixes_ids_and_limits() {
    let rows = fixtures();
    assert_eq!(rows.len(), 196);
    for f in rows {
        let limits = Limits {
            max_packet: f.body.len(),
            max_collection: 0,
            ..Limits::default()
        };
        let p = WorldEditPacket::decode(f.name, &f.body, f.version, limits).unwrap();
        assert_eq!(p.encode(f.version, limits).unwrap(), f.body);
        let raw = p.packet(f.version, limits).unwrap();
        assert_eq!(raw.id, f.id);
        assert_eq!(raw.data, f.body);
        assert_eq!(
            f.version
                .packet_id(State::Play, Direction::Serverbound, f.name)
                .unwrap(),
            f.id
        );
        for end in 0..f.body.len() {
            assert!(WorldEditPacket::decode(f.name, &f.body[..end], f.version, limits).is_err());
        }
        let mut extra = f.body.clone();
        extra.push(0);
        assert!(WorldEditPacket::decode(f.name, &extra, f.version, Limits::default()).is_err());
        let small = Limits {
            max_packet: f.body.len() - 1,
            ..limits
        };
        assert!(p.encode(f.version, small).is_err());
        assert!(WorldEditPacket::decode(f.name, &f.body, f.version, small).is_err());
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
            State::Play,
        ] {
            assert!(
                DecodedPacket::decode(state, f.name, &f.body, f.version, limits)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
#[test]
fn fields_use_wide_seeds_raw_flags_and_unclamped_wire_values() {
    for f in fixtures() {
        let p = WorldEditPacket::decode(f.name, &f.body, f.version, Limits::default()).unwrap();
        match p {
            WorldEditPacket::Structure(p) => {
                let seed = f
                    .case
                    .strip_prefix("seed-")
                    .unwrap()
                    .parse::<i64>()
                    .unwrap();
                assert_eq!(p.seed, seed);
                assert_eq!(p.offset, [-128, -48, 127]);
                assert_eq!(p.size, [-1, 0, 127]);
                assert_eq!(p.metadata, "data 😀");
                assert_eq!(p.name, "unparsed structure 😀");
                let expected = match seed {
                    0 => 0,
                    300 => 0x80000000,
                    1099511627776 => 0x7fc00001,
                    i64::MIN => 0x7f800000,
                    i64::MAX => 0xff800000,
                    _ => panic!(),
                };
                assert_eq!(p.integrity.to_bits(), expected);
                if seed == i64::MAX {
                    assert_eq!(p.flags, 255);
                }
            }
            WorldEditPacket::Jigsaw(p) => {
                assert_eq!(p.priorities.is_some(), f.version.protocol() >= 765);
                if let Some(q) = p.priorities {
                    assert_eq!((q.selection, q.placement), (i32::MIN, i32::MAX));
                }
                if f.case == "raw-joint" {
                    assert_eq!(p.joint_type, "Future Joint 😀");
                    assert_eq!(p.final_state, "unparsed [state]");
                    assert!(p.name.is_empty());
                }
            }
            WorldEditPacket::Command(p) => {
                assert_eq!(p.command, "say Rustwire 😀");
                let n = f
                    .case
                    .strip_prefix("mode-")
                    .unwrap()
                    .parse::<i32>()
                    .unwrap();
                assert_eq!(p.mode as i32, n);
                assert_eq!(p.flags, [0, 7, 255][n as usize]);
            }
            WorldEditPacket::Generate(p) if f.case == "signed" => {
                assert_eq!(p.levels, i32::MIN);
                assert!(p.keep_jigsaws);
            }
            _ => {}
        }
    }
}
#[test]
fn jigsaw_priorities_are_explicitly_versioned() {
    let mut jigsaw = UpdateJigsawBlock {
        position: BlockPosition { x: 0, y: 0, z: 0 },
        name: "a".into(),
        target: "b".into(),
        pool: "c".into(),
        final_state: "air".into(),
        joint_type: "aligned".into(),
        priorities: None,
    };
    let legacy = jigsaw.encode(v(764), Limits::default()).unwrap();
    assert!(jigsaw.encode(v(765), Limits::default()).is_err());
    assert!(UpdateJigsawBlock::decode(&legacy, v(765), Limits::default()).is_err());
    jigsaw.priorities = Some(JigsawPriorities {
        selection: -1,
        placement: 300,
    });
    let modern = jigsaw.encode(v(765), Limits::default()).unwrap();
    assert!(jigsaw.encode(v(764), Limits::default()).is_err());
    assert!(UpdateJigsawBlock::decode(&modern, v(764), Limits::default()).is_err());
}
#[test]
fn metadata_limits_enums_identifiers_and_atomic_io_are_strict() {
    let f = fixtures()
        .into_iter()
        .find(|f| f.name == "update_structure_block")
        .unwrap();
    let mut value = UpdateStructureBlock::decode(&f.body, f.version, Limits::default()).unwrap();
    value.metadata = "😀".repeat(64);
    let b = value.encode(f.version, Limits::default()).unwrap();
    assert_eq!(
        UpdateStructureBlock::decode(&b, f.version, Limits::default())
            .unwrap()
            .metadata,
        value.metadata
    );
    value.metadata.push('x');
    assert!(value.encode(f.version, Limits::default()).is_err());
    for bad in [-1, 4, i32::MAX] {
        assert!(StructureAction::from_id(bad).is_err());
        assert!(StructureMode::from_id(bad).is_err());
        assert!(StructureRotation::from_id(bad).is_err());
    }
    assert!(CommandBlockMode::from_id(3).is_err());
    assert!(StructureMirror::from_id(3).is_err());
    let c = UpdateCommandBlock {
        position: BlockPosition { x: 0, y: 0, z: 0 },
        command: "x".into(),
        mode: CommandBlockMode::Auto,
        flags: 255,
    };
    let mut b = c.encode(v(776), Limits::default()).unwrap();
    b[10] = 3;
    assert!(matches!(
        UpdateCommandBlock::decode(&b, v(776), Limits::default()),
        Err(Error::Invalid("command-block mode"))
    ));
    let mut r = Reader::new(&b, Limits::default());
    assert!(UpdateCommandBlock::read(&mut r, v(776)).is_err());
    assert_eq!(r.position(), 0);
    let mut w = Writer::new();
    w.raw(&[7, 8]);
    assert!(c
        .write(
            &mut w,
            v(776),
            Limits {
                max_packet: 1,
                ..Limits::default()
            }
        )
        .is_err());
    assert_eq!(w.as_slice(), [7, 8]);
    let mut j = UpdateJigsawBlock {
        position: c.position,
        name: "BAD:KEY".into(),
        target: "b".into(),
        pool: "c".into(),
        final_state: "anything".into(),
        joint_type: "Future".into(),
        priorities: Some(JigsawPriorities {
            selection: 0,
            placement: 0,
        }),
    };
    assert!(j.encode(v(776), Limits::default()).is_err());
    j.name = "a".into();
    let b = j.encode(v(776), Limits::default()).unwrap();
    let limits = Limits {
        max_string_chars: 1,
        ..Limits::default()
    };
    assert!(j.encode(v(776), limits).is_err());
    assert!(UpdateJigsawBlock::decode(&b, v(776), limits).is_err());
}
#[test]
fn fixture_mutations_never_panic_and_successful_encodings_stay_stable() {
    for f in fixtures() {
        for i in 0..f.body.len() {
            let mut b = f.body.clone();
            b[i] ^= 255;
            if let Ok(p) = WorldEditPacket::decode(f.name, &b, f.version, Limits::default()) {
                let canonical = p.encode(f.version, Limits::default()).unwrap();
                assert_eq!(
                    WorldEditPacket::decode(f.name, &canonical, f.version, Limits::default())
                        .unwrap()
                        .encode(f.version, Limits::default())
                        .unwrap(),
                    canonical
                );
            }
        }
    }
}
