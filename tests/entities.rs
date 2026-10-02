use rustwire_mc::{
    packet::entity::*,
    version::{Direction, State},
    Error, Limits, Version,
};
// Independent wire fixtures. These helpers do not use the library encoder.
fn vint(v: &mut Vec<u8>, n: i32) {
    let mut n = n as u32;
    loop {
        let b = (n & 127) as u8;
        n >>= 7;
        v.push(b | if n == 0 { 0 } else { 128 });
        if n == 0 {
            break;
        }
    }
}
fn doubles(v: &mut Vec<u8>, values: &[f64]) {
    for value in values {
        v.extend(value.to_be_bytes());
    }
}
fn shorts(v: &mut Vec<u8>, values: &[i16]) {
    for value in values {
        v.extend(value.to_be_bytes());
    }
}
fn fixture(name: &str, version: Version) -> Vec<u8> {
    let mut b = vec![];
    match name {
        "spawn_entity" => {
            vint(&mut b, 300);
            b.extend([7; 16]);
            vint(&mut b, 2);
            doubles(&mut b, &[1.25, -64.0, 30_000_000.0]);
            if version.protocol() >= 773 {
                b.push(0);
            }
            b.extend([255, 64, 128]);
            vint(&mut b, -1);
            if version.protocol() < 773 {
                shorts(&mut b, &[-32768, 0, 32767]);
            }
        }
        "rel_entity_move" => {
            vint(&mut b, 300);
            shorts(&mut b, &[-32768, 4096, 32767]);
            b.push(1);
        }
        "entity_move_look" => {
            vint(&mut b, 300);
            shorts(&mut b, &[-32768, 4096, 32767]);
            b.extend([64, 255, 0]);
        }
        "entity_look" => {
            vint(&mut b, 300);
            b.extend([64, 255, 1]);
        }
        "entity_velocity" => {
            vint(&mut b, 300);
            if version.protocol() >= 773 {
                b.extend([1, 0, 255, 252, 255, 252]);
            } else {
                shorts(&mut b, &[-32768, 0, 32767]);
            }
        }
        "entity_teleport" => {
            vint(&mut b, 300);
            doubles(&mut b, &[1.25, -64.0, 30_000_000.0]);
            if (768..=775).contains(&version.protocol()) {
                doubles(&mut b, &[-1.0, 0.0, 1.0]);
                b.extend(90f32.to_be_bytes());
                b.extend((-45f32).to_be_bytes());
                b.extend(0x1ff_u32.to_be_bytes());
            } else {
                b.extend([64, 255]);
            }
            b.push(1);
        }
        "sync_entity_position" => {
            vint(&mut b, 300);
            doubles(&mut b, &[1.0, 2.0, 3.0, -1.0, 0.0, 1.0]);
            b.extend(90f32.to_be_bytes());
            b.extend((-45f32).to_be_bytes());
            b.push(1);
        }
        "entity_destroy" => {
            vint(&mut b, 3);
            vint(&mut b, 0);
            vint(&mut b, 300);
            vint(&mut b, i32::MAX);
        }
        "entity_status" => {
            b.extend(300_i32.to_be_bytes());
            b.push(255);
        }
        "update_health" => {
            b.extend(19.5f32.to_be_bytes());
            vint(&mut b, 20);
            b.extend(5f32.to_be_bytes());
        }
        "abilities" => {
            b.push(15);
            b.extend(0.05f32.to_be_bytes());
            b.extend(0.1f32.to_be_bytes());
        }
        _ => unreachable!(),
    }
    b
}
const NAMES: &[&str] = &[
    "spawn_entity",
    "rel_entity_move",
    "entity_move_look",
    "entity_look",
    "entity_velocity",
    "entity_teleport",
    "entity_destroy",
    "entity_status",
    "update_health",
    "abilities",
];
#[test]
fn independent_wire_fixtures_all_14_families() {
    for &v in Version::ALL {
        for &name in NAMES.iter().chain(if v.protocol() >= 768 {
            ["sync_entity_position"].as_slice()
        } else {
            &[]
        }) {
            let bytes = fixture(name, v);
            let packet = EntityPacket::decode(name, &bytes, v, Limits::default()).unwrap();
            let encoded = packet.encode(v, Limits::default()).unwrap();
            assert_eq!(encoded.data, bytes, "{v} {name}");
            assert_eq!(
                encoded.id,
                v.packet_id(State::Play, Direction::Clientbound, name)
                    .unwrap()
            );
        }
        assert_eq!(
            SpawnEntity::decode(&fixture("spawn_entity", v), v, Limits::default())
                .unwrap()
                .position,
            [1.25, -64.0, 30_000_000.0]
        );
        assert_eq!(
            RelativeMove::decode(&fixture("rel_entity_move", v), v, Limits::default())
                .unwrap()
                .delta_blocks(),
            [-8.0, 1.0, 32767.0 / 4096.0]
        );
        let teleport =
            EntityTeleport::decode(&fixture("entity_teleport", v), v, Limits::default()).unwrap();
        assert_eq!(
            matches!(teleport.transform, TeleportTransform::Relative { .. }),
            (768..=775).contains(&v.protocol())
        );
        let spawn = SpawnEntity::decode(&fixture("spawn_entity", v), v, Limits::default()).unwrap();
        assert_eq!(
            (spawn.entity_id, spawn.entity_type, spawn.data),
            (300, 2, -1)
        );
        assert_eq!(spawn.uuid, [7; 16]);
        assert_eq!(
            (spawn.pitch.0, spawn.yaw.0, spawn.head_yaw.0),
            (255, 64, 128)
        );
        assert_eq!(
            spawn.velocity,
            if v.protocol() >= 773 {
                Velocity::LowPrecision(LowPrecisionVector::ZERO)
            } else {
                Velocity::Fixed([-32768, 0, 32767])
            }
        );
        let look = EntityLook::decode(&fixture("entity_look", v), v, Limits::default()).unwrap();
        assert_eq!(
            look,
            EntityLook {
                entity_id: 300,
                yaw: Angle(64),
                pitch: Angle(255),
                on_ground: true
            }
        );
        let moved =
            MoveAndLook::decode(&fixture("entity_move_look", v), v, Limits::default()).unwrap();
        assert_eq!(
            moved,
            MoveAndLook {
                entity_id: 300,
                delta: [-32768, 4096, 32767],
                yaw: Angle(64),
                pitch: Angle(255),
                on_ground: false
            }
        );
        assert_eq!(
            DestroyEntities::decode(&fixture("entity_destroy", v), v, Limits::default())
                .unwrap()
                .entity_ids,
            [0, 300, i32::MAX]
        );
        assert_eq!(
            EntityStatus::decode(&fixture("entity_status", v), v, Limits::default()).unwrap(),
            EntityStatus {
                entity_id: 300,
                status: -1
            }
        );
        assert_eq!(
            Health::decode(&fixture("update_health", v), v, Limits::default()).unwrap(),
            Health {
                health: 19.5,
                food: 20,
                saturation: 5.0
            }
        );
        assert_eq!(
            PlayerAbilities::decode(&fixture("abilities", v), v, Limits::default()).unwrap(),
            PlayerAbilities {
                flags: 15,
                flying_speed: 0.05,
                walking_speed: 0.1
            }
        );
    }
}
#[test]
fn every_fixture_rejects_truncation_trailing_bytes_and_packet_budget() {
    for &v in Version::ALL {
        for &name in NAMES.iter().chain(if v.protocol() >= 768 {
            ["sync_entity_position"].as_slice()
        } else {
            &[]
        }) {
            let bytes = fixture(name, v);
            for n in 0..bytes.len() {
                assert!(
                    EntityPacket::decode(name, &bytes[..n], v, Limits::default()).is_err(),
                    "{v} {name} prefix {n}"
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(EntityPacket::decode(name, &trailing, v, Limits::default()).is_err());
            let limits = Limits {
                max_packet: bytes.len() - 1,
                ..Limits::default()
            };
            assert!(EntityPacket::decode(name, &bytes, v, limits).is_err());
            let value = EntityPacket::decode(name, &bytes, v, Limits::default()).unwrap();
            assert!(value.encode(v, limits).is_err());
        }
    }
}
#[test]
fn low_precision_vectors_zero_quantization_and_unsigned_continuation() {
    let v = Version::V1_21_9;
    let l = Limits::default();
    let zero = EntityVelocity::decode(&[42, 0], v, l).unwrap();
    assert_eq!(zero.velocity.blocks_per_tick(), [0.0; 3]);
    assert_eq!(zero.encode(v, l).unwrap(), [42, 0]);
    // Mixed-endian 48-bit layout: scale=1, quantized [-1,0,1].
    let mut bytes = vec![42, 1, 0, 255, 252, 255, 252];
    let p = EntityVelocity::decode(&bytes, v, l).unwrap();
    assert_eq!(p.velocity.blocks_per_tick(), [-1.0, 0.0, 1.0]);
    bytes[1] = 5;
    bytes.push(1); // scale=(1<<2)|1 = 5
    let p = EntityVelocity::decode(&bytes, v, l).unwrap();
    assert_eq!(p.velocity.blocks_per_tick(), [-5.0, 0.0, 5.0]);
    assert_eq!(p.encode(v, l).unwrap(), bytes);
    bytes[1] = 7;
    bytes.pop();
    bytes.extend([255, 255, 255, 255, 15]);
    let p = EntityVelocity::decode(&bytes, v, l).unwrap();
    assert_eq!(
        p.velocity.blocks_per_tick(),
        [-17_179_869_183.0, 0.0, 17_179_869_183.0]
    );
    assert_eq!(p.encode(v, l).unwrap(), bytes);
    for coordinates in [
        [0.0; 3],
        [-0.2, 1.2, 0.4],
        [17_179_869_183.0, 0.0, -17_179_869_183.0],
    ] {
        let quantized = LowPrecisionVector::quantize(coordinates).unwrap();
        let value = quantized.value();
        for i in 0..3 {
            assert!(
                (coordinates[i] - value[i]).abs() <= (quantized.scale as f64 / 32766.0).max(1e-14)
            );
        }
        let packet = EntityVelocity {
            entity_id: 42,
            velocity: Velocity::LowPrecision(quantized),
        };
        assert_eq!(
            EntityVelocity::decode(&packet.encode(v, l).unwrap(), v, l).unwrap(),
            packet
        );
    }
    assert!(LowPrecisionVector::quantize([f64::NAN, 0.0, 0.0]).is_err());
    assert!(LowPrecisionVector::quantize([f64::INFINITY, 0.0, 0.0]).is_err());
    assert!(LowPrecisionVector::quantize([17_179_869_184.0, 0.0, 0.0]).is_err());
    assert!(EntityVelocity::decode(&[42, 4, 0, 0, 0, 0, 0, 0], v, l).is_err());
}
#[test]
fn malformed_ids_floats_flags_and_unimplemented_metadata_fail_closed() {
    let l = Limits::default();
    for &v in Version::ALL {
        let mut b = fixture("spawn_entity", v);
        b[19..27].copy_from_slice(&f64::NAN.to_be_bytes());
        assert!(SpawnEntity::decode(&b, v, l).is_err());
        let mut b = fixture("entity_look", v);
        *b.last_mut().unwrap() = 2;
        assert!(EntityLook::decode(&b, v, l).is_err());
        assert!(EntityLook::decode(&[255, 255, 255, 255, 15, 0, 0, 0], v, l).is_err());
        let mut b = fixture("update_health", v);
        b[..4].copy_from_slice(&f32::INFINITY.to_be_bytes());
        assert!(Health::decode(&b, v, l).is_err());
        assert!(matches!(
            EntityPacket::decode("entity_metadata", &[0, 255], v, l),
            Err(Error::Unsupported(_))
        ));
        assert!(DestroyEntities::decode(&[2, 1], v, l).is_err());
        assert!(DestroyEntities::decode(
            &[2, 1, 2],
            v,
            Limits {
                max_collection: 1,
                ..l
            }
        )
        .is_err());
        assert!(DestroyEntities {
            entity_ids: vec![1, -1]
        }
        .encode(v, l)
        .is_err());
        if (768..=775).contains(&v.protocol()) {
            let mut b = fixture("entity_teleport", v);
            let n = b.len();
            b[n - 5..n - 1].copy_from_slice(&512u32.to_be_bytes());
            assert!(EntityTeleport::decode(&b, v, l).is_err());
        }
    }
    let old = EntityVelocity {
        entity_id: 1,
        velocity: Velocity::Fixed([0; 3]),
    };
    assert!(old.encode(Version::V1_21_9, l).is_err());
    let abs = EntityTeleport {
        entity_id: 1,
        transform: TeleportTransform::Absolute {
            position: [0.0; 3],
            yaw: Angle(0),
            pitch: Angle(0),
        },
        on_ground: false,
    };
    assert!(abs.encode(Version::V1_21_2, l).is_err());
    assert!(SyncEntityPosition::decode(&[], Version::V1_20, l).is_err());
}
