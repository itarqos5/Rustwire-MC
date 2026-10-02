//! Join-game world metadata, separate from login authentication success.
use crate::{
    codec::{BlockPosition, Reader},
    nbt::{Nbt, RootFormat},
    registry::{DimensionInfo, RegistryStore},
    Error, Limits, Result, Version,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DimensionRef {
    Name(String),
    Id(u32),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeathLocation {
    pub dimension: String,
    pub position: BlockPosition,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnInfo {
    pub dimension_type: DimensionRef,
    pub dimension_name: String,
    pub hashed_seed: i64,
    pub game_mode: u8,
    pub previous_game_mode: i8,
    pub is_debug: bool,
    pub is_flat: bool,
    pub death: Option<DeathLocation>,
    pub portal_cooldown: i32,
    pub sea_level: Option<i32>,
}
impl SpawnInfo {
    pub(crate) fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let dimension_type = if version.protocol() >= 766 {
            let id = r.var_i32()?;
            if id < 0 {
                return Err(Error::Invalid("negative dimension registry ID"));
            }
            DimensionRef::Id(id as u32)
        } else {
            DimensionRef::Name(r.string(32767)?.into())
        };
        let dimension_name = r.string(32767)?.into();
        let hashed_seed = r.i64()?;
        let game_mode = r.u8()?;
        let previous_game_mode = r.u8()? as i8;
        Self::tail(
            r,
            version,
            dimension_type,
            dimension_name,
            hashed_seed,
            game_mode,
            previous_game_mode,
        )
    }
    fn tail(
        r: &mut Reader<'_>,
        version: Version,
        dimension_type: DimensionRef,
        dimension_name: String,
        hashed_seed: i64,
        game_mode: u8,
        previous_game_mode: i8,
    ) -> Result<Self> {
        if game_mode > 3 || !(-1..=3).contains(&previous_game_mode) {
            return Err(Error::Invalid("game mode"));
        }
        let is_debug = r.bool()?;
        let is_flat = r.bool()?;
        let death = if r.bool()? {
            Some(DeathLocation {
                dimension: r.string(32767)?.into(),
                position: BlockPosition::unpack(r.i64()?),
            })
        } else {
            None
        };
        let portal_cooldown = r.var_i32()?;
        let sea_level = if version.protocol() >= 768 {
            Some(r.var_i32()?)
        } else {
            None
        };
        Ok(Self {
            dimension_type,
            dimension_name,
            hashed_seed,
            game_mode,
            previous_game_mode,
            is_debug,
            is_flat,
            death,
            portal_cooldown,
            sea_level,
        })
    }
    pub fn dimension(&self, registries: &RegistryStore) -> Result<DimensionInfo> {
        match &self.dimension_type {
            DimensionRef::Name(name) => registries.dimension_by_key(name),
            DimensionRef::Id(id) => registries.dimension_by_id(*id),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct JoinGame {
    pub entity_id: i32,
    pub hardcore: bool,
    pub world_names: Vec<String>,
    /// Present only in 1.20/1.20.1. Apply as RegistryData::Legacy before looking up dimensions.
    pub dimension_codec: Option<Nbt>,
    pub max_players: i32,
    pub view_distance: i32,
    pub simulation_distance: i32,
    pub reduced_debug_info: bool,
    pub respawn_screen: bool,
    pub limited_crafting: Option<bool>,
    pub spawn: SpawnInfo,
    pub online_mode: Option<bool>,
    pub enforces_secure_chat: Option<bool>,
}
impl JoinGame {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("join-game bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let entity_id = r.i32()?;
        let hardcore = r.bool()?;
        let legacy_modes = if version.protocol() == 763 {
            Some((r.u8()?, r.u8()? as i8))
        } else {
            None
        };
        let count = r.count(limits.max_collection.min(1024))?;
        let mut world_names = Vec::with_capacity(count);
        for _ in 0..count {
            world_names.push(r.string(32767)?.into());
        }
        let dimension_codec = if version.protocol() == 763 {
            Some(
                Nbt::read(&mut r, RootFormat::Named)?
                    .ok_or(Error::Invalid("missing legacy dimension registry"))?,
            )
        } else {
            None
        };
        let legacy_world = if version.protocol() == 763 {
            Some((
                DimensionRef::Name(r.string(32767)?.into()),
                r.string(32767)?.into(),
                r.i64()?,
            ))
        } else {
            None
        };
        let max_players = r.var_i32()?;
        let view_distance = r.var_i32()?;
        let simulation_distance = r.var_i32()?;
        if max_players < 0 || view_distance < 0 || simulation_distance < 0 {
            return Err(Error::Invalid("negative world setting"));
        }
        let reduced_debug_info = r.bool()?;
        let respawn_screen = r.bool()?;
        let limited_crafting = if version.protocol() >= 764 {
            Some(r.bool()?)
        } else {
            None
        };
        let spawn = if let Some((dimension_type, dimension_name, hashed_seed)) = legacy_world {
            let (game_mode, previous_game_mode) = legacy_modes.unwrap();
            SpawnInfo::tail(
                &mut r,
                version,
                dimension_type,
                dimension_name,
                hashed_seed,
                game_mode,
                previous_game_mode,
            )?
        } else {
            SpawnInfo::read(&mut r, version)?
        };
        let online_mode = if version.protocol() >= 776 {
            Some(r.bool()?)
        } else {
            None
        };
        let enforces_secure_chat = if version.protocol() >= 766 {
            Some(r.bool()?)
        } else {
            None
        };
        r.finish()?;
        Ok(Self {
            entity_id,
            hardcore,
            world_names,
            dimension_codec,
            max_players,
            view_distance,
            simulation_distance,
            reduced_debug_info,
            respawn_screen,
            limited_crafting,
            spawn,
            online_mode,
            enforces_secure_chat,
        })
    }
}
