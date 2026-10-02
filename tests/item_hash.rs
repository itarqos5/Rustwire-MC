use rustwire_mc::{
    nbt::{Nbt, NbtString, Tag, TagType},
    packet::{inventory::*, item_hash::*},
    Error, Limits, Version,
};
fn v(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn hash(name: &'static str, value: ComponentValue, p: i32) -> i32 {
    hash_component(&Component { name, value }, v(p), Limits::default()).unwrap()
}
fn custom() -> Nbt {
    Nbt::anonymous(Tag::Compound(vec![
        ("rustwire".into(), Tag::String("probe".into())),
        (
            "nested".into(),
            Tag::Compound(vec![("a".into(), Tag::Int(7))]),
        ),
    ]))
}
#[test]
fn official_nbt_primitive_arrays_and_map_fixtures() {
    for (tag, expected) in [
        (Tag::ByteArray(vec![-1, 0, 1]), 1975537757),
        (Tag::IntArray(vec![-1, 0, i32::MAX]), -996367392),
        (Tag::LongArray(vec![-1, 0, i64::MAX]), -1890946177),
    ] {
        assert_eq!(
            hash_nbt(&Nbt::anonymous(tag), Limits::default()).unwrap(),
            expected
        );
    }
    assert_eq!(hash_nbt(&custom(), Limits::default()).unwrap(), 1195301885);
    let mut reversed = custom();
    if let Tag::Compound(v) = &mut reversed.root {
        v.reverse()
    }
    assert_eq!(hash_nbt(&reversed, Limits::default()).unwrap(), 1195301885);
    let mut duplicate = custom();
    if let Tag::Compound(v) = &mut duplicate.root {
        v.insert(0, ("rustwire".into(), Tag::Int(-99)))
    }
    assert_eq!(hash_nbt(&duplicate, Limits::default()).unwrap(), 1195301885);
}
#[test]
fn official_component_codec_hashes_all_modern_families() {
    for p in 770..=776 {
        assert_eq!(hash("damage", ComponentValue::VarInt(3), p), -499649379);
        assert_eq!(hash("unbreakable", ComponentValue::Unit, p), -982207288);
        assert_eq!(hash("rarity", ComponentValue::VarInt(2), p), -1420566726);
        assert_eq!(
            hash("custom_data", ComponentValue::Nbt(custom()), p),
            1195301885
        );
        assert_eq!(
            hash(
                "block_state",
                ComponentValue::BlockState(vec![("axis".into(), "y".into())]),
                p
            ),
            -1422114890
        );
        assert_eq!(
            hash(
                "custom_model_data",
                ComponentValue::CustomModelData(CustomModelData::default()),
                p
            ),
            -982207288
        );
        assert_eq!(
            hash(
                "custom_model_data",
                ComponentValue::CustomModelData(CustomModelData {
                    floats: vec![1.5],
                    flags: vec![true],
                    ..Default::default()
                }),
                p
            ),
            -677891201
        );
        assert_eq!(
            hash(
                "tooltip_display",
                ComponentValue::TooltipDisplay {
                    hide_tooltip: false,
                    hidden_components: vec![]
                },
                p
            ),
            -982207288
        );
        assert_eq!(
            hash("enchantable", ComponentValue::VarInt(10), p),
            -554861910
        );
    }
}
#[test]
fn item_patch_hashes_feed_modern_clicks() {
    for p in 770..=776 {
        let slot = Slot::Item(ItemStack {
            item_id: 1,
            count: 3,
            data: ItemData::Components(ComponentPatch {
                added: vec![
                    Component {
                        name: "damage",
                        value: ComponentValue::VarInt(3),
                    },
                    Component {
                        name: "custom_data",
                        value: ComponentValue::Nbt(custom()),
                    },
                ],
                removed: vec!["enchantments"],
            }),
        });
        let hashed = HashedItemStack::from_slot(&slot, v(p), Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(
            hashed.components,
            vec![("damage", -499649379), ("custom_data", 1195301885)]
        );
        assert_eq!(hashed.removed_components, vec!["enchantments"]);
        assert_eq!(hashed.item_id, 1);
        assert_eq!(hashed.count, 3);
        assert!(
            HashedItemStack::from_slot(&Slot::Empty, v(p), Limits::default())
                .unwrap()
                .is_none()
        );
    }
    assert!(matches!(
        HashedItemStack::from_slot(&Slot::Empty, v(769), Limits::default()),
        Err(Error::Unsupported(_))
    ));
}
#[test]
fn component_normalization_matches_codec_semantics() {
    let p = 776;
    assert_eq!(
        hash("item_model", ComponentValue::String("stone".into()), p),
        hash(
            "item_model",
            ComponentValue::String("minecraft:stone".into()),
            p
        )
    );
    assert_eq!(
        hash(
            "block_state",
            ComponentValue::BlockState(vec![
                ("axis".into(), "x".into()),
                ("axis".into(), "y".into())
            ]),
            p
        ),
        -1422114890
    );
    assert_eq!(
        hash(
            "tooltip_display",
            ComponentValue::TooltipDisplay {
                hide_tooltip: true,
                hidden_components: vec!["damage", "damage"]
            },
            p
        ),
        hash(
            "tooltip_display",
            ComponentValue::TooltipDisplay {
                hide_tooltip: true,
                hidden_components: vec!["damage"]
            },
            p
        )
    );
}
#[test]
fn refuses_unsupported_shapes_and_malformed_components() {
    for (name, value) in [
        (
            "custom_name",
            ComponentValue::Nbt(Nbt::anonymous(Tag::Compound(vec![(
                "translate".into(),
                Tag::String("item.minecraft.stone".into()),
            )]))),
        ),
        ("creative_slot_lock", ComponentValue::Unit),
    ] {
        assert!(matches!(
            hash_component(&Component { name, value }, v(776), Limits::default()),
            Err(Error::Unsupported(_))
        ));
    }
    for (name, value) in [
        ("damage", ComponentValue::String("3".into())),
        ("rarity", ComponentValue::VarInt(4)),
        (
            "custom_data",
            ComponentValue::Nbt(Nbt::anonymous(Tag::Int(3))),
        ),
        ("item_model", ComponentValue::String("Invalid key".into())),
    ] {
        assert!(hash_component(&Component { name, value }, v(776), Limits::default()).is_err());
    }
}
#[test]
fn hashing_preserves_nbt_widths_strings_and_limits() {
    assert_ne!(
        hash_nbt(&Nbt::anonymous(Tag::Byte(1)), Limits::default()).unwrap(),
        hash_nbt(&Nbt::anonymous(Tag::Int(1)), Limits::default()).unwrap()
    );
    assert_eq!(
        hash_nbt(
            &Nbt::anonymous(Tag::String(NbtString(vec![0xd800]))),
            Limits::default()
        )
        .unwrap(),
        -423001966
    );
    assert!(hash_nbt(
        &custom(),
        Limits {
            max_nbt_nodes: 2,
            ..Limits::default()
        }
    )
    .is_err());
    assert!(hash_nbt(
        &custom(),
        Limits {
            max_packet: 2,
            ..Limits::default()
        }
    )
    .is_err());
    assert!(hash_nbt(
        &Nbt::anonymous(Tag::List {
            element_type: TagType::Int,
            elements: vec![Tag::Byte(1)]
        }),
        Limits::default()
    )
    .is_err());
    let mut nested = Tag::Int(1);
    for _ in 0..66 {
        nested = Tag::Compound(vec![("a".into(), nested)])
    }
    assert!(hash_nbt(&Nbt::anonymous(nested), Limits::default()).is_err());
    let c = Component {
        name: "custom_model_data",
        value: ComponentValue::CustomModelData(CustomModelData {
            floats: vec![1., 2.],
            ..Default::default()
        }),
    };
    assert!(hash_component(
        &c,
        v(776),
        Limits {
            max_collection: 1,
            ..Limits::default()
        }
    )
    .is_err());
}

#[test]
fn persistent_codec_ranges_are_narrower_than_wire_primitives() {
    for (name, value) in [
        ("max_stack_size", ComponentValue::VarInt(0)),
        ("max_stack_size", ComponentValue::VarInt(100)),
        ("max_damage", ComponentValue::VarInt(0)),
        ("damage", ComponentValue::VarInt(-1)),
        ("repair_cost", ComponentValue::VarInt(-1)),
        ("enchantable", ComponentValue::VarInt(0)),
        ("ominous_bottle_amplifier", ComponentValue::VarInt(5)),
        ("potion_duration_scale", ComponentValue::Float(-0.5)),
        ("potion_duration_scale", ComponentValue::Float(f32::NAN)),
        ("minimum_attack_charge", ComponentValue::Float(1.1)),
    ] {
        let component = Component { name, value };
        assert!(matches!(
            hash_component(&component, v(776), Limits::default()),
            Err(Error::Invalid(_))
        ));
        let slot = Slot::Item(ItemStack {
            item_id: 1,
            count: 1,
            data: ItemData::Components(ComponentPatch {
                added: vec![component],
                removed: vec![],
            }),
        });
        assert!(matches!(
            HashedItemStack::from_slot(&slot, v(776), Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
}
