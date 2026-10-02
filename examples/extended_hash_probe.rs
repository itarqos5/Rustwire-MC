//! Normal survival-inventory prediction checks for disposable loopback Paper worlds.
//! Run only with tools/paper/validate_extended_hashes.py. No creative actions or OP.
use rustwire_mc::{
    connection::{Connection, Event, TypedEvent},
    nbt::Tag,
    packet::{
        self,
        chat::ChatComponent,
        common::{self, CommonPacket},
        inventory::{
            ClickHeader, ClickMode, ComponentValue, HashedContainerClick, HashedItemStack,
            ItemData, Slot,
        },
        item_hash::hash_component,
        typed::{ChatPacket, DecodedPacket, InventoryPacket},
        ClientSettings,
    },
    version::{Direction, State},
    Error, Limits, Result, Version,
};
use std::{collections::BTreeMap, io::Write, time::Duration};

#[derive(Default)]
struct Prediction {
    slots: BTreeMap<i16, Slot>,
    state_id: i32,
    active: Option<Phase>,
    completed: usize,
}
struct Phase {
    id: String,
    bad: bool,
    target: Slot,
    corrections: usize,
    target_reconciled: bool,
}
impl Prediction {
    fn slot(&mut self, index: i16, slot: &Slot) {
        self.slots.insert(index, slot.clone());
        if let Some(phase) = &mut self.active {
            phase.corrections += 1;
            if index == 45 && slot == &phase.target {
                phase.target_reconciled = true;
            }
            println!(
                "HASH_CORRECTION case={} slot={index} target_match={}",
                phase.id,
                index == 45 && slot == &phase.target
            );
        }
    }
    fn observe(&mut self, packet: &InventoryPacket) {
        match packet {
            InventoryPacket::Content(p) if p.window_id == 0 => {
                self.state_id = p.state_id;
                for (index, slot) in p.items.iter().enumerate() {
                    self.slot(index as i16, slot);
                }
            }
            InventoryPacket::Slot(p) if p.window_id == 0 => {
                self.state_id = p.state_id;
                self.slot(p.slot, &p.item);
            }
            InventoryPacket::PlayerSlot(p) => {
                let index = match p.slot {
                    0..=8 => p.slot + 36,
                    9..=35 => p.slot,
                    40 => 45,
                    _ => return,
                };
                self.slot(index as i16, &p.item);
            }
            InventoryPacket::Cursor(_) if self.active.is_some() => {
                self.active.as_mut().unwrap().corrections += 1;
            }
            _ => {}
        }
    }
    fn start(
        &mut self,
        id: &str,
        bad: bool,
        version: Version,
        limits: Limits,
    ) -> Result<rustwire_mc::frame::RawPacket> {
        if self.active.is_some() {
            return Err(Error::Invalid("overlapping hash scenario"));
        }
        let item = self.slots.get(&36).cloned().unwrap_or_default();
        if case_id(&item).as_deref() != Some(id)
            || self.slots.get(&45).is_some_and(|s| *s != Slot::Empty)
        {
            return Err(Error::Invalid("hash scenario source/target precondition"));
        }
        let Slot::Item(stack) = &item else {
            return Err(Error::Invalid("missing test item"));
        };
        let ItemData::Components(patch) = &stack.data else {
            return Err(Error::Invalid("missing component patch"));
        };
        let mut hashed = HashedItemStack::from_slot(&item, version, limits)?;
        for component in &patch.added {
            println!(
                "HASH_VALUE case={id} component={} value={}",
                component.name,
                hash_component(component, version, limits)?
            );
        }
        if bad {
            let component = patch
                .added
                .iter()
                .find(|c| c.name != "custom_data")
                .ok_or(Error::Invalid("missing new-component negative control"))?;
            let entry = hashed
                .as_mut()
                .unwrap()
                .components
                .iter_mut()
                .find(|(key, _)| *key == component.name)
                .ok_or(Error::Invalid("missing predicted component hash"))?;
            let original = entry.1;
            entry.1 ^= 1;
            println!(
                "HASH_MUTATION component={} original={original} submitted={} xor=1",
                component.name, entry.1
            );
        }
        let packet = HashedContainerClick {
            header: ClickHeader {
                window_id: 0,
                state_id: self.state_id,
                slot: 36,
                button: 40,
                mode: ClickMode::Swap,
            },
            changed_slots: vec![(36, None), (45, hashed)],
            carried_item: None,
        }
        .packet(version, limits)?;
        self.slots.insert(36, Slot::Empty);
        self.slots.insert(45, item.clone());
        self.active = Some(Phase {
            id: id.into(),
            bad,
            target: item,
            corrections: 0,
            target_reconciled: false,
        });
        println!(
            "HASH_SENT case={id} bad={bad} source=36 target=45 state_id={}",
            self.state_id
        );
        Ok(packet)
    }
    fn finish(&mut self, id: &str) -> Result<()> {
        let phase = self
            .active
            .take()
            .ok_or(Error::Invalid("hash end without start"))?;
        if phase.id != id {
            return Err(Error::Invalid("hash end case mismatch"));
        }
        let passed = if phase.bad {
            phase.corrections > 0 && phase.target_reconciled
        } else {
            phase.corrections == 0
        };
        println!(
            "HASH_RESULT case={id} bad={} corrections={} target_reconciled={} passed={passed}",
            phase.bad, phase.corrections, phase.target_reconciled
        );
        if !passed {
            return Err(Error::Invalid(
                "inventory hash prediction/correction check failed",
            ));
        }
        self.completed += 1;
        Ok(())
    }
}
fn case_id(slot: &Slot) -> Option<String> {
    let Slot::Item(item) = slot else {
        return None;
    };
    let ItemData::Components(patch) = &item.data else {
        return None;
    };
    patch
        .added
        .iter()
        .find_map(|component| match &component.value {
            ComponentValue::Nbt(nbt) if component.name == "custom_data" => {
                match nbt.root.get("rustwire_hash_case") {
                    Some(Tag::String(value)) => Some(value.to_string_lossy()),
                    _ => None,
                }
            }
            _ => None,
        })
}
fn marker(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(value) => {
            let value = value.to_string_lossy();
            value.starts_with("RW_HASH:").then_some(value)
        }
        Tag::Compound(entries) => entries.iter().find_map(|(_, value)| marker(value)),
        Tag::List { elements, .. } => elements.iter().find_map(marker),
        _ => None,
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 || !matches!(args[1].as_str(), "127.0.0.1" | "::1") {
        return Err("usage: extended_hash_probe 127.0.0.1 PORT VERSION UNIQUE_USERNAME".into());
    }
    let version: Version = args[3].parse()?;
    if version.protocol() < 770 {
        return Err("hashed inventory starts at protocol 770".into());
    }
    let username = &args[4];
    if !username.starts_with("RWH")
        || username.len() > 16
        || !username.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err("expected unique RWH-prefixed test username".into());
    }
    let port: u16 = args[2].parse()?;
    let limits = Limits::default();
    let mut connection = Connection::connect(
        (args[1].as_str(), port),
        version,
        Duration::from_secs(15),
        limits,
    )?;
    connection.start_login(&args[1], port, username, [0x48; 16])?;
    let mut state = Prediction::default();
    let mut ready = false;
    let mut loaded = false;
    for _ in 0..100_000 {
        match connection.next_typed_event()? {
            TypedEvent::Control(Event::LoginSuccess(_)) => {
                connection.send_settings(&ClientSettings::default())?
            }
            TypedEvent::Control(Event::KnownPacks(_)) => connection.select_known_packs(&[])?,
            TypedEvent::Control(Event::CookieRequest(key)) => {
                connection.answer_cookie(&key, None)?
            }
            TypedEvent::Control(Event::LoginPluginRequest { id, .. }) => {
                connection.answer_login_plugin(id, None)?
            }
            TypedEvent::Control(Event::Position(position)) => {
                connection.send(&position.acknowledgement(version)?)?;
                if !loaded
                    && version
                        .packet_id(State::Play, Direction::Serverbound, "player_loaded")
                        .is_ok()
                {
                    connection.send(&packet::named(
                        version,
                        State::Play,
                        "player_loaded",
                        vec![],
                    )?)?;
                    loaded = true;
                }
                if !ready {
                    ready = true;
                    println!("HASH_READY username={username}");
                }
            }
            TypedEvent::Control(Event::EncryptionRequested(request)) => {
                if request.should_authenticate {
                    return Err("offline test only".into());
                }
                #[cfg(feature = "crypto")]
                {
                    let response = rustwire_mc::crypto::encryption_response(
                        &request.public_key,
                        &request.verify_token,
                    )?;
                    connection.complete_encryption(&response)?;
                }
                #[cfg(not(feature = "crypto"))]
                return Err("enable crypto for requested offline encryption".into());
            }
            TypedEvent::Decoded(DecodedPacket::Common(CommonPacket::ChunkBatchFinished(_))) => {
                connection.send(&common::chunk_batch_received(version, 20.)?)?
            }
            TypedEvent::Decoded(DecodedPacket::Common(
                CommonPacket::ResourcePack(_)
                | CommonPacket::CodeOfConduct(_)
                | CommonPacket::Transfer(_),
            )) => return Err("unexpected application-consent challenge".into()),
            TypedEvent::Decoded(DecodedPacket::Inventory(packet)) => state.observe(&packet),
            TypedEvent::Decoded(DecodedPacket::Chat(ChatPacket::System(chat))) => {
                let value = match chat.content {
                    ChatComponent::Nbt(nbt) => marker(&nbt.root),
                    ChatComponent::Json(_) => None,
                };
                if let Some(value) = value {
                    let parts: Vec<_> = value.split(':').collect();
                    match parts.as_slice() {
                        ["RW_HASH", "START", id, control] if matches!(*control, "good" | "bad") => {
                            connection.send(&state.start(
                                id,
                                *control == "bad",
                                version,
                                limits,
                            )?)?
                        }
                        ["RW_HASH", "END", id] => state.finish(id)?,
                        ["RW_HASH", "DONE"] if state.active.is_none() && state.completed > 0 => {
                            println!("HASH_DONE cases={}", state.completed);
                            return Ok(());
                        }
                        _ => return Err("invalid hash scenario marker".into()),
                    }
                }
            }
            TypedEvent::Raw {
                name,
                unsupported: Some(reason),
                ..
            } => return Err(format!("unsupported packet {name:?}: {reason}").into()),
            TypedEvent::Raw {
                name: Some("window_items" | "set_slot" | "set_player_inventory" | "set_cursor_item"),
                ..
            } => return Err("inventory scenario packet unexpectedly raw".into()),
            TypedEvent::Disconnected(reason) => {
                return Err(format!("server disconnected: {reason:?}").into())
            }
            _ => {}
        }
        std::io::stdout().flush()?;
    }
    Err("bounded hash probe packet budget exhausted".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwire_mc::{
        nbt::Nbt,
        packet::inventory::{Component, ComponentPatch, ItemStack},
    };
    fn fixture() -> Slot {
        Slot::Item(ItemStack {
            item_id: 1,
            count: 1,
            data: ItemData::Components(ComponentPatch {
                added: vec![
                    Component {
                        name: "custom_data",
                        value: ComponentValue::Nbt(Nbt::anonymous(Tag::Compound(vec![(
                            "rustwire_hash_case".into(),
                            Tag::String("fixture".into()),
                        )]))),
                    },
                    Component {
                        name: "custom_name",
                        value: ComponentValue::Nbt(Nbt::anonymous(Tag::String("Fixture".into()))),
                    },
                ],
                removed: vec![],
            }),
        })
    }
    fn begun(bad: bool) -> Prediction {
        let mut state = Prediction::default();
        state.slots.insert(36, fixture());
        state
            .start(
                "fixture",
                bad,
                Version::from_protocol(770).unwrap(),
                Limits::default(),
            )
            .unwrap();
        state
    }
    #[test]
    fn good_prediction_requires_no_correction() {
        begun(false).finish("fixture").unwrap();
        let mut state = begun(false);
        state.slot(45, &fixture());
        assert!(state.finish("fixture").is_err());
    }
    #[test]
    fn wrong_hash_requires_matching_target_reconciliation() {
        assert!(begun(true).finish("fixture").is_err());
        let mut unrelated = begun(true);
        unrelated.slot(36, &Slot::Empty);
        assert!(unrelated.finish("fixture").is_err());
        let mut wrong_item = begun(true);
        wrong_item.slot(45, &Slot::Empty);
        assert!(wrong_item.finish("fixture").is_err());
        let mut corrected = begun(true);
        corrected.slot(45, &fixture());
        corrected.finish("fixture").unwrap();
    }
    #[test]
    fn requires_matching_fixture_and_empty_target() {
        let version = Version::from_protocol(770).unwrap();
        let mut state = Prediction::default();
        assert!(state
            .start("fixture", false, version, Limits::default())
            .is_err());
        state.slots.insert(36, fixture());
        assert!(state
            .start("wrong_case", false, version, Limits::default())
            .is_err());
        state.slots.insert(45, fixture());
        assert!(state
            .start("fixture", false, version, Limits::default())
            .is_err());
    }
    #[test]
    fn marker_and_phase_are_explicit() {
        assert!(marker(&Tag::String("something RW_HASH:DONE".into())).is_none());
        assert_eq!(
            marker(&Tag::String("RW_HASH:DONE".into())),
            Some("RW_HASH:DONE".into())
        );
        assert!(Prediction::default().finish("fixture").is_err());
        assert!(begun(false).finish("other").is_err());
    }
}
