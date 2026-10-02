//! Bounded, destructive gameplay test client for disposable loopback servers only.
//! The matching tools/paper/validate_gameplay.py harness owns server-side actions.
use rustwire_mc::{
    chunk::ChunkData,
    connection::{Connection, Event, TypedEvent},
    packet::{
        self,
        blocks::WorldPacket,
        chat::ChatComponent,
        common::{self, CommonPacket},
        entity::EntityPacket,
        interact::{self, ClientCommand, Hand},
        inventory::{ComponentValue, ItemData, SetSelectedSlot, Slot},
        typed::{ChatPacket, DecodedPacket, InventoryPacket},
        ClientSettings,
    },
    registry::RegistryStore,
    version::{Direction, State},
    Limits, Version,
};
use std::{
    collections::BTreeMap,
    io::Write,
    time::{Duration, Instant},
};
fn record(counts: &mut BTreeMap<&'static str, u64>, category: &'static str) {
    *counts.entry(category).or_default() += 1;
    println!("EVENT category={category}");
}
fn slot_observations(slot: &Slot, counts: &mut BTreeMap<&'static str, u64>) {
    let Slot::Item(item) = slot else { return };
    match &item.data {
        ItemData::Legacy(Some(nbt)) => {
            if nbt.root.get("Damage").and_then(|t| t.as_i32()) == Some(3) {
                record(counts, "item_damage");
            }
            if nbt.root.get("rustwire_probe").is_some() {
                record(counts, "item_custom");
            }
        }
        ItemData::Components(patch) => {
            for c in &patch.added {
                if c.name == "damage" && matches!(c.value, ComponentValue::VarInt(3)) {
                    record(counts, "item_damage");
                }
                if c.name == "custom_data" {
                    if let ComponentValue::Nbt(nbt) = &c.value {
                        if nbt.root.get("rustwire_probe").is_some() {
                            record(counts, "item_custom");
                        }
                    }
                }
            }
        }
        _ => {}
    }
}
fn nbt_contains(tag: &rustwire_mc::nbt::Tag, needle: &str) -> bool {
    use rustwire_mc::nbt::Tag;
    match tag {
        Tag::String(text) => text.to_string_lossy().contains(needle),
        Tag::Compound(entries) => entries.iter().any(|(_, tag)| nbt_contains(tag, needle)),
        Tag::List { elements, .. } => elements.iter().any(|tag| nbt_contains(tag, needle)),
        _ => false,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        return Err("usage: gameplay_probe 127.0.0.1 PORT VERSION [MIN_SECONDS]".into());
    }
    let host = &args[1];
    if host != "127.0.0.1" && host != "::1" {
        return Err("gameplay_probe only permits loopback disposable test servers".into());
    }
    let port = args[2].parse()?;
    let version: Version = args[3].parse()?;
    let min_seconds = args
        .get(4)
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(45);
    let limits = Limits::default();
    let mut c = Connection::connect(
        (host.as_str(), port),
        version,
        Duration::from_secs(20),
        limits,
    )?;
    c.start_login(host, port, "Rustwire", [0; 16])?;
    let started = Instant::now();
    let mut registries = RegistryStore::default();
    let mut sections = 24;
    let mut ready = false;
    let mut loaded = false;
    let mut counts = BTreeMap::new();
    for _ in 0..100_000 {
        match c.next_typed_event()? {
            TypedEvent::Control(Event::LoginSuccess(_)) => {
                if version.has_configuration() {
                    c.send_settings(&ClientSettings::default())?;
                }
            }
            TypedEvent::Control(Event::Joined(mut world)) => {
                if let Some(nbt) = world.dimension_codec.take() {
                    registries.apply(rustwire_mc::registry::RegistryData::Legacy(nbt))?;
                }
                sections = world.spawn.dimension(&registries)?.section_count();
                if !version.has_configuration() {
                    c.send_settings(&ClientSettings::default())?;
                }
                record(&mut counts, "join");
            }
            TypedEvent::Control(Event::Registry(registry)) => registries.apply(registry)?,
            TypedEvent::Control(Event::KnownPacks(_)) => c.select_known_packs(&[])?,
            TypedEvent::Control(Event::CookieRequest(key)) => c.answer_cookie(&key, None)?,
            TypedEvent::Control(Event::LoginPluginRequest { id, .. }) => {
                c.answer_login_plugin(id, None)?
            }
            TypedEvent::Control(Event::Ready) => record(&mut counts, "configuration"),
            TypedEvent::Control(Event::KeepAlive(_)) => record(&mut counts, "keepalive"),
            TypedEvent::Control(Event::Position(position)) => {
                c.send(&position.acknowledgement(version)?)?;
                record(&mut counts, "teleport_ack");
                if !loaded
                    && version
                        .packet_id(State::Play, Direction::Serverbound, "player_loaded")
                        .is_ok()
                {
                    c.send(&packet::named(
                        version,
                        State::Play,
                        "player_loaded",
                        vec![],
                    )?)?;
                    loaded = true;
                }
                if !ready {
                    if position.relative_flags == 0 {
                        c.send(&packet::player_position(
                            version,
                            position.x + 0.125,
                            position.y,
                            position.z,
                            true,
                            false,
                        )?)?;
                        record(&mut counts, "movement_sent");
                    }
                    c.send(&SetSelectedSlot { slot: 1 }.packet(version, limits)?)?;
                    c.send(&interact::swing_arm(version, Hand::Main)?)?;
                    record(&mut counts, "selected_slot_sent");
                    ready = true;
                    println!("PROBE_READY");
                    std::io::stdout().flush()?;
                }
            }
            TypedEvent::Control(Event::EncryptionRequested(request)) => {
                if request.should_authenticate {
                    return Err("gameplay probe requires offline mode".into());
                }
                #[cfg(feature = "crypto")]
                {
                    let response = rustwire_mc::crypto::encryption_response(
                        &request.public_key,
                        &request.verify_token,
                    )?;
                    c.complete_encryption(&response)?;
                }
                #[cfg(not(feature = "crypto"))]
                return Err("enable crypto for requested offline encryption".into());
            }
            TypedEvent::Decoded(DecodedPacket::Common(packet)) => match packet {
                CommonPacket::ChunkBatchFinished(_) => {
                    c.send(&common::chunk_batch_received(version, 20.)?)?
                }
                CommonPacket::ResourcePack(_)
                | CommonPacket::CodeOfConduct(_)
                | CommonPacket::Transfer(_) => {
                    return Err(
                        "unexpected application-consent challenge in disposable test server".into(),
                    )
                }
                _ => {}
            },
            TypedEvent::Decoded(DecodedPacket::Entity(packet)) => match packet {
                EntityPacket::Spawn(entity) => {
                    record(&mut counts, "entity_spawn");
                    println!(
                        "ENTITY id={} type={} uuid={:02x?}",
                        entity.entity_id, entity.entity_type, entity.uuid
                    );
                }
                EntityPacket::Move(_)
                | EntityPacket::MoveLook(_)
                | EntityPacket::Teleport(_)
                | EntityPacket::SyncPosition(_) => record(&mut counts, "entity_move"),
                EntityPacket::Destroy(_) => record(&mut counts, "entity_remove"),
                EntityPacket::Velocity(_) => record(&mut counts, "entity_velocity"),
                EntityPacket::Health(health) => {
                    record(&mut counts, "health");
                    println!("HEALTH value={}", health.health);
                }
                _ => {}
            },
            TypedEvent::Decoded(DecodedPacket::Metadata(metadata)) => {
                record(&mut counts, "metadata");
                println!(
                    "METADATA entity={} entries={}",
                    metadata.entity_id,
                    metadata.entries.len()
                );
            }
            TypedEvent::Decoded(DecodedPacket::World(packet)) => match packet {
                WorldPacket::Block(_) | WorldPacket::SectionBlocks(_) => {
                    record(&mut counts, "block_update")
                }
                WorldPacket::Respawn(respawn) => {
                    sections = respawn.spawn.dimension(&registries)?.section_count();
                    loaded = false;
                    record(&mut counts, "respawn");
                }
                _ => {}
            },
            TypedEvent::Decoded(DecodedPacket::Inventory(packet)) => {
                record(&mut counts, "inventory");
                match packet {
                    InventoryPacket::Content(p) => {
                        for item in &p.items {
                            slot_observations(item, &mut counts);
                        }
                        slot_observations(&p.carried_item, &mut counts);
                    }
                    InventoryPacket::Slot(p) => slot_observations(&p.item, &mut counts),
                    InventoryPacket::PlayerSlot(p) => slot_observations(&p.item, &mut counts),
                    InventoryPacket::Cursor(p) => slot_observations(&p.item, &mut counts),
                    _ => {}
                }
            }
            TypedEvent::Decoded(DecodedPacket::Chat(ChatPacket::System(chat))) => {
                record(&mut counts, "system_chat");
                let has_marker = match chat.content {
                    ChatComponent::Json(text) => text.contains("Rustwire gameplay probe"),
                    ChatComponent::Nbt(nbt) => nbt_contains(&nbt.root, "Rustwire gameplay probe"),
                };
                if has_marker {
                    record(&mut counts, "chat_marker");
                }
            }
            TypedEvent::Raw {
                name: Some("map_chunk"),
                packet,
                ..
            } => {
                let chunk = ChunkData::decode(&packet.data, version, sections, limits)?;
                if !chunk.sections.is_empty() {
                    record(&mut counts, "chunk");
                }
            }
            TypedEvent::Raw {
                name: Some("death_combat_event"),
                ..
            } => {
                record(&mut counts, "death");
                c.send(&ClientCommand::Respawn.encode(version)?)?;
                record(&mut counts, "respawn_requested");
            }
            TypedEvent::Raw {
                name,
                unsupported: Some(reason),
                ..
            } => println!("UNSUPPORTED name={name:?} reason={reason}"),
            TypedEvent::Disconnected(reason) => {
                return Err(format!("server disconnected: {reason:?}").into())
            }
            _ => {}
        }
        if started.elapsed() >= Duration::from_secs(min_seconds)
            && counts.get("respawn").copied().unwrap_or(0) > 0
        {
            let required = [
                "chunk",
                "inventory",
                "item_damage",
                "item_custom",
                "system_chat",
                "chat_marker",
                "entity_spawn",
                "entity_move",
                "entity_remove",
                "block_update",
                "health",
                "respawn",
                "keepalive",
            ];
            println!(
                "PROBE_SUMMARY protocol={} seconds={:.3} counts={counts:?}",
                version.protocol(),
                started.elapsed().as_secs_f64()
            );
            let missing: Vec<_> = required
                .into_iter()
                .filter(|key| counts.get(key).copied().unwrap_or(0) == 0)
                .collect();
            if !missing.is_empty() {
                return Err(format!("missing required typed categories: {missing:?}").into());
            }
            return Ok(());
        }
    }
    Err("gameplay probe packet budget exhausted".into())
}
