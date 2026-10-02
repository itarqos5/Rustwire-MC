use rustwire_mc::{
    codec::BlockPosition,
    nbt::{Nbt, Tag},
    packet::{
        entity_metadata::holders::{PlayerSkinPatch, RegistryHolder, ResolvableProfile},
        entity_metadata::particles::{Particle, ParticleData},
        entity_metadata::*,
        inventory::{ComponentPatch, ItemData, ItemStack, Slot},
        DeathLocation,
    },
    Error, Limits, Version,
};
// Independent source-schema serializer lists, deliberately not obtained from the codec.
fn names(v: Version) -> Vec<&'static str> {
    let tail = match v.protocol() {
        763..=765 => "compound_tag particle villager_data optional_unsigned_int pose cat_variant frog_variant optional_global_pos painting_variant sniffer_state vector3 quaternion",
        766..=769 => "compound_tag particle particles villager_data optional_unsigned_int pose cat_variant wolf_variant frog_variant optional_global_pos painting_variant sniffer_state armadillo_state vector3 quaternion",
        770..=772 => "compound_tag particle particles villager_data optional_unsigned_int pose cat_variant cow_variant wolf_variant wolf_sound_variant frog_variant pig_variant chicken_variant optional_global_pos painting_variant sniffer_state armadillo_state vector3 quaternion",
        773 => "particle particles villager_data optional_unsigned_int pose cat_variant cow_variant wolf_variant wolf_sound_variant frog_variant pig_variant chicken_variant optional_global_pos painting_variant sniffer_state armadillo_state copper_golem_state weathering_copper_golem_state vector3 quaternion resolvable_profile",
        774 => "particle particles villager_data optional_unsigned_int pose cat_variant cow_variant wolf_variant wolf_sound_variant frog_variant pig_variant chicken_variant zombie_nautilus_variant optional_global_pos painting_variant sniffer_state armadillo_state copper_golem_state weathering_copper_golem_state vector3 quaternion resolvable_profile humanoid_arm",
        775..=776 => "particle particles villager_data optional_unsigned_int pose cat_variant cat_sound_variant cow_variant cow_sound_variant wolf_variant wolf_sound_variant frog_variant pig_variant pig_sound_variant chicken_variant chicken_sound_variant zombie_nautilus_variant optional_global_pos painting_variant sniffer_state armadillo_state copper_golem_state weathering_copper_golem_state vector3 quaternion resolvable_profile humanoid_arm",
        _ => unreachable!(),
    };
    "byte int long float string component optional_component item_stack boolean rotations block_pos optional_block_pos direction optional_uuid block_state optional_block_state".split(' ').chain(tail.split(' ')).collect()
}
fn vint(b: &mut Vec<u8>, n: u64) {
    let mut n = n;
    loop {
        let low = (n & 127) as u8;
        n >>= 7;
        b.push(low | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
fn text(b: &mut Vec<u8>, s: &str) {
    vint(b, s.len() as u64);
    b.extend(s.as_bytes());
}
fn pos(b: &mut Vec<u8>) {
    let raw = (((-17i64 as u64) & 0x3ffffff) << 38) | (25u64 << 12) | 64;
    b.extend(raw.to_be_bytes());
}
fn point() -> BlockPosition {
    BlockPosition {
        x: -17,
        y: 64,
        z: 25,
    }
}
fn payload(name: &str, v: Version) -> Option<(Vec<u8>, MetadataValue)> {
    use MetadataValue as V;
    let mut b = vec![];
    let value = match name {
        "particle" => {
            b.push(0);
            V::Particle(Particle {
                kind: if v.protocol() <= 765 {
                    "ambient_entity_effect"
                } else {
                    "angry_villager"
                },
                data: ParticleData::Unit,
            })
        }
        "particles" => {
            b.push(0);
            V::Particles(vec![])
        }
        "resolvable_profile" => {
            b.extend([0; 8]);
            V::ResolvableProfile(ResolvableProfile::Partial {
                name: None,
                uuid: None,
                properties: vec![],
                skin_patch: PlayerSkinPatch::default(),
            })
        }
        "painting_variant" if v.protocol() >= 767 => {
            b.push(2);
            V::PaintingVariant(RegistryHolder::RegistryId(1))
        }
        "wolf_variant" if (767..=769).contains(&v.protocol()) => {
            b.push(2);
            V::WolfVariant(RegistryHolder::RegistryId(1))
        }
        "byte" => {
            b.push(128);
            V::Byte(-128)
        }
        "int" => {
            vint(&mut b, u32::MAX as u64);
            V::VarInt(-1)
        }
        "long" => {
            vint(&mut b, i64::MIN as u64);
            V::VarLong(i64::MIN)
        }
        "float" => {
            b.extend(19.5f32.to_be_bytes());
            V::Float(19.5)
        }
        "string" => {
            text(&mut b, "entity");
            V::String("entity".into())
        }
        "component" | "optional_component" => {
            let optional = name == "optional_component";
            if optional {
                b.push(1);
            }
            if v.protocol() < 765 {
                text(&mut b, "\"hi\"");
                if optional {
                    V::OptionalJsonComponent(Some("\"hi\"".into()))
                } else {
                    V::JsonComponent("\"hi\"".into())
                }
            } else {
                b.extend([8, 0, 2, b'h', b'i']);
                let n = Nbt::anonymous(Tag::String("hi".into()));
                if optional {
                    V::OptionalNbt(Some(n))
                } else {
                    V::Nbt(n)
                }
            }
        }
        "compound_tag" => {
            b.push(10);
            if v.protocol() == 763 {
                b.extend([0, 0]);
            }
            b.extend([3, 0, 1, b'x', 0, 0, 0, 1, 0]);
            V::Nbt(Nbt {
                name: if v.protocol() == 763 {
                    Some("".into())
                } else {
                    None
                },
                root: Tag::Compound(vec![("x".into(), Tag::Int(1))]),
            })
        }
        "item_stack" => {
            if v.protocol() <= 765 {
                b.extend([1, 1, 2, 0]);
            } else {
                b.extend([2, 1, 0, 0]);
            }
            V::Slot(Box::new(Slot::Item(ItemStack {
                item_id: 1,
                count: 2,
                data: if v.protocol() <= 765 {
                    ItemData::Legacy(None)
                } else {
                    ItemData::Components(ComponentPatch::default())
                },
            })))
        }
        "boolean" => {
            b.push(1);
            V::Bool(true)
        }
        "rotations" | "vector3" => {
            for n in [1f32, -2.5, 180.0] {
                b.extend(n.to_be_bytes());
            }
            V::Vec3([1.0, -2.5, 180.0])
        }
        "quaternion" => {
            for n in [0f32, -1.0, 0.5, 1.0] {
                b.extend(n.to_be_bytes());
            }
            V::Quaternion([0.0, -1.0, 0.5, 1.0])
        }
        "block_pos" => {
            pos(&mut b);
            V::BlockPosition(point())
        }
        "optional_block_pos" => {
            b.push(1);
            pos(&mut b);
            V::OptionalBlockPosition(Some(point()))
        }
        "optional_uuid" => {
            b.push(1);
            b.extend([9; 16]);
            V::OptionalUuid(Some([9; 16]))
        }
        "optional_block_state" => {
            vint(&mut b, 22000);
            V::OptionalBlockState(Some(22000))
        }
        "optional_unsigned_int" => {
            vint(&mut b, i32::MAX as u64 + 1);
            V::OptionalUnsignedInt(Some(i32::MAX as u32))
        }
        "villager_data" => {
            b.extend([2, 3, 4]);
            V::VillagerData {
                kind: 2,
                profession: 3,
                level: 4,
            }
        }
        "optional_global_pos" => {
            b.push(1);
            text(&mut b, "minecraft:overworld");
            pos(&mut b);
            V::OptionalGlobalPosition(Some(DeathLocation {
                dimension: "minecraft:overworld".into(),
                position: point(),
            }))
        }
        "block_state" => {
            vint(&mut b, 22000);
            V::VarInt(22000)
        }
        _ => {
            b.push(1);
            V::VarInt(1)
        }
    };
    Some((b, value))
}
fn fixture(v: Version) -> (Vec<u8>, EntityMetadata) {
    let mut b = vec![0xac, 0x02];
    let mut entries = vec![];
    for (id, name) in names(v).into_iter().enumerate() {
        if let Some((payload, value)) = payload(name, v) {
            b.push(id as u8);
            vint(&mut b, id as u64);
            b.extend(payload);
            entries.push(MetadataEntry {
                index: id as u8,
                serializer: name,
                value,
            });
        }
    }
    b.push(255);
    (
        b,
        EntityMetadata {
            entity_id: 300,
            entries,
        },
    )
}
#[test]
fn exact_ids_and_independent_value_fixtures_all_14_families() {
    for &v in Version::ALL {
        let expected_names = names(v);
        assert_eq!(
            metadata_registry(v)
                .iter()
                .map(|(n, _)| *n)
                .collect::<Vec<_>>(),
            expected_names
        );
        for (id, name) in expected_names.into_iter().enumerate() {
            assert_eq!(serializer_id(v, name).unwrap(), id as i32);
            assert_eq!(
                metadata_registry(v)[id].1 != MetadataWire::Unsupported,
                payload(name, v).is_some(),
                "{v} {name}"
            );
        }
        let (bytes, expected) = fixture(v);
        assert_eq!(
            EntityMetadata::decode(&bytes, v, Limits::default()).unwrap(),
            expected,
            "{v}"
        );
        assert_eq!(expected.encode(v, Limits::default()).unwrap(), bytes, "{v}");
    }
}
#[test]
fn all_prefixes_trailing_bytes_and_resource_limits() {
    for &v in Version::ALL {
        let (bytes, expected) = fixture(v);
        for n in 0..bytes.len() {
            assert!(
                EntityMetadata::decode(&bytes[..n], v, Limits::default()).is_err(),
                "{v} prefix {n}"
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(EntityMetadata::decode(&trailing, v, Limits::default()).is_err());
        for limits in [
            Limits {
                max_packet: bytes.len() - 1,
                ..Limits::default()
            },
            Limits {
                max_collection: 2,
                ..Limits::default()
            },
            Limits {
                max_nbt_nodes: 1,
                ..Limits::default()
            },
            Limits {
                max_string_chars: 1,
                ..Limits::default()
            },
        ] {
            assert!(EntityMetadata::decode(&bytes, v, limits).is_err());
            assert!(expected.encode(v, limits).is_err());
        }
    }
}
#[test]
fn absent_optional_values_and_unsigned_integer_boundary() {
    for &v in Version::ALL {
        let optional = [
            "optional_component",
            "optional_block_pos",
            "optional_uuid",
            "optional_block_state",
            "optional_unsigned_int",
            "optional_global_pos",
        ];
        let mut b = vec![1];
        for (index, name) in optional.iter().enumerate() {
            b.push(index as u8);
            vint(
                &mut b,
                names(v).iter().position(|n| n == name).unwrap() as u64,
            );
            b.push(0);
        }
        b.push(255);
        let decoded = EntityMetadata::decode(&b, v, Limits::default()).unwrap();
        assert_eq!(decoded.encode(v, Limits::default()).unwrap(), b);
        assert!(matches!(
            decoded.entries[4].value,
            MetadataValue::OptionalUnsignedInt(None)
        ));
        let mut present_zero = vec![1, 0];
        vint(
            &mut present_zero,
            names(v)
                .iter()
                .position(|n| *n == "optional_unsigned_int")
                .unwrap() as u64,
        );
        present_zero.extend([1, 255]);
        assert_eq!(
            EntityMetadata::decode(&present_zero, v, Limits::default())
                .unwrap()
                .entries[0]
                .value,
            MetadataValue::OptionalUnsignedInt(Some(0))
        );
    }
}
#[test]
fn unsupported_values_are_never_guessed_or_skipped() {
    for &v in Version::ALL {
        let id = names(v)
            .iter()
            .position(|name| *name == "particle")
            .unwrap();
        let mut bytes = vec![1, 0];
        vint(&mut bytes, id as u64);
        vint(&mut bytes, 999);
        bytes.push(255);
        assert!(matches!(
            EntityMetadata::decode(&bytes, v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            EntityMetadata::decode(&[1, 0, 0xe7, 7, 255], v, Limits::default()),
            Err(Error::Unsupported(_))
        ));
    }
}
#[test]
fn duplicate_indexes_invalid_enums_nan_and_wrong_value_shapes_fail() {
    for &v in Version::ALL {
        let l = Limits::default();
        assert!(EntityMetadata::decode(&[1, 0, 0, 1, 0, 0, 1, 255], v, l).is_err());
        assert!(EntityMetadata::decode(&[1, 0, 8, 2, 255], v, l).is_err());
        assert!(EntityMetadata::decode(&[1, 0, 12, 6, 255], v, l).is_err());
        let mut b = vec![1, 0, 3];
        b.extend(f32::NAN.to_be_bytes());
        b.push(255);
        assert!(EntityMetadata::decode(&b, v, l).is_err());
        let wrong = EntityMetadata {
            entity_id: 1,
            entries: vec![MetadataEntry {
                index: 0,
                serializer: "byte",
                value: MetadataValue::String("wrong".into()),
            }],
        };
        assert!(wrong.encode(v, l).is_err());
        let mut wrong = wrong;
        wrong.entries[0].serializer = "optional_block_state";
        wrong.entries[0].value = MetadataValue::OptionalBlockState(Some(0));
        assert!(wrong.encode(v, l).is_err());
        wrong.entries[0].serializer = "optional_unsigned_int";
        wrong.entries[0].value = MetadataValue::OptionalUnsignedInt(Some(u32::MAX));
        assert!(wrong.encode(v, l).is_err());
        let mut reserved = EntityMetadata {
            entity_id: 1,
            entries: vec![MetadataEntry {
                index: 255,
                serializer: "byte",
                value: MetadataValue::Byte(0),
            }],
        };
        assert!(reserved.encode(v, l).is_err());
        reserved.entries[0].index = 0;
        reserved.entries.push(reserved.entries[0].clone());
        assert!(reserved.encode(v, l).is_err());
        let entries = (0..255)
            .map(|index| MetadataEntry {
                index,
                serializer: "byte",
                value: MetadataValue::Byte(0),
            })
            .collect();
        let full = EntityMetadata {
            entity_id: i32::MAX,
            entries,
        };
        assert_eq!(
            EntityMetadata::decode(&full.encode(v, l).unwrap(), v, l).unwrap(),
            full
        );
    }
}
#[test]
fn nbt_budget_is_shared_across_metadata_and_nested_slots() {
    let v = Version::V1_20;
    // Direct metadata compound consumes two nodes; classic slot compound also
    // consumes two. max_nbt_nodes=3 must fail, even though either fits alone.
    let mut bytes = vec![1, 0, 16];
    bytes.extend([10, 0, 0, 3, 0, 1, b'x', 0, 0, 0, 1, 0]);
    bytes.extend([1, 7, 1, 1, 1, 10, 0, 0, 3, 0, 1, b'y', 0, 0, 0, 2, 0, 255]);
    let p = EntityMetadata::decode(&bytes, v, Limits::default()).unwrap();
    let small = Limits {
        max_nbt_nodes: 3,
        ..Limits::default()
    };
    assert!(EntityMetadata::decode(&bytes, v, small).is_err());
    assert!(p.encode(v, small).is_err());
    assert_eq!(p.encode(v, Limits::default()).unwrap(), bytes);
}
