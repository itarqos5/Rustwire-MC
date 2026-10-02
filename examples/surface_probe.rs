//! Isolated Paper command/suggestion and world-effect interoperability probe.
//! Receives and decodes packets; never executes a client command.
use rustwire_mc::{
    connection::{Connection, Event},
    packet::{
        self,
        chat::ChatComponent,
        commands::{
            CommandNodeKind, CommandSuggestionRequest, ParserProperties, SuggestionProvider,
        },
        common::{self, CommonPacket},
        entity::EntityPacket,
        entity_metadata::{holders::RegistryHolder, particles::ParticleData},
        inventory::{Slot, SoundHolder},
        typed::{ChatPacket, CommandPacket, DecodedPacket, WorldEffectPacket},
        world_effects::ExplosionData,
        ClientSettings,
    },
    version::{Direction, State},
    Limits, Version,
};
use std::{
    collections::BTreeSet,
    io::Write,
    time::{Duration, Instant},
};

fn contains(tag: &rustwire_mc::nbt::Tag, needle: &str) -> bool {
    use rustwire_mc::nbt::Tag;
    match tag {
        Tag::String(s) => s.to_string_lossy().contains(needle),
        Tag::Compound(v) => v.iter().any(|(_, t)| contains(t, needle)),
        Tag::List { elements, .. } => elements.iter().any(|t| contains(t, needle)),
        _ => false,
    }
}
fn sound(sound: &SoundHolder) -> String {
    match sound {
        RegistryHolder::RegistryId(id) => format!("registry:{id}"),
        RegistryHolder::Inline(s) => format!("inline:{}", s.name),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || args[1] != "127.0.0.1" {
        return Err("usage: surface_probe 127.0.0.1 PORT VERSION (disposable server only)".into());
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
    let mut ready = false;
    let mut loaded = false;
    let mut sent_request = false;
    let mut fixtures = BTreeSet::new();
    for _ in 0..100_000 {
        if started.elapsed() > Duration::from_secs(120) {
            return Err("bounded surface probe timed out".into());
        }
        match c.next_event()? {
            Event::LoginSuccess(_) if version.has_configuration() => {
                c.send_settings(&ClientSettings::default())?
            }
            Event::Joined(_) if !version.has_configuration() => {
                c.send_settings(&ClientSettings::default())?
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
                println!(
                    "VALUE category=position x={} y={} z={} flags={}",
                    p.x, p.y, p.z, p.relative_flags
                );
            }
            Event::Packet {
                state,
                name: Some(name),
                packet,
            } => {
                let decoded =
                    match DecodedPacket::decode(state, name, &packet.data, version, limits) {
                        Ok(value) => value,
                        Err(error) => {
                            println!(
                                "DECODE_ERROR name={name} error={error:?} bytes={:02x?}",
                                packet.data
                            );
                            return Err(error.into());
                        }
                    };
                if decoded.is_none()
                    && matches!(
                        name,
                        "declare_commands"
                            | "tab_complete"
                            | "world_particles"
                            | "explosion"
                            | "sound_effect"
                            | "entity_sound_effect"
                            | "stop_sound"
                            | "world_event"
                    )
                {
                    println!("RAW_SCENARIO name={name} bytes={:02x?}", packet.data);
                    return Err("raw scenario packet".into());
                }
                let encoded = match &decoded {
                    Some(DecodedPacket::Command(CommandPacket::Tree(v))) => {
                        Some(v.encode(version, limits)?)
                    }
                    Some(DecodedPacket::Command(CommandPacket::Suggestions(v))) => {
                        Some(v.encode(version, limits)?)
                    }
                    Some(DecodedPacket::WorldEffect(v)) => Some(match v {
                        WorldEffectPacket::Particles(v) => v.encode(version, limits)?,
                        WorldEffectPacket::Explosion(v) => v.encode(version, limits)?,
                        WorldEffectPacket::Sound(v) => v.encode(version, limits)?,
                        WorldEffectPacket::EntitySound(v) => v.encode(version, limits)?,
                        WorldEffectPacket::StopSound(v) => v.encode(version, limits)?,
                        WorldEffectPacket::Event(v) => v.encode(version, limits)?,
                    }),
                    _ => None,
                };
                if let Some(encoded) = encoded {
                    let equal = encoded == packet.data;
                    println!("VALUE category=wire_roundtrip packet={name} equal={equal}");
                    if !equal {
                        return Err("real received packet failed byte-exact re-encoding".into());
                    }
                }
                match decoded {
                    Some(DecodedPacket::Command(CommandPacket::Tree(tree))) => {
                        let root = tree.nodes.get(tree.root_index as usize).ok_or("missing tree root")?;
                        let valid_refs = tree.nodes.iter().all(|n| n.children.iter().all(|&i| (i as usize) < tree.nodes.len()) && n.redirect.is_none_or(|i| (i as usize) < tree.nodes.len()));
                        let names: BTreeSet<_> = root.children.iter().filter_map(|&i| match &tree.nodes[i as usize].kind { CommandNodeKind::Literal { name } => Some(name.as_str()), _ => None }).collect();
                        let arguments = tree.nodes.iter().filter(|n| matches!(n.kind, CommandNodeKind::Argument { .. })).count();
                        let redirects = tree.nodes.iter().filter(|n| n.redirect.is_some()).count();
                        println!("VALUE category=command_tree root={} nodes={} root_kind={} valid_refs={valid_refs} msg={} arguments={arguments} redirects={redirects} names={}", tree.root_index, tree.nodes.len(), matches!(root.kind, CommandNodeKind::Root), names.contains("msg"), names.into_iter().collect::<Vec<_>>().join(","));
                        for node in &tree.nodes {
                            if let CommandNodeKind::Argument { name, parser, suggestions } = &node.kind {
                                if name == "targets" {
                                    if let ParserProperties::Entity { single, players_only } = parser.properties {
                                        let message_child = node.children.iter().any(|&i| matches!(&tree.nodes[i as usize].kind, CommandNodeKind::Argument { name, .. } if name == "message"));
                                        println!("VALUE category=command_target single={single} players_only={players_only} ask_server={} message_child={message_child}", matches!(suggestions, Some(SuggestionProvider::AskServer)));
                                    }
                                }
                            }
                            if let (CommandNodeKind::Literal { name }, Some(target)) = (&node.kind, node.redirect) {
                                if let CommandNodeKind::Literal { name: target } = &tree.nodes[target as usize].kind {
                                    println!("VALUE category=command_redirect name={name} target={target}");
                                }
                            }
                        }
                        for (i, node) in tree.nodes.iter().enumerate() {
                            if let CommandNodeKind::Argument { name, parser, suggestions } = &node.kind { println!("NODE index={i} name={name} parser={:?} properties={:?} suggestions={suggestions:?} children={:?} redirect={:?}", parser.kind, parser.properties, node.children, node.redirect); }
                        }
                    }
                    Some(DecodedPacket::Command(CommandPacket::Suggestions(s))) => println!("VALUE category=suggestions transaction={} start={} length={} matches={} rustwire={} tooltips={} requested={sent_request}", s.transaction_id, s.start, s.length, s.matches.len(), s.matches.iter().any(|m| m.text == "Rustwire"), s.matches.iter().filter(|m| m.tooltip.is_some()).count()),
                    Some(DecodedPacket::WorldEffect(effect)) => match effect {
                        WorldEffectPacket::Particles(p) => {
                            println!("VALUE category=particle_{} long={} always={} x={} y={} z={} dx={} dy={} dz={} speed={} count={}", p.particle.kind, p.long_distance, p.always_show.map_or("absent".into(), |b| b.to_string()), p.position[0], p.position[1], p.position[2], p.offset[0], p.offset[1], p.offset[2], p.speed, p.count);
                            match p.particle.data {
                                ParticleData::DustRgb { color, scale } => println!("VALUE category=dust_data layout=rgb r={} g={} b={} scale={scale}", color[0], color[1], color[2]),
                                ParticleData::DustPacked { color, scale } => println!("VALUE category=dust_data layout=packed color={color} scale={scale}"),
                                ParticleData::TransitionRgb { from, to, scale } => println!("VALUE category=transition_data layout=rgb from={},{},{} to={},{},{} scale={scale}", from[0],from[1],from[2],to[0],to[1],to[2]),
                                ParticleData::TransitionPacked { from, to, scale } => println!("VALUE category=transition_data layout=packed from={from} to={to} scale={scale}"),
                                ParticleData::Item(item) => if let Slot::Item(item) = *item { println!("VALUE category=item_data layout=slot id={} count={}", item.item_id, item.count); },
                                ParticleData::ItemTemplate(item) => println!("VALUE category=item_data layout=template id={} count={}", item.item_id, item.count),
                                _ => {}
                            }
                        }
                        WorldEffectPacket::Sound(s) => println!("VALUE category=sound sound={} source={:?} x={} y={} z={} volume={} pitch={} seed={}", sound(&s.sound), s.source, s.position.x, s.position.y, s.position.z, s.volume, s.pitch, s.seed),
                        WorldEffectPacket::StopSound(s) => println!("VALUE category=stop_sound source={} sound={}", s.source.map_or("none".into(), |v| format!("{v:?}")), s.sound.as_deref().unwrap_or("none")),
                        WorldEffectPacket::EntitySound(s) => println!("VALUE category=entity_sound sound={} source={:?} entity={} fixture={} volume={} pitch={} seed={}", sound(&s.sound), s.source, s.entity_id, fixtures.contains(&s.entity_id), s.volume, s.pitch, s.seed),
                        WorldEffectPacket::Event(e) => println!("VALUE category=world_event id={} x={} y={} z={} data={} global={}",e.event_id,e.position.x,e.position.y,e.position.z,e.data,e.global),
                        WorldEffectPacket::Explosion(e) => {
                            println!("VALUE category=explosion x={} y={} z={}",e.center[0],e.center[1],e.center[2]);
                            match e.data {
                                ExplosionData::Legacy(e) => {
                                    println!("VALUE category=explosion_layout layout=legacy radius={} blocks={} knockback={},{},{} effects={}", e.radius,e.affected_block_offsets.len(),e.player_knockback[0],e.player_knockback[1],e.player_knockback[2],e.effects.is_some());
                                    if let Some(v) = e.effects { println!("VALUE category=explosion_effects interaction={:?} small={} large={} sound={}",v.block_interaction,v.small_particle.kind,v.large_particle.kind,sound(&v.sound)); }
                                }
                                ExplosionData::Modern(e) => {
                                    println!("VALUE category=explosion_layout layout=modern knockback={} particle={} sound={} block_effects={}",e.player_knockback.is_some(),e.particle.kind,sound(&e.sound),e.block_effects.is_some());
                                    if let Some(v) = e.block_effects { println!("VALUE category=explosion_blocks radius={} count={} particles={} positive_weights={} finite_parameters={}",v.radius,v.block_count,v.particles.len(),v.particles.iter().all(|p|p.weight>0),v.particles.iter().all(|p|p.scaling.is_finite()&&p.speed.is_finite())); for p in v.particles { println!("EXPLOSION_PARTICLE kind={} scaling={} speed={} weight={}",p.particle.kind,p.scaling,p.speed,p.weight); } }
                                }
                            }
                        }
                    },
                    Some(DecodedPacket::Entity(EntityPacket::Spawn(e))) if e.position == [2.5, 200.0, 0.5] => { fixtures.insert(e.entity_id); println!("VALUE category=fixture_entity entity={} type={}",e.entity_id,e.entity_type); }
                    Some(DecodedPacket::Common(p)) => match p {
                        CommonPacket::ChunkBatchFinished(_) => c.send(&common::chunk_batch_received(version, 20.)?)?,
                        CommonPacket::ResourcePack(_) | CommonPacket::CodeOfConduct(_) | CommonPacket::Transfer(_) => return Err("unexpected application consent challenge".into()),
                        _ => {}
                    },
                    Some(DecodedPacket::Chat(ChatPacket::System(chat))) => {
                        let has = |s: &str| match &chat.content { ChatComponent::Json(v) => v.contains(s), ChatComponent::Nbt(v) => contains(&v.root,s) };
                        if has("RUSTWIRE_SURFACE_REQUEST") && !sent_request {
                            c.send(&CommandSuggestionRequest { transaction_id: 1701, text: "/msg Rustw".into() }.packet(version, limits)?)?;
                            sent_request = true; println!("ACTION kind=suggestion_request transaction=1701 text=/msg_Rustw");
                        }
                        if has("RUSTWIRE_SURFACE_DONE") { println!("PROBE_DONE protocol={} seconds={:.3}",version.protocol(),started.elapsed().as_secs_f64()); return Ok(()); }
                    }
                    _ => {}
                }
            }
            Event::Disconnected(reason) => return Err(format!("disconnected: {reason:?}").into()),
            _ => {}
        }
    }
    Err("surface packet bound exceeded".into())
}
