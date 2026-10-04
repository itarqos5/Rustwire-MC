//! Serverbound editing and NBT-query envelopes, without permission or game-state policy.
//!
//! Creative updates from 770 carry length-framed component bytes, explicitly
//! separate from ordinary unframed slots. Those bytes are retained, not interpreted.
use super::inventory::{self, Budget, Slot};
use crate::{
    codec::{BlockPosition, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

trait Body: Sized {
    fn read_body(r: &mut Reader<'_>, version: Version, budget: &mut Budget) -> Result<Self>;
    fn write_body(&self, w: &mut Writer, version: Version, budget: &mut Budget) -> Result<()>;
}
macro_rules! packets {
    ($($variant:ident($ty:ty) => $name:literal),* $(,)?) => {
        $(impl $ty {
            /// Reads one body transactionally, leaving the reader unchanged on error.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                version.packet_id(State::Play, Direction::Serverbound, $name)?;
                let limits = r.limits;
                let input = &r.remaining()[..r.remaining().len().min(limits.max_packet)];
                let mut bounded = Reader::new(input, limits);
                let value = Self::read_body(&mut bounded, version, &mut Budget::new(limits))?;
                r.take(bounded.position())?;
                Ok(value)
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet { return Err(Error::Limit("editing packet bytes")); }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                version.packet_id(State::Play, Direction::Serverbound, $name)?;
                let mut w = Writer::new();
                let mut b = Budget::new(limits);
                self.write_body(&mut w, version, &mut b)?;
                b.check_bytes(&w)?;
                Ok(w.into_inner())
            }
            /// Appends a complete body only; the destination is unchanged on error.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?); Ok(())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(version.packet_id(State::Play, Direction::Serverbound, $name)?,
                    self.encode(version, limits)?))
            }
        })*
        #[derive(Debug, Clone, PartialEq)]
        pub enum EditingPacket { $($variant($ty)),* }
        impl EditingPacket {
            pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                Ok(match name { $($name => Self::$variant(<$ty>::decode(bytes, version, limits)?),)*
                    _ => return Err(Error::Unsupported("editing packet")), })
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                match self { $(Self::$variant(value) => value.encode(version, limits)),* }
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                match self { $(Self::$variant(value) => value.packet(version, limits)),* }
            }
        }
    };
}
packets! {
    Sign(UpdateSign) => "update_sign",
    Book(EditBook) => "edit_book",
    Rename(NameItem) => "name_item",
    Beacon(SetBeaconEffect) => "set_beacon_effect",
    BlockQuery(QueryBlockNbt) => "query_block_nbt",
    EntityQuery(QueryEntityNbt) => "query_entity_nbt",
    Creative(SetCreativeSlot) => "set_creative_slot",
}
fn var_len(n: i32) -> usize {
    let mut n = n as u32;
    let mut len = 1;
    while n > 127 {
        len += 1;
        n >>= 7;
    }
    len
}
fn preflight(w: &Writer, n: usize, limits: Limits) -> Result<()> {
    if w.as_slice().len() > limits.max_packet
        || n > limits.max_packet.saturating_sub(w.as_slice().len())
    {
        return Err(Error::Limit("editing packet bytes"));
    }
    Ok(())
}
fn string(w: &mut Writer, text: &str, maximum: usize, b: &Budget) -> Result<()> {
    let maximum = maximum.min(b.limits.max_string_chars);
    if text.encode_utf16().count() > maximum || text.len() > maximum.saturating_mul(3) {
        return Err(Error::Limit("editing string length"));
    }
    if text.len() > i32::MAX as usize {
        return Err(Error::Limit("editing string bytes"));
    }
    let n = text
        .len()
        .checked_add(var_len(text.len() as i32))
        .ok_or(Error::Limit("editing string bytes"))?;
    preflight(w, n, b.limits)?;
    w.string(text, maximum)
}
fn nonnegative(n: i32) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid("negative creative registry ID"))
    } else {
        Ok(n)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateSign {
    pub position: BlockPosition,
    pub front: bool,
    /// Four plain strings, each limited to 384 UTF-16 units on the wire.
    pub lines: [String; 4],
}
impl Body for UpdateSign {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            front: r.bool()?,
            lines: [
                r.string(384)?.to_owned(),
                r.string(384)?.to_owned(),
                r.string(384)?.to_owned(),
                r.string(384)?.to_owned(),
            ],
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, b: &mut Budget) -> Result<()> {
        w.i64(self.position.pack()?);
        w.bool(self.front);
        for line in &self.lines {
            string(w, line, 384, b)?;
        }
        Ok(())
    }
}
/// Edit a writable book or sign it with a title. `slot` is an inventory slot,
/// despite the historical schema field name `hand`; it is not a Hand enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditBook {
    pub slot: i32,
    pub pages: Vec<String>,
    pub title: Option<String>,
}
impl EditBook {
    fn caps(v: Version) -> (usize, usize, usize) {
        if v.protocol() < 768 {
            (200, 8192, 128)
        } else {
            (100, 1024, 32)
        }
    }
}
impl Body for EditBook {
    fn read_body(r: &mut Reader<'_>, v: Version, b: &mut Budget) -> Result<Self> {
        let slot = r.var_i32()?;
        let (pages_cap, page_cap, title_cap) = Self::caps(v);
        let n = b.count(r)?;
        if n > pages_cap {
            return Err(Error::Limit("book page count"));
        }
        if n > r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut pages = Vec::with_capacity(n);
        for _ in 0..n {
            pages.push(r.string(page_cap)?.to_owned());
        }
        let title = if r.bool()? {
            Some(r.string(title_cap)?.to_owned())
        } else {
            None
        };
        Ok(Self { slot, pages, title })
    }
    fn write_body(&self, w: &mut Writer, v: Version, b: &mut Budget) -> Result<()> {
        let (pages_cap, page_cap, title_cap) = Self::caps(v);
        if self.pages.len() > pages_cap {
            return Err(Error::Limit("book page count"));
        }
        w.var_i32(self.slot);
        b.write_count(self.pages.len(), w)?;
        for page in &self.pages {
            string(w, page, page_cap, b)?;
        }
        w.bool(self.title.is_some());
        if let Some(title) = &self.title {
            string(w, title, title_cap, b)?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameItem {
    pub name: String,
}
impl Body for NameItem {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            name: r.string(32767)?.to_owned(),
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, b: &mut Budget) -> Result<()> {
        string(w, &self.name, 32767, b)
    }
}
/// Explicit presence is retained even for a present negative VarInt. No effect
/// registry resolution or server permission validation is performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetBeaconEffect {
    pub primary: Option<i32>,
    pub secondary: Option<i32>,
}
impl Body for SetBeaconEffect {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        let primary = if r.bool()? { Some(r.var_i32()?) } else { None };
        let secondary = if r.bool()? { Some(r.var_i32()?) } else { None };
        Ok(Self { primary, secondary })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        for value in [self.primary, self.secondary] {
            w.bool(value.is_some());
            if let Some(value) = value {
                w.var_i32(value);
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryBlockNbt {
    pub transaction_id: i32,
    pub position: BlockPosition,
}
impl Body for QueryBlockNbt {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            transaction_id: r.var_i32()?,
            position: BlockPosition::unpack(r.i64()?),
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.var_i32(self.transaction_id);
        w.i64(self.position.pack()?);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryEntityNbt {
    pub transaction_id: i32,
    pub entity_id: i32,
}
impl Body for QueryEntityNbt {
    fn read_body(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            transaction_id: r.var_i32()?,
            entity_id: r.var_i32()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.var_i32(self.transaction_id);
        w.var_i32(self.entity_id);
        Ok(())
    }
}
/// Length-framed component data from protocol 770. The bytes are opaque,
/// including for known IDs; callers must separately validate their contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FramedComponent {
    pub type_id: i32,
    pub data: Vec<u8>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UntrustedItemStack {
    pub item_id: i32,
    pub count: i32,
    pub added: Vec<FramedComponent>,
    pub removed: Vec<i32>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UntrustedSlot {
    Empty,
    Item(UntrustedItemStack),
}
#[derive(Debug, Clone, PartialEq)]
pub enum CreativeItem {
    /// Ordinary classic/component slot through 769, using shared semantic codecs.
    Unframed(Slot),
    /// From 770, component data has per-component byte lengths.
    Framed(UntrustedSlot),
}
#[derive(Debug, Clone, PartialEq)]
pub struct SetCreativeSlot {
    /// Signed wire short, including -1. Does not grant creative inventory access.
    pub slot: i16,
    pub item: CreativeItem,
}
fn untrusted_read(r: &mut Reader<'_>, b: &mut Budget) -> Result<UntrustedSlot> {
    b.charge(1)?;
    let count = r.var_i32()?;
    if count == 0 {
        return Ok(UntrustedSlot::Empty);
    }
    if count < 0 {
        return Err(Error::Invalid("creative item count"));
    }
    let item_id = nonnegative(r.var_i32()?)?;
    let added_count = b.count(r)?;
    let removed_count = b.count(r)?;
    if added_count
        .checked_mul(2)
        .and_then(|n| n.checked_add(removed_count))
        .ok_or(Error::Limit("creative component count"))?
        > r.remaining().len()
    {
        return Err(Error::Eof);
    }
    let mut added = Vec::with_capacity(added_count);
    for _ in 0..added_count {
        let type_id = nonnegative(r.var_i32()?)?;
        let data = r.bytes(b.limits.max_packet)?.to_vec();
        added.push(FramedComponent { type_id, data });
    }
    let mut removed = Vec::with_capacity(removed_count);
    for _ in 0..removed_count {
        removed.push(nonnegative(r.var_i32()?)?);
    }
    Ok(UntrustedSlot::Item(UntrustedItemStack {
        item_id,
        count,
        added,
        removed,
    }))
}
fn untrusted_write(value: &UntrustedSlot, w: &mut Writer, b: &mut Budget) -> Result<()> {
    b.charge(1)?;
    let item = match value {
        UntrustedSlot::Empty => {
            w.u8(0);
            return b.check_bytes(w);
        }
        UntrustedSlot::Item(item) => item,
    };
    if item.count <= 0 {
        return Err(Error::Invalid("creative item count"));
    }
    w.var_i32(item.count);
    w.var_i32(nonnegative(item.item_id)?);
    b.write_count(item.added.len(), w)?;
    b.write_count(item.removed.len(), w)?;
    for component in &item.added {
        w.var_i32(nonnegative(component.type_id)?);
        if component.data.len() > i32::MAX as usize {
            return Err(Error::Limit("creative component bytes"));
        }
        let n = component
            .data
            .len()
            .checked_add(var_len(component.data.len() as i32))
            .ok_or(Error::Limit("creative component bytes"))?;
        preflight(w, n, b.limits)?;
        w.var_i32(component.data.len() as i32);
        w.raw(&component.data);
    }
    for &id in &item.removed {
        w.var_i32(nonnegative(id)?);
        b.check_bytes(w)?;
    }
    Ok(())
}
impl Body for SetCreativeSlot {
    fn read_body(r: &mut Reader<'_>, v: Version, b: &mut Budget) -> Result<Self> {
        let slot = r.i16()?;
        let item = if v.protocol() < 770 {
            CreativeItem::Unframed(inventory::read_slot(r, v, b, 0)?)
        } else {
            CreativeItem::Framed(untrusted_read(r, b)?)
        };
        Ok(Self { slot, item })
    }
    fn write_body(&self, w: &mut Writer, v: Version, b: &mut Budget) -> Result<()> {
        w.i16(self.slot);
        match (&self.item, v.protocol()) {
            (CreativeItem::Unframed(slot), 763..=769) => inventory::write_slot(slot, w, v, b, 0),
            (CreativeItem::Framed(slot), 770..=776) => untrusted_write(slot, w, b),
            _ => Err(Error::Invalid("creative item framing version")),
        }
    }
}
