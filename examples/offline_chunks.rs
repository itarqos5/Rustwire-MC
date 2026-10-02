//! Connect only to an offline-mode server you own or are authorized to test.
//! Usage: offline_chunks HOST PORT VERSION [SECTION_COUNT] [MIN_SECONDS]
use rustwire_mc::{
    chunk::ChunkData,
    codec::Writer,
    connection::{Connection, Event},
    packet::{self, ClientSettings},
    registry::RegistryStore,
    version::State,
    Limits,
};
use std::time::{Duration, Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        return Err("usage: offline_chunks HOST PORT VERSION [SECTION_COUNT] [MIN_SECONDS]".into());
    }
    let host = &args[1];
    let port = args[2].parse()?;
    let version = args[3].parse()?;
    let mut sections = args.get(4).map(|x| x.parse()).transpose()?.unwrap_or(24);
    let min_seconds = args
        .get(5)
        .map(|x| x.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
    let started = Instant::now();
    let mut keep_alives = 0usize;
    let mut registry_packets = 0usize;
    let mut configuration_completions = 0usize;
    let mut teleports = 0usize;
    let limits = Limits::default();
    let mut c = Connection::connect(
        (host.as_str(), port),
        version,
        Duration::from_secs(20),
        limits,
    )?;
    // Offline servers compute the effective UUID themselves; this is not account authentication.
    c.start_login(host, port, "Rustwire", [0; 16])?;
    let mut registries = RegistryStore::default();
    let mut chunks = 0;
    let mut loaded = false;
    for _ in 0..100_000 {
        match c.next_event().map_err(|e| format!("control packet: {e}"))? {
            Event::LoginSuccess(p) => {
                println!("login {}", p.username);
                if version.has_configuration() {
                    c.send_settings(&ClientSettings::default())?;
                }
            }
            Event::Registry(r) => {
                registry_packets += 1;
                registries.apply(r)?;
            }
            Event::Ready => {
                configuration_completions += 1;
                println!("configuration complete: {configuration_completions}");
            }
            Event::KeepAlive(_) => {
                keep_alives += 1;
                println!("keepalive acknowledged: {keep_alives}");
            }
            Event::KnownPacks(_) => c.select_known_packs(&[])?,
            Event::CookieRequest(key) => c.answer_cookie(&key, None)?,
            Event::LoginPluginRequest { id, .. } => c.answer_login_plugin(id, None)?,
            Event::Reconfigure => {
                registries.clear();
                loaded = false;
                c.send_settings(&ClientSettings::default())?;
            }
            Event::Position(p) => {
                teleports += 1;
                println!("teleport acknowledged: {}", p.teleport_id);
                c.send(&p.acknowledgement(version)?)?;
                if !loaded
                    && version
                        .packet_id(
                            State::Play,
                            rustwire_mc::version::Direction::Serverbound,
                            "player_loaded",
                        )
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
            }
            Event::Joined(mut world) => {
                if let Some(codec) = world.dimension_codec.take() {
                    registries.apply(rustwire_mc::registry::RegistryData::Legacy(codec))?;
                }
                let dimension = world.spawn.dimension(&registries)?;
                if args.get(4).is_none() {
                    sections = dimension.section_count();
                }
                println!(
                    "joined dimension {}: min_y={}, height={}, sections={sections}",
                    world.spawn.dimension_name, dimension.min_y, dimension.height
                );
                if !version.has_configuration() {
                    c.send_settings(&ClientSettings::default())?;
                }
            }
            Event::Packet {
                name: Some("map_chunk"),
                packet,
                ..
            } => {
                if let Ok(path) = std::env::var("RUSTWIRE_CAPTURE_CHUNK") {
                    std::fs::write(path, &packet.data)?;
                }
                let chunk = ChunkData::decode(&packet.data, version, sections, limits)
                    .map_err(|e| format!("chunk packet: {e} ({} bytes)", packet.data.len()))?;
                chunks += 1;
                println!(
                    "chunk {},{}: {} sections; first block ID {:?}",
                    chunk.x,
                    chunk.z,
                    chunk.sections.len(),
                    chunk.sections.first().and_then(|s| s.blocks.get(0))
                );
            }
            Event::Packet {
                name: Some("chunk_batch_finished"),
                ..
            } => {
                let mut w = Writer::new();
                w.f32(20.);
                c.send(&packet::named(
                    version,
                    State::Play,
                    "chunk_batch_received",
                    w.into_inner(),
                )?)?;
            }
            Event::Packet {
                name: Some("code_of_conduct" | "add_resource_pack" | "resource_pack_send"),
                ..
            } => {
                return Err(
                    "server requires an explicit application decision about conduct/resource packs"
                        .into(),
                )
            }
            Event::EncryptionRequested(request) => {
                if request.should_authenticate {
                    return Err(
                        "online-mode server: use a legitimately authenticated Minecraft session"
                            .into(),
                    );
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
                return Err("server requests encryption; rerun with --features crypto".into());
            }
            Event::Disconnected(data) => {
                return Err(format!(
                    "server disconnected with {} bytes of reason data",
                    data.len()
                )
                .into())
            }
            _ => {}
        }
        if chunks >= 4 && started.elapsed() >= Duration::from_secs(min_seconds) {
            println!(
                "validated: {chunks} chunks, {keep_alives} keepalives, {teleports} teleports, {registry_packets} registry packets, {configuration_completions} configurations, {} registries, {:?}",
                registries.registries.len(),
                started.elapsed()
            );
            return Ok(());
        }
    }
    Err("no four chunks within packet budget".into())
}
