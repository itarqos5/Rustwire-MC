//! Explicit release families. Unknown protocols fail closed.
use crate::{Error, Result};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Version(i32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    Handshake,
    Status,
    Login,
    Configuration,
    Play,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Clientbound,
    Serverbound,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketInfo {
    pub state: State,
    pub direction: Direction,
    pub id: i32,
    pub name: &'static str,
}
#[path = "catalog/mod.rs"]
mod catalog;
impl Version {
    pub const V1_20: Self = Self(763);
    pub const V1_20_2: Self = Self(764);
    pub const V1_20_3: Self = Self(765);
    pub const V1_20_5: Self = Self(766);
    pub const V1_21: Self = Self(767);
    pub const V1_21_2: Self = Self(768);
    pub const V1_21_4: Self = Self(769);
    pub const V1_21_5: Self = Self(770);
    pub const V1_21_6: Self = Self(771);
    pub const V1_21_7: Self = Self(772);
    pub const V1_21_9: Self = Self(773);
    pub const V1_21_11: Self = Self(774);
    pub const V26_1: Self = Self(775);
    pub const V26_2: Self = Self(776);
    pub const ALL: &'static [Self] = &[
        Self(763),
        Self(764),
        Self(765),
        Self(766),
        Self(767),
        Self(768),
        Self(769),
        Self(770),
        Self(771),
        Self(772),
        Self(773),
        Self(774),
        Self(775),
        Self(776),
    ];
    pub fn from_protocol(protocol: i32) -> Result<Self> {
        if (763..=776).contains(&protocol) {
            Ok(Self(protocol))
        } else {
            Err(Error::Unsupported("Minecraft release protocol"))
        }
    }
    pub const fn protocol(self) -> i32 {
        self.0
    }
    pub const fn has_configuration(self) -> bool {
        self.0 >= 764
    }
    pub fn releases(self) -> &'static [&'static str] {
        match self.0 {
            763 => &["1.20", "1.20.1"],
            764 => &["1.20.2"],
            765 => &["1.20.3", "1.20.4"],
            766 => &["1.20.5", "1.20.6"],
            767 => &["1.21", "1.21.1"],
            768 => &["1.21.2", "1.21.3"],
            769 => &["1.21.4"],
            770 => &["1.21.5"],
            771 => &["1.21.6"],
            772 => &["1.21.7", "1.21.8"],
            773 => &["1.21.9", "1.21.10"],
            774 => &["1.21.11"],
            775 => &["26.1", "26.1.1", "26.1.2"],
            776 => &["26.2"],
            _ => unreachable!(),
        }
    }
    fn table(self) -> (&'static [PacketInfo], &'static [(usize, usize); 10]) {
        use catalog::*;
        match self.0 {
            763 => (P763, &P763_RANGES),
            764 => (P764, &P764_RANGES),
            765 => (P765, &P765_RANGES),
            766 => (P766, &P766_RANGES),
            767 => (P767, &P767_RANGES),
            768 => (P768, &P768_RANGES),
            769 => (P769, &P769_RANGES),
            770 => (P770, &P770_RANGES),
            771 => (P771, &P771_RANGES),
            772 => (P772, &P772_RANGES),
            773 => (P773, &P773_RANGES),
            774 => (P774, &P774_RANGES),
            775 => (P775, &P775_RANGES),
            776 => (P776, &P776_RANGES),
            _ => unreachable!(),
        }
    }
    pub fn catalog(self) -> &'static [PacketInfo] {
        self.table().0
    }
    /// Packets for one wire state and direction, sorted by numeric ID.
    pub fn packets(self, state: State, direction: Direction) -> &'static [PacketInfo] {
        let (catalog, ranges) = self.table();
        let (start, end) = ranges[state as usize * 2 + direction as usize];
        &catalog[start..end]
    }
    pub fn packet(
        self,
        state: State,
        direction: Direction,
        id: i32,
    ) -> Option<&'static PacketInfo> {
        if id < 0 {
            return None;
        }
        let packets = self.packets(state, direction);
        // Modern packet IDs are dense. Sparse legacy IDs use a bounded binary fallback.
        if let Some(packet) = packets.get(id as usize) {
            if packet.id == id {
                return Some(packet);
            }
        }
        packets
            .binary_search_by_key(&id, |p| p.id)
            .ok()
            .map(|index| &packets[index])
    }
    pub fn packet_id(self, state: State, direction: Direction, name: &str) -> Result<i32> {
        self.packets(state, direction)
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.id)
            .ok_or(Error::Unsupported(
                "packet in selected version/state/direction",
            ))
    }
}
impl std::str::FromStr for Version {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|v| v.releases().contains(&s))
            .ok_or(Error::Unsupported("Minecraft release name"))
    }
}
impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (protocol {})",
            self.releases().last().unwrap(),
            self.0
        )
    }
}
