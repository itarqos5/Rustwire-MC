use rustwire_mc::{
    codec::BlockPosition,
    nbt::{Nbt, Tag},
    packet::{
        entity_metadata::{
            holders::RegistryHolder,
            particles::{Particle, ParticleData},
        },
        inventory::{
            Component, ComponentPatch, ComponentValue, ItemData, ItemStack, Slot, SoundEvent,
        },
        world_effects::*,
    },
    version::{Direction, State},
    Error, Limits, Version,
};

fn vint(bytes: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        let low = (value & 127) as u8;
        value >>= 7;
        bytes.push(low | if value == 0 { 0 } else { 128 });
        if value == 0 {
            break;
        }
    }
}
fn text(bytes: &mut Vec<u8>, value: &str) {
    vint(bytes, value.len() as i32);
    bytes.extend(value.as_bytes());
}
fn floats(bytes: &mut Vec<u8>, values: &[f32]) {
    for v in values {
        bytes.extend(v.to_be_bytes());
    }
}
fn doubles(bytes: &mut Vec<u8>, values: &[f64]) {
    for v in values {
        bytes.extend(v.to_be_bytes());
    }
}
fn block(state: i32) -> Particle {
    Particle {
        kind: "block",
        data: ParticleData::BlockState(state),
    }
}
// Registry positions independently fixed by the inspected release registrations.
fn block_id(v: Version) -> u8 {
    if v.protocol() <= 765 {
        2
    } else {
        1
    }
}
fn block_wire(bytes: &mut Vec<u8>, v: Version, state: i32) {
    bytes.push(block_id(v));
    vint(bytes, state);
}
fn inline_sound() -> RegistryHolder<SoundEvent> {
    RegistryHolder::Inline(SoundEvent {
        name: "minecraft:test".into(),
        fixed_range: Some(8.0),
    })
}
fn sound_wire(bytes: &mut Vec<u8>, tagged: bool) {
    if tagged {
        bytes.push(0);
    }
    text(bytes, "minecraft:test");
    bytes.push(1);
    floats(bytes, &[8.0]);
}
macro_rules! check {
    ($ty:ty, $v:expr, $value:expr, $bytes:expr, $name:expr) => {{
        let v = $v;
        let value = $value;
        let bytes = $bytes;
        assert_eq!(
            <$ty>::decode(&bytes, v, Limits::default()).unwrap(),
            value,
            "{v} {}",
            $name
        );
        assert_eq!(
            value.encode(v, Limits::default()).unwrap(),
            bytes,
            "{v} {}",
            $name
        );
        let packet = value.packet(v, Limits::default()).unwrap();
        assert_eq!(
            packet.id,
            v.packet_id(State::Play, Direction::Clientbound, $name)
                .unwrap()
        );
        assert_eq!(packet.data, bytes);
        for n in 0..bytes.len() {
            assert!(
                <$ty>::decode(&bytes[..n], v, Limits::default()).is_err(),
                "{v} {} truncated {n}",
                $name
            );
        }
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(<$ty>::decode(&extra, v, Limits::default()).is_err());
        let limited = Limits {
            max_packet: bytes.len() - 1,
            ..Limits::default()
        };
        assert!(matches!(
            <$ty>::decode(&bytes, v, limited),
            Err(Error::Limit(_))
        ));
        assert!(matches!(value.encode(v, limited), Err(Error::Limit(_))));
    }};
}
fn world_fixture(v: Version) -> (WorldParticles, Vec<u8>) {
    let value = WorldParticles {
        long_distance: true,
        always_show: (v.protocol() >= 769).then_some(false),
        position: [1.25, -2.5, 3.75],
        offset: [0.125, 0.25, -0.5],
        speed: 2.0,
        count: 300,
        particle: block(300),
    };
    let mut bytes = vec![];
    if v.protocol() <= 765 {
        bytes.push(block_id(v));
    }
    bytes.push(1);
    if v.protocol() >= 769 {
        bytes.push(0);
    }
    doubles(&mut bytes, &[1.25, -2.5, 3.75]);
    floats(&mut bytes, &[0.125, 0.25, -0.5, 2.0]);
    bytes.extend(300i32.to_be_bytes());
    if v.protocol() >= 766 {
        bytes.push(block_id(v));
    }
    vint(&mut bytes, 300);
    (value, bytes)
}
fn explosion_fixture(v: Version, knockback: bool) -> (Explosion, Vec<u8>) {
    let mut bytes = vec![];
    doubles(&mut bytes, &[1.25, -2.5, 3.75]);
    let data = if v.protocol() < 768 {
        floats(&mut bytes, &[4.0]);
        bytes.push(2);
        bytes.extend([128, 127, 255, 0, 1, 2]);
        floats(&mut bytes, &[0.125, -0.25, 0.5]);
        let effects = if v.protocol() >= 765 {
            bytes.push(3);
            block_wire(&mut bytes, v, 300);
            block_wire(&mut bytes, v, 301);
            sound_wire(&mut bytes, v.protocol() >= 766);
            Some(Box::new(LegacyExplosionEffects {
                block_interaction: BlockInteraction::TriggerBlock,
                small_particle: block(300),
                large_particle: block(301),
                sound: inline_sound(),
            }))
        } else {
            None
        };
        ExplosionData::Legacy(LegacyExplosion {
            radius: 4.0,
            affected_block_offsets: vec![[-128, 127, -1], [0, 1, 2]],
            player_knockback: [0.125, -0.25, 0.5],
            effects,
        })
    } else {
        if v.protocol() >= 773 {
            floats(&mut bytes, &[4.0]);
            bytes.extend(300i32.to_be_bytes());
        }
        bytes.push(u8::from(knockback));
        if knockback {
            doubles(&mut bytes, &[0.125, -0.25, 0.5]);
        }
        block_wire(&mut bytes, v, 300);
        vint(&mut bytes, 129);
        let block_effects = if v.protocol() >= 773 {
            bytes.push(2);
            block_wire(&mut bytes, v, 300);
            floats(&mut bytes, &[0.5, 0.25]);
            bytes.push(0);
            block_wire(&mut bytes, v, 301);
            floats(&mut bytes, &[1.0, 2.0]);
            bytes.push(3);
            Some(ExplosionBlockEffects {
                radius: 4.0,
                block_count: 300,
                particles: vec![
                    WeightedExplosionParticle {
                        particle: block(300),
                        scaling: 0.5,
                        speed: 0.25,
                        weight: 0,
                    },
                    WeightedExplosionParticle {
                        particle: block(301),
                        scaling: 1.0,
                        speed: 2.0,
                        weight: 3,
                    },
                ],
            })
        } else {
            None
        };
        ExplosionData::Modern(ModernExplosion {
            player_knockback: knockback.then_some([0.125, -0.25, 0.5]),
            particle: block(300),
            sound: RegistryHolder::RegistryId(128),
            block_effects,
        })
    };
    (
        Explosion {
            center: [1.25, -2.5, 3.75],
            data,
        },
        bytes,
    )
}
#[test]
fn independent_world_particle_goldens_all_fourteen_protocols() {
    for &v in Version::ALL {
        let (value, bytes) = world_fixture(v);
        check!(WorldParticles, v, value, bytes, "world_particles");
    }
}
#[test]
fn independent_explosion_goldens_all_fourteen_protocols() {
    for &v in Version::ALL {
        for knockback in [false, true] {
            let (value, bytes) = explosion_fixture(v, knockback);
            check!(Explosion, v, value, bytes, "explosion");
        }
    }
}
#[test]
fn independent_sound_goldens_preserve_fixed_point_and_holder_ids() {
    for &v in Version::ALL {
        for inline in [false, true] {
            let source = if v.protocol() >= 771 {
                SoundSource::Ui
            } else {
                SoundSource::Voice
            };
            let value = SoundEffect {
                sound: if inline {
                    inline_sound()
                } else {
                    RegistryHolder::RegistryId(128)
                },
                source,
                position: SoundPosition {
                    x: i32::MIN,
                    y: -1,
                    z: i32::MAX,
                },
                volume: 0.5,
                pitch: 2.0,
                seed: i64::MIN + 7,
            };
            let mut bytes = vec![];
            if inline {
                sound_wire(&mut bytes, true);
            } else {
                vint(&mut bytes, 129);
            }
            bytes.push(if v.protocol() >= 771 { 10 } else { 9 });
            bytes.extend(i32::MIN.to_be_bytes());
            bytes.extend((-1i32).to_be_bytes());
            bytes.extend(i32::MAX.to_be_bytes());
            floats(&mut bytes, &[0.5, 2.0]);
            bytes.extend((i64::MIN + 7).to_be_bytes());
            assert_eq!(
                value.position.blocks(),
                [-268_435_456.0, -0.125, 268_435_455.875]
            );
            check!(SoundEffect, v, value, bytes, "sound_effect");
            let value = EntitySoundEffect {
                sound: if inline {
                    inline_sound()
                } else {
                    RegistryHolder::RegistryId(128)
                },
                source,
                entity_id: 300,
                volume: 0.5,
                pitch: 2.0,
                seed: i64::MIN + 7,
            };
            let mut bytes = vec![];
            if inline {
                sound_wire(&mut bytes, true);
            } else {
                vint(&mut bytes, 129);
            }
            bytes.push(if v.protocol() >= 771 { 10 } else { 9 });
            vint(&mut bytes, 300);
            floats(&mut bytes, &[0.5, 2.0]);
            bytes.extend((i64::MIN + 7).to_be_bytes());
            check!(EntitySoundEffect, v, value, bytes, "entity_sound_effect");
        }
    }
}
#[test]
fn independent_stop_sound_and_level_event_goldens_all_protocols() {
    for &v in Version::ALL {
        for flags in 0..=3 {
            let source = (flags & 1 != 0).then_some(SoundSource::Blocks);
            let sound = (flags & 2 != 0).then(|| "minecraft:test".into());
            let value = StopSound { source, sound };
            let mut bytes = vec![flags];
            if flags & 1 != 0 {
                bytes.push(4);
            }
            if flags & 2 != 0 {
                text(&mut bytes, "minecraft:test");
            }
            check!(StopSound, v, value, bytes, "stop_sound");
        }
        // The entire i32 event/data domain remains available to callers.
        let value = WorldEvent {
            event_id: i32::MIN,
            position: BlockPosition { x: -1, y: -2, z: 2 },
            data: i32::MAX,
            global: true,
        };
        let mut bytes = i32::MIN.to_be_bytes().to_vec();
        bytes.extend(0xffff_ffc0_0000_2ffeu64.to_be_bytes());
        bytes.extend(i32::MAX.to_be_bytes());
        bytes.push(1);
        check!(WorldEvent, v, value, bytes, "world_event");
    }
}
#[test]
fn version_shape_boundaries_fail_closed() {
    for &v in Version::ALL {
        let (mut world, _) = world_fixture(v);
        world.always_show = if world.always_show.is_some() {
            None
        } else {
            Some(false)
        };
        assert!(matches!(
            world.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let (mut explosion, _) = explosion_fixture(v, true);
        match &mut explosion.data {
            ExplosionData::Legacy(x) => {
                if x.effects.is_some() {
                    x.effects = None;
                } else {
                    x.effects = Some(Box::new(LegacyExplosionEffects {
                        block_interaction: BlockInteraction::Keep,
                        small_particle: block(1),
                        large_particle: block(1),
                        sound: inline_sound(),
                    }));
                }
            }
            ExplosionData::Modern(x) => {
                if x.block_effects.is_some() {
                    x.block_effects = None;
                } else {
                    x.block_effects = Some(ExplosionBlockEffects {
                        radius: 1.0,
                        block_count: 0,
                        particles: vec![],
                    });
                }
            }
        }
        assert!(matches!(
            explosion.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let other = if v.protocol() < 768 {
            Version::V26_2
        } else {
            Version::V1_20
        };
        let (wrong, _) = explosion_fixture(other, false);
        assert!(matches!(
            wrong.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        assert_eq!(SoundSource::from_id(10, v).is_ok(), v.protocol() >= 771);
        assert_eq!(SoundSource::Ui.id(v).is_ok(), v.protocol() >= 771);
    }
    let (mut explosion, _) = explosion_fixture(Version::V1_20_3, true);
    let ExplosionData::Legacy(x) = &mut explosion.data else {
        unreachable!()
    };
    x.effects.as_mut().unwrap().sound = RegistryHolder::RegistryId(0);
    assert!(matches!(
        explosion.encode(Version::V1_20_3, Limits::default()),
        Err(Error::Invalid(_))
    ));
}
#[test]
fn malformed_flags_discriminants_numbers_and_strings_fail_closed() {
    for &v in Version::ALL {
        for flag in 4..=255 {
            assert!(matches!(
                StopSound::decode(&[flag], v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        for source in [-1, 11, i32::MAX] {
            let mut bytes = vec![1];
            vint(&mut bytes, source);
            assert!(matches!(
                StopSound::decode(&bytes, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        let (mut world, bytes) = world_fixture(v);
        world.count = -1;
        assert!(matches!(
            world.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        world.count = 1;
        world.position[0] = f64::INFINITY;
        assert!(matches!(
            world.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let mut malformed = bytes.clone();
        let bool_index = usize::from(v.protocol() <= 765);
        malformed[bool_index] = 2;
        assert!(matches!(
            WorldParticles::decode(&malformed, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let particle_index = if v.protocol() <= 765 {
            0
        } else {
            45 + usize::from(v.protocol() >= 769)
        };
        let mut unknown = bytes.clone();
        unknown.splice(particle_index..particle_index + 1, [255, 255, 255, 255, 7]);
        assert!(matches!(
            WorldParticles::decode(&unknown, v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
        let mut negative = bytes.clone();
        negative.splice(particle_index..particle_index + 1, [255, 255, 255, 255, 15]);
        assert!(matches!(
            WorldParticles::decode(&negative, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        for name in ["Minecraft:test", "minecraft:a b", "a:b:c", "😀"] {
            let bad = StopSound {
                source: None,
                sound: Some(name.into()),
            };
            assert!(matches!(
                bad.encode(v, Limits::default()),
                Err(Error::Invalid(_))
            ));
            let mut raw = vec![2];
            text(&mut raw, name);
            assert!(matches!(
                StopSound::decode(&raw, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        let mut bad_utf8 = vec![2, 1, 255];
        assert!(StopSound::decode(&bad_utf8, v, Limits::default()).is_err());
        bad_utf8[1] = 127;
        assert!(StopSound::decode(&bad_utf8, v, Limits::default()).is_err());
    }
}
#[test]
fn sound_holder_errors_and_budgets_apply_to_all_sound_packets() {
    for &v in Version::ALL {
        let mut value = SoundEffect {
            sound: inline_sound(),
            source: SoundSource::Master,
            position: SoundPosition { x: 0, y: 0, z: 0 },
            volume: 1.0,
            pitch: 1.0,
            seed: 0,
        };
        let raw = value.encode(v, Limits::default()).unwrap();
        let limited = Limits {
            max_string_chars: 3,
            ..Limits::default()
        };
        assert!(matches!(value.encode(v, limited), Err(Error::Limit(_))));
        assert!(matches!(
            SoundEffect::decode(&raw, v, limited),
            Err(Error::Limit(_))
        ));
        for id in [-1, i32::MAX] {
            value.sound = RegistryHolder::RegistryId(id);
            assert!(matches!(
                value.encode(v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        value.sound = RegistryHolder::RegistryId(0);
        value.volume = f32::NAN;
        assert!(matches!(
            value.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        value.volume = 1.0;
        value.sound = RegistryHolder::Inline(SoundEvent {
            name: "minecraft:test".into(),
            fixed_range: Some(f32::INFINITY),
        });
        assert!(matches!(
            value.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let mut bad = raw.clone();
        bad[17..21].copy_from_slice(&f32::INFINITY.to_be_bytes());
        assert!(matches!(
            SoundEffect::decode(&bad, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let mut bad = raw;
        bad[16] = 2;
        assert!(matches!(
            SoundEffect::decode(&bad, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
}
#[test]
fn explosion_aggregate_limits_and_weight_overflow() {
    for &v in Version::ALL {
        let (value, raw) = explosion_fixture(v, false);
        let needed = match v.protocol() {
            763 | 764 => 2,
            765..=767 => 4,
            768..=772 => 1,
            _ => 5,
        };
        let exact = Limits {
            max_collection: needed,
            ..Limits::default()
        };
        assert_eq!(value.encode(v, exact).unwrap(), raw);
        assert_eq!(Explosion::decode(&raw, v, exact).unwrap(), value);
        let short = Limits {
            max_collection: needed - 1,
            ..Limits::default()
        };
        assert!(matches!(value.encode(v, short), Err(Error::Limit(_))));
        assert!(matches!(
            Explosion::decode(&raw, v, short),
            Err(Error::Limit(_))
        ));
        if v.protocol() >= 773 {
            let mut value = value;
            let ExplosionData::Modern(x) = &mut value.data else {
                unreachable!()
            };
            x.block_effects.as_mut().unwrap().particles[0].weight = i32::MAX;
            assert!(matches!(
                value.encode(v, Limits::default()),
                Err(Error::Invalid(_))
            ));
            let mut overflow = raw.clone();
            let first_weight = 32 + 1 + 3 + 2 + 1 + 3 + 8;
            overflow.splice(first_weight..first_weight + 1, [255, 255, 255, 255, 7]);
            assert!(matches!(
                Explosion::decode(&overflow, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
            let mut negative = raw;
            negative.splice(first_weight..first_weight + 1, [255, 255, 255, 255, 15]);
            assert!(matches!(
                Explosion::decode(&negative, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
    }
}
fn item_particle(v: Version, nbt: Nbt) -> Particle {
    let item = ItemStack {
        item_id: 1,
        count: 1,
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
    Particle {
        kind: "item",
        data: if v.protocol() >= 775 {
            ParticleData::ItemTemplate(Box::new(item))
        } else {
            ParticleData::Item(Box::new(Slot::Item(item)))
        },
    }
}
#[test]
fn item_particles_share_nbt_node_and_collection_budgets_across_explosion() {
    for &v in Version::ALL.iter().filter(|v| v.protocol() >= 765) {
        let particle = item_particle(
            v,
            Nbt::anonymous(Tag::Compound(vec![("k".into(), Tag::Int(1))])),
        );
        let (mut value, _) = explosion_fixture(v, false);
        let roots = if let ExplosionData::Legacy(data) = &mut value.data {
            let effects = data.effects.as_mut().unwrap();
            effects.small_particle = particle.clone();
            effects.large_particle = particle;
            2
        } else if let ExplosionData::Modern(data) = &mut value.data {
            data.particle = particle.clone();
            if let Some(block) = &mut data.block_effects {
                for entry in &mut block.particles {
                    entry.particle = particle.clone();
                }
                3
            } else {
                1
            }
        } else {
            unreachable!()
        };
        let raw = value.encode(v, Limits::default()).unwrap();
        let limited = Limits {
            max_nbt_nodes: roots * 2 - 1,
            ..Limits::default()
        };
        assert!(
            matches!(value.encode(v, limited), Err(Error::Limit(_))),
            "{v}"
        );
        assert!(
            matches!(Explosion::decode(&raw, v, limited), Err(Error::Limit(_))),
            "{v}"
        );
        let exact = Limits {
            max_nbt_nodes: roots * 2,
            ..Limits::default()
        };
        assert_eq!(value.encode(v, exact).unwrap(), raw);
        assert_eq!(Explosion::decode(&raw, v, exact).unwrap(), value);
    }
}
#[test]
fn item_particle_nbt_depth_and_nested_item_depth_are_bounded() {
    for &v in Version::ALL {
        let nbt = Nbt {
            name: if v.protocol() == 763 {
                Some("".into())
            } else {
                None
            },
            root: Tag::Compound(vec![(
                "n".into(),
                Tag::Compound(vec![("v".into(), Tag::Int(1))]),
            )]),
        };
        let (mut value, _) = world_fixture(v);
        value.particle = item_particle(v, nbt);
        let raw = value.encode(v, Limits::default()).unwrap();
        let limited = Limits {
            max_nbt_depth: 1,
            ..Limits::default()
        };
        assert!(
            matches!(value.encode(v, limited), Err(Error::Limit(_))),
            "{v}"
        );
        assert!(
            matches!(
                WorldParticles::decode(&raw, v, limited),
                Err(Error::Limit(_))
            ),
            "{v}"
        );
        if v.protocol() >= 766 {
            let mut item = ItemStack {
                item_id: 1,
                count: 1,
                data: ItemData::Components(ComponentPatch::default()),
            };
            for _ in 0..3 {
                item = ItemStack {
                    item_id: 1,
                    count: 1,
                    data: ItemData::Components(ComponentPatch {
                        added: vec![Component {
                            name: "bundle_contents",
                            value: if v.protocol() >= 775 {
                                ComponentValue::ItemTemplates(vec![item])
                            } else {
                                ComponentValue::Items(vec![Slot::Item(item)])
                            },
                        }],
                        removed: vec![],
                    }),
                };
            }
            value.particle = Particle {
                kind: "item",
                data: if v.protocol() >= 775 {
                    ParticleData::ItemTemplate(Box::new(item))
                } else {
                    ParticleData::Item(Box::new(Slot::Item(item)))
                },
            };
            let raw = value.encode(v, Limits::default()).unwrap();
            assert!(
                matches!(value.encode(v, limited), Err(Error::Limit(_))),
                "{v}"
            );
            assert!(
                matches!(
                    WorldParticles::decode(&raw, v, limited),
                    Err(Error::Limit(_))
                ),
                "{v}"
            );
        }
    }
}
#[test]
fn impossible_explosion_counts_rejected_before_allocation() {
    for &v in Version::ALL {
        let (_, mut raw) = explosion_fixture(v, false);
        let count_index = if v.protocol() < 768 {
            Some(28)
        } else if v.protocol() >= 773 {
            Some(38)
        } else {
            None
        };
        if let Some(i) = count_index {
            raw.splice(i..i + 1, [255, 255, 255, 255, 7]);
            assert!(matches!(
                Explosion::decode(&raw, v, Limits::default()),
                Err(Error::Limit(_))
            ));
        }
    }
}
#[test]
fn bounded_mutations_never_panic_and_successes_reencode() {
    for &v in Version::ALL {
        let (_, world) = world_fixture(v);
        let (_, explosion) = explosion_fixture(v, false);
        let limits = Limits {
            max_collection: 64,
            max_nbt_nodes: 64,
            max_nbt_depth: 8,
            max_packet: 1024,
            ..Limits::default()
        };
        for original in [&world, &explosion] {
            for index in 0..original.len() {
                for mutation in [0, 1, 127, 128, 255] {
                    let mut raw = original.clone();
                    raw[index] = mutation;
                    if std::ptr::eq(original, &world) {
                        if let Ok(value) = WorldParticles::decode(&raw, v, limits) {
                            let bytes = value.encode(v, limits).unwrap();
                            assert_eq!(WorldParticles::decode(&bytes, v, limits).unwrap(), value);
                        }
                    } else if let Ok(value) = Explosion::decode(&raw, v, limits) {
                        let bytes = value.encode(v, limits).unwrap();
                        assert_eq!(Explosion::decode(&bytes, v, limits).unwrap(), value);
                    }
                }
            }
        }
    }
}

#[test]
fn explosion_malformed_known_fields_fail_closed_all_boundaries() {
    for &v in Version::ALL {
        let (_, raw) = explosion_fixture(v, false);
        if (765..=767).contains(&v.protocol()) {
            let mut bad = raw.clone();
            bad[47] = 4;
            assert!(matches!(
                Explosion::decode(&bad, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        if v.protocol() < 768 {
            let mut negative = raw.clone();
            negative.splice(28..29, [255, 255, 255, 255, 15]);
            assert!(matches!(
                Explosion::decode(&negative, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        } else {
            let index = if v.protocol() >= 773 { 32 } else { 24 };
            let mut bad = raw.clone();
            bad[index] = 2;
            assert!(matches!(
                Explosion::decode(&bad, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
            let mut unknown = raw.clone();
            unknown[index + 1] = 127;
            assert!(matches!(
                Explosion::decode(&unknown, v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
            let mut invalid_state = raw.clone();
            invalid_state.splice(index + 2..index + 4, [255, 255, 255, 255, 15]);
            assert!(matches!(
                Explosion::decode(&invalid_state, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        if v.protocol() >= 773 {
            let mut negative = raw;
            negative[28..32].copy_from_slice(&(-1i32).to_be_bytes());
            assert!(matches!(
                Explosion::decode(&negative, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
    }
}
#[test]
fn scalar_counts_are_not_allocations_and_sound_id_extremes_survive() {
    for &v in Version::ALL {
        let (mut world, _) = world_fixture(v);
        for count in [0, i32::MAX] {
            world.count = count;
            let one_particle = Limits {
                max_collection: 1,
                ..Limits::default()
            };
            let raw = world.encode(v, one_particle).unwrap();
            assert_eq!(
                WorldParticles::decode(&raw, v, one_particle).unwrap(),
                world
            );
            let zero_particles = Limits {
                max_collection: 0,
                ..Limits::default()
            };
            assert!(matches!(
                world.encode(v, zero_particles),
                Err(Error::Limit(_))
            ));
            assert!(matches!(
                WorldParticles::decode(&raw, v, zero_particles),
                Err(Error::Limit(_))
            ));
        }
        for (sound, prefix) in [
            (RegistryHolder::RegistryId(0), vec![1]),
            (
                RegistryHolder::RegistryId(i32::MAX - 1),
                vec![255, 255, 255, 255, 7],
            ),
            (
                RegistryHolder::Inline(SoundEvent {
                    name: "x".into(),
                    fixed_range: None,
                }),
                vec![0, 1, b'x', 0],
            ),
        ] {
            let value = EntitySoundEffect {
                sound,
                source: SoundSource::Master,
                entity_id: 0,
                volume: 1.0,
                pitch: 1.0,
                seed: 0,
            };
            let mut raw = prefix;
            raw.extend([0, 0]);
            floats(&mut raw, &[1.0, 1.0]);
            raw.extend(0i64.to_be_bytes());
            check!(EntitySoundEffect, v, value, raw, "entity_sound_effect");
        }
        for id in 0..=9 {
            assert_eq!(SoundSource::from_id(id, v).unwrap().id(v).unwrap(), id);
        }
    }
}
#[test]
fn sound_and_level_event_mutations_never_panic() {
    macro_rules! mutate {
        ($ty:ty, $v:expr, $raw:expr) => {{
            let limits = Limits {
                max_collection: 16,
                max_string_chars: 32,
                max_packet: 128,
                ..Limits::default()
            };
            let original = $raw;
            for index in 0..original.len() {
                for mutation in [0, 1, 127, 128, 255] {
                    let mut raw = original.clone();
                    raw[index] = mutation;
                    if let Ok(value) = <$ty>::decode(&raw, $v, limits) {
                        let encoded = value.encode($v, limits).unwrap();
                        assert_eq!(<$ty>::decode(&encoded, $v, limits).unwrap(), value);
                    }
                }
            }
        }};
    }
    for &v in Version::ALL {
        let sound = SoundEffect {
            sound: inline_sound(),
            source: SoundSource::Master,
            position: SoundPosition { x: -1, y: 0, z: 1 },
            volume: 1.0,
            pitch: 1.0,
            seed: 7,
        };
        mutate!(SoundEffect, v, sound.encode(v, Limits::default()).unwrap());
        let entity = EntitySoundEffect {
            sound: inline_sound(),
            source: SoundSource::Master,
            entity_id: 300,
            volume: 1.0,
            pitch: 1.0,
            seed: 7,
        };
        mutate!(
            EntitySoundEffect,
            v,
            entity.encode(v, Limits::default()).unwrap()
        );
        mutate!(
            StopSound,
            v,
            StopSound {
                source: Some(SoundSource::Master),
                sound: Some("minecraft:test".into())
            }
            .encode(v, Limits::default())
            .unwrap()
        );
        mutate!(
            WorldEvent,
            v,
            WorldEvent {
                event_id: 2001,
                position: BlockPosition { x: -1, y: 0, z: 1 },
                data: 300,
                global: false
            }
            .encode(v, Limits::default())
            .unwrap()
        );
    }
}
