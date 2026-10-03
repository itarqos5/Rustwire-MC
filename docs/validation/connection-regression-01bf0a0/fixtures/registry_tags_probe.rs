//! Connect only to an offline-mode server you own or are authorized to test.
//! Frozen-snapshot supplemental registry/tag probe. HOST must be 127.0.0.1.
//! Usage: registry_tags_probe HOST PORT VERSION [MIN_SECONDS]
use rustwire_mc::{
    chunk::ChunkData,
    codec::Writer,
    connection::{Connection, Event},
    packet::{self, ClientSettings},
    registry::RegistryStore,
    version::State,
    Limits,
};
use std::{net::Shutdown, time::{Duration, Instant}};
use rustwire_mc::packet::common::CommonPacket;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        return Err("usage: registry_tags_probe HOST PORT VERSION [MIN_SECONDS]".into());
    }
    let host = &args[1];
    if host != "127.0.0.1" { return Err("probe only permits IPv4 loopback".into()); }
    let port = args[2].parse()?;
    let version = args[3].parse()?;
    let mut sections = 0usize;
    let mut tag_packets = 0usize;
    let mut tag_registries = 0usize;
    let mut tag_count = 0usize;
    let mut tag_members = 0usize;
    let mut compression = false;
    let mut registry_entries = 0usize;
    let mut registry_roundtrips = 0usize;
    let mut tag_roundtrips = 0usize;
    let mut chunk_roundtrips = 0usize;
    let mut chunk_wire_equal = 0usize;
    let min_seconds = args
        .get(4)
        .map(|x| x.parse::<u64>())
        .transpose()?
        .unwrap_or(20);
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
    c.start_login(host, port, "RustwireTags", [0; 16])?;
    let mut registries = RegistryStore::default();
    let mut chunks = 0;
    let mut loaded = false;
    for _ in 0..100_000 {
        match c.next_event().map_err(|e| format!("control packet: {e}"))? {
            Event::Compression(threshold) => {
                compression = threshold.is_some();
                println!("compression negotiated: {threshold:?}");
            }
            Event::LoginSuccess(p) => {
                println!("login {}", p.username);
                if version.has_configuration() {
                    c.send_settings(&ClientSettings::default())?;
                }
            }
            Event::Registry(r) => {
                registry_packets += 1;
                match &r {
                    rustwire_mc::registry::RegistryData::Legacy(_) => println!("registry wire: legacy compound"),
                    rustwire_mc::registry::RegistryData::Entries { registry, entries } => {
                        registry_entries += entries.len();
                        println!("registry wire: {registry}, entries={}, present_nbt={}", entries.len(), entries.iter().filter(|e| e.data.is_some()).count());
                    }
                }
                let encoded = r.encode(version, limits)?;
                if rustwire_mc::registry::RegistryData::decode(&encoded, version, limits)? != r { return Err("registry semantic roundtrip failed".into()); }
                registry_roundtrips += 1;
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
                sections = dimension.section_count();
                println!("DERIVED_SECTIONS={sections}");
                println!(
                    "joined dimension {}: min_y={}, height={}, sections={sections}",
                    world.spawn.dimension_name, dimension.min_y, dimension.height
                );
                if !version.has_configuration() {
                    c.send_settings(&ClientSettings::default())?;
                }
            }
            Event::Packet { state, name: Some("tags"), packet } => {
                let Some(CommonPacket::Tags(tags)) = CommonPacket::decode("tags", &packet.data, version, limits)? else { return Err("tag dispatch missing".into()); };
                let state_checked = rustwire_mc::packet::tags::UpdateTags::decode_in_state(&packet.data, version, state, limits)?;
                if tags != state_checked { return Err("tag readers disagree".into()); }
                if tags.encode(version, limits)? != packet.data { return Err("tag wire roundtrip failed".into()); }
                tag_roundtrips += 1;
                tag_packets += 1;
                tag_registries += tags.registries.len();
                tag_count += tags.registries.iter().map(|r| r.tags.len()).sum::<usize>();
                tag_members += tags.registries.iter().flat_map(|r| &r.tags).map(|t| t.entries.len()).sum::<usize>();
                println!("tags decoded: state={state:?}, registries={}, tags={tag_count}, members={tag_members}, bytes={}", tags.registries.len(), packet.data.len());
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
                let encoded = chunk.encode(version, limits)?;
                if ChunkData::decode(&encoded, version, sections, limits)? != chunk { return Err("chunk semantic roundtrip failed".into()); }
                chunk_roundtrips += 1;
                if encoded == packet.data { chunk_wire_equal += 1; }
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
            if keep_alives == 0 || teleports == 0 || tag_packets == 0 || sections == 0 || registries.registries.is_empty() || !compression || (version.has_configuration() && configuration_completions == 0) {
                return Err("missing required control, tags, registry, dimension or compression evidence".into());
            }
            println!("ROUNDTRIPS registry={registry_roundtrips} tags={tag_roundtrips} chunks={chunk_roundtrips} chunk_wire_equal={chunk_wire_equal}");
            println!("PROBE_SUMMARY tags_packets={tag_packets} tag_registries={tag_registries} tags={tag_count} members={tag_members} modern_registry_entries={registry_entries} sections={sections} elapsed_ms={}", started.elapsed().as_millis());
            println!(
                "validated: {chunks} chunks, {keep_alives} keepalives, {teleports} teleports, {registry_packets} registry packets, {configuration_completions} configurations, {} registries, {:?}",
                registries.registries.len(),
                started.elapsed()
            );
            c.into_inner().shutdown(Shutdown::Both)?;
            println!("CLEAN_LOCAL_SHUTDOWN");
            return Ok(());
        }
    }
    Err("no four chunks within packet budget".into())
}
