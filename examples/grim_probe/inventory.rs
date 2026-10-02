//! One vanilla player-inventory SWAP prediction, derived from observed slots.
use rustwire_mc::{
    frame::RawPacket,
    packet::{
        inventory::{
            ClickHeader, ClickMode, ContainerClick, HashedContainerClick, HashedItemStack, Slot,
        },
        typed::InventoryPacket,
    },
    Error, Limits, Result, Version,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Inventory {
    slots: BTreeMap<i16, Slot>,
    state_id: i32,
    carried: Slot,
}
impl Inventory {
    pub fn observe(&mut self, packet: &InventoryPacket) -> Result<()> {
        match packet {
            InventoryPacket::Content(p) if p.window_id == 0 => {
                if p.items.len() > 46 {
                    return Err(Error::Unsupported("nonstandard player inventory"));
                }
                self.state_id = p.state_id;
                self.slots = p
                    .items
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (i as i16, s.clone()))
                    .collect();
                self.carried = p.carried_item.clone();
            }
            InventoryPacket::Slot(p) if p.window_id == 0 => {
                if !(0..46).contains(&p.slot) {
                    return Err(Error::Invalid("player inventory slot"));
                }
                self.state_id = p.state_id;
                self.slots.insert(p.slot, p.item.clone());
            }
            InventoryPacket::Slot(p) if p.window_id == -1 => self.carried = p.item.clone(),
            InventoryPacket::Slot(p) if p.window_id == -2 => {
                self.player_slot(i32::from(p.slot), &p.item)
            }
            InventoryPacket::PlayerSlot(p) => self.player_slot(p.slot, &p.item),
            InventoryPacket::Cursor(p) => self.carried = p.item.clone(),
            InventoryPacket::Open(_) => {
                return Err(Error::Unsupported("unexpected server-opened inventory"))
            }
            _ => {}
        }
        Ok(())
    }
    fn player_slot(&mut self, slot: i32, item: &Slot) {
        let window_slot = match slot {
            0..=8 => slot + 36,
            9..=35 => slot,
            36..=39 => 44 - slot,
            40 => 45,
            _ => return,
        };
        self.slots.insert(window_slot as i16, item.clone());
    }
    pub fn offhand_count(&self) -> i32 {
        match self.slots.get(&45) {
            Some(Slot::Item(item)) => item.count,
            _ => 0,
        }
    }
    pub fn swap_to_offhand(&mut self, version: Version, limits: Limits) -> Result<RawPacket> {
        let source = self
            .slots
            .get(&36)
            .ok_or(Error::State("hotbar snapshot missing"))?
            .clone();
        let target = self
            .slots
            .get(&45)
            .ok_or(Error::State("offhand snapshot missing"))?
            .clone();
        if !matches!(&source, Slot::Item(item) if item.count == 8)
            || target != Slot::Empty
            || self.carried != Slot::Empty
        {
            return Err(Error::State(
                "expected eight seeded wind charges, empty offhand and cursor",
            ));
        }
        let header = ClickHeader {
            window_id: 0,
            state_id: self.state_id,
            slot: 36,
            button: 40,
            mode: ClickMode::Swap,
        };
        let packet = if version.protocol() >= 770 {
            HashedContainerClick {
                header,
                changed_slots: vec![
                    (36, HashedItemStack::from_slot(&target, version, limits)?),
                    (45, HashedItemStack::from_slot(&source, version, limits)?),
                ],
                carried_item: HashedItemStack::from_slot(&self.carried, version, limits)?,
            }
            .packet(version, limits)?
        } else {
            ContainerClick {
                header,
                changed_slots: vec![(36, target.clone()), (45, source.clone())],
                carried_item: self.carried.clone(),
            }
            .packet(version, limits)?
        };
        self.slots.insert(36, target);
        self.slots.insert(45, source);
        println!(
            "GRIM_INVENTORY_PREDICTION state_id={} source=36 target=45 button=40",
            self.state_id
        );
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustwire_mc::packet::inventory::{
        ComponentPatch, ContainerContent, ItemData, ItemStack, SetContainerSlot,
    };
    fn seeded(version: Version) -> Inventory {
        let mut items = vec![Slot::Empty; 46];
        items[36] = Slot::Item(ItemStack {
            item_id: 1,
            count: 8,
            data: if version.protocol() < 766 {
                ItemData::Legacy(None)
            } else {
                ItemData::Components(ComponentPatch::default())
            },
        });
        let mut inventory = Inventory::default();
        inventory
            .observe(&InventoryPacket::Content(ContainerContent {
                window_id: 0,
                state_id: 7,
                items,
                carried_item: Slot::Empty,
            }))
            .unwrap();
        inventory
    }
    #[test]
    fn observed_state_and_full_or_hashed_predictions_all_versions() {
        for &version in Version::ALL {
            let mut inventory = seeded(version);
            inventory
                .observe(&InventoryPacket::Slot(SetContainerSlot {
                    window_id: 0,
                    state_id: 19,
                    slot: 1,
                    item: Slot::Empty,
                }))
                .unwrap();
            let packet = inventory
                .swap_to_offhand(version, Limits::default())
                .unwrap();
            if version.protocol() < 770 {
                let click =
                    ContainerClick::decode(&packet.data, version, Limits::default()).unwrap();
                assert_eq!(click.header.state_id, 19);
                assert_eq!(click.changed_slots[0], (36, Slot::Empty));
                assert!(matches!(&click.changed_slots[1],(45,Slot::Item(item)) if item.count==8));
            } else {
                let click =
                    HashedContainerClick::decode(&packet.data, version, Limits::default()).unwrap();
                assert_eq!(click.header.state_id, 19);
                assert_eq!(click.changed_slots[0], (36, None));
                assert_eq!(click.changed_slots[1].1.as_ref().unwrap().count, 8);
            }
            assert_eq!(inventory.offhand_count(), 8);
            assert!(inventory
                .swap_to_offhand(version, Limits::default())
                .is_err());
        }
    }
    #[test]
    fn missing_snapshot_is_not_invented() {
        assert!(Inventory::default()
            .swap_to_offhand(Version::V1_21, Limits::default())
            .is_err());
    }
}
