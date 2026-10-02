use rustwire_mc::{
    codec::BlockPosition,
    packet::{blocks::*, DimensionRef},
    version::{Direction, State},
    Limits, Version,
};
// Independent fixture construction, without calling Rustwire's Writer or codecs.
fn vint(b: &mut Vec<u8>, n: u64) {
    let mut n = n;
    loop {
        let low = (n & 127) as u8;
        n >>= 7;
        b.push(low | if n != 0 { 128 } else { 0 });
        if n == 0 {
            break;
        }
    }
}
fn string(b: &mut Vec<u8>, s: &str) {
    vint(b, s.len() as u64);
    b.extend(s.as_bytes());
}
fn position(x: i32, y: i32, z: i32) -> [u8; 8] {
    ((((x as u64) & 0x3ffffff) << 38) | (((z as u64) & 0x3ffffff) << 12) | ((y as u64) & 0xfff))
        .to_be_bytes()
}
fn fixture(name: &str, v: Version) -> Vec<u8> {
    let mut b = vec![];
    match name {
        "block_change" => {
            b.extend(position(-33_554_432, -64, 33_554_431));
            vint(&mut b, 22_000);
        }
        "multi_block_change" => {
            let section = ((-10i64 as u64 & 0x3fffff) << 42)
                | ((22u64 & 0x3fffff) << 20)
                | (-4i64 as u64 & 0xfffff);
            b.extend(section.to_be_bytes());
            vint(&mut b, 2);
            vint(&mut b, (22_000 << 12) | (15 << 8) | (10 << 4) | 1);
            // A real VarLong, larger than the 32-bit fixture-schema alias can encode.
            vint(&mut b, ((i32::MAX as u64) << 12) | (5 << 8) | 15);
        }
        "unload_chunk" => {
            if v.protocol() == 763 {
                b.extend((-100i32).to_be_bytes());
                b.extend(200i32.to_be_bytes());
            } else {
                b.extend(200i32.to_be_bytes());
                b.extend((-100i32).to_be_bytes());
            }
        }
        "respawn" => {
            if v.protocol() >= 766 {
                vint(&mut b, 42);
            } else {
                string(&mut b, "minecraft:overworld");
            }
            string(&mut b, "minecraft:the_nether");
            b.extend((-9i64).to_be_bytes());
            b.extend([1, 255, 0, 1]);
            if v.protocol() == 763 {
                b.push(3);
            }
            b.push(1);
            string(&mut b, "minecraft:overworld");
            b.extend(position(-17, 64, 25));
            vint(&mut b, 300);
            if v.protocol() >= 768 {
                vint(&mut b, 63);
            }
            if v.protocol() != 763 {
                b.push(3);
            }
        }
        _ => unreachable!(),
    }
    b
}
const NAMES: &[&str] = &[
    "block_change",
    "multi_block_change",
    "unload_chunk",
    "respawn",
];
#[test]
fn independent_block_and_respawn_fixtures_all_14_families() {
    for &v in Version::ALL {
        for &name in NAMES {
            let bytes = fixture(name, v);
            let decoded = WorldPacket::decode(name, &bytes, v, Limits::default()).unwrap();
            let encoded = decoded.encode(v, Limits::default()).unwrap();
            assert_eq!(encoded.data, bytes, "{v} {name}");
            assert_eq!(
                encoded.id,
                v.packet_id(State::Play, Direction::Clientbound, name)
                    .unwrap()
            );
        }
        let unload =
            UnloadChunk::decode(&fixture("unload_chunk", v), v, Limits::default()).unwrap();
        assert_eq!(unload, UnloadChunk { x: -100, z: 200 });
        let respawn = Respawn::decode(&fixture("respawn", v), v, Limits::default()).unwrap();
        assert_eq!(
            respawn.spawn.sea_level,
            if v.protocol() >= 768 { Some(63) } else { None }
        );
        assert_eq!(
            respawn.spawn.dimension_type,
            if v.protocol() >= 766 {
                DimensionRef::Id(42)
            } else {
                DimensionRef::Name("minecraft:overworld".into())
            }
        );
        assert_eq!(
            respawn.spawn.death.unwrap().position,
            BlockPosition {
                x: -17,
                y: 64,
                z: 25
            }
        );
        let updates =
            SectionBlockUpdates::decode(&fixture("multi_block_change", v), v, Limits::default())
                .unwrap();
        assert_eq!(
            updates.section,
            SectionPosition {
                x: -10,
                y: -4,
                z: 22
            }
        );
        assert_eq!(
            updates.records[0],
            SectionBlockUpdate {
                x: 15,
                y: 1,
                z: 10,
                state_id: 22_000
            }
        );
        assert_eq!(updates.records[1].state_id, i32::MAX);
        assert_eq!(
            updates.records[0].world_position(updates.section).unwrap(),
            BlockPosition {
                x: -145,
                y: -63,
                z: 362
            }
        );
    }
}
#[test]
fn world_fixtures_reject_every_truncation_trailing_bytes_and_exceeded_budgets() {
    for &v in Version::ALL {
        for &name in NAMES {
            let bytes = fixture(name, v);
            for n in 0..bytes.len() {
                assert!(
                    WorldPacket::decode(name, &bytes[..n], v, Limits::default()).is_err(),
                    "{v} {name} prefix {n}"
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(WorldPacket::decode(name, &trailing, v, Limits::default()).is_err());
            let l = Limits {
                max_packet: bytes.len() - 1,
                ..Limits::default()
            };
            assert!(WorldPacket::decode(name, &bytes, v, l).is_err());
            let decoded = WorldPacket::decode(name, &bytes, v, Limits::default()).unwrap();
            assert!(decoded.encode(v, l).is_err());
        }
    }
}
#[test]
fn section_coordinate_boundaries_and_record_limits() {
    for x in [-2_097_152, -1, 0, 2_097_151] {
        for y in [-524_288, -1, 0, 524_287] {
            for z in [-2_097_152, -1, 0, 2_097_151] {
                let p = SectionPosition { x, y, z };
                assert_eq!(SectionPosition::unpack(p.pack().unwrap()), p);
            }
        }
    }
    assert!(SectionPosition {
        x: 2_097_152,
        y: 0,
        z: 0
    }
    .pack()
    .is_err());
    assert!(SectionPosition {
        x: 0,
        y: -524_289,
        z: 0
    }
    .pack()
    .is_err());
    let v = Version::V26_2;
    let l = Limits::default();
    let section = SectionPosition { x: 0, y: 0, z: 0 };
    let record = SectionBlockUpdate {
        x: 15,
        y: 15,
        z: 15,
        state_id: i32::MAX,
    };
    let updates = SectionBlockUpdates {
        section,
        records: vec![record; 4096],
    };
    assert_eq!(
        SectionBlockUpdates::decode(&updates.encode(v, l).unwrap(), v, l).unwrap(),
        updates
    );
    let updates = SectionBlockUpdates {
        section,
        records: vec![record; 4097],
    };
    assert!(updates.encode(v, l).is_err());
    let mut bad = vec![0; 8];
    vint(&mut bad, 4097);
    assert!(SectionBlockUpdates::decode(&bad, v, l).is_err());
    bad.truncate(8);
    bad.push(1);
    vint(&mut bad, (i32::MAX as u64 + 1) << 12);
    assert!(SectionBlockUpdates::decode(&bad, v, l).is_err());
    let bad_record = SectionBlockUpdate { x: 16, ..record };
    assert!(SectionBlockUpdates {
        section,
        records: vec![bad_record]
    }
    .encode(v, l)
    .is_err());
    assert!(SectionBlockUpdates::decode(
        &fixture("multi_block_change", v),
        v,
        Limits {
            max_collection: 1,
            ..l
        }
    )
    .is_err());
}
#[test]
fn malformed_block_and_respawn_fields_fail() {
    let l = Limits::default();
    for &v in Version::ALL {
        let mut b = vec![0; 8];
        b.extend([255, 255, 255, 255, 15]);
        assert!(BlockUpdate::decode(&b, v, l).is_err());
        assert!(BlockUpdate {
            position: BlockPosition {
                x: 0,
                y: 2048,
                z: 0
            },
            state_id: 1
        }
        .encode(v, l)
        .is_err());
        let mut respawn = Respawn::decode(&fixture("respawn", v), v, l).unwrap();
        respawn.spawn.portal_cooldown = -1;
        assert!(respawn.encode(v, l).is_err());
        respawn.spawn.portal_cooldown = 0;
        respawn.spawn.previous_game_mode = -2;
        assert!(respawn.encode(v, l).is_err());
        respawn.spawn.previous_game_mode = -1;
        respawn.spawn.sea_level = if v.protocol() < 768 { Some(63) } else { None };
        assert!(respawn.encode(v, l).is_err());
        if v.protocol() >= 766 {
            let mut b = fixture("respawn", v);
            *b.last_mut().unwrap() = 4;
            assert!(Respawn::decode(&b, v, l).is_err());
        }
        assert!(Respawn::decode(
            &fixture("respawn", v),
            v,
            Limits {
                max_string_chars: 3,
                ..l
            }
        )
        .is_err());
    }
}

#[test]
fn respawn_retains_both_flag_bits_even_in_legacy_protocols() {
    for &v in Version::ALL {
        for flags in 0..=3 {
            let mut bytes = fixture("respawn", v);
            let index = if v.protocol() == 763 {
                // Two ASCII identifier lengths + contents, seed, modes/debug/flat.
                2 + "minecraft:overworld".len() + "minecraft:the_nether".len() + 12
            } else {
                bytes.len() - 1
            };
            bytes[index] = flags;
            let value = Respawn::decode(&bytes, v, Limits::default()).unwrap();
            assert_eq!(
                value.data,
                RespawnData {
                    attributes: flags & 1 != 0,
                    entity_data: flags & 2 != 0,
                }
            );
            assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
            bytes[index] = 4;
            assert!(Respawn::decode(&bytes, v, Limits::default()).is_err());
        }
    }
}
