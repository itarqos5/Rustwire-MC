use rustwire_mc::{
    codec::BlockPosition,
    packet::{
        entity_metadata::particles::{
            particle_id, particle_registry, Particle, ParticleData as D, VibrationDestination,
        },
        entity_metadata::{EntityMetadata, MetadataEntry, MetadataValue},
        inventory::{ComponentPatch, ItemData, ItemStack, Slot},
    },
    Error, Limits, Version,
};
fn vint(b: &mut Vec<u8>, n: i32) {
    let mut n = n as u32;
    loop {
        let byte = (n & 127) as u8;
        n >>= 7;
        b.push(byte | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
fn floats(b: &mut Vec<u8>, values: &[f32]) {
    for n in values {
        b.extend(n.to_be_bytes());
    }
}
fn text(b: &mut Vec<u8>, s: &str) {
    vint(b, s.len() as i32);
    b.extend(s.as_bytes());
}
// Independent registry positions checked against each release's registration order.
// [block, dust, transition, sculk_charge, item, vibration, shriek, trail]
fn ids(v: Version) -> [i32; 8] {
    match v.protocol() {
        763 | 764 => [2, 14, 15, 31, 40, 41, 93, -1],
        765 => [2, 14, 15, 33, 42, 43, 96, -1],
        766 | 767 => [1, 13, 14, 35, 44, 45, 99, -1],
        768 => [1, 13, 14, 35, 44, 45, 100, 46],
        769 => [1, 13, 14, 36, 45, 46, 101, 47],
        770..=772 => [1, 13, 14, 37, 46, 47, 102, 48],
        773 | 774 => [1, 14, 15, 38, 47, 48, 103, 49],
        775 => [1, 14, 15, 38, 47, 48, 105, 49],
        776 => [1, 21, 22, 45, 54, 55, 112, 56],
        _ => unreachable!(),
    }
}
fn check(v: Version, kind: &'static str, id: i32, payload: Vec<u8>, data: D) {
    let mut bytes = vec![];
    vint(&mut bytes, id);
    bytes.extend(payload);
    assert_eq!(particle_id(v, kind).unwrap(), id, "{v} {kind}");
    let expected = Particle { kind, data };
    assert_eq!(
        Particle::decode(&bytes, v, Limits::default()).unwrap(),
        expected,
        "{v} {kind}"
    );
    assert_eq!(
        expected.encode(v, Limits::default()).unwrap(),
        bytes,
        "{v} {kind}"
    );
    for n in 0..bytes.len() {
        assert!(
            Particle::decode(&bytes[..n], v, Limits::default()).is_err(),
            "{v} {kind} truncated {n}"
        );
    }
    bytes.push(0);
    assert!(Particle::decode(&bytes, v, Limits::default()).is_err());
}
#[test]
fn independent_particle_fixtures_all_fourteen_protocols() {
    for &v in Version::ALL {
        let p = v.protocol();
        let id = ids(v);
        let expected_len = match p {
            763 | 764 => 95,
            765 => 101,
            766 | 767 => 109,
            768 => 111,
            769 => 112,
            770..=772 => 114,
            773 | 774 => 115,
            775 => 117,
            776 => 125,
            _ => unreachable!(),
        };
        assert_eq!(particle_registry(v).len(), expected_len);
        check(v, "block", id[0], vec![0xac, 2], D::BlockState(300));
        let mut bytes = vec![];
        let data = if p < 768 {
            floats(&mut bytes, &[0.25, 0.5, 1.0, 2.0]);
            D::DustRgb {
                color: [0.25, 0.5, 1.0],
                scale: 2.0,
            }
        } else {
            bytes.extend(0x123456i32.to_be_bytes());
            floats(&mut bytes, &[2.0]);
            D::DustPacked {
                color: 0x123456,
                scale: 2.0,
            }
        };
        check(v, "dust", id[1], bytes, data);
        let mut bytes = vec![];
        let data = if p < 768 {
            floats(&mut bytes, &[0.25, 0.5, 1.0]);
            if p <= 765 {
                floats(&mut bytes, &[2.0]);
            }
            floats(&mut bytes, &[0.75, 0.25, 0.0]);
            if p >= 766 {
                floats(&mut bytes, &[2.0]);
            }
            D::TransitionRgb {
                from: [0.25, 0.5, 1.0],
                to: [0.75, 0.25, 0.0],
                scale: 2.0,
            }
        } else {
            bytes.extend(0x123456i32.to_be_bytes());
            bytes.extend(0x987654i32.to_be_bytes());
            floats(&mut bytes, &[2.0]);
            D::TransitionPacked {
                from: 0x123456,
                to: 0x987654,
                scale: 2.0,
            }
        };
        check(v, "dust_color_transition", id[2], bytes, data);
        check(
            v,
            "sculk_charge",
            id[3],
            1.5f32.to_be_bytes().to_vec(),
            D::SculkCharge { roll: 1.5 },
        );
        check(v, "shriek", id[6], vec![0xc8, 1], D::Shriek { delay: 200 });
        let mut bytes = vec![];
        if p <= 765 {
            text(&mut bytes, "minecraft:entity");
        } else {
            bytes.push(1);
        }
        vint(&mut bytes, 300);
        floats(&mut bytes, &[1.625]);
        bytes.push(20);
        check(
            v,
            "vibration",
            id[5],
            bytes,
            D::Vibration {
                destination: VibrationDestination::Entity {
                    entity_id: 300,
                    eye_height: 1.625,
                },
                ticks: 20,
            },
        );
        let mut bytes = vec![];
        if p <= 765 {
            text(&mut bytes, "minecraft:block");
        } else {
            bytes.push(0);
        }
        bytes.extend(((1u64 << 38) | (2u64 << 12) | 64).to_be_bytes());
        bytes.push(5);
        check(
            v,
            "vibration",
            id[5],
            bytes,
            D::Vibration {
                destination: VibrationDestination::Block(BlockPosition { x: 1, y: 64, z: 2 }),
                ticks: 5,
            },
        );
        let bytes = if p <= 765 {
            vec![1, 5, 2, 0]
        } else if p <= 774 {
            vec![2, 5, 0, 0]
        } else {
            vec![5, 2, 0, 0]
        };
        let item = ItemStack {
            item_id: 5,
            count: 2,
            data: if p <= 765 {
                ItemData::Legacy(None)
            } else {
                ItemData::Components(ComponentPatch::default())
            },
        };
        check(
            v,
            "item",
            id[4],
            bytes,
            if p <= 774 {
                D::Item(Box::new(Slot::Item(item)))
            } else {
                D::ItemTemplate(Box::new(item))
            },
        );
        if p >= 768 {
            let mut bytes = vec![];
            for n in [1.0f64, -2.0, 3.5] {
                bytes.extend(n.to_be_bytes());
            }
            bytes.extend(0x123456i32.to_be_bytes());
            if p >= 769 {
                bytes.push(30);
            }
            check(
                v,
                "trail",
                id[7],
                bytes,
                D::Trail {
                    target: [1.0, -2.0, 3.5],
                    color: 0x123456,
                    duration: if p >= 769 { Some(30) } else { None },
                },
            );
        }
    }
}
#[test]
fn recent_colors_power_and_geysers_are_semantic() {
    for &v in Version::ALL {
        let p = v.protocol();
        if p >= 766 {
            let id = if p < 773 {
                20
            } else if p < 776 {
                21
            } else {
                28
            };
            check(
                v,
                "entity_effect",
                id,
                0x80123456u32.to_be_bytes().to_vec(),
                D::Color(0x80123456u32 as i32),
            );
        }
        if p >= 773 {
            let offset = if p == 776 { 7 } else { 0 };
            check(
                v,
                "dragon_breath",
                8 + offset,
                1.5f32.to_be_bytes().to_vec(),
                D::Power(1.5),
            );
            check(
                v,
                "flash",
                42 + offset,
                0x123456i32.to_be_bytes().to_vec(),
                D::Color(0x123456),
            );
            let mut bytes = 0x123456i32.to_be_bytes().to_vec();
            floats(&mut bytes, &[0.5]);
            check(
                v,
                "effect",
                16 + offset,
                bytes,
                D::Spell {
                    color: 0x123456,
                    power: 0.5,
                },
            );
        }
        if p == 776 {
            for (name, id) in [("geyser", 7), ("geyser_plume", 10)] {
                check(
                    v,
                    name,
                    id,
                    10i32.to_be_bytes().to_vec(),
                    D::Geyser { water_blocks: 10 },
                );
            }
            for (name, id) in [("geyser_base", 8), ("geyser_poof", 9)] {
                let mut bytes = 10i32.to_be_bytes().to_vec();
                floats(&mut bytes, &[0.75]);
                check(
                    v,
                    name,
                    id,
                    bytes,
                    D::GeyserBase {
                        water_blocks: 10,
                        burst_impulse_base: 0.75,
                    },
                );
            }
        }
    }
}
#[test]
fn modern_respawn_empty_particle_list_and_mixed_list() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 766) {
        let serializer = if v.protocol() < 773 { 18 } else { 17 };
        // LivingEntity effect particles reset on respawn: empty list then terminator.
        let bytes = vec![1, 10, serializer, 0, 255];
        let decoded = EntityMetadata::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(decoded.entries[0].value, MetadataValue::Particles(vec![]));
        assert_eq!(decoded.encode(v, Limits::default()).unwrap(), bytes);
        let mut bytes = vec![1, 10, serializer, 2, 0, ids(v)[0] as u8, 42, 255];
        let decoded = EntityMetadata::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(decoded.encode(v, Limits::default()).unwrap(), bytes);
        let small = Limits {
            max_collection: 2,
            ..Limits::default()
        };
        assert!(matches!(
            EntityMetadata::decode(&bytes, v, small),
            Err(Error::Limit(_))
        ));
        assert!(matches!(decoded.encode(v, small), Err(Error::Limit(_))));
        bytes[3] = 128;
        assert!(EntityMetadata::decode(&bytes, v, Limits::default()).is_err());
    }
}
#[test]
fn malformed_unknown_and_wrong_release_particles_fail_safely() {
    for &v in Version::ALL {
        assert!(matches!(
            Particle::decode(&[0xff, 0xff, 0xff, 0xff, 7], v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            Particle::decode(&[0xff, 0xff, 0xff, 0xff, 15], v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let mut bytes = vec![ids(v)[3] as u8];
        bytes.extend(f32::INFINITY.to_be_bytes());
        assert!(matches!(
            Particle::decode(&bytes, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let wrong = Particle {
            kind: "dust",
            data: D::Unit,
        };
        assert!(matches!(
            wrong.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let mut bytes = vec![ids(v)[5] as u8];
        if v.protocol() <= 765 {
            text(&mut bytes, "minecraft:unknown");
        } else {
            bytes.push(2);
        }
        assert!(matches!(
            Particle::decode(&bytes, v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
        let bytes = vec![0];
        let small = Limits {
            max_collection: 0,
            ..Limits::default()
        };
        assert!(matches!(
            Particle::decode(&bytes, v, small),
            Err(Error::Limit(_))
        ));
    }
    let v = Version::ALL
        .iter()
        .copied()
        .find(|v| v.protocol() == 776)
        .unwrap();
    let value = EntityMetadata {
        entity_id: 1,
        entries: vec![MetadataEntry {
            index: 10,
            serializer: "particles",
            value: MetadataValue::Particles(vec![Particle {
                kind: "item",
                data: D::ItemTemplate(Box::new(ItemStack {
                    item_id: 1,
                    count: 1,
                    data: ItemData::Components(ComponentPatch::default()),
                })),
            }]),
        }],
    };
    let bytes = value.encode(v, Limits::default()).unwrap();
    let small = Limits {
        max_collection: 2,
        ..Limits::default()
    };
    assert!(matches!(value.encode(v, small), Err(Error::Limit(_))));
    assert!(matches!(
        EntityMetadata::decode(&bytes, v, small),
        Err(Error::Limit(_))
    ));
}
