//! Bounded loopback-only streamed-world interoperability probe.
use rustwire_mc::{
    chunk::LightData,
    connection::{Connection, Event},
    nbt::Tag,
    packet::{
        self,
        chat::ChatComponent,
        common::{self, CommonPacket},
        typed::{ChatPacket, ChunkUpdatePacket, DecodeContext, DecodedPacket},
        ClientSettings,
    },
    registry::{RegistryData, RegistryStore},
    version::{Direction, State},
    Limits, Version,
};
use std::{
    io::Write,
    time::{Duration, Instant},
};

fn contains(tag: &Tag, needle: &str) -> bool {
    match tag {
        Tag::String(s) => s.to_string_lossy().contains(needle),
        Tag::Compound(v) => v.iter().any(|(_, t)| contains(t, needle)),
        Tag::List { elements, .. } => elements.iter().any(|t| contains(t, needle)),
        _ => false,
    }
}
fn sample_layers(
    mask: &[i64],
    empty: &[i64],
    arrays: &[Vec<u8>],
    section: usize,
    index: usize,
) -> Option<u8> {
    let bit = |mask: &[i64], i: usize| {
        mask.get(i / 64)
            .is_some_and(|&w| w as u64 & (1u64 << (i % 64)) != 0)
    };
    if bit(empty, section) {
        return Some(0);
    }
    if !bit(mask, section) {
        return None;
    }
    let array = (0..section).filter(|&i| bit(mask, i)).count();
    arrays
        .get(array)
        .and_then(|a| a.get(index / 2))
        .map(|&b| (b >> (4 * (index % 2))) & 15)
}
fn sample(light: &LightData, section: usize, index: usize) -> Option<u8> {
    sample_layers(
        &light.block_mask,
        &light.empty_block_mask,
        &light.block_arrays,
        section,
        index,
    )
}
fn sign_values(root: Option<&Tag>) -> (bool, bool, bool) {
    let front = root.and_then(|r| r.get("front_text"));
    let red = matches!(front.and_then(|f| f.get("color")), Some(Tag::String(v)) if v.equals("red"));
    let glow = matches!(
        front.and_then(|f| f.get("has_glowing_text")),
        Some(Tag::Byte(1))
    );
    let wax = matches!(root.and_then(|r| r.get("is_waxed")), Some(Tag::Byte(1)));
    (red, glow, wax)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || args[1] != "127.0.0.1" {
        return Err("usage: chunk_update_probe 127.0.0.1 PORT VERSION".into());
    }
    let port = args[2].parse()?;
    let version: Version = args[3].parse()?;
    let limits = Limits::default();
    let mut c = Connection::connect(
        (args[1].as_str(), port),
        version,
        Duration::from_secs(20),
        limits,
    )?;
    c.start_login(&args[1], port, "Rustwire", [0; 16])?;
    let started = Instant::now();
    let (mut ready, mut loaded) = (false, false);
    let mut registries = RegistryStore::default();
    let mut context = DecodeContext::default();
    let mut min_y = 0;
    let mut stage = "initial";
    for _ in 0..100_000 {
        if started.elapsed() > Duration::from_secs(120) {
            return Err("bounded chunk update probe timed out".into());
        }
        match c.next_event()? {
            Event::LoginSuccess(_) if version.has_configuration() => {
                c.send_settings(&ClientSettings::default())?
            }
            Event::Registry(registry) => registries.apply(registry)?,
            Event::Joined(mut world) => {
                if let Some(nbt) = world.dimension_codec.take() {
                    registries.apply(RegistryData::Legacy(nbt))?;
                }
                let dimension = world.spawn.dimension(&registries)?;
                context = DecodeContext::for_dimension(&dimension);
                min_y = dimension.min_y;
                let biomes = registries
                    .get("minecraft:worldgen/biome")
                    .ok_or("missing biome registry")?;
                let desert = biomes.by_key("minecraft:desert").ok_or("missing desert")?.0;
                let plains = biomes.by_key("minecraft:plains").ok_or("missing plains")?.0;
                println!("VALUE category=context source=registry_join sections={} min_y={} height={} desert={} plains={}",dimension.section_count(),dimension.min_y,dimension.height,desert,plains);
                if !version.has_configuration() {
                    c.send_settings(&ClientSettings::default())?;
                }
            }
            Event::KnownPacks(_) => c.select_known_packs(&[])?,
            Event::CookieRequest(key) => c.answer_cookie(&key, None)?,
            Event::LoginPluginRequest { id, .. } => c.answer_login_plugin(id, None)?,
            Event::EncryptionRequested(request) => {
                if request.should_authenticate {
                    return Err("requires isolated offline mode".into());
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
                return Err("enable crypto for offline encryption".into());
            }
            Event::Position(p) => {
                c.send(&p.acknowledgement(version)?)?;
                if p.relative_flags & 7 == 0 {
                    c.send(&packet::player_position(
                        version, p.x, p.y, p.z, false, false,
                    )?)?;
                }
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
                    println!("PROBE_READY");
                    std::io::stdout().flush()?;
                    ready = true;
                }
            }
            Event::Packet {
                state,
                name: Some(name),
                packet,
            } => {
                let decoded = match DecodedPacket::decode_with_context(
                    state,
                    name,
                    &packet.data,
                    version,
                    limits,
                    context,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        println!("DECODE_ERROR name={name} error={e:?}");
                        return Err(e.into());
                    }
                };
                if matches!(name, "map_chunk" | "update_light" | "chunk_biomes") {
                    let no_context_raw =
                        DecodedPacket::decode(state, name, &packet.data, version, limits)?
                            .is_none();
                    println!("VALUE category=context_dispatch packet={name} no_context_raw={no_context_raw} contextual={}",matches!(decoded,Some(DecodedPacket::ChunkUpdate(_))));
                }
                if let Some(DecodedPacket::ChunkUpdate(v)) = &decoded {
                    let sections = context.section_count.ok_or("missing dimension context")?;
                    let encoded = match v {
                        ChunkUpdatePacket::FullChunk(v) => v.encode(version, limits)?,
                        ChunkUpdatePacket::Light(v) => v.encode(version, sections, limits)?,
                        ChunkUpdatePacket::Biomes(v) => v.encode(version, sections, limits)?,
                        ChunkUpdatePacket::BlockEntity(v) => v.encode(version, limits)?,
                        ChunkUpdatePacket::ViewPosition(v) => v.encode(version, limits)?,
                        ChunkUpdatePacket::ViewDistance(v) => v.encode(version, limits)?,
                        ChunkUpdatePacket::SimulationDistance(v) => v.encode(version, limits)?,
                    };
                    let equal = encoded == packet.data;
                    let semantic = DecodedPacket::decode_with_context(
                        state, name, &encoded, version, limits, context,
                    )?
                    .as_ref()
                        == decoded.as_ref();
                    let allowed_padding = (name == "map_chunk"
                        && [763, 770].contains(&version.protocol()))
                        || (name == "chunk_biomes" && version.protocol() == 770);
                    println!("VALUE category=wire_roundtrip packet={name} equal={equal} semantic={semantic} canonical_delta={}",packet.data.len() as i64-encoded.len() as i64);
                    if !semantic || (!equal && !allowed_padding) {
                        return Err("received packet failed canonical re-encoding".into());
                    }
                    match v {
                        ChunkUpdatePacket::FullChunk(v) => {
                            println!("VALUE category=full_chunk x={} z={} sections={} embedded={} optional_compounds={}",v.x,v.z,v.sections.len(),v.block_entities.len(),v.block_entities.iter().all(|e|e.data.as_ref().is_none_or(|n|matches!(n.root,Tag::Compound(_)))));
                            if v.x == 0 && v.z == 0 {
                                for e in &v.block_entities {
                                    if e.local_x == 2 && e.local_z == 2 && e.y == 200 {
                                        let (red, glow, wax) =
                                            sign_values(e.data.as_ref().map(|n| &n.root));
                                        println!("VALUE category=embedded_sign x=2 y=200 z=2 kind={} red={red} glow={glow} wax={wax}",e.kind);
                                    }
                                }
                            }
                        }
                        ChunkUpdatePacket::Light(v) => {
                            let section = ((200 - min_y) / 16 + 1) as usize;
                            let source = sample(&v.light, section, 8 * 256 + 16 + 1)
                                .map_or("unchanged".into(), |v| v.to_string());
                            let neighbor = sample(&v.light, section, 8 * 256 + 16)
                                .map_or("unchanged".into(), |v| v.to_string());
                            let sky = sample_layers(
                                &v.light.sky_mask,
                                &v.light.empty_sky_mask,
                                &v.light.sky_arrays,
                                section,
                                8 * 256 + 16 + 1,
                            )
                            .map_or("unchanged".into(), |v| v.to_string());
                            let mask_bit = |mask: &[i64]| {
                                mask.get(section / 64)
                                    .is_some_and(|&w| w as u64 & (1u64 << (section % 64)) != 0)
                            };
                            let sky_bit = mask_bit(&v.light.sky_mask);
                            let block_empty = mask_bit(&v.light.empty_block_mask);
                            let mask = |v: &[i64]| {
                                v.iter()
                                    .map(|&w| format!("{:x}", w as u64))
                                    .collect::<Vec<_>>()
                                    .join(",")
                            };
                            println!("VALUE category=light stage={stage} x={} z={} source={source} neighbor={neighbor} sky={sky} section={section} sample_index=2065 neighbor_index=2064 sky_bit={sky_bit} block_empty={block_empty} sky_mask={} block_mask={} empty_block_mask={} arrays={} sky_arrays={} valid_lengths={}",v.chunk_x,v.chunk_z,mask(&v.light.sky_mask),mask(&v.light.block_mask),mask(&v.light.empty_block_mask),v.light.block_arrays.len(),v.light.sky_arrays.len(),v.light.block_arrays.iter().chain(&v.light.sky_arrays).all(|a|a.len()==2048));
                        }
                        ChunkUpdatePacket::Biomes(v) => {
                            let registry = registries
                                .get("minecraft:worldgen/biome")
                                .ok_or("missing biome registry")?;
                            let desert = registry
                                .by_key("minecraft:desert")
                                .ok_or("missing desert")?
                                .0;
                            let plains = registry
                                .by_key("minecraft:plains")
                                .ok_or("missing plains")?
                                .0;
                            for chunk in &v.chunks {
                                let target = ((192 - min_y) / 16) as usize;
                                let desert_count = chunk
                                    .sections
                                    .iter()
                                    .flat_map(|s| s.values())
                                    .filter(|v| *v == Some(desert))
                                    .count();
                                let plains_count = chunk
                                    .sections
                                    .iter()
                                    .flat_map(|s| s.values())
                                    .filter(|v| *v == Some(plains))
                                    .count();
                                let pattern = chunk.sections.iter().enumerate().all(|(i, s)| {
                                    (0..4).all(|y| {
                                        (0..4).all(|z| {
                                            (0..4).all(|x| {
                                                s.get_xyz(x, y, z)
                                                    == Some(
                                                        if stage == "biome_mixed"
                                                            && i == target
                                                            && x < 2
                                                            && z < 2
                                                        {
                                                            plains
                                                        } else {
                                                            desert
                                                        },
                                                    )
                                            })
                                        })
                                    })
                                });
                                println!("VALUE category=biomes stage={stage} x={} z={} sections={} desert={desert_count} plains={plains_count} exact_pattern={pattern}",chunk.chunk_x,chunk.chunk_z,chunk.sections.len());
                            }
                        }
                        ChunkUpdatePacket::BlockEntity(v) => {
                            let root = v.data.as_ref().map(|n| &n.root);
                            let (red, glow, wax) = sign_values(root);
                            println!("VALUE category=block_entity x={} y={} z={} kind={} compound={} red={red} glow={glow} wax={wax}",v.position.x,v.position.y,v.position.z,v.kind,root.is_some_and(|r|matches!(r,Tag::Compound(_))));
                        }
                        ChunkUpdatePacket::ViewPosition(v) => println!(
                            "VALUE category=view_position x={} z={}",
                            v.chunk_x, v.chunk_z
                        ),
                        ChunkUpdatePacket::ViewDistance(v) => {
                            println!("VALUE category=view_distance distance={}", v.distance)
                        }
                        ChunkUpdatePacket::SimulationDistance(v) => {
                            println!("VALUE category=simulation_distance distance={}", v.distance)
                        }
                    }
                }
                match decoded {
                    Some(DecodedPacket::Common(CommonPacket::ChunkBatchFinished(_))) => {
                        c.send(&common::chunk_batch_received(version, 20.)?)?
                    }
                    Some(DecodedPacket::Common(
                        CommonPacket::ResourcePack(_)
                        | CommonPacket::CodeOfConduct(_)
                        | CommonPacket::Transfer(_),
                    )) => return Err("unexpected application consent challenge".into()),
                    Some(DecodedPacket::Chat(ChatPacket::System(chat))) => {
                        let has = |s: &str| match &chat.content {
                            ChatComponent::Json(v) => v.contains(s),
                            ChatComponent::Nbt(v) => contains(&v.root, s),
                        };
                        for candidate in ["light_on", "light_off", "biome_desert", "biome_mixed"] {
                            if has(&format!("RUSTWIRE_CHUNK_{candidate}")) {
                                stage = candidate;
                                println!("STAGE name={stage}");
                            }
                        }
                        if has("RUSTWIRE_CHUNK_DONE") {
                            println!(
                                "PROBE_DONE protocol={} seconds={:.3}",
                                version.protocol(),
                                started.elapsed().as_secs_f64()
                            );
                            return Ok(());
                        }
                    }
                    None if matches!(
                        name,
                        "map_chunk"
                            | "update_light"
                            | "chunk_biomes"
                            | "tile_entity_data"
                            | "update_view_position"
                            | "update_view_distance"
                            | "simulation_distance"
                    ) =>
                    {
                        println!("RAW_SCENARIO name={name}");
                        return Err("raw scenario packet".into());
                    }
                    _ => {}
                }
            }
            Event::Disconnected(reason) => return Err(format!("disconnected: {reason:?}").into()),
            _ => {}
        }
    }
    Err("packet bound exceeded".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibble_sampling_respects_sparse_masks_and_coordinate_order() {
        let mut light = LightData {
            block_mask: vec![(1 << 1) | (1 << 17)],
            block_arrays: vec![vec![0; 2048], vec![0; 2048]],
            ..LightData::default()
        };
        let index = 8 * 256 + 16 + 1;
        light.block_arrays[1][index / 2] = 0xfe;
        light.block_arrays[1][(index + 1) / 2] = 0x0e;
        assert_eq!(sample(&light, 17, index), Some(15));
        assert_eq!(sample(&light, 17, index - 1), Some(14));
        assert_eq!(sample(&light, 17, index + 1), Some(14));
        assert_eq!(sample(&light, 16, index), None);
        light.empty_block_mask = vec![1 << 16];
        assert_eq!(sample(&light, 16, index), Some(0));
    }
}
