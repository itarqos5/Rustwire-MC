//! Bounded Paper+Grim compatibility exercise for authorized loopback fixtures.
//! Usage: grim_probe 127.0.0.1 PORT VERSION
//! This is a small flat-stone client simulation, not a general game client.
//! It does not disable checks, grant exemptions, or attempt to evade detections.
//! The only intentionally invalid action is the clearly separated final negative
//! control: three duplicate START_SPRINTING packets, expected to flag BadPacketsF.
#[path = "grim_probe/inventory.rs"]
mod inventory;
#[path = "grim_probe/simulation.rs"]
mod simulation;
#[path = "grim_probe/transport.rs"]
mod transport;

use inventory::Inventory;
use rustwire_mc::{
    codec::Reader,
    connection::{Event, TypedEvent},
    packet::{
        self,
        chat::ChatComponent,
        common::CommonPacket,
        entity::EntityPacket,
        interact::{self, EntityAction, Hand, InputKeys, PlayerInput, UseItem},
        inventory::CloseContainer,
        movement::PlayerMovement,
        typed::{ChatPacket, DecodedPacket},
    },
    version::{Direction, State},
    Error, Limits, Result, Version,
};
use simulation::FlatWalk;
use std::{
    io::Write,
    net::{IpAddr, SocketAddr},
    sync::mpsc::RecvTimeoutError,
    time::{Duration, Instant},
};
use transport::Transport;

const TICK: Duration = Duration::from_millis(50);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Waiting,
    Walk,
    Inventory,
    Wind,
    Ended,
    Negative,
}
struct Probe {
    version: Version,
    limits: Limits,
    walk: FlatWalk,
    inventory: Inventory,
    phase: Phase,
    phase_tick: u32,
    entity_id: Option<i32>,
    ready: bool,
    teleports: u32,
    fixture_settle_ticks: u32,
    loaded: bool,
    last_position: [f64; 3],
    last_rotation: [f32; 2],
    last_ground: bool,
    position_reminder: u32,
    keys: InputKeys,
    inventory_sent: bool,
    wind_sent: u32,
    negative_sent: bool,
    ticks: u32,
    movement_packets: u32,
}
impl Probe {
    fn new(version: Version, limits: Limits) -> Self {
        Self {
            version,
            limits,
            walk: FlatWalk::default(),
            inventory: Inventory::default(),
            phase: Phase::Waiting,
            phase_tick: 0,
            entity_id: None,
            ready: false,
            teleports: 0,
            fixture_settle_ticks: 0,
            loaded: false,
            last_position: [0.; 3],
            last_rotation: [0.; 2],
            last_ground: false,
            position_reminder: 0,
            keys: InputKeys::default(),
            inventory_sent: false,
            wind_sent: 0,
            negative_sent: false,
            ticks: 0,
            movement_packets: 0,
        }
    }
    fn marker(&mut self, marker: &str) -> Result<bool> {
        let next = match marker {
            "GRIM_VALID_BEGIN" if self.phase == Phase::Waiting => {
                self.walk.verify_fixture()?;
                if self.fixture_settle_ticks < 2 || !self.last_ground {
                    return Err(Error::State(
                        "fixture requires stationary floor-contact ticks after teleport",
                    ));
                }
                Phase::Walk
            }
            "GRIM_INVENTORY" if self.phase == Phase::Walk && self.phase_tick >= 40 => {
                Phase::Inventory
            }
            "GRIM_WIND" if self.phase == Phase::Inventory && self.inventory_sent => Phase::Wind,
            "GRIM_VALID_END" if self.phase == Phase::Wind && self.wind_sent == 2 => Phase::Ended,
            "GRIM_NEGATIVE" if self.phase == Phase::Ended => Phase::Negative,
            "GRIM_DONE" if self.phase == Phase::Negative && self.negative_sent => return Ok(true),
            _ => return Err(Error::State("unexpected or premature Grim phase marker")),
        };
        self.phase = next;
        self.phase_tick = 0;
        println!("GRIM_PHASE marker={marker}");
        Ok(false)
    }
    fn event(&mut self, event: TypedEvent, transport: &mut Transport) -> Result<bool> {
        match event {
            TypedEvent::Control(Event::Compression(threshold)) => {
                transport.compression(threshold)?
            }
            TypedEvent::Control(Event::Joined(world)) => {
                if world.spawn.game_mode != 0 {
                    return Err(Error::State("Grim fixture requires ordinary survival mode"));
                }
                self.entity_id = Some(world.entity_id);
            }
            TypedEvent::Control(Event::Position(position)) => {
                if self.phase != Phase::Waiting || self.teleports >= 2 || position.teleport_id < 0 {
                    return Err(Error::Unsupported(
                        "unexpected teleport/correction during the active flat-stone test",
                    ));
                }
                self.walk.teleport(&position)?;
                if self.teleports == 1 {
                    self.walk.verify_fixture()?;
                }
                self.teleports += 1;
                self.fixture_settle_ticks = 0;
                transport.send(&position.acknowledgement(self.version)?)?;
                // Vanilla sends its resolved position/rotation directly after a
                // teleport acknowledgement, with on-ground false.
                transport.send(
                    &PlayerMovement {
                        position: Some(self.walk.position),
                        rotation: Some(self.walk.rotation),
                        on_ground: false,
                        horizontal_collision: false,
                    }
                    .packet(self.version, self.limits)?,
                )?;
                self.last_position = self.walk.position;
                self.last_rotation = self.walk.rotation;
                self.last_ground = false;
                self.position_reminder = 0;
                if !self.loaded
                    && self
                        .version
                        .packet_id(State::Play, Direction::Serverbound, "player_loaded")
                        .is_ok()
                {
                    transport.send(&packet::named(
                        self.version,
                        State::Play,
                        "player_loaded",
                        vec![],
                    )?)?;
                    self.loaded = true;
                }
                println!(
                    "GRIM_TELEPORT id={} position={:?} rotation={:?}",
                    position.teleport_id, self.walk.position, self.walk.rotation
                );
                if !self.ready {
                    self.ready = true;
                    println!("GRIM_PROBE_READY");
                    std::io::stdout().flush()?;
                }
                transport.complete_position()?;
            }
            TypedEvent::Decoded(DecodedPacket::Inventory(packet)) => {
                self.inventory.observe(&packet)?
            }
            TypedEvent::Decoded(DecodedPacket::Chat(ChatPacket::System(chat))) => {
                for marker in [
                    "GRIM_VALID_BEGIN",
                    "GRIM_INVENTORY",
                    "GRIM_WIND",
                    "GRIM_VALID_END",
                    "GRIM_NEGATIVE",
                    "GRIM_DONE",
                ] {
                    if contains(&chat.content, marker) {
                        return self.marker(marker);
                    }
                }
            }
            TypedEvent::Decoded(DecodedPacket::Entity(EntityPacket::Velocity(velocity)))
                if Some(velocity.entity_id) == self.entity_id =>
            {
                if self.phase != Phase::Waiting
                    && velocity
                        .velocity
                        .blocks_per_tick()
                        .iter()
                        .any(|v| v.abs() > 1e-10)
                {
                    return Err(Error::Unsupported(
                        "player knockback is outside the flat-stone simulation",
                    ));
                }
            }
            TypedEvent::Raw {
                name: Some("explosion"),
                packet,
                ..
            } => reject_explosion_knockback(&packet.data, self.version, self.limits)?,
            TypedEvent::Raw {
                name: Some("window_items" | "set_slot" | "set_player_inventory" | "set_cursor_item"),
                unsupported: Some(_),
                ..
            } => {
                return Err(Error::Unsupported(
                    "inventory snapshot could not be decoded",
                ))
            }
            TypedEvent::Decoded(DecodedPacket::Common(
                CommonPacket::ResourcePack(_)
                | CommonPacket::CodeOfConduct(_)
                | CommonPacket::Transfer(_),
            )) => return Err(Error::Unsupported("unexpected consent or transfer request")),
            TypedEvent::Disconnected(reason) => {
                return Err(Error::Disconnect(format!("{reason:?}")))
            }
            _ => {}
        }
        Ok(false)
    }
    fn tick(&mut self, transport: &Transport) -> Result<()> {
        if self.phase == Phase::Waiting {
            // The second, harness-owned teleport identifies the known stone
            // floor. A stationary gravity/collision tick makes ground contact
            // before applying any ground acceleration. The teleport response
            // itself correctly used on-ground=false; it is not a physics tick.
            if self.teleports >= 2 && self.walk.verify_fixture().is_ok() {
                self.walk.tick(false);
                self.send_movement(transport)?;
                self.fixture_settle_ticks += 1;
            }
            if self.ready && self.version.protocol() >= 768 {
                transport.send(&interact::tick_end(self.version)?)?;
            }
            return Ok(());
        }
        self.ticks += 1;
        let forward = self.phase == Phase::Walk && self.phase_tick < 20;
        let keys = InputKeys {
            forward,
            ..InputKeys::default()
        };
        if keys != self.keys {
            if self.version.protocol() >= 768 {
                transport.send(&PlayerInput::Keys(keys).encode(self.version)?)?;
            }
            self.keys = keys;
        }
        self.walk.tick(forward);
        if self.phase == Phase::Wind && self.phase_tick == 0 {
            self.walk.rotation = [0., -90.];
        }
        // UI/use actions are processed before this tick's movement report.
        // In particular, carried-item changes must not follow a flying packet.
        match self.phase {
            Phase::Walk if self.phase_tick == 40 => {
                println!("GRIM_WALK_DONE position={:?}", self.walk.position)
            }
            Phase::Inventory if self.phase_tick == 0 => {
                // Opening the player inventory is client-local. No open packet
                // exists here; SWAP button 40 selects the offhand slot.
                transport.send(&self.inventory.swap_to_offhand(self.version, self.limits)?)?;
                self.inventory_sent = true;
                println!(
                    "GRIM_ACTION phase=inventory tick={} offhand_prediction={}",
                    self.ticks,
                    self.inventory.offhand_count()
                );
            }
            Phase::Inventory if self.phase_tick == 2 => transport
                .send(&CloseContainer { window_id: 0 }.packet(self.version, self.limits)?)?,
            Phase::Wind if self.phase_tick == 2 || self.phase_tick == 16 => {
                if self.version.protocol() < 766 {
                    return Err(Error::Unsupported("wind charges were introduced in 1.20.5"));
                }
                if self.inventory.offhand_count() < 2 {
                    return Err(Error::State("wind charges missing from offhand snapshot"));
                }
                transport.send(
                    &UseItem {
                        hand: Hand::Off,
                        sequence: self.wind_sent,
                        rotation: (self.version.protocol() >= 767).then_some(self.walk.rotation),
                    }
                    .encode(self.version)?,
                )?;
                transport.send(&interact::swing_arm(self.version, Hand::Off)?)?;
                self.wind_sent += 1;
                println!(
                    "GRIM_ACTION phase=wind tick={} use={} hand=off pitch=-90",
                    self.ticks, self.wind_sent
                );
            }
            Phase::Negative if self.phase_tick == 0 => {
                let id = self
                    .entity_id
                    .ok_or(Error::State("joined player entity ID missing"))?;
                for _ in 0..3 {
                    transport.send(&EntityAction::StartSprinting.encode(self.version, id)?)?;
                }
                self.negative_sent = true;
                println!(
                    "GRIM_ACTION phase=negative tick={} duplicate_start_sprinting=3",
                    self.ticks
                );
            }
            Phase::Negative if self.phase_tick == 1 => {
                let id = self
                    .entity_id
                    .ok_or(Error::State("joined player entity ID missing"))?;
                transport.send(&EntityAction::StopSprinting.encode(self.version, id)?)?;
                println!("GRIM_NEGATIVE_RESET tick={} sprinting=false", self.ticks);
            }
            _ => {}
        }
        self.send_movement(transport)?;
        if self.phase == Phase::Walk && self.phase_tick == 0 {
            println!(
                "GRIM_ACTION phase=walk tick={} position={:?}",
                self.ticks, self.walk.position
            );
        }
        if self.version.protocol() >= 768 {
            transport.send(&interact::tick_end(self.version)?)?;
        }
        self.phase_tick += 1;
        Ok(())
    }
    fn send_movement(&mut self, transport: &Transport) -> Result<()> {
        self.position_reminder += 1;
        let distance_squared = self
            .walk
            .position
            .iter()
            .zip(self.last_position)
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f64>();
        let position_changed =
            distance_squared > 0.0002_f64.powi(2) || self.position_reminder >= 20;
        let rotation_changed = self.walk.rotation != self.last_rotation;
        if position_changed || rotation_changed || self.walk.on_ground != self.last_ground {
            transport.send(
                &PlayerMovement {
                    position: position_changed.then_some(self.walk.position),
                    rotation: rotation_changed.then_some(self.walk.rotation),
                    on_ground: self.walk.on_ground,
                    horizontal_collision: false,
                }
                .packet(self.version, self.limits)?,
            )?;
            self.movement_packets += 1;
            if position_changed {
                self.last_position = self.walk.position;
                self.position_reminder = 0;
            }
            if rotation_changed {
                self.last_rotation = self.walk.rotation;
            }
            self.last_ground = self.walk.on_ground;
        }
        Ok(())
    }
}
fn contains(component: &ChatComponent, needle: &str) -> bool {
    fn in_tag(tag: &rustwire_mc::nbt::Tag, needle: &str) -> bool {
        use rustwire_mc::nbt::Tag;
        match tag {
            Tag::String(s) => s.to_string_lossy().contains(needle),
            Tag::Compound(v) => v.iter().any(|(_, t)| in_tag(t, needle)),
            Tag::List { elements, .. } => elements.iter().any(|t| in_tag(t, needle)),
            _ => false,
        }
    }
    match component {
        ChatComponent::Json(s) => s.contains(needle),
        ChatComponent::Nbt(n) => in_tag(&n.root, needle),
    }
}
fn reject_explosion_knockback(bytes: &[u8], version: Version, limits: Limits) -> Result<()> {
    let mut r = Reader::new(bytes, limits);
    for _ in 0..3 {
        r.f64()?;
    }
    let motion = if version.protocol() < 768 {
        r.f32()?;
        let count = r.count(limits.max_collection)?;
        r.take(
            count
                .checked_mul(3)
                .ok_or(Error::Limit("explosion blocks"))?,
        )?;
        [
            f64::from(r.f32()?),
            f64::from(r.f32()?),
            f64::from(r.f32()?),
        ]
    } else {
        if version.protocol() >= 774 {
            r.f32()?;
            r.i32()?;
        }
        if r.bool()? {
            [r.f64()?, r.f64()?, r.f64()?]
        } else {
            [0.; 3]
        }
    };
    if motion.iter().any(|v| !v.is_finite() || *v != 0.) {
        return Err(Error::Unsupported(
            "explosion knockback is outside the flat-stone simulation",
        ));
    }
    Ok(())
}
fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: grim_probe 127.0.0.1 PORT VERSION".into());
    }
    let ip: IpAddr = args[1].parse()?;
    if !ip.is_loopback() {
        return Err("grim_probe only permits authorized disposable loopback servers".into());
    }
    let version: Version = args[3].parse()?;
    let limits = Limits::default();
    let mut transport = Transport::connect(SocketAddr::new(ip, args[2].parse()?), version, limits)?;
    let mut probe = Probe::new(version, limits);
    let started = Instant::now();
    let mut next_tick = started + TICK;
    loop {
        let now = Instant::now();
        if started.elapsed() > Duration::from_secs(90) {
            return Err("Grim probe deadline exceeded before GRIM_DONE".into());
        }
        if now >= next_tick {
            let lateness = now.duration_since(next_tick);
            if probe.phase != Phase::Waiting && lateness > TICK {
                return Err(format!("GRIM_CLIENT_LIMIT tick scheduler delayed {} ms; no catch-up movement burst attempted",lateness.as_millis()).into());
            }
            probe.tick(&transport)?;
            next_tick = if lateness > TICK {
                now + TICK
            } else {
                next_tick + TICK
            };
            continue;
        }
        match transport.events.recv_timeout(next_tick.duration_since(now)) {
            Ok(Ok(event)) => {
                if probe.event(event, &mut transport)? {
                    println!("GRIM_PROBE_DONE protocol={} ticks={} movement_packets={} wind_uses={} seconds={:.3}",version.protocol(),probe.ticks,probe.movement_packets,probe.wind_sent,started.elapsed().as_secs_f64());
                    transport.stop()?;
                    return Ok(());
                }
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err("Grim reader stopped unexpectedly".into())
            }
        }
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("GRIM_CLIENT_LIMIT {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwire_mc::codec::Writer;
    #[test]
    fn setup_corrections_fail_before_echoing_any_more_packets() {
        for (teleports, id) in [(1, -1), (2, 3)] {
            let (mut transport, server) = Transport::capture().unwrap();
            let mut probe = Probe::new(Version::V26_2, Limits::default());
            probe.teleports = teleports;
            let correction = packet::PositionSync {
                teleport_id: id,
                x: 0.5,
                y: -60.,
                z: 0.5,
                velocity: Some([0.; 3]),
                yaw: 0.,
                pitch: 0.,
                relative_flags: 0,
            };
            assert!(probe
                .event(
                    TypedEvent::Control(Event::Position(correction)),
                    &mut transport
                )
                .is_err());
            server
                .set_read_timeout(Some(Duration::from_millis(50)))
                .unwrap();
            assert!(server.peek(&mut [0; 1]).is_err());
        }
    }
    #[test]
    fn modern_waiting_play_ticks_emit_tick_end_without_inventing_movement() {
        let version = Version::V26_2;
        let limits = Limits::default();
        let (transport, server) = Transport::capture().unwrap();
        let mut capture = rustwire_mc::connection::Connection::new(server, version, limits);
        let mut probe = Probe::new(version, limits);
        probe.ready = true;
        probe.teleports = 1;
        probe.tick(&transport).unwrap();
        assert_eq!(
            capture.receive().unwrap().id,
            version
                .packet_id(State::Play, Direction::Serverbound, "tick_end")
                .unwrap()
        );
        assert_eq!(probe.movement_packets, 0);
    }
    #[test]
    fn inventory_and_use_actions_precede_coincident_position_heartbeat() {
        use rustwire_mc::{
            connection::Connection,
            packet::{
                inventory::{ComponentPatch, ContainerContent, ItemData, ItemStack, Slot},
                typed::InventoryPacket,
            },
        };
        for version in [Version::V1_21, Version::V1_21_5, Version::V26_2] {
            let limits = Limits::default();
            let (transport, server) = Transport::capture().unwrap();
            let mut capture = Connection::new(server, version, limits);
            let mut probe = Probe::new(version, limits);
            probe.walk.position = [0.5, -60., 0.5];
            probe.walk.tick(false);
            probe.walk.tick(false);
            probe.last_position = probe.walk.position;
            probe.last_ground = true;
            probe.phase = Phase::Inventory;
            probe.position_reminder = 19;
            let mut items = vec![Slot::Empty; 46];
            items[36] = Slot::Item(ItemStack {
                item_id: 1,
                count: 8,
                data: ItemData::Components(ComponentPatch::default()),
            });
            probe
                .inventory
                .observe(&InventoryPacket::Content(ContainerContent {
                    window_id: 0,
                    state_id: 3,
                    items,
                    carried_item: Slot::Empty,
                }))
                .unwrap();
            probe.tick(&transport).unwrap();
            let mut expected = vec!["window_click", "position"];
            if version.protocol() >= 768 {
                expected.push("tick_end");
            }
            for name in expected {
                let packet = capture.receive().unwrap();
                assert_eq!(
                    packet.id,
                    version
                        .packet_id(State::Play, Direction::Serverbound, name)
                        .unwrap(),
                    "{version:?}: {name}"
                );
            }
            probe.phase = Phase::Wind;
            probe.phase_tick = 2;
            probe.walk.rotation = [0., -90.];
            probe.last_rotation = probe.walk.rotation;
            probe.position_reminder = 19;
            probe.tick(&transport).unwrap();
            let mut expected = vec!["use_item", "arm_animation", "position"];
            if version.protocol() >= 768 {
                expected.push("tick_end");
            }
            for name in expected {
                assert_eq!(
                    capture.receive().unwrap().id,
                    version
                        .packet_id(State::Play, Direction::Serverbound, name)
                        .unwrap(),
                    "{version:?}: {name}"
                );
            }
        }
    }

    #[test]
    fn negative_control_cannot_run_before_completed_valid_actions() {
        let mut probe = Probe::new(Version::V1_21_5, Limits::default());
        assert!(probe.marker("GRIM_NEGATIVE").is_err());
        probe.walk.position = [0.5, -60., 0.5];
        assert!(probe.marker("GRIM_VALID_BEGIN").is_err());
        probe.fixture_settle_ticks = 2;
        probe.last_ground = true;
        probe.marker("GRIM_VALID_BEGIN").unwrap();
        assert!(probe.marker("GRIM_INVENTORY").is_err());
        probe.phase_tick = 40;
        probe.marker("GRIM_INVENTORY").unwrap();
        assert!(probe.marker("GRIM_WIND").is_err());
        probe.inventory_sent = true;
        probe.marker("GRIM_WIND").unwrap();
        assert!(probe.marker("GRIM_VALID_END").is_err());
        probe.wind_sent = 2;
        probe.marker("GRIM_VALID_END").unwrap();
        probe.marker("GRIM_NEGATIVE").unwrap();
        assert!(probe.marker("GRIM_DONE").is_err());
        probe.negative_sent = true;
        assert!(probe.marker("GRIM_DONE").unwrap());
    }
    #[test]
    fn explosion_checks_versioned_knockback_prefixes() {
        for &version in Version::ALL {
            for nonzero in [false, true] {
                let mut w = Writer::new();
                for _ in 0..3 {
                    w.f64(0.);
                }
                if version.protocol() < 768 {
                    w.f32(1.);
                    w.var_i32(0);
                    w.f32(0.);
                    w.f32(if nonzero { 0.5 } else { 0. });
                    w.f32(0.);
                } else {
                    if version.protocol() >= 774 {
                        w.f32(1.);
                        w.i32(0);
                    }
                    w.bool(nonzero);
                    if nonzero {
                        for _ in 0..3 {
                            w.f64(0.5);
                        }
                    }
                }
                assert_eq!(
                    reject_explosion_knockback(w.as_slice(), version, Limits::default()).is_err(),
                    nonzero
                );
            }
        }
    }
}
