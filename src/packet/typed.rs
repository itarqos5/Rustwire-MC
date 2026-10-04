//! Optional semantic dispatch over the common clientbound packet subset.
//! `None` means no typed codec is provided. Unsupported nested layouts are errors
//! here; Connection::next_typed_event preserves their complete raw packets.
use crate::{
    packet::{
        self, blocks::WorldPacket, chat, common::CommonPacket, entity::EntityPacket,
        entity_metadata::EntityMetadata, entity_state::EntityStatePacket, inventory,
        player::PlayerPacket,
    },
    version::State,
    Error, Limits, Result, Version,
};
#[derive(Debug, Clone, PartialEq)]
pub enum InventoryPacket {
    Content(inventory::ContainerContent),
    Property(inventory::ContainerProperty),
    Mount(inventory::OpenMountScreen),
    Cooldown(inventory::SetCooldown),
    Trades(inventory::MerchantOffers),
    Slot(inventory::SetContainerSlot),
    Open(inventory::OpenScreen),
    Close(inventory::CloseContainer),
    Selected(inventory::SetSelectedSlot),
    PlayerSlot(inventory::SetPlayerInventory),
    Cursor(inventory::SetCursorItem),
}
impl InventoryPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "craft_progress_bar" => Self::Property(inventory::ContainerProperty::decode(
                bytes, version, limits,
            )?),
            "open_horse_window" => {
                Self::Mount(inventory::OpenMountScreen::decode(bytes, version, limits)?)
            }
            "set_cooldown" => {
                Self::Cooldown(inventory::SetCooldown::decode(bytes, version, limits)?)
            }
            "trade_list" => {
                Self::Trades(inventory::MerchantOffers::decode(bytes, version, limits)?)
            }
            "window_items" => {
                Self::Content(inventory::ContainerContent::decode(bytes, version, limits)?)
            }
            "set_slot" => Self::Slot(inventory::SetContainerSlot::decode(bytes, version, limits)?),
            "open_window" => Self::Open(inventory::OpenScreen::decode(bytes, version, limits)?),
            "close_window" => {
                Self::Close(inventory::CloseContainer::decode(bytes, version, limits)?)
            }
            "held_item_slot" => {
                Self::Selected(inventory::SetSelectedSlot::decode(bytes, version, limits)?)
            }
            "set_player_inventory" => Self::PlayerSlot(inventory::SetPlayerInventory::decode(
                bytes, version, limits,
            )?),
            "set_cursor_item" => {
                Self::Cursor(inventory::SetCursorItem::decode(bytes, version, limits)?)
            }
            _ => return Err(Error::Unsupported("typed inventory packet")),
        })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum ChatPacket {
    Suggestions(super::server_metadata::ChatSuggestions),
    Hide(super::server_metadata::HideMessage),
    System(chat::SystemChat),
    Disguised(chat::DisguisedChat),
    Player(Box<chat::PlayerChat>),
}
#[derive(Debug, Clone, PartialEq)]
pub enum CommandPacket {
    Tree(super::commands::CommandTree),
    Suggestions(super::commands::CommandSuggestions),
}
#[derive(Debug, Clone, PartialEq)]
pub enum WorldEffectPacket {
    Particles(super::world_effects::WorldParticles),
    Explosion(Box<super::world_effects::Explosion>),
    Sound(super::world_effects::SoundEffect),
    EntitySound(super::world_effects::EntitySoundEffect),
    StopSound(super::world_effects::StopSound),
    Event(super::world_effects::WorldEvent),
}
/// Dimension context for packets whose section count is not carried on the wire.
/// Refresh this after Join Game/Respawn or a configuration change. The default
/// preserves full-chunk/light/biome packets as raw rather than guessing a height.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecodeContext {
    pub section_count: Option<usize>,
}
impl DecodeContext {
    pub fn for_dimension(dimension: &crate::registry::DimensionInfo) -> Self {
        Self {
            section_count: Some(dimension.section_count()),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkUpdatePacket {
    FullChunk(Box<crate::chunk::ChunkData>),
    Light(super::chunk_updates::UpdateLight),
    Biomes(super::chunk_updates::ChunkBiomes),
    BlockEntity(super::chunk_updates::TileEntityData),
    ViewPosition(super::chunk_updates::UpdateViewPosition),
    ViewDistance(super::chunk_updates::UpdateViewDistance),
    SimulationDistance(super::chunk_updates::SimulationDistance),
}
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayPacket {
    BossBar(super::overlay::BossBar),
    PlayerList(super::overlay::PlayerListHeaderFooter),
}
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedPacket {
    CustomPayload(super::custom_payload::CustomPayload),
    Dialog(super::dialog::DialogPacket),
    GameRules(super::game_rules::GameRuleValues),
    LowDiskSpaceWarning,
    Waypoint(super::waypoint::TrackedWaypoint),
    LegacyRecipes(super::recipe_declarations::LegacyDeclareRecipes),
    ModernRecipes(super::recipe_properties::ModernDeclareRecipes),
    RecipeControl(super::recipe::RecipeControlPacket),
    RecipeDisplay(Box<super::recipe_display::RecipeDisplayPacket>),
    ServerData(super::server_metadata::ServerData),
    PingResponse(super::server_metadata::PingResponse),
    Advancement(super::advancements::AdvancementPacket),
    WorldControl(super::world_control::WorldControlPacket),
    Hud(super::hud::HudPacket),
    Map(super::map::MapData),
    Statistics(super::statistics::Statistics),
    WorldState(super::world_state::WorldStatePacket),
    Scoreboard(Box<super::scoreboard::ScoreboardPacket>),
    Overlay(OverlayPacket),
    ChunkUpdate(ChunkUpdatePacket),
    Command(CommandPacket),
    WorldEffect(WorldEffectPacket),
    Entity(EntityPacket),
    EntityControl(super::entity_control::EntityControlPacket),
    EntityState(EntityStatePacket),
    Player(PlayerPacket),
    Metadata(EntityMetadata),
    World(WorldPacket),
    Inventory(InventoryPacket),
    Chat(ChatPacket),
    Common(CommonPacket),
}
impl DecodedPacket {
    pub fn decode(
        state: State,
        name: &str,
        bytes: &[u8],
        version: Version,
        limits: Limits,
    ) -> Result<Option<Self>> {
        Self::decode_with_context(
            state,
            name,
            bytes,
            version,
            limits,
            DecodeContext::default(),
        )
    }
    /// Semantic dispatch with explicit world dimensions for lighting and biomes.
    /// Missing context returns None for those packets; malformed bytes with
    /// supplied context still fail instead of silently falling back.
    pub fn decode_with_context(
        state: State,
        name: &str,
        bytes: &[u8],
        version: Version,
        limits: Limits,
        context: DecodeContext,
    ) -> Result<Option<Self>> {
        if !matches!(state, State::Configuration | State::Play) {
            return Ok(None);
        }
        if name == "custom_payload" {
            return Ok(Some(Self::CustomPayload(
                super::custom_payload::CustomPayload::decode(
                    bytes,
                    version,
                    state,
                    crate::version::Direction::Clientbound,
                    limits,
                )?,
            )));
        }
        if let Some(dialog) =
            super::dialog::DialogPacket::decode(state, name, bytes, version, limits)?
        {
            return Ok(Some(Self::Dialog(dialog)));
        }
        // Unlike the shared common packets, reset_chat exists only during
        // configuration. Never interpret an arbitrary play name as this signal.
        if name == "reset_chat" && state != State::Configuration {
            return Ok(None);
        }
        if let Some(common) = CommonPacket::decode(name, bytes, version, limits)? {
            return Ok(Some(Self::Common(common)));
        }
        if state != State::Play {
            return Ok(None);
        }
        let packet = match name {
            "game_rule_values" => Self::GameRules(super::game_rules::GameRuleValues::decode(
                bytes, version, limits,
            )?),
            "low_disk_space_warning" => {
                super::game_rules::LowDiskSpaceWarning::decode(bytes, version, limits)?;
                Self::LowDiskSpaceWarning
            }
            "tracked_waypoint" => Self::Waypoint(super::waypoint::TrackedWaypoint::decode(
                bytes, version, limits,
            )?),
            "declare_recipes" if version.protocol() < 768 => Self::LegacyRecipes(
                super::recipe_declarations::LegacyDeclareRecipes::decode(bytes, version, limits)?,
            ),
            "declare_recipes" => Self::ModernRecipes(
                super::recipe_properties::ModernDeclareRecipes::decode(bytes, version, limits)?,
            ),
            "unlock_recipes" | "recipe_book_settings" | "recipe_book_remove" => {
                Self::RecipeControl(super::recipe::RecipeControlPacket::decode(
                    name, bytes, version, limits,
                )?)
            }
            "craft_recipe_response" if version.protocol() < 768 => Self::RecipeControl(
                super::recipe::RecipeControlPacket::decode(name, bytes, version, limits)?,
            ),
            "recipe_book_add" | "craft_recipe_response" => Self::RecipeDisplay(Box::new(
                super::recipe_display::RecipeDisplayPacket::decode(name, bytes, version, limits)?,
            )),
            "server_data" => Self::ServerData(super::server_metadata::ServerData::decode(
                bytes, version, limits,
            )?),
            "ping_response" => Self::PingResponse(super::server_metadata::PingResponse::decode(
                bytes, version, limits,
            )?),
            "chat_suggestions" => Self::Chat(ChatPacket::Suggestions(
                super::server_metadata::ChatSuggestions::decode(bytes, version, limits)?,
            )),
            "hide_message" => Self::Chat(ChatPacket::Hide(
                super::server_metadata::HideMessage::decode(bytes, version, limits)?,
            )),
            "advancements" | "select_advancement_tab" => Self::Advancement(
                super::advancements::AdvancementPacket::decode(name, bytes, version, limits)?,
            ),
            "block_break_animation"
            | "block_action"
            | "open_sign_entity"
            | "nbt_query_response"
            | "collect"
            | "vehicle_move"
            | "face_player"
            | "player_rotation"
            | "set_projectile_power"
            | "set_ticking_state"
            | "step_tick" => Self::WorldControl(super::world_control::WorldControlPacket::decode(
                name, bytes, version, limits,
            )?),
            "clear_titles" | "action_bar" | "set_title_text" | "set_title_subtitle"
            | "set_title_time" | "open_book" | "experience" | "enter_combat_event"
            | "end_combat_event" | "death_combat_event" => {
                Self::Hud(super::hud::HudPacket::decode(name, bytes, version, limits)?)
            }
            "map" => Self::Map(super::map::MapData::decode(bytes, version, limits)?),
            "statistics" => Self::Statistics(super::statistics::Statistics::decode(
                bytes, version, limits,
            )?),
            "game_state_change"
            | "update_time"
            | "spawn_position"
            | "difficulty"
            | "initialize_world_border"
            | "world_border_center"
            | "world_border_lerp_size"
            | "world_border_size"
            | "world_border_warning_delay"
            | "world_border_warning_reach"
            | "acknowledge_player_digging" => Self::WorldState(
                super::world_state::WorldStatePacket::decode(name, bytes, version, limits)?,
            ),
            "scoreboard_objective"
            | "scoreboard_display_objective"
            | "scoreboard_score"
            | "reset_score"
            | "teams" => Self::Scoreboard(Box::new(super::scoreboard::ScoreboardPacket::decode(
                name, bytes, version, limits,
            )?)),
            "boss_bar" => Self::Overlay(OverlayPacket::BossBar(super::overlay::BossBar::decode(
                bytes, version, limits,
            )?)),
            "playerlist_header" => Self::Overlay(OverlayPacket::PlayerList(
                super::overlay::PlayerListHeaderFooter::decode(bytes, version, limits)?,
            )),
            "map_chunk" => {
                let Some(sections) = context.section_count else {
                    return Ok(None);
                };
                Self::ChunkUpdate(ChunkUpdatePacket::FullChunk(Box::new(
                    crate::chunk::ChunkData::decode(bytes, version, sections, limits)?,
                )))
            }
            "update_light" => {
                let Some(sections) = context.section_count else {
                    return Ok(None);
                };
                Self::ChunkUpdate(ChunkUpdatePacket::Light(
                    super::chunk_updates::UpdateLight::decode(bytes, version, sections, limits)?,
                ))
            }
            "chunk_biomes" => {
                let Some(sections) = context.section_count else {
                    return Ok(None);
                };
                Self::ChunkUpdate(ChunkUpdatePacket::Biomes(
                    super::chunk_updates::ChunkBiomes::decode(bytes, version, sections, limits)?,
                ))
            }
            "tile_entity_data" => Self::ChunkUpdate(ChunkUpdatePacket::BlockEntity(
                super::chunk_updates::TileEntityData::decode(bytes, version, limits)?,
            )),
            "update_view_position" => Self::ChunkUpdate(ChunkUpdatePacket::ViewPosition(
                super::chunk_updates::UpdateViewPosition::decode(bytes, version, limits)?,
            )),
            "update_view_distance" => Self::ChunkUpdate(ChunkUpdatePacket::ViewDistance(
                super::chunk_updates::UpdateViewDistance::decode(bytes, version, limits)?,
            )),
            "simulation_distance" => Self::ChunkUpdate(ChunkUpdatePacket::SimulationDistance(
                super::chunk_updates::SimulationDistance::decode(bytes, version, limits)?,
            )),
            "declare_commands" => Self::Command(CommandPacket::Tree(
                super::commands::CommandTree::decode(bytes, version, limits)?,
            )),
            "tab_complete" => Self::Command(CommandPacket::Suggestions(
                super::commands::CommandSuggestions::decode(bytes, version, limits)?,
            )),
            "world_particles" => Self::WorldEffect(WorldEffectPacket::Particles(
                super::world_effects::WorldParticles::decode(bytes, version, limits)?,
            )),
            "explosion" => Self::WorldEffect(WorldEffectPacket::Explosion(Box::new(
                super::world_effects::Explosion::decode(bytes, version, limits)?,
            ))),
            "sound_effect" => Self::WorldEffect(WorldEffectPacket::Sound(
                super::world_effects::SoundEffect::decode(bytes, version, limits)?,
            )),
            "entity_sound_effect" => Self::WorldEffect(WorldEffectPacket::EntitySound(
                super::world_effects::EntitySoundEffect::decode(bytes, version, limits)?,
            )),
            "stop_sound" => Self::WorldEffect(WorldEffectPacket::StopSound(
                super::world_effects::StopSound::decode(bytes, version, limits)?,
            )),
            "world_event" => Self::WorldEffect(WorldEffectPacket::Event(
                super::world_effects::WorldEvent::decode(bytes, version, limits)?,
            )),
            "set_passengers"
            | "attach_entity"
            | "entity_head_rotation"
            | "camera"
            | "animation"
            | "damage_event"
            | "hurt_animation"
            | "move_minecart" => Self::EntityControl(
                super::entity_control::EntityControlPacket::decode(name, bytes, version, limits)?,
            ),
            "spawn_entity"
            | "rel_entity_move"
            | "entity_move_look"
            | "entity_look"
            | "entity_velocity"
            | "entity_teleport"
            | "sync_entity_position"
            | "entity_destroy"
            | "entity_status"
            | "update_health"
            | "abilities" => Self::Entity(EntityPacket::decode(name, bytes, version, limits)?),
            "player_info" | "player_remove" => {
                Self::Player(PlayerPacket::decode(name, bytes, version, limits)?)
            }
            "entity_equipment"
            | "entity_update_attributes"
            | "entity_effect"
            | "remove_entity_effect" => {
                Self::EntityState(EntityStatePacket::decode(name, bytes, version, limits)?)
            }
            "entity_metadata" => Self::Metadata(EntityMetadata::decode(bytes, version, limits)?),
            "block_change" | "multi_block_change" | "unload_chunk" | "respawn" => {
                Self::World(WorldPacket::decode(name, bytes, version, limits)?)
            }
            "craft_progress_bar"
            | "open_horse_window"
            | "set_cooldown"
            | "trade_list"
            | "window_items"
            | "set_slot"
            | "open_window"
            | "close_window"
            | "held_item_slot"
            | "set_player_inventory"
            | "set_cursor_item" => {
                Self::Inventory(InventoryPacket::decode(name, bytes, version, limits)?)
            }
            "system_chat" => Self::Chat(ChatPacket::System(chat::SystemChat::decode(
                bytes, version, limits,
            )?)),
            "profileless_chat" => Self::Chat(ChatPacket::Disguised(chat::DisguisedChat::decode(
                bytes, version, limits,
            )?)),
            "player_chat" => Self::Chat(ChatPacket::Player(Box::new(chat::PlayerChat::decode(
                bytes, version, limits,
            )?))),
            _ => return Ok(None),
        };
        Ok(Some(packet))
    }
}
/// Shared helper for codecs that build a named serverbound packet.
pub fn raw_interaction(
    version: Version,
    name: &str,
    data: Vec<u8>,
) -> Result<crate::frame::RawPacket> {
    packet::named(version, State::Play, name, data)
}
