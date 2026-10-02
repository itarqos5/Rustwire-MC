use rustwire_mc::{codec::BlockPosition, packet::interact::*, Version};

// Independent schema-derived packet IDs, indexed by protocol 763..=776.
const SWING: [i32; 14] = [47, 50, 51, 54, 54, 56, 58, 59, 60, 60, 60, 60, 63, 62];
const COMMAND: [i32; 14] = [7, 8, 8, 9, 9, 10, 10, 10, 11, 11, 11, 11, 12, 12];
const ABILITIES: [i32; 14] = [28, 31, 32, 35, 35, 37, 38, 38, 39, 39, 39, 39, 40, 40];
const INPUT: [i32; 14] = [31, 34, 35, 38, 38, 40, 41, 41, 42, 42, 42, 42, 43, 43];
const DIG: [i32; 14] = [29, 32, 33, 36, 36, 38, 39, 39, 40, 40, 40, 40, 41, 41];
const PLACE: [i32; 14] = [49, 52, 53, 56, 56, 58, 60, 62, 63, 63, 63, 63, 66, 65];
const ITEM: [i32; 14] = [50, 53, 54, 57, 57, 59, 61, 63, 64, 64, 64, 64, 67, 66];
const ACTION: [i32; 14] = [30, 33, 34, 37, 37, 39, 40, 40, 41, 41, 41, 41, 42, 42];
const ENTITY: [i32; 14] = [16, 18, 19, 22, 22, 24, 24, 24, 25, 25, 25, 25, 26, 26];

#[test]
fn control_packets_all_families() {
    for (i, &version) in Version::ALL.iter().enumerate() {
        let swing = swing_arm(version, Hand::Off).unwrap();
        assert_eq!((swing.id, swing.data), (SWING[i], vec![1]));
        let command = ClientCommand::Respawn.encode(version).unwrap();
        assert_eq!((command.id, command.data), (COMMAND[i], vec![0]));
        assert_eq!(
            ClientCommand::RequestStatistics
                .encode(version)
                .unwrap()
                .data,
            [1]
        );
        let abilities = player_abilities(version, true).unwrap();
        assert_eq!((abilities.id, abilities.data), (ABILITIES[i], vec![2]));
        assert_eq!(player_abilities(version, false).unwrap().data, [0]);
        if version.protocol() >= 768 {
            let tick = tick_end(version).unwrap();
            assert_eq!(
                tick.id,
                if version.protocol() >= 775 {
                    13
                } else if version.protocol() >= 771 {
                    12
                } else {
                    11
                }
            );
            assert!(tick.data.is_empty());
        } else {
            assert!(tick_end(version).is_err());
        }
    }
}
#[test]
fn analogue_and_key_input_exact_fixtures() {
    let legacy = PlayerInput::Legacy {
        sideways: -0.5,
        forward: 1.0,
        jump: true,
        dismount: true,
    };
    let keys = PlayerInput::Keys(InputKeys {
        forward: true,
        left: true,
        jump: true,
        sprint: true,
        ..Default::default()
    });
    assert_eq!(
        InputKeys {
            forward: true,
            backward: true,
            left: true,
            right: true,
            jump: true,
            shift: true,
            sprint: true
        }
        .bits(),
        0x7f
    );
    for (i, &version) in Version::ALL.iter().enumerate() {
        let input = if version.protocol() < 768 {
            legacy
        } else {
            keys
        };
        let packet = input.encode(version).unwrap();
        assert_eq!(packet.id, INPUT[i]);
        let expected = if version.protocol() < 768 {
            vec![0xbf, 0, 0, 0, 0x3f, 0x80, 0, 0, 3]
        } else {
            vec![0x55]
        };
        assert_eq!(packet.data, expected);
        assert!(if version.protocol() < 768 {
            keys
        } else {
            legacy
        }
        .encode(version)
        .is_err());
    }
}
#[test]
fn digging_exact_packed_position_and_sequence() {
    let action = Digging {
        action: DiggingAction::Finish,
        position: BlockPosition { x: -1, y: 64, z: 2 },
        face: BlockFace::East,
        sequence: 130,
    };
    for (i, &version) in Version::ALL.iter().enumerate() {
        let packet = action.encode(version).unwrap();
        assert_eq!(packet.id, DIG[i]);
        assert_eq!(
            packet.data,
            [2, 0xff, 0xff, 0xff, 0xc0, 0, 0, 0x20, 0x40, 5, 0x82, 1]
        );
    }
}
#[test]
fn use_block_border_field_boundary() {
    let block = UseBlock {
        hand: Hand::Off,
        position: BlockPosition { x: 0, y: 0, z: 0 },
        face: BlockFace::Top,
        cursor: [0.5, 1.0, 0.0],
        inside_block: true,
        world_border_hit: false,
        sequence: 130,
    };
    for (i, &version) in Version::ALL.iter().enumerate() {
        let packet = block.encode(version).unwrap();
        assert_eq!(packet.id, PLACE[i]);
        let mut expected = vec![
            1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0x3f, 0, 0, 0, 0x3f, 0x80, 0, 0, 0, 0, 0, 0, 1,
        ];
        if version.protocol() >= 768 {
            expected.push(0);
        }
        expected.extend([0x82, 1]);
        assert_eq!(packet.data, expected);
        assert_eq!(
            UseBlock {
                world_border_hit: true,
                ..block
            }
            .encode(version)
            .is_ok(),
            version.protocol() >= 768
        );
    }
}
#[test]
fn use_item_rotation_boundary() {
    for (i, &version) in Version::ALL.iter().enumerate() {
        let item = UseItem {
            hand: Hand::Main,
            sequence: 130,
            rotation: (version.protocol() >= 767).then_some([90.0, -45.0]),
        };
        let packet = item.encode(version).unwrap();
        assert_eq!(packet.id, ITEM[i]);
        let mut expected = vec![0, 0x82, 1];
        if version.protocol() >= 767 {
            expected.extend([0x42, 0xb4, 0, 0, 0xc2, 0x34, 0, 0]);
        }
        assert_eq!(packet.data, expected);
        assert!(UseItem {
            rotation: if item.rotation.is_some() {
                None
            } else {
                Some([0.0, 0.0])
            },
            ..item
        }
        .encode(version)
        .is_err());
    }
}
#[test]
fn entity_action_ids_shift_at_771() {
    for (i, &version) in Version::ALL.iter().enumerate() {
        let packet = EntityAction::StartHorseJump(100)
            .encode(version, 42)
            .unwrap();
        assert_eq!(packet.id, ACTION[i]);
        assert_eq!(
            packet.data,
            [42, if version.protocol() < 771 { 5 } else { 3 }, 100]
        );
        assert_eq!(
            EntityAction::StartSneaking.encode(version, 42).is_ok(),
            version.protocol() < 771
        );
        assert_eq!(
            EntityAction::StopSneaking.encode(version, 42).is_ok(),
            version.protocol() < 771
        );
        assert!(EntityAction::StartHorseJump(101)
            .encode(version, 42)
            .is_err());
    }
}
#[test]
fn entity_interaction_and_dedicated_attack_boundaries() {
    for (i, &version) in Version::ALL.iter().enumerate() {
        let attack = UseEntity {
            target: 130,
            interaction: EntityInteraction::Attack,
            sneaking: false,
        }
        .encode(version)
        .unwrap();
        if version.protocol() >= 775 {
            assert_eq!((attack.id, attack.data), (1, vec![0x82, 1]));
        } else {
            assert_eq!((attack.id, attack.data), (ENTITY[i], vec![0x82, 1, 1, 0]));
        }
        let interact = UseEntity {
            target: 130,
            interaction: EntityInteraction::Interact { hand: Hand::Off },
            sneaking: true,
        }
        .encode(version);
        if version.protocol() >= 775 {
            assert!(interact.is_err());
        } else {
            assert_eq!(interact.unwrap().data, [0x82, 1, 0, 1, 1]);
        }
        let at = UseEntity {
            target: 130,
            interaction: EntityInteraction::InteractAt {
                hand: Hand::Off,
                location: [0.0; 3],
            },
            sneaking: true,
        }
        .encode(version)
        .unwrap();
        assert_eq!(at.id, ENTITY[i]);
        if version.protocol() >= 775 {
            // The verified lpVec3 codec encodes the zero vector as one zero byte.
            assert_eq!(at.data, [0x82, 1, 1, 0, 1]);
        } else {
            assert_eq!(
                at.data,
                [0x82, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1]
            );
        }
    }
}
#[test]
fn invalid_coordinates_ranges_and_non_finite_values() {
    let version = Version::V1_20;
    let dig = Digging {
        action: DiggingAction::Start,
        position: BlockPosition { x: 0, y: 0, z: 0 },
        face: BlockFace::Top,
        sequence: 0,
    };
    assert!(Digging {
        sequence: u32::MAX,
        ..dig
    }
    .encode(version)
    .is_err());
    assert!(Digging {
        position: BlockPosition {
            x: 33_554_432,
            y: 0,
            z: 0
        },
        ..dig
    }
    .encode(version)
    .is_err());
    assert!(Digging {
        position: BlockPosition {
            x: 0,
            y: 2048,
            z: 0
        },
        ..dig
    }
    .encode(version)
    .is_err());
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1.01, -1.01] {
        assert!(PlayerInput::Legacy {
            sideways: invalid,
            forward: 0.0,
            jump: false,
            dismount: false
        }
        .encode(version)
        .is_err());
    }
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1.01, -0.01] {
        assert!(UseBlock {
            hand: Hand::Main,
            position: dig.position,
            face: BlockFace::Top,
            cursor: [invalid, 0.0, 0.0],
            inside_block: false,
            world_border_hit: false,
            sequence: 0
        }
        .encode(version)
        .is_err());
    }
    for &version in Version::ALL {
        assert!(EntityAction::LeaveBed.encode(version, -1).is_err());
        assert!(UseEntity {
            target: -1,
            interaction: EntityInteraction::Attack,
            sneaking: false
        }
        .encode(version)
        .is_err());
        assert!(UseEntity {
            target: 0,
            interaction: EntityInteraction::InteractAt {
                hand: Hand::Main,
                location: [f32::NAN, 0.0, 0.0]
            },
            sneaking: false
        }
        .encode(version)
        .is_err());
        assert!(UseItem {
            hand: Hand::Main,
            sequence: u32::MAX,
            rotation: (version.protocol() >= 767).then_some([0.0, 0.0])
        }
        .encode(version)
        .is_err());
        if version.protocol() >= 767 {
            assert!(UseItem {
                hand: Hand::Main,
                sequence: 0,
                rotation: Some([f32::NAN, 0.0])
            }
            .encode(version)
            .is_err());
        }
    }
}
