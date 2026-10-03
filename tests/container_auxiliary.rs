//! Independent bodies emitted and checked by ContainerAuxiliaryOracle.java.
use rustwire_mc::{
    nbt::{Nbt, Tag},
    packet::{
        inventory::*,
        typed::{DecodedPacket, InventoryPacket},
    },
    version::{Direction, State},
    Error, Limits, Version,
};
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn fixtures() -> impl Iterator<Item = (Version, &'static str, Vec<u8>, Vec<i32>)> {
    include_str!("fixtures/container-auxiliary.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('|').collect();
            (
                Version::from_protocol(fields[0].parse().unwrap()).unwrap(),
                fields[1],
                hex(fields[2]),
                fields[3]
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse().unwrap())
                    .collect(),
            )
        })
}
fn roundtrip(
    label: &str,
    bytes: &[u8],
    version: Version,
    limits: Limits,
) -> rustwire_mc::Result<Vec<u8>> {
    if label.starts_with("property_") {
        ContainerProperty::decode(bytes, version, limits)?.encode(version, limits)
    } else if label.starts_with("horse_") {
        OpenMountScreen::decode(bytes, version, limits)?.encode(version, limits)
    } else if label.starts_with("button_") {
        ContainerButtonClick::decode(bytes, version, limits)?.encode(version, limits)
    } else if label.starts_with("select_") {
        SelectTrade::decode(bytes, version, limits)?.encode(version, limits)
    } else if label == "cooldown" {
        SetCooldown::decode(bytes, version, limits)?.encode(version, limits)
    } else {
        MerchantOffers::decode(bytes, version, limits)?.encode(version, limits)
    }
}
fn merchant_fixture(protocol: i32, name: &str) -> (Version, Vec<u8>) {
    let (v, _, bytes, _) = fixtures()
        .find(|(v, label, _, _)| v.protocol() == protocol && *label == name)
        .unwrap();
    (v, bytes)
}
#[test]
fn all_release_oracle_bodies_decode_encode_and_reject_truncation() {
    let mut protocols = std::collections::BTreeSet::new();
    for (version, label, bytes, _) in fixtures() {
        protocols.insert(version.protocol());
        assert_eq!(
            roundtrip(label, &bytes, version, Limits::default()).unwrap(),
            bytes,
            "{version:?} {label}"
        );
        for end in 0..bytes.len() {
            assert!(
                roundtrip(label, &bytes[..end], version, Limits::default()).is_err(),
                "{version:?} {label} prefix {end}"
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(roundtrip(label, &trailing, version, Limits::default()).is_err());
        assert!(matches!(
            roundtrip(
                label,
                &bytes,
                version,
                Limits {
                    max_packet: bytes.len() - 1,
                    ..Limits::default()
                }
            ),
            Err(Error::Limit(_))
        ));
    }
    assert_eq!(protocols.len(), 14);
}
#[test]
fn release_integer_domains_and_serverbound_catalog_ids() {
    for (v, label, bytes, fields) in fixtures() {
        let l = Limits::default();
        let actual = if label.starts_with("property_") {
            let p = ContainerProperty::decode(&bytes, v, l).unwrap();
            vec![p.window_id, p.property as i32, p.value as i32]
        } else if label.starts_with("horse_") {
            let p = OpenMountScreen::decode(&bytes, v, l).unwrap();
            vec![p.window_id, p.inventory_size, p.entity_id]
        } else if label.starts_with("button_") {
            let p = ContainerButtonClick::decode(&bytes, v, l).unwrap();
            let raw = p.packet(v, l).unwrap();
            assert_eq!(
                raw.id,
                v.packet_id(State::Play, Direction::Serverbound, "enchant_item")
                    .unwrap()
            );
            assert_eq!(raw.data, bytes);
            vec![p.window_id, p.button_id]
        } else if label.starts_with("select_") {
            let p = SelectTrade::decode(&bytes, v, l).unwrap();
            let raw = p.packet(v, l).unwrap();
            assert_eq!(
                raw.id,
                v.packet_id(State::Play, Direction::Serverbound, "select_trade")
                    .unwrap()
            );
            assert_eq!(raw.data, bytes);
            vec![p.slot]
        } else {
            continue;
        };
        assert_eq!(actual, fields, "{v:?} {label}");
    }
}
#[test]
fn narrowing_encoders_reject_instead_of_truncating() {
    let l = Limits::default();
    for &v in Version::ALL {
        for id in [-1, 256, 300] {
            let property = ContainerProperty {
                window_id: id,
                property: 0,
                value: 0,
            };
            let mount = OpenMountScreen {
                window_id: id,
                inventory_size: -1,
                entity_id: i32::MIN,
            };
            assert_eq!(property.encode(v, l).is_ok(), v.protocol() >= 768);
            assert_eq!(mount.encode(v, l).is_ok(), v.protocol() >= 768);
        }
        for value in [-129, 128, 300] {
            assert_eq!(
                ContainerButtonClick {
                    window_id: value,
                    button_id: value
                }
                .encode(v, l)
                .is_ok(),
                v.protocol() >= 766
            );
        }
    }
}
#[test]
fn merchant_layouts_component_values_and_float_payloads() {
    for (v, label, bytes, _) in fixtures() {
        if !(label.starts_with("trades_")
            || label.starts_with("price_")
            || label.starts_with("cost_count_")
            || label.starts_with("merchant_empty_"))
        {
            continue;
        }
        let p = MerchantOffers::decode(&bytes, v, Limits::default()).unwrap();
        if label.starts_with("merchant_empty_") {
            assert!(p.offers.is_empty());
            let id: i32 = label.trim_start_matches("merchant_empty_").parse().unwrap();
            assert_eq!(p.window_id, id);
            continue;
        }
        assert_eq!(
            (
                p.window_id,
                p.villager_level,
                p.experience,
                p.show_progress,
                p.can_restock
            ),
            (7, -1, -2, true, false)
        );
        assert_eq!(p.offers.len(), 1);
        let offer = &p.offers[0];
        assert_eq!(
            (
                offer.disabled,
                offer.uses,
                offer.max_uses,
                offer.xp,
                offer.special_price,
                offer.demand
            ),
            (false, 2, 10, -2, -3, -4)
        );
        let bits = if let Some(bits) = label.strip_prefix("price_") {
            u32::from_str_radix(bits, 16).unwrap()
        } else {
            0x3fc00000
        };
        assert_eq!(offer.price_multiplier.to_bits(), bits);
        let Slot::Item(result) = &offer.result else {
            panic!("missing result")
        };
        assert_eq!((result.item_id, result.count), (1, 2));
        let has_second = label.starts_with("trades_true_");
        match &offer.inputs {
            MerchantInputs::Legacy { first, second } => {
                assert!(v.protocol() < 766);
                let Slot::Item(first) = first else {
                    panic!("missing input")
                };
                assert_eq!((first.item_id, first.count), (1, 3));
                assert_eq!(!matches!(second, Slot::Empty), has_second);
            }
            MerchantInputs::Components { first, second } => {
                assert!(v.protocol() >= 766);
                assert_eq!(first.item_id, 1);
                let count = label
                    .strip_prefix("cost_count_")
                    .map(|s| s.parse().unwrap())
                    .unwrap_or(3);
                assert_eq!(first.count, count);
                assert_eq!(second.is_some(), has_second);
                if label.starts_with("trades_") && label.ends_with("_true") {
                    assert_eq!(
                        first.components,
                        vec![Component {
                            name: "custom_data",
                            value: ComponentValue::Nbt(Nbt::anonymous(Tag::Compound(vec![])))
                        }]
                    );
                } else {
                    assert!(first.components.is_empty());
                }
            }
        }
    }
}
#[test]
fn clientbound_dispatch_and_cooldown_boundaries() {
    for (v, label, bytes, _) in fixtures() {
        let name = if label.starts_with("property_") {
            "craft_progress_bar"
        } else if label.starts_with("horse_") {
            "open_horse_window"
        } else if label == "cooldown" {
            "set_cooldown"
        } else if label.starts_with("trades_") {
            "trade_list"
        } else {
            continue;
        };
        assert!(matches!(
            DecodedPacket::decode(State::Play, name, &bytes, v, Limits::default()).unwrap(),
            Some(DecodedPacket::Inventory(_))
        ));
        assert!(
            DecodedPacket::decode(State::Configuration, name, &bytes, v, Limits::default())
                .unwrap()
                .is_none()
        );
        if label == "cooldown" {
            let p = SetCooldown::decode(&bytes, v, Limits::default()).unwrap();
            assert_eq!(p.ticks, -1);
            assert_eq!(
                p.target,
                if v.protocol() < 768 {
                    CooldownTarget::Item(1)
                } else {
                    CooldownTarget::Group("minecraft:probe".into())
                }
            );
            let mismatched = SetCooldown {
                target: if v.protocol() < 768 {
                    CooldownTarget::Group("x".into())
                } else {
                    CooldownTarget::Item(1)
                },
                ticks: 0,
            };
            assert!(matches!(
                mismatched.encode(v, Limits::default()),
                Err(Error::Unsupported(_))
            ));
        }
    }
    let v = Version::V1_21_5;
    assert!(SetCooldown {
        target: CooldownTarget::Group("Bad:Name".into()),
        ticks: 1
    }
    .encode(v, Limits::default())
    .is_err());
    assert!(SetCooldown {
        target: CooldownTarget::Group("minecraft:probe".into()),
        ticks: 1
    }
    .encode(
        v,
        Limits {
            max_string_chars: 3,
            ..Limits::default()
        }
    )
    .is_err());
}
#[test]
fn merchant_budgets_are_shared_by_costs_results_components_and_nbt() {
    let (v, bytes) = merchant_fixture(770, "trades_true_true");
    let mut p = MerchantOffers::decode(&bytes, v, Limits::default()).unwrap();
    for max_collection in [0, 1, 4] {
        let l = Limits {
            max_collection,
            ..Limits::default()
        };
        assert!(matches!(
            MerchantOffers::decode(&bytes, v, l),
            Err(Error::Limit(_))
        ));
        assert!(matches!(p.encode(v, l), Err(Error::Limit(_))));
    }
    let l = Limits {
        max_collection: 5,
        ..Limits::default()
    };
    assert_eq!(p.encode(v, l).unwrap(), bytes);
    assert!(MerchantOffers::decode(&bytes, v, l).is_ok());
    if let MerchantInputs::Components {
        first,
        second: Some(second),
    } = &mut p.offers[0].inputs
    {
        second.components = first.components.clone();
    } else {
        panic!()
    }
    let bytes = p.encode(v, Limits::default()).unwrap();
    for max_nbt_nodes in [0, 1] {
        let l = Limits {
            max_nbt_nodes,
            ..Limits::default()
        };
        assert!(matches!(
            MerchantOffers::decode(&bytes, v, l),
            Err(Error::Limit(_))
        ));
        assert!(matches!(p.encode(v, l), Err(Error::Limit(_))));
    }
    let l = Limits {
        max_nbt_nodes: 2,
        ..Limits::default()
    };
    assert!(MerchantOffers::decode(&bytes, v, l).is_ok());
    assert!(p.encode(v, l).is_ok());
    let l = Limits {
        max_nbt_depth: 0,
        ..Limits::default()
    };
    assert!(MerchantOffers::decode(&bytes, v, l).is_err());
    assert!(p.encode(v, l).is_err());
}
#[test]
fn invalid_collections_booleans_ids_and_cross_release_inputs_fail() {
    let (v, bytes) = merchant_fixture(770, "trades_false_false");
    for data in [
        hex("07ffffffff07"),
        hex("07ffffffff0f"),
        hex("07010103ffffffff07"),
        hex("0701ffffffff0f03"),
        hex("07010103ffffffff0f"),
    ] {
        assert!(MerchantOffers::decode(
            &data,
            v,
            Limits {
                max_collection: 100,
                ..Limits::default()
            }
        )
        .is_err());
    }
    let mut malformed = bytes.clone();
    malformed[9] = 2; // Optional second-cost discriminator.
    assert!(MerchantOffers::decode(&malformed, v, Limits::default()).is_err());
    let mut malformed = bytes.clone();
    *malformed.last_mut().unwrap() = 2;
    assert!(MerchantOffers::decode(&malformed, v, Limits::default()).is_err());
    let p = MerchantOffers::decode(&bytes, v, Limits::default()).unwrap();
    assert!(matches!(
        p.encode(Version::from_protocol(763).unwrap(), Limits::default()),
        Err(Error::Unsupported(_))
    ));
    let (old, bytes) = merchant_fixture(763, "trades_false_false");
    let p = MerchantOffers::decode(&bytes, old, Limits::default()).unwrap();
    assert!(matches!(
        p.encode(v, Limits::default()),
        Err(Error::Unsupported(_))
    ));
    let mut unknown = hex("0701010301ffffff7f");
    unknown.extend([9, 8, 7, 6]);
    assert!(matches!(
        InventoryPacket::decode("trade_list", &unknown, v, Limits::default()),
        Err(Error::Unsupported(_))
    ));
}

#[test]
fn modern_merchant_results_require_nonempty_slots() {
    for protocol in 766..=776 {
        let (v, bytes) = merchant_fixture(protocol, "trades_false_false");
        let mut p = MerchantOffers::decode(&bytes, v, Limits::default()).unwrap();
        p.offers[0].result = Slot::Empty;
        assert!(matches!(
            p.encode(v, Limits::default()),
            Err(Error::Invalid(_))
        ));
        let mut empty = bytes[..5].to_vec();
        empty.push(0);
        empty.extend_from_slice(&bytes[9..]);
        assert!(matches!(
            MerchantOffers::decode(&empty, v, Limits::default()),
            Err(Error::Invalid(_))
        ));
    }
}
