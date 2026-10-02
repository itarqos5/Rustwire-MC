//! A bounded positive/negative prediction test for a disposable offline world.
use rustwire_mc::{
    frame::RawPacket,
    packet::{
        inventory::{
            ClickHeader, ClickMode, ComponentValue, HashedContainerClick, HashedItemStack,
            ItemData, Slot,
        },
        typed::InventoryPacket,
    },
    Error, Limits, Result, Version,
};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct HashProbe {
    slots: BTreeMap<i16, Slot>,
    state_id: i32,
    phase: u8,
    corrections: usize,
}
impl HashProbe {
    pub fn observe(&mut self, packet: &InventoryPacket) {
        match packet {
            InventoryPacket::Content(p) if p.window_id == 0 => {
                self.state_id = p.state_id;
                for (i, s) in p.items.iter().enumerate() {
                    self.slots.insert(i as i16, s.clone());
                }
                if self.phase > 0 {
                    self.corrections += 1;
                }
            }
            InventoryPacket::Slot(p) if p.window_id == 0 => {
                self.state_id = p.state_id;
                self.slots.insert(p.slot, p.item.clone());
                if self.phase > 0 {
                    self.corrections += 1;
                }
            }
            InventoryPacket::PlayerSlot(p) => {
                let slot = match p.slot {
                    0..=8 => p.slot + 36,
                    9..=35 => p.slot,
                    40 => 45,
                    _ => return,
                };
                self.slots.insert(slot as i16, p.item.clone());
                if self.phase > 0 {
                    self.corrections += 1;
                }
            }
            InventoryPacket::Cursor(_) if self.phase > 0 => {
                self.corrections += 1;
            }
            _ => {}
        }
    }
    pub fn marker(
        &mut self,
        marker: &str,
        version: Version,
        limits: Limits,
    ) -> Result<Option<(RawPacket, &'static str)>> {
        match marker {
            "RUSTWIRE_HASH_BAD" => {
                let source = *self
                    .slots
                    .iter()
                    .find(|(_, s)| custom_item(s))
                    .ok_or(Error::Invalid("hash probe custom item missing"))?
                    .0;
                self.phase = 1;
                self.corrections = 0;
                Ok(Some((
                    self.swap(source, 45, 40, true, version, limits)?,
                    "hash_bad_sent",
                )))
            }
            "RUSTWIRE_HASH_BAD_END" => {
                if self.phase != 1 || self.corrections == 0 {
                    return Err(Error::Invalid(
                        "negative hash control did not trigger correction",
                    ));
                }
                println!("EVENT category=hash_negative_control");
                println!("HASH_CONTROL bad_corrections={}", self.corrections);
                self.phase = 0;
                Ok(None)
            }
            "RUSTWIRE_HASH_GOOD" => {
                self.phase = 2;
                self.corrections = 0;
                Ok(Some((
                    self.swap(45, 36, 0, false, version, limits)?,
                    "hash_good_sent",
                )))
            }
            "RUSTWIRE_HASH_GOOD_END" => {
                if self.phase != 2 || self.corrections != 0 {
                    return Err(Error::Invalid(
                        "derived hashes triggered inventory correction",
                    ));
                }
                println!("EVENT category=hash_positive_control");
                println!("HASH_CONTROL good_corrections=0");
                self.phase = 0;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
    fn swap(
        &mut self,
        source: i16,
        target: i16,
        button: i8,
        bad: bool,
        version: Version,
        limits: Limits,
    ) -> Result<RawPacket> {
        let before_source = self.slots.get(&source).cloned().unwrap_or_default();
        let before_target = self.slots.get(&target).cloned().unwrap_or_default();
        let expected_source = HashedItemStack::from_slot(&before_target, version, limits)?;
        let mut expected_target = HashedItemStack::from_slot(&before_source, version, limits)?;
        if bad {
            let hash = expected_target
                .as_mut()
                .and_then(|s| s.components.first_mut())
                .ok_or(Error::Invalid("negative control requires component hash"))?;
            hash.1 ^= 1;
        }
        let packet = HashedContainerClick {
            header: ClickHeader {
                window_id: 0,
                state_id: self.state_id,
                slot: source,
                button,
                mode: ClickMode::Swap,
            },
            changed_slots: vec![(source, expected_source), (target, expected_target)],
            carried_item: None,
        }
        .packet(version, limits)?;
        self.slots.insert(source, before_target);
        self.slots.insert(target, before_source);
        println!(
            "HASH_SWAP source={source} target={target} state={} bad={bad}",
            self.state_id
        );
        Ok(packet)
    }
}
fn custom_item(slot: &Slot) -> bool {
    let Slot::Item(item) = slot else { return false };
    let ItemData::Components(patch) = &item.data else {
        return false;
    };
    patch.added.iter().any(|c| {
        c.name == "custom_data"
            && matches!(&c.value,ComponentValue::Nbt(n) if n.root.get("rustwire_probe").is_some())
    })
}
