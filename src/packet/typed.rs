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
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedPacket {
    Command(CommandPacket),
    WorldEffect(WorldEffectPacket),
    Entity(EntityPacket),
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
        if !matches!(state, State::Configuration | State::Play) {
            return Ok(None);
        }
        if let Some(common) = CommonPacket::decode(name, bytes, version, limits)? {
            return Ok(Some(Self::Common(common)));
        }
        if state != State::Play {
            return Ok(None);
        }
        let packet = match name {
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
            "window_items"
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
