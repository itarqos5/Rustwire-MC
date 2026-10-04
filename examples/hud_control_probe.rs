//! Bounded receive-side HUD/world-control checks on disposable loopback Paper.
//! This is a protocol probe, not a renderer or player-physics implementation.
use rustwire_mc::{
    connection::{Connection, Event},
    packet::{
        self,
        blocks::WorldPacket,
        chat::ChatComponent,
        hud::HudPacket,
        interact::ClientCommand,
        typed::{ChatPacket, DecodedPacket},
        world_control::WorldControlPacket,
        ClientSettings,
    },
    version::{Direction, State},
    Limits, Version,
};
use std::{
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
fn text_contains(text: &ChatComponent, needle: &str) -> bool {
    match text {
        ChatComponent::Json(s) => s.contains(needle),
        ChatComponent::Nbt(n) => contains(&n.root, needle),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 || args[1] != "127.0.0.1" {
        return Err(
            "usage: hud_control_probe 127.0.0.1 PORT VERSION (disposable server only)".into(),
        );
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
    let (mut ready, mut loaded, mut own_entity) = (false, false, None);
    let mut roundtrips = 0;
    let (mut requested_respawn, mut received_respawn, mut acknowledged_respawn) =
        (false, false, false);
    loop {
        if started.elapsed() > Duration::from_secs(120) {
            return Err("bounded HUD/control probe timed out".into());
        }
        match c.next_event()? {
            Event::LoginSuccess(_) if version.has_configuration() => {
                c.send_settings(&ClientSettings::default())?
            }
            Event::Joined(world) => {
                own_entity = Some(world.entity_id);
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
                if received_respawn && !acknowledged_respawn {
                    acknowledged_respawn = true;
                    println!("LIFECYCLE event=respawn_position_acknowledged");
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
                let decoded =
                    match DecodedPacket::decode(state, name, &packet.data, version, limits) {
                        Ok(value) => value,
                        Err(error) => {
                            println!("DECODE_ERROR name={name} error={error:?}");
                            return Err(error.into());
                        }
                    };
                let encoded = match &decoded {
                    Some(DecodedPacket::Hud(v)) => Some(v.encode(version, limits)?),
                    Some(DecodedPacket::WorldControl(v)) => Some(v.encode(version, limits)?),
                    _ => None,
                };
                if let Some(encoded) = encoded {
                    let equal = encoded == packet.data;
                    println!("VALUE category=wire_roundtrip packet={name} equal={equal}");
                    if !equal {
                        return Err("HUD/control roundtrip mismatch".into());
                    }
                    roundtrips += 1;
                }
                match decoded {
                    Some(DecodedPacket::Hud(v)) => match v {
                        HudPacket::ClearTitles(v) => println!("VALUE category=clear reset={}", v.reset),
                        HudPacket::Title(v) => println!("VALUE category=title fixture={}", text_contains(&v.text, "RUSTWIRE_TITLE")),
                        HudPacket::Subtitle(v) => println!("VALUE category=subtitle fixture={}", text_contains(&v.text, "RUSTWIRE_SUBTITLE")),
                        HudPacket::ActionBar(v) => println!("VALUE category=actionbar fixture={}", text_contains(&v.text, "RUSTWIRE_ACTIONBAR")),
                        HudPacket::TitleTime(v) => println!("VALUE category=title_time fade_in={} stay={} fade_out={}", v.fade_in, v.stay, v.fade_out),
                        HudPacket::Experience(v) => println!("VALUE category=experience level={} total={} bar_bits={:08x}", v.level, v.total, v.bar.to_bits()),
                        HudPacket::DeathCombat(v) => {
                            println!("VALUE category=death own_entity={} player_text={}", Some(v.player_id) == own_entity, text_contains(&v.message, "Rustwire"));
                            if requested_respawn { return Err("duplicate fixture death".into()); }
                            println!("LIFECYCLE event=death_received");
                            c.send(&ClientCommand::Respawn.encode(version)?)?;
                            requested_respawn = true;
                            println!("ACTION kind=respawn");
                        }
                        _ => {}
                    },
                    Some(DecodedPacket::WorldControl(v)) => match v {
                        WorldControlPacket::FacePlayer(v) => println!("VALUE category=face anchor={:?} x={} y={} z={} entity={}", v.source_anchor, v.position[0], v.position[1], v.position[2], v.entity.is_some()),
                        WorldControlPacket::Ticking(v) => println!("VALUE category=ticking rate={} frozen={}", v.tick_rate, v.frozen),
                        WorldControlPacket::Step(v) => println!("VALUE category=step ticks={}", v.ticks),
                        WorldControlPacket::Rotation(v) => println!("VALUE category=rotation yaw={} pitch={} relative_yaw={} relative_pitch={}", v.yaw, v.pitch, v.relative.is_some_and(|r| r.yaw), v.relative.is_some_and(|r| r.pitch)),
                        _ => {}
                    },
                    Some(DecodedPacket::World(WorldPacket::Respawn(_))) => {
                        if !requested_respawn || received_respawn { return Err("unexpected respawn packet".into()); }
                        received_respawn = true;
                        loaded = false;
                        println!("LIFECYCLE event=respawn_received");
                    }
                    Some(DecodedPacket::Chat(ChatPacket::System(v))) if text_contains(&v.content, "RUSTWIRE_HUD_DONE") => {
                        if !acknowledged_respawn { return Err("finish before respawn readiness".into()); }
                        println!("PROBE_DONE protocol={} roundtrips={roundtrips}", version.protocol());
                        return Ok(());
                    }
                    _ => {}
                }
            }
            Event::Disconnected(reason) => return Err(format!("disconnected: {reason:?}").into()),
            _ => {}
        }
    }
}
