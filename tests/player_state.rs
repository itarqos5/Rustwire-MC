use rustwire_mc::{
    nbt::{Nbt, Tag},
    packet::{
        chat::ChatComponent,
        entity_state::*,
        inventory::{self, ComponentPatch, ItemData, ItemStack, Slot},
        player::*,
        typed::DecodedPacket,
        ProfileProperty,
    },
    version::{Direction, State},
    Error, Limits, Version,
};

// Synthetic wire fixtures are built independently, without Rustwire writers.
fn vint(b: &mut Vec<u8>, n: i32) {
    let mut n = n as u32;
    loop {
        let low = (n & 127) as u8;
        n >>= 7;
        b.push(low | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
fn string(b: &mut Vec<u8>, s: &str) {
    vint(b, s.len() as i32);
    b.extend(s.as_bytes());
}
fn full_player(v: Version) -> Vec<u8> {
    let mut b = vec![
        if v.protocol() >= 769 {
            255
        } else if v.protocol() == 768 {
            127
        } else {
            63
        },
        1,
    ];
    b.extend([11; 16]);
    string(&mut b, "Rustwire");
    b.push(2);
    string(&mut b, "textures");
    string(&mut b, "value");
    b.push(1);
    string(&mut b, "signature");
    string(&mut b, "textures");
    string(&mut b, "another");
    b.push(0);
    b.push(1);
    b.extend([22; 16]);
    b.extend((-123_i64).to_be_bytes());
    b.extend([3, 1, 2, 3, 2, 4, 5]); // bounded, deliberately unverified key envelope
    b.extend([3, 1]);
    vint(&mut b, -1);
    b.push(1);
    if v.protocol() < 765 {
        string(&mut b, "{\"text\":\"A\"}");
    } else {
        b.extend([8, 0, 1, b'A']);
    }
    if v.protocol() >= 768 {
        vint(&mut b, -129);
    }
    if v.protocol() >= 769 {
        b.push(1);
    }
    b
}
fn equipment(v: Version) -> Vec<u8> {
    let mut b = vec![0xac, 2, 0x80]; // entity300, main hand, continue
    if v.protocol() <= 765 {
        b.extend([1, 1, 2, 0]);
    } else {
        b.extend([2, 1, 0, 0]);
    }
    b.extend([5, 0]); // empty helmet, stop
    b
}
fn attributes(v: Version) -> Vec<u8> {
    let mut b = vec![0xac, 2, 1];
    if v.protocol() < 766 {
        string(&mut b, "minecraft:generic.max_health");
    } else {
        vint(&mut b, 300);
    } // unresolved version-specific registry ID
    b.extend(20_f64.to_be_bytes());
    b.push(3);
    for op in 0..3 {
        if v.protocol() < 767 {
            b.extend([op; 16]);
        } else {
            string(&mut b, &format!("example:m{op}"));
        }
        b.extend((-0.5_f64).to_be_bytes());
        b.push(op);
    }
    b
}
fn effect(v: Version) -> Vec<u8> {
    let mut b = vec![0xac, 2, 3];
    if v.protocol() < 766 {
        b.push(255);
    } else {
        vint(&mut b, 300);
    }
    vint(&mut b, -1);
    b.push(if v.protocol() < 766 { 7 } else { 15 });
    if v.protocol() < 766 {
        b.extend([1, 10]);
        if v.protocol() == 763 {
            b.extend([0, 0]);
        }
        b.extend([3, 0, 1, b'x', 0, 0, 0, 7, 0]);
    }
    b
}
fn fixture(name: &str, v: Version) -> Vec<u8> {
    match name {
        "player_info" => full_player(v),
        "player_remove" => {
            let mut b = vec![2];
            b.extend([11; 16]);
            b.extend([22; 16]);
            b
        }
        "entity_equipment" => equipment(v),
        "entity_update_attributes" => attributes(v),
        "entity_effect" => effect(v),
        "remove_entity_effect" => vec![0xac, 2, 3],
        _ => unreachable!(),
    }
}
fn encode(decoded: &DecodedPacket, v: Version, limits: Limits) -> rustwire_mc::Result<Vec<u8>> {
    match decoded {
        DecodedPacket::Player(PlayerPacket::Info(p)) => p.encode(v, limits),
        DecodedPacket::Player(PlayerPacket::Remove(p)) => p.encode(v, limits),
        DecodedPacket::EntityState(EntityStatePacket::Equipment(p)) => p.encode(v, limits),
        DecodedPacket::EntityState(EntityStatePacket::Attributes(p)) => p.encode(v, limits),
        DecodedPacket::EntityState(EntityStatePacket::Effect(p)) => p.encode(v, limits),
        DecodedPacket::EntityState(EntityStatePacket::RemoveEffect(p)) => p.encode(v, limits),
        p => panic!("wrong dispatch {p:?}"),
    }
}
const NAMES: &[&str] = &[
    "player_info",
    "player_remove",
    "entity_equipment",
    "entity_update_attributes",
    "entity_effect",
    "remove_entity_effect",
];
#[test]
fn independent_golden_bodies_all_six_families_all_fourteen_versions() {
    for &v in Version::ALL {
        for &name in NAMES {
            v.packet_id(State::Play, Direction::Clientbound, name)
                .unwrap();
            let b = fixture(name, v);
            let p = DecodedPacket::decode(State::Play, name, &b, v, Limits::default())
                .unwrap()
                .unwrap();
            assert_eq!(encode(&p, v, Limits::default()).unwrap(), b, "{v:?} {name}");
            assert!(
                DecodedPacket::decode(State::Configuration, name, &b, v, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        let p = PlayerInfo::decode(&full_player(v), v, Limits::default()).unwrap();
        assert_eq!(p.entries[0].game_mode, Some(GameMode::Spectator));
        assert_eq!(p.entries[0].latency, Some(-1));
        assert_eq!(p.entries[0].profile.as_ref().unwrap().properties.len(), 2);
        assert_eq!(
            p.entries[0]
                .chat_session
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .public_key,
            [1, 2, 3]
        );
        assert_eq!(
            p.entries[0].list_order,
            (v.protocol() >= 768).then_some(-129)
        );
        assert_eq!(p.entries[0].show_hat, (v.protocol() >= 769).then_some(true));
        let p = EntityEquipment::decode(&equipment(v), v, Limits::default()).unwrap();
        assert_eq!(p.entity_id, 300);
        assert_eq!(p.equipment[1].slot, EquipmentSlot::Head);
        assert_eq!(p.equipment[1].item, Slot::Empty);
        let p = EntityAttributes::decode(&attributes(v), v, Limits::default()).unwrap();
        assert_eq!(p.attributes[0].base_value, 20.0);
        assert_eq!(
            p.attributes[0].modifiers[2].operation,
            AttributeOperation::AddMultipliedTotal
        );
        let p = EntityEffect::decode(&effect(v), v, Limits::default()).unwrap();
        assert_eq!(p.duration, -1);
        assert_eq!(p.amplifier, if v.protocol() < 766 { 255 } else { 300 });
    }
}
#[test]
fn every_golden_prefix_trailing_byte_and_total_packet_budget_fails_closed() {
    for &v in Version::ALL {
        for &name in NAMES {
            let b = fixture(name, v);
            for end in 0..b.len() {
                assert!(
                    DecodedPacket::decode(State::Play, name, &b[..end], v, Limits::default())
                        .is_err(),
                    "{v:?} {name} prefix {end}"
                );
            }
            let mut trailing = b.clone();
            trailing.push(0);
            assert!(
                DecodedPacket::decode(State::Play, name, &trailing, v, Limits::default()).is_err()
            );
            let limits = Limits {
                max_packet: b.len() - 1,
                ..Limits::default()
            };
            assert!(matches!(
                DecodedPacket::decode(State::Play, name, &b, v, limits),
                Err(Error::Limit(_))
            ));
            let p = DecodedPacket::decode(State::Play, name, &b, v, Limits::default())
                .unwrap()
                .unwrap();
            assert!(matches!(encode(&p, v, limits), Err(Error::Limit(_))));
            let exact = Limits {
                max_packet: b.len(),
                ..Limits::default()
            };
            assert_eq!(encode(&p, v, exact).unwrap(), b);
        }
    }
}
#[test]
fn list_order_and_hat_use_official_action_ordinals() {
    for &v in Version::ALL {
        let mut order = vec![64, 1];
        order.extend([0; 16]);
        order.extend([0xac, 2]);
        let result = PlayerInfo::decode(&order, v, Limits::default());
        if v.protocol() < 768 {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().entries[0].list_order, Some(300));
        }
        let mut hat = vec![128, 1];
        hat.extend([0; 16]);
        hat.push(1);
        let result = PlayerInfo::decode(&hat, v, Limits::default());
        if v.protocol() < 769 {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().entries[0].show_hat, Some(true));
        }
    }
    assert!(PlayerInfoActions {
        update_list_order: true,
        ..Default::default()
    }
    .mask(Version::V1_21)
    .is_err());
    assert!(PlayerInfoActions {
        update_hat: true,
        ..Default::default()
    }
    .mask(Version::V1_21_2)
    .is_err());
}
#[test]
fn optional_roster_fields_distinguish_unchanged_from_explicit_clear() {
    for &v in Version::ALL {
        let mut b = vec![34, 1];
        b.extend([0; 16]);
        b.extend([0, 0]);
        let p = PlayerInfo::decode(&b, v, Limits::default()).unwrap();
        assert_eq!(p.entries[0].chat_session, Some(None));
        assert_eq!(p.entries[0].display_name, Some(None));
        assert_eq!(p.entries[0].profile, None);
        assert_eq!(p.encode(v, Limits::default()).unwrap(), b);
        let empty = PlayerInfo::decode(&[0, 0], v, Limits::default()).unwrap();
        assert!(empty.entries.is_empty());
        assert!(PlayerRemove::decode(&[0], v, Limits::default())
            .unwrap()
            .players
            .is_empty());
    }
}
#[test]
fn roster_mask_field_mismatch_duplicate_ids_and_invalid_enums_are_rejected() {
    let v = Version::V26_2;
    let mut p = PlayerInfo::decode(&full_player(v), v, Limits::default()).unwrap();
    p.entries[0].latency = None;
    assert!(p.encode(v, Limits::default()).is_err());
    for mask in [4, 8, 128] {
        let mut b = vec![mask, 1];
        b.extend([0; 16]);
        b.push(if mask == 4 { 4 } else { 2 });
        assert!(PlayerInfo::decode(&b, v, Limits::default()).is_err());
    }
    let mut b = vec![0, 2];
    b.extend([1; 32]);
    assert!(PlayerInfo::decode(&b, v, Limits::default()).is_err());
    let p = PlayerInfo {
        actions: Default::default(),
        entries: vec![PlayerInfoEntry::default(); 2],
    };
    assert!(p.encode(v, Limits::default()).is_err());
    let mut b = vec![2];
    b.extend([1; 32]);
    assert!(PlayerRemove::decode(&b, v, Limits::default()).is_err());
    assert!(PlayerRemove {
        players: vec![[1; 16]; 2]
    }
    .encode(v, Limits::default())
    .is_err());
}
fn profile_packet(properties: Vec<ProfileProperty>) -> PlayerInfo {
    PlayerInfo {
        actions: PlayerInfoActions {
            add_player: true,
            ..Default::default()
        },
        entries: vec![PlayerInfoEntry {
            profile: Some(PlayerProfile {
                name: "N".into(),
                properties,
            }),
            ..Default::default()
        }],
    }
}
#[test]
fn profile_property_limits_change_at_766_and_allow_repeated_property_names() {
    let one = ProfileProperty {
        name: "x".into(),
        value: "v".into(),
        signature: None,
    };
    for &v in Version::ALL {
        for p in [
            profile_packet(vec![one.clone(); 17]),
            profile_packet(vec![ProfileProperty {
                name: "n".repeat(65),
                ..one.clone()
            }]),
            profile_packet(vec![ProfileProperty {
                signature: Some("s".repeat(1025)),
                ..one.clone()
            }]),
        ] {
            let old = p.encode(Version::V1_20_3, Limits::default()).unwrap();
            assert_eq!(
                PlayerInfo::decode(&old, v, Limits::default()).is_ok(),
                v.protocol() < 766
            );
            assert_eq!(p.encode(v, Limits::default()).is_ok(), v.protocol() < 766);
        }
        let p = profile_packet(vec![
            ProfileProperty {
                name: "n".repeat(64),
                signature: Some("s".repeat(1024)),
                ..one.clone()
            };
            16
        ]);
        let bytes = p.encode(v, Limits::default()).unwrap();
        assert_eq!(PlayerInfo::decode(&bytes, v, Limits::default()).unwrap(), p);
    }
}
#[test]
fn username_string_and_shared_collection_limits() {
    let v = Version::V26_2;
    let mut p = profile_packet(vec![]);
    p.entries[0].profile.as_mut().unwrap().name = "x".repeat(17);
    assert!(p.encode(v, Limits::default()).is_err());
    let mut b = vec![1, 1];
    b.extend([0; 16]);
    string(&mut b, &"x".repeat(17));
    b.push(0);
    assert!(PlayerInfo::decode(&b, v, Limits::default()).is_err());
    let p = profile_packet(vec![
        ProfileProperty {
            name: "n".into(),
            value: "v".into(),
            signature: None
        };
        2
    ]);
    let b = p.encode(v, Limits::default()).unwrap();
    let limits = Limits {
        max_collection: 2,
        ..Limits::default()
    };
    assert!(matches!(p.encode(v, limits), Err(Error::Limit(_))));
    assert!(matches!(
        PlayerInfo::decode(&b, v, limits),
        Err(Error::Limit(_))
    ));
    let limits = Limits {
        max_string_chars: 0,
        ..Limits::default()
    };
    assert!(p.encode(v, limits).is_err());
    assert!(PlayerInfo::decode(&b, v, limits).is_err());
    for name in ["player_info", "player_remove", "entity_update_attributes"] {
        let b = fixture(name, v);
        let limits = Limits {
            max_collection: 0,
            ..Limits::default()
        };
        assert!(DecodedPacket::decode(State::Play, name, &b, v, limits).is_err());
    }
}
#[test]
fn chat_session_lengths_and_aggregate_budget_are_enforced_without_authentication() {
    let v = Version::V26_2;
    for (key, sig) in [(512, 4096), (513, 1), (1, 4097)] {
        let mut b = vec![2, 1];
        b.extend([0; 16]);
        b.push(1);
        b.extend([0; 24]);
        vint(&mut b, key);
        b.extend(vec![0; key as usize]);
        vint(&mut b, sig);
        b.extend(vec![0; sig as usize]);
        let result = PlayerInfo::decode(&b, v, Limits::default());
        if key == 512 {
            let p = result.unwrap();
            assert_eq!(p.encode(v, Limits::default()).unwrap(), b);
            let limits = Limits {
                max_collection: 4608,
                ..Limits::default()
            };
            assert!(p.encode(v, limits).is_err());
            assert!(PlayerInfo::decode(&b, v, limits).is_err());
        } else {
            assert!(result.is_err());
        }
    }
    let mut p = PlayerInfo::decode(&full_player(v), v, Limits::default()).unwrap();
    p.entries[0]
        .chat_session
        .as_mut()
        .unwrap()
        .as_mut()
        .unwrap()
        .public_key = vec![0; 513];
    assert!(p.encode(v, Limits::default()).is_err());
}
#[test]
fn display_components_share_nbt_node_budget_across_roster_entries() {
    let v = Version::V26_2;
    let mut b = vec![32, 2];
    for id in 1..=2 {
        b.extend([id; 16]);
        b.extend([1, 8, 0, 1, b'A']);
    }
    let p = PlayerInfo::decode(&b, v, Limits::default()).unwrap();
    let limits = Limits {
        max_nbt_nodes: 1,
        ..Limits::default()
    };
    assert!(p.encode(v, limits).is_err());
    assert!(PlayerInfo::decode(&b, v, limits).is_err());
    let mut invalid = p;
    invalid.entries[0].display_name = Some(Some(ChatComponent::Json("x".into())));
    assert!(invalid.encode(v, Limits::default()).is_err());
    let mut absent = vec![32, 1];
    absent.extend([0; 16]);
    absent.extend([1, 0]);
    assert!(PlayerInfo::decode(&absent, v, Limits::default()).is_err());
}
#[test]
fn equipment_body_and_saddle_boundaries_continuation_and_duplicates() {
    for &v in Version::ALL {
        for (slot, id, min) in [
            (EquipmentSlot::Body, 6, 766),
            (EquipmentSlot::Saddle, 7, 770),
        ] {
            let p = EntityEquipment {
                entity_id: 1,
                equipment: vec![EquipmentEntry {
                    slot,
                    item: Slot::Empty,
                }],
            };
            let b = [1, id, 0];
            assert_eq!(p.encode(v, Limits::default()).is_ok(), v.protocol() >= min);
            assert_eq!(
                EntityEquipment::decode(&b, v, Limits::default()).is_ok(),
                v.protocol() >= min
            );
        }
        for b in [
            &[1][..],
            &[1, 128, 0][..],
            &[1, 128, 0, 0, 0][..],
            &[1, 8, 0][..],
            &[1, 255, 0][..],
        ] {
            assert!(EntityEquipment::decode(b, v, Limits::default()).is_err());
        }
        assert!(EntityEquipment {
            entity_id: 1,
            equipment: vec![]
        }
        .encode(v, Limits::default())
        .is_err());
        let entry = EquipmentEntry {
            slot: EquipmentSlot::Head,
            item: Slot::Empty,
        };
        assert!(EntityEquipment {
            entity_id: 1,
            equipment: vec![entry; 2]
        }
        .encode(v, Limits::default())
        .is_err());
    }
}
#[test]
fn equipment_nested_slots_share_collection_and_nbt_budgets() {
    let v = Version::V1_20;
    let item = Slot::Item(ItemStack {
        item_id: 1,
        count: 1,
        data: ItemData::Legacy(Some(Nbt {
            name: Some("".into()),
            root: Tag::Compound(vec![]),
        })),
    });
    let p = EntityEquipment {
        entity_id: 1,
        equipment: vec![
            EquipmentEntry {
                slot: EquipmentSlot::MainHand,
                item: item.clone(),
            },
            EquipmentEntry {
                slot: EquipmentSlot::OffHand,
                item,
            },
        ],
    };
    let b = p.encode(v, Limits::default()).unwrap();
    for limits in [
        Limits {
            max_collection: 3,
            ..Limits::default()
        },
        Limits {
            max_nbt_nodes: 1,
            ..Limits::default()
        },
    ] {
        assert!(p.encode(v, limits).is_err());
        assert!(EntityEquipment::decode(&b, v, limits).is_err());
    }
    let exact = Limits {
        max_collection: 4,
        max_nbt_nodes: 2,
        ..Limits::default()
    };
    assert_eq!(p.encode(v, exact).unwrap(), b);
    assert_eq!(EntityEquipment::decode(&b, v, exact).unwrap(), p);
}
#[test]
fn unsupported_equipment_component_is_an_explicit_error() {
    let v = Version::V1_21_5;
    let mut b = vec![1, 0, 1, 1, 1, 0];
    vint(&mut b, i32::MAX);
    b.extend([1, 2, 3]);
    assert!(matches!(
        DecodedPacket::decode(State::Play, "entity_equipment", &b, v, Limits::default()),
        Err(Error::Unsupported(_))
    ));
}
#[test]
fn attribute_operation_is_byte_until_765_and_varint_from_766() {
    for &v in Version::ALL {
        let mut b = attributes(v);
        // Expand the final valid operation2 into a valid noncanonical VarInt.
        b.pop();
        b.extend([0x82, 0]);
        let p = EntityAttributes::decode(&b, v, Limits::default());
        assert_eq!(p.is_ok(), v.protocol() >= 766);
        let mut b = attributes(v);
        *b.last_mut().unwrap() = 3;
        assert!(EntityAttributes::decode(&b, v, Limits::default()).is_err());
    }
}
#[test]
fn attribute_ids_representations_duplicates_and_aggregate_limits() {
    for &v in Version::ALL {
        let mut p = EntityAttributes::decode(&attributes(v), v, Limits::default()).unwrap();
        let limited = Limits {
            max_collection: 3,
            ..Limits::default()
        };
        assert!(p.encode(v, limited).is_err());
        assert!(EntityAttributes::decode(&attributes(v), v, limited).is_err());
        let duplicate = p.attributes[0].modifiers[0].clone();
        p.attributes[0].modifiers.push(duplicate);
        assert!(p.encode(v, Limits::default()).is_err());
        let mut p = EntityAttributes::decode(&attributes(v), v, Limits::default()).unwrap();
        p.attributes.push(p.attributes[0].clone());
        assert!(p.encode(v, Limits::default()).is_err());
        let mut p = EntityAttributes::decode(&attributes(v), v, Limits::default()).unwrap();
        p.attributes[0].key = if v.protocol() < 766 {
            AttributeKey::Registry(0)
        } else {
            AttributeKey::Resource("example:a".into())
        };
        assert!(p.encode(v, Limits::default()).is_err());
        let mut p = EntityAttributes::decode(&attributes(v), v, Limits::default()).unwrap();
        p.attributes[0].modifiers[0].id = if v.protocol() < 767 {
            AttributeModifierId::Resource("example:a".into())
        } else {
            AttributeModifierId::Uuid([0; 16])
        };
        assert!(p.encode(v, Limits::default()).is_err());
    }
    for v in [Version::V1_20, Version::V26_2] {
        let mut b = vec![1, 2];
        for _ in 0..2 {
            if v.protocol() < 766 {
                string(&mut b, "example:a");
            } else {
                b.push(0);
            }
            b.extend(1_f64.to_be_bytes());
            b.push(0);
        }
        assert!(EntityAttributes::decode(&b, v, Limits::default()).is_err());
        let mut b = vec![1, 1];
        if v.protocol() < 766 {
            string(&mut b, "example:a");
        } else {
            b.push(0);
        }
        b.extend(1_f64.to_be_bytes());
        b.push(2);
        for _ in 0..2 {
            if v.protocol() < 767 {
                b.extend([0; 16]);
            } else {
                string(&mut b, "example:a");
            }
            b.extend(1_f64.to_be_bytes());
            b.push(0);
        }
        assert!(EntityAttributes::decode(&b, v, Limits::default()).is_err());
    }
}
#[test]
fn nonfinite_attribute_values_and_invalid_resource_names_are_rejected() {
    for &v in Version::ALL {
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut p = EntityAttributes::decode(&attributes(v), v, Limits::default()).unwrap();
            p.attributes[0].base_value = n;
            assert!(p.encode(v, Limits::default()).is_err());
            p.attributes[0].base_value = 1.0;
            p.attributes[0].modifiers[0].amount = n;
            assert!(p.encode(v, Limits::default()).is_err());
            let mut b = vec![1, 1];
            if v.protocol() < 766 {
                string(&mut b, "example:a");
            } else {
                b.push(0);
            }
            b.extend(n.to_be_bytes());
            b.push(0);
            assert!(EntityAttributes::decode(&b, v, Limits::default()).is_err());
        }
    }
    for name in ["Bad:a", "a:a:a", "a:?"] {
        let mut b = vec![1, 1];
        string(&mut b, name);
        b.extend(0_f64.to_be_bytes());
        b.push(0);
        assert!(EntityAttributes::decode(&b, Version::V1_20, Limits::default()).is_err());
    }
}
#[test]
fn effect_flags_factor_root_and_amplifier_boundaries() {
    for &v in Version::ALL {
        let p = EntityEffect::decode(&effect(v), v, Limits::default()).unwrap();
        let mut invalid = p.clone();
        invalid.flags.blend = true;
        assert_eq!(
            invalid.encode(v, Limits::default()).is_ok(),
            v.protocol() >= 766
        );
        let mut invalid = p.clone();
        invalid.amplifier = -1;
        assert!(invalid.encode(v, Limits::default()).is_err());
        let mut invalid = p.clone();
        invalid.amplifier = 256;
        assert_eq!(
            invalid.encode(v, Limits::default()).is_ok(),
            v.protocol() >= 766
        );
        let mut invalid = p;
        invalid.factor_data = Some(Nbt {
            name: None,
            root: Tag::Int(7),
        });
        assert!(invalid.encode(v, Limits::default()).is_err());
        let mut b = vec![1, 1, 0, 0, 16];
        if v.protocol() < 766 {
            b.push(0);
        }
        assert!(EntityEffect::decode(&b, v, Limits::default()).is_err());
        assert!(EffectFlags::from_mask(128, v).is_err());
    }
    for &v in &[Version::V1_20, Version::V1_20_2, Version::V1_20_3] {
        let mut b = vec![1, 1, 0, 0, 0, 1, 3];
        if v.protocol() == 763 {
            b.extend([0, 0]);
        }
        b.extend(0_i32.to_be_bytes());
        assert!(EntityEffect::decode(&b, v, Limits::default()).is_err());
        let limits = Limits {
            max_nbt_nodes: 1,
            ..Limits::default()
        };
        assert!(EntityEffect::decode(&effect(v), v, limits).is_err());
        let p = EntityEffect::decode(&effect(v), v, Limits::default()).unwrap();
        assert!(p.encode(v, limits).is_err());
    }
}
#[test]
fn negative_ids_and_huge_lengths_fail_without_allocation_or_fallback() {
    for &v in Version::ALL {
        for name in [
            "entity_equipment",
            "entity_update_attributes",
            "entity_effect",
            "remove_entity_effect",
        ] {
            let b = [255, 255, 255, 255, 15];
            assert!(matches!(
                DecodedPacket::decode(State::Play, name, &b, v, Limits::default()),
                Err(Error::Invalid(_))
            ));
        }
        for name in ["player_info", "player_remove", "entity_update_attributes"] {
            let mut b = if name == "player_remove" {
                vec![]
            } else {
                vec![0]
            };
            vint(&mut b, i32::MAX);
            assert!(matches!(
                DecodedPacket::decode(State::Play, name, &b, v, Limits::default()),
                Err(Error::Limit(_))
            ));
        }
        let empty = EntityAttributes {
            entity_id: 0,
            attributes: vec![],
        };
        assert_eq!(empty.encode(v, Limits::default()).unwrap(), [0, 0]);
    }
}
#[test]
fn equipment_components_roundtrip_and_truncated_known_components_are_errors() {
    for &v in &Version::ALL[3..] {
        let p = EntityEquipment {
            entity_id: 1,
            equipment: vec![EquipmentEntry {
                slot: EquipmentSlot::Chest,
                item: Slot::Item(ItemStack {
                    item_id: 1,
                    count: 1,
                    data: ItemData::Components(ComponentPatch {
                        added: vec![],
                        removed: vec![],
                    }),
                }),
            }],
        };
        let b = p.encode(v, Limits::default()).unwrap();
        assert_eq!(
            EntityEquipment::decode(&b, v, Limits::default()).unwrap(),
            p
        );
        let mut b = vec![1, 0, 1, 1, 1, 0];
        vint(&mut b, inventory::component_id(v, "damage").unwrap());
        assert!(matches!(
            DecodedPacket::decode(State::Play, "entity_equipment", &b, v, Limits::default()),
            Err(Error::Eof)
        ));
    }
}

struct Duplex {
    input: std::io::Cursor<Vec<u8>>,
    output: Vec<u8>,
}
impl std::io::Read for Duplex {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.input, b)
    }
}
impl std::io::Write for Duplex {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.output.extend(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn connection(
    packet: rustwire_mc::frame::RawPacket,
) -> rustwire_mc::connection::Connection<Duplex> {
    use rustwire_mc::{
        connection::{Connection, Event, TypedEvent},
        frame::{FrameCodec, RawPacket},
    };
    let v = Version::V1_21_5;
    let codec = FrameCodec::default();
    let mut success = vec![0; 16];
    success.extend([1, b'N', 0]);
    let mut input = codec.encode(&RawPacket::new(2, success)).unwrap();
    input.extend(
        codec
            .encode(&RawPacket::new(
                v.packet_id(
                    State::Configuration,
                    Direction::Clientbound,
                    "finish_configuration",
                )
                .unwrap(),
                vec![],
            ))
            .unwrap(),
    );
    input.extend(codec.encode(&packet).unwrap());
    let mut c = Connection::new(
        Duplex {
            input: std::io::Cursor::new(input),
            output: vec![],
        },
        v,
        Limits::default(),
    );
    c.start_login("localhost", 25565, "N", [0; 16]).unwrap();
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    assert!(matches!(
        c.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    c
}
#[test]
fn connection_dispatch_exposes_new_families_and_rejects_malformed_known_packets() {
    use rustwire_mc::{connection::TypedEvent, frame::RawPacket};
    let v = Version::V1_21_5;
    for &name in NAMES {
        let id = v
            .packet_id(State::Play, Direction::Clientbound, name)
            .unwrap();
        let p = RawPacket::new(id, fixture(name, v));
        assert!(matches!(
            connection(p).next_typed_event().unwrap(),
            TypedEvent::Decoded(_)
        ));
        assert!(connection(RawPacket::new(id, vec![]))
            .next_typed_event()
            .is_err());
    }
    let mut b = vec![1, 0, 1, 1, 1, 0];
    vint(&mut b, i32::MAX);
    b.extend([9, 8, 7]);
    let p = RawPacket::new(
        v.packet_id(State::Play, Direction::Clientbound, "entity_equipment")
            .unwrap(),
        b,
    );
    let expected = p.clone();
    match connection(p).next_typed_event().unwrap() {
        TypedEvent::Raw {
            packet,
            name: Some("entity_equipment"),
            unsupported: Some(_),
            ..
        } => assert_eq!(packet, expected),
        other => panic!("unexpected {other:?}"),
    }
}
#[test]
fn bounded_deterministic_mutations_do_not_panic() {
    let limits = Limits {
        max_packet: 4096,
        max_collection: 64,
        max_nbt_nodes: 64,
        max_nbt_depth: 8,
        max_string_chars: 64,
        ..Limits::default()
    };
    let mut state = 0x83b9_46f2_u32;
    for &v in Version::ALL {
        for &name in NAMES {
            let original = fixture(name, v);
            for _ in 0..128 {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let mut b = original.clone();
                let index = (state as usize) % b.len();
                b[index] = (state >> 24) as u8;
                if state & 1 != 0 {
                    b.truncate(index + 1);
                }
                let _ = DecodedPacket::decode(State::Play, name, &b, v, limits);
            }
        }
    }
}

#[test]
fn default_namespace_aliases_preserve_text_but_do_not_bypass_duplicate_checks() {
    for v in [Version::V1_20, Version::V26_2] {
        for alias in ["foo", ":foo"] {
            let p = if v.protocol() < 766 {
                EntityAttributes {
                    entity_id: 1,
                    attributes: vec![EntityAttribute {
                        key: AttributeKey::Resource(alias.into()),
                        base_value: 1.0,
                        modifiers: vec![],
                    }],
                }
            } else {
                EntityAttributes {
                    entity_id: 1,
                    attributes: vec![EntityAttribute {
                        key: AttributeKey::Registry(1),
                        base_value: 1.0,
                        modifiers: vec![AttributeModifier {
                            id: AttributeModifierId::Resource(alias.into()),
                            amount: 1.0,
                            operation: AttributeOperation::AddValue,
                        }],
                    }],
                }
            };
            let b = p.encode(v, Limits::default()).unwrap();
            assert_eq!(
                EntityAttributes::decode(&b, v, Limits::default()).unwrap(),
                p
            );
            let mut duplicate = p.clone();
            let mut b = vec![1];
            if v.protocol() < 766 {
                duplicate.attributes.push(EntityAttribute {
                    key: AttributeKey::Resource("minecraft:foo".into()),
                    ..p.attributes[0].clone()
                });
                b.push(2);
                for name in [alias, "minecraft:foo"] {
                    string(&mut b, name);
                    b.extend(1_f64.to_be_bytes());
                    b.push(0);
                }
            } else {
                duplicate.attributes[0].modifiers.push(AttributeModifier {
                    id: AttributeModifierId::Resource("minecraft:foo".into()),
                    ..p.attributes[0].modifiers[0].clone()
                });
                b.extend([1, 1]);
                b.extend(1_f64.to_be_bytes());
                b.push(2);
                for name in [alias, "minecraft:foo"] {
                    string(&mut b, name);
                    b.extend(1_f64.to_be_bytes());
                    b.push(0);
                }
            }
            assert!(duplicate.encode(v, Limits::default()).is_err());
            assert!(EntityAttributes::decode(&b, v, Limits::default()).is_err());
        }
    }
}

#[test]
fn resource_syntax_empty_paths_and_26_1_namespace_boundary_follow_server_codec() {
    for &v in Version::ALL {
        for value in ["", ":", "minecraft:", "example:", "..:foo"] {
            let p = if v.protocol() < 766 {
                EntityAttributes {
                    entity_id: 1,
                    attributes: vec![EntityAttribute {
                        key: AttributeKey::Resource(value.into()),
                        base_value: 1.0,
                        modifiers: vec![],
                    }],
                }
            } else {
                EntityAttributes {
                    entity_id: 1,
                    attributes: vec![EntityAttribute {
                        key: AttributeKey::Registry(1),
                        base_value: 1.0,
                        modifiers: vec![AttributeModifier {
                            id: AttributeModifierId::Resource(value.into()),
                            amount: 1.0,
                            operation: AttributeOperation::AddValue,
                        }],
                    }],
                }
            };
            // At766 modifier identifiers are still UUIDs; the string-bearing
            // modern representation starts at767.
            if v.protocol() == 766 {
                continue;
            }
            let valid = !(v.protocol() >= 775 && value == "..:foo");
            let mut b = vec![1, 1];
            if v.protocol() < 766 {
                string(&mut b, value);
                b.extend(1_f64.to_be_bytes());
                b.push(0);
            } else {
                b.push(1);
                b.extend(1_f64.to_be_bytes());
                b.push(1);
                string(&mut b, value);
                b.extend(1_f64.to_be_bytes());
                b.push(0);
            }
            assert_eq!(
                p.encode(v, Limits::default()).is_ok(),
                valid,
                "{v:?} {value}"
            );
            let result = EntityAttributes::decode(&b, v, Limits::default());
            assert_eq!(result.is_ok(), valid, "{v:?} {value}");
            if valid {
                assert_eq!(result.unwrap(), p);
            }
        }
    }
}
