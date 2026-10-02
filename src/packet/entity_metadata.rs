//! Bounded, typed entity metadata with release-specific serializer registries.
//!
//! Serializer IDs come from the pinned `research/protocols/*.json` schemas.
//! `optional_global_pos` uses a dimension identifier AND packed position in all
//! supported releases; the older upstream schema's string-only alias is wrong.
//! This and the distinct optional-state/optional-integer encodings were verified
//! by inspecting the 1.20.1 server codec. No server implementation is included.
//!
//! Particle options, registry/inline variant holders and resolvable profiles are
//! semantic values. Unknown IDs are Unsupported because these are not length-framed.
//! Item-stack support has exactly the same component limits as `inventory::Slot`.
//! Keep the original RawPacket when a typed decode returns Unsupported.
pub mod holders;
pub mod particles;
use super::{
    inventory::{self, Budget, Slot},
    DeathLocation,
};
use crate::{
    codec::{BlockPosition, Reader, Writer},
    nbt::{Nbt, RootFormat, Tag},
    Error, Limits, Result, Version,
};
use holders::{PaintingVariant, RegistryHolder, ResolvableProfile, WolfVariant};
use particles::Particle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataWire {
    Byte,
    VarInt,
    VarLong,
    Float,
    String,
    JsonComponent,
    OptionalJsonComponent,
    NbtComponent,
    OptionalNbtComponent,
    Slot,
    Bool,
    Vec3,
    BlockPosition,
    OptionalBlockPosition,
    Direction,
    OptionalUuid,
    BlockState,
    OptionalBlockState,
    Compound,
    VillagerData,
    OptionalUnsignedInt,
    NonnegativeVarInt,
    OptionalGlobalPosition,
    Quaternion,
    HumanoidArm,
    Particle,
    Particles,
    PaintingVariant,
    WolfVariant,
    ResolvableProfile,
    Unsupported,
}

// BEGIN GENERATED METADATA REGISTRIES
/// Index is the wire serializer ID; Unsupported entries must never be skipped.
/// Slots can additionally reject unsupported item-component payloads.
pub fn metadata_registry(version: Version) -> &'static [(&'static str, MetadataWire)] {
    match version.protocol() {
        763 | 764 => REGISTRY_0,
        765 => REGISTRY_1,
        766 => REGISTRY_2,
        767..=769 => REGISTRY_3,
        770..=772 => REGISTRY_4,
        773 => REGISTRY_5,
        774 => REGISTRY_6,
        775 | 776 => REGISTRY_7,
        _ => unreachable!(),
    }
}
const REGISTRY_0: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::JsonComponent),
    ("optional_component", MetadataWire::OptionalJsonComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("compound_tag", MetadataWire::Compound),
    ("particle", MetadataWire::Particle),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::NonnegativeVarInt),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
];
const REGISTRY_1: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::NbtComponent),
    ("optional_component", MetadataWire::OptionalNbtComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("compound_tag", MetadataWire::Compound),
    ("particle", MetadataWire::Particle),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::NonnegativeVarInt),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
];
const REGISTRY_2: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::NbtComponent),
    ("optional_component", MetadataWire::OptionalNbtComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("compound_tag", MetadataWire::Compound),
    ("particle", MetadataWire::Particle),
    ("particles", MetadataWire::Particles),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_variant", MetadataWire::NonnegativeVarInt),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::NonnegativeVarInt),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("armadillo_state", MetadataWire::NonnegativeVarInt),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
];
const REGISTRY_3: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::NbtComponent),
    ("optional_component", MetadataWire::OptionalNbtComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("compound_tag", MetadataWire::Compound),
    ("particle", MetadataWire::Particle),
    ("particles", MetadataWire::Particles),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_variant", MetadataWire::WolfVariant),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::PaintingVariant),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("armadillo_state", MetadataWire::NonnegativeVarInt),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
];
const REGISTRY_4: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::NbtComponent),
    ("optional_component", MetadataWire::OptionalNbtComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("compound_tag", MetadataWire::Compound),
    ("particle", MetadataWire::Particle),
    ("particles", MetadataWire::Particles),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("cow_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_sound_variant", MetadataWire::NonnegativeVarInt),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("pig_variant", MetadataWire::NonnegativeVarInt),
    ("chicken_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::PaintingVariant),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("armadillo_state", MetadataWire::NonnegativeVarInt),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
];
const REGISTRY_5: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::NbtComponent),
    ("optional_component", MetadataWire::OptionalNbtComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("particle", MetadataWire::Particle),
    ("particles", MetadataWire::Particles),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("cow_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_sound_variant", MetadataWire::NonnegativeVarInt),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("pig_variant", MetadataWire::NonnegativeVarInt),
    ("chicken_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::PaintingVariant),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("armadillo_state", MetadataWire::NonnegativeVarInt),
    ("copper_golem_state", MetadataWire::NonnegativeVarInt),
    (
        "weathering_copper_golem_state",
        MetadataWire::NonnegativeVarInt,
    ),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
    ("resolvable_profile", MetadataWire::ResolvableProfile),
];
const REGISTRY_6: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::NbtComponent),
    ("optional_component", MetadataWire::OptionalNbtComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("particle", MetadataWire::Particle),
    ("particles", MetadataWire::Particles),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("cow_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_sound_variant", MetadataWire::NonnegativeVarInt),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("pig_variant", MetadataWire::NonnegativeVarInt),
    ("chicken_variant", MetadataWire::NonnegativeVarInt),
    ("zombie_nautilus_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::PaintingVariant),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("armadillo_state", MetadataWire::NonnegativeVarInt),
    ("copper_golem_state", MetadataWire::NonnegativeVarInt),
    (
        "weathering_copper_golem_state",
        MetadataWire::NonnegativeVarInt,
    ),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
    ("resolvable_profile", MetadataWire::ResolvableProfile),
    ("humanoid_arm", MetadataWire::HumanoidArm),
];
const REGISTRY_7: &[(&str, MetadataWire)] = &[
    ("byte", MetadataWire::Byte),
    ("int", MetadataWire::VarInt),
    ("long", MetadataWire::VarLong),
    ("float", MetadataWire::Float),
    ("string", MetadataWire::String),
    ("component", MetadataWire::NbtComponent),
    ("optional_component", MetadataWire::OptionalNbtComponent),
    ("item_stack", MetadataWire::Slot),
    ("boolean", MetadataWire::Bool),
    ("rotations", MetadataWire::Vec3),
    ("block_pos", MetadataWire::BlockPosition),
    ("optional_block_pos", MetadataWire::OptionalBlockPosition),
    ("direction", MetadataWire::Direction),
    ("optional_uuid", MetadataWire::OptionalUuid),
    ("block_state", MetadataWire::BlockState),
    ("optional_block_state", MetadataWire::OptionalBlockState),
    ("particle", MetadataWire::Particle),
    ("particles", MetadataWire::Particles),
    ("villager_data", MetadataWire::VillagerData),
    ("optional_unsigned_int", MetadataWire::OptionalUnsignedInt),
    ("pose", MetadataWire::NonnegativeVarInt),
    ("cat_variant", MetadataWire::NonnegativeVarInt),
    ("cat_sound_variant", MetadataWire::NonnegativeVarInt),
    ("cow_variant", MetadataWire::NonnegativeVarInt),
    ("cow_sound_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_variant", MetadataWire::NonnegativeVarInt),
    ("wolf_sound_variant", MetadataWire::NonnegativeVarInt),
    ("frog_variant", MetadataWire::NonnegativeVarInt),
    ("pig_variant", MetadataWire::NonnegativeVarInt),
    ("pig_sound_variant", MetadataWire::NonnegativeVarInt),
    ("chicken_variant", MetadataWire::NonnegativeVarInt),
    ("chicken_sound_variant", MetadataWire::NonnegativeVarInt),
    ("zombie_nautilus_variant", MetadataWire::NonnegativeVarInt),
    ("optional_global_pos", MetadataWire::OptionalGlobalPosition),
    ("painting_variant", MetadataWire::PaintingVariant),
    ("sniffer_state", MetadataWire::NonnegativeVarInt),
    ("armadillo_state", MetadataWire::NonnegativeVarInt),
    ("copper_golem_state", MetadataWire::NonnegativeVarInt),
    (
        "weathering_copper_golem_state",
        MetadataWire::NonnegativeVarInt,
    ),
    ("vector3", MetadataWire::Vec3),
    ("quaternion", MetadataWire::Quaternion),
    ("resolvable_profile", MetadataWire::ResolvableProfile),
    ("humanoid_arm", MetadataWire::HumanoidArm),
];

// END GENERATED METADATA REGISTRIES

pub fn serializer_id(version: Version, name: &str) -> Result<i32> {
    metadata_registry(version)
        .iter()
        .position(|(n, _)| *n == name)
        .map(|id| id as i32)
        .ok_or(Error::Unsupported(
            "metadata serializer in selected release",
        ))
}
#[derive(Debug, Clone, PartialEq)]
pub enum MetadataValue {
    Byte(i8),
    VarInt(i32),
    VarLong(i64),
    Float(f32),
    String(String),
    /// Legacy JSON component. Its JSON text is preserved, not rendered.
    JsonComponent(String),
    OptionalJsonComponent(Option<String>),
    Nbt(Nbt),
    OptionalNbt(Option<Nbt>),
    Slot(Box<Slot>),
    Particle(Particle),
    Particles(Vec<Particle>),
    PaintingVariant(RegistryHolder<PaintingVariant>),
    WolfVariant(RegistryHolder<WolfVariant>),
    ResolvableProfile(ResolvableProfile),
    Bool(bool),
    /// Rotations are pitch/yaw/roll; vector3 fields are x/y/z.
    Vec3([f32; 3]),
    Quaternion([f32; 4]),
    BlockPosition(BlockPosition),
    OptionalBlockPosition(Option<BlockPosition>),
    OptionalUuid(Option<[u8; 16]>),
    /// Raw state ID; zero means absent and cannot represent Some(0).
    OptionalBlockState(Option<i32>),
    /// Zero on wire means absent, otherwise the nonnegative value plus one.
    OptionalUnsignedInt(Option<u32>),
    VillagerData {
        kind: i32,
        profession: i32,
        level: i32,
    },
    OptionalGlobalPosition(Option<DeathLocation>),
}
#[derive(Debug, Clone, PartialEq)]
pub struct MetadataEntry {
    pub index: u8,
    pub serializer: &'static str,
    pub value: MetadataValue,
}
#[derive(Debug, Clone, PartialEq)]
pub struct EntityMetadata {
    pub entity_id: i32,
    pub entries: Vec<MetadataEntry>,
}
fn nonnegative(n: i32) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid("negative metadata registry ID"))
    } else {
        Ok(n)
    }
}
fn finite(n: f32) -> Result<f32> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(Error::Invalid("non-finite metadata float"))
    }
}
fn required_nbt(r: &mut Reader<'_>, format: RootFormat, b: &mut Budget) -> Result<Nbt> {
    inventory::read_nbt(r, format, b)?.ok_or(Error::Invalid("missing metadata NBT"))
}
fn read_value(
    r: &mut Reader<'_>,
    wire: MetadataWire,
    version: Version,
    b: &mut Budget,
) -> Result<MetadataValue> {
    use MetadataValue as V;
    Ok(match wire {
        MetadataWire::Byte => V::Byte(r.u8()? as i8),
        MetadataWire::VarInt => V::VarInt(r.var_i32()?),
        MetadataWire::VarLong => V::VarLong(r.var_i64()?),
        MetadataWire::Float => V::Float(finite(r.f32()?)?),
        MetadataWire::String => V::String(r.string(32767)?.into()),
        MetadataWire::JsonComponent => V::JsonComponent(r.string(262144)?.into()),
        MetadataWire::OptionalJsonComponent => V::OptionalJsonComponent(if r.bool()? {
            Some(r.string(262144)?.into())
        } else {
            None
        }),
        MetadataWire::NbtComponent => V::Nbt(required_nbt(r, RootFormat::Anonymous, b)?),
        MetadataWire::OptionalNbtComponent => V::OptionalNbt(if r.bool()? {
            Some(required_nbt(r, RootFormat::Anonymous, b)?)
        } else {
            None
        }),
        MetadataWire::Compound => {
            let value = required_nbt(r, RootFormat::for_version(version), b)?;
            if !matches!(value.root, Tag::Compound(_)) {
                return Err(Error::Invalid("metadata compound tag"));
            }
            V::Nbt(value)
        }
        MetadataWire::Slot => V::Slot(Box::new(inventory::read_slot(r, version, b, 0)?)),
        MetadataWire::Particle => V::Particle(particles::read(r, version, b)?),
        MetadataWire::Particles => {
            // Particle nodes charge the shared budget individually; preflight the
            // count before allocation without charging each node twice.
            let count = r.count(b.limits.max_collection)?;
            let mut values = Vec::new();
            for _ in 0..count {
                values.push(particles::read(r, version, b)?);
            }
            V::Particles(values)
        }
        MetadataWire::PaintingVariant => V::PaintingVariant(holders::read_painting(r, version, b)?),
        MetadataWire::WolfVariant => V::WolfVariant(holders::read_wolf(r, version, b)?),
        MetadataWire::ResolvableProfile => {
            V::ResolvableProfile(holders::read_profile(r, version, b)?)
        }
        MetadataWire::Bool => V::Bool(r.bool()?),
        MetadataWire::Vec3 => V::Vec3([finite(r.f32()?)?, finite(r.f32()?)?, finite(r.f32()?)?]),
        MetadataWire::Quaternion => V::Quaternion([
            finite(r.f32()?)?,
            finite(r.f32()?)?,
            finite(r.f32()?)?,
            finite(r.f32()?)?,
        ]),
        MetadataWire::BlockPosition => V::BlockPosition(BlockPosition::unpack(r.i64()?)),
        MetadataWire::OptionalBlockPosition => V::OptionalBlockPosition(if r.bool()? {
            Some(BlockPosition::unpack(r.i64()?))
        } else {
            None
        }),
        MetadataWire::OptionalUuid => {
            V::OptionalUuid(if r.bool()? { Some(r.uuid()?) } else { None })
        }
        MetadataWire::BlockState | MetadataWire::NonnegativeVarInt => {
            V::VarInt(nonnegative(r.var_i32()?)?)
        }
        MetadataWire::Direction | MetadataWire::HumanoidArm => {
            let n = nonnegative(r.var_i32()?)?;
            let max = if wire == MetadataWire::Direction {
                5
            } else {
                1
            };
            if n > max {
                return Err(Error::Invalid("metadata enum"));
            }
            V::VarInt(n)
        }
        MetadataWire::OptionalBlockState => {
            let n = nonnegative(r.var_i32()?)?;
            V::OptionalBlockState(if n == 0 { None } else { Some(n) })
        }
        MetadataWire::OptionalUnsignedInt => {
            let raw = r.var_i32()? as u32;
            if raw > (i32::MAX as u32) + 1 {
                return Err(Error::Invalid("optional metadata unsigned integer"));
            }
            V::OptionalUnsignedInt(if raw == 0 { None } else { Some(raw - 1) })
        }
        MetadataWire::VillagerData => V::VillagerData {
            kind: nonnegative(r.var_i32()?)?,
            profession: nonnegative(r.var_i32()?)?,
            level: nonnegative(r.var_i32()?)?,
        },
        MetadataWire::OptionalGlobalPosition => V::OptionalGlobalPosition(if r.bool()? {
            Some(DeathLocation {
                dimension: r.string(32767)?.into(),
                position: BlockPosition::unpack(r.i64()?),
            })
        } else {
            None
        }),
        MetadataWire::Unsupported => {
            return Err(Error::Unsupported("complex metadata serializer payload"))
        }
    })
}
fn write_value(
    value: &MetadataValue,
    wire: MetadataWire,
    w: &mut Writer,
    version: Version,
    limits: Limits,
    b: &mut Budget,
) -> Result<()> {
    use MetadataValue as V;
    match (wire, value) {
        (MetadataWire::Byte, V::Byte(n)) => w.u8(*n as u8),
        (MetadataWire::VarInt, V::VarInt(n)) => w.var_i32(*n),
        (MetadataWire::VarLong, V::VarLong(n)) => w.var_i64(*n),
        (MetadataWire::Float, V::Float(n)) => w.f32(finite(*n)?),
        (MetadataWire::String, V::String(s)) => w.string(s, limits.max_string_chars.min(32767))?,
        (MetadataWire::JsonComponent, V::JsonComponent(s)) => {
            w.string(s, limits.max_string_chars.min(262144))?
        }
        (MetadataWire::OptionalJsonComponent, V::OptionalJsonComponent(s)) => {
            w.bool(s.is_some());
            if let Some(s) = s {
                w.string(s, limits.max_string_chars.min(262144))?;
            }
        }
        (MetadataWire::NbtComponent, V::Nbt(n)) => {
            inventory::write_nbt(Some(n), w, RootFormat::Anonymous, b)?
        }
        (MetadataWire::OptionalNbtComponent, V::OptionalNbt(n)) => {
            w.bool(n.is_some());
            if let Some(n) = n {
                inventory::write_nbt(Some(n), w, RootFormat::Anonymous, b)?;
            }
        }
        (MetadataWire::Compound, V::Nbt(n)) => {
            if !matches!(n.root, Tag::Compound(_)) {
                return Err(Error::Invalid("metadata compound tag"));
            }
            inventory::write_nbt(Some(n), w, RootFormat::for_version(version), b)?;
        }
        (MetadataWire::Slot, V::Slot(slot)) => inventory::write_slot(slot, w, version, b, 0)?,
        (MetadataWire::Particle, V::Particle(value)) => particles::write(value, w, version, b)?,
        (MetadataWire::Particles, V::Particles(values)) => {
            if values.len() > b.limits.max_collection.min(i32::MAX as usize) {
                return Err(Error::Limit("metadata particle count"));
            }
            w.var_i32(values.len() as i32);
            for value in values {
                particles::write(value, w, version, b)?;
            }
        }
        (MetadataWire::PaintingVariant, V::PaintingVariant(value)) => {
            holders::write_painting(value, w, version, b)?
        }
        (MetadataWire::WolfVariant, V::WolfVariant(value)) => {
            holders::write_wolf(value, w, version, b)?
        }
        (MetadataWire::ResolvableProfile, V::ResolvableProfile(value)) => {
            holders::write_profile(value, w, version, b)?
        }
        (MetadataWire::Bool, V::Bool(n)) => w.bool(*n),
        (MetadataWire::Vec3, V::Vec3(v)) => {
            for n in v {
                w.f32(finite(*n)?);
            }
        }
        (MetadataWire::Quaternion, V::Quaternion(v)) => {
            for n in v {
                w.f32(finite(*n)?);
            }
        }
        (MetadataWire::BlockPosition, V::BlockPosition(p)) => w.i64(p.pack()?),
        (MetadataWire::OptionalBlockPosition, V::OptionalBlockPosition(p)) => {
            w.bool(p.is_some());
            if let Some(p) = p {
                w.i64(p.pack()?);
            }
        }
        (MetadataWire::OptionalUuid, V::OptionalUuid(uuid)) => {
            w.bool(uuid.is_some());
            if let Some(uuid) = uuid {
                w.raw(uuid);
            }
        }
        (MetadataWire::BlockState | MetadataWire::NonnegativeVarInt, V::VarInt(n)) => {
            w.var_i32(nonnegative(*n)?)
        }
        (MetadataWire::Direction | MetadataWire::HumanoidArm, V::VarInt(n)) => {
            let n = nonnegative(*n)?;
            let max = if wire == MetadataWire::Direction {
                5
            } else {
                1
            };
            if n > max {
                return Err(Error::Invalid("metadata enum"));
            }
            w.var_i32(n);
        }
        (MetadataWire::OptionalBlockState, V::OptionalBlockState(n)) => {
            let n = match n {
                Some(n) if *n > 0 => *n,
                None => 0,
                _ => return Err(Error::Invalid("optional metadata block state")),
            };
            w.var_i32(n);
        }
        (MetadataWire::OptionalUnsignedInt, V::OptionalUnsignedInt(n)) => {
            let raw = match n {
                Some(n) if *n <= i32::MAX as u32 => *n + 1,
                None => 0,
                _ => return Err(Error::Invalid("optional metadata unsigned integer")),
            };
            w.var_i32(raw as i32);
        }
        (
            MetadataWire::VillagerData,
            V::VillagerData {
                kind,
                profession,
                level,
            },
        ) => {
            w.var_i32(nonnegative(*kind)?);
            w.var_i32(nonnegative(*profession)?);
            w.var_i32(nonnegative(*level)?);
        }
        (MetadataWire::OptionalGlobalPosition, V::OptionalGlobalPosition(p)) => {
            w.bool(p.is_some());
            if let Some(p) = p {
                w.string(&p.dimension, limits.max_string_chars.min(32767))?;
                w.i64(p.position.pack()?);
            }
        }
        (MetadataWire::Unsupported, _) => {
            return Err(Error::Unsupported("complex metadata serializer payload"))
        }
        _ => return Err(Error::Invalid("metadata value does not match serializer")),
    }
    b.check_bytes(w)
}
impl EntityMetadata {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("metadata packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let entity_id = nonnegative(r.var_i32()?)?;
        let mut entries = Vec::new();
        let mut seen = [false; 255];
        let mut b = Budget::new(limits);
        loop {
            let index = r.u8()?;
            if index == 255 {
                break;
            }
            if seen[index as usize] {
                return Err(Error::Invalid("duplicate metadata index"));
            }
            seen[index as usize] = true;
            b.charge(1)?;
            let id = nonnegative(r.var_i32()?)? as usize;
            let (serializer, wire) = metadata_registry(version)
                .get(id)
                .copied()
                .ok_or(Error::Unsupported("unknown metadata serializer ID"))?;
            let value = read_value(&mut r, wire, version, &mut b)?;
            entries.push(MetadataEntry {
                index,
                serializer,
                value,
            });
        }
        r.finish()?;
        Ok(Self { entity_id, entries })
    }
    /// Encodes a complete body atomically; slot and NBT budgets are shared across entries.
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        if self.entries.len() > 255.min(limits.max_collection) {
            return Err(Error::Limit("metadata entry count"));
        }
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        let mut seen = [false; 255];
        w.var_i32(nonnegative(self.entity_id)?);
        for entry in &self.entries {
            if entry.index == 255 || seen[entry.index as usize] {
                return Err(Error::Invalid("reserved or duplicate metadata index"));
            }
            seen[entry.index as usize] = true;
            b.charge(1)?;
            let id = serializer_id(version, entry.serializer)?;
            let wire = metadata_registry(version)[id as usize].1;
            w.u8(entry.index);
            w.var_i32(id);
            write_value(&entry.value, wire, &mut w, version, limits, &mut b)?;
        }
        w.u8(255);
        b.check_bytes(&w)?;
        Ok(w.into_inner())
    }
}
