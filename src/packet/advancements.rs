//! Bounded advancement updates and tab selection for protocols 763–776.
//!
//! Entries retain wire order and duplicates; this is not an advancement state
//! engine. Requirements and progress names are ordinary strings, while resource
//! identifiers use the crate's shared syntax check without normalization or
//! registry lookup. Display text is opaque JSON/NBT. Icon component support is
//! exactly the existing inventory subset; unsupported unframed payloads fail.
//!
//! See `docs/advancements-wire-audit.md` for schema and implementation evidence.
use super::{
    chat::ChatComponent,
    inventory::{self, Budget, ItemStack, Slot},
};
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    nbt::RootFormat,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvancementFrame {
    Task,
    Challenge,
    Goal,
}

/// The icon layout changes at protocol 775, independently of container slots.
#[derive(Clone, Debug, PartialEq)]
pub enum AdvancementIcon {
    /// Protocols 763–774, including the empty-stack sentinel.
    Slot(Slot),
    /// Protocols 775–776: ID before count, with a patch even when count is zero.
    Template(ItemStack),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdvancementDisplay {
    pub title: ChatComponent,
    pub description: ChatComponent,
    pub icon: AdvancementIcon,
    pub frame: AdvancementFrame,
    /// Full flag word. Bit 0 must agree with background_texture presence;
    /// bit 1 is show-toast and bit 2 is hidden. Other bits are retained.
    pub flags: u32,
    pub background_texture: Option<String>,
    /// Raw wire coordinates; no finite-value or layout constraints are imposed.
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Advancement {
    pub id: String,
    pub parent_id: Option<String>,
    pub display: Option<AdvancementDisplay>,
    /// Required Some (possibly empty) only at 763; must be None at 764+.
    pub criteria: Option<Vec<String>>,
    /// Outer groups are ANDed, names within a group ORed by the game. This
    /// codec does not validate names against criteria or prior client state.
    pub requirements: Vec<Vec<String>>,
    pub sends_telemetry_data: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CriterionProgress {
    /// Ordinary criterion name, not a resource-location identifier.
    pub criterion: String,
    /// Signed milliseconds since the Unix epoch. None means not achieved.
    pub achieved_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdvancementProgress {
    pub id: String,
    pub criteria: Vec<CriterionProgress>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Advancements {
    pub reset: bool,
    pub added: Vec<Advancement>,
    pub removed: Vec<String>,
    pub progress: Vec<AdvancementProgress>,
    /// Required Some at 770+ and absent before 770.
    pub show_advancements: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectAdvancementTab {
    pub id: Option<String>,
}

/// Serverbound advancement-screen notification. No separate "seen" action is
/// present: opening/selecting a tab uses action 0, closing the screen action 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdvancementTab {
    Opened(String),
    Closed,
}

trait Body: Sized {
    fn read_body(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self>;
    fn write_body(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()>;
}
macro_rules! codec {
    ($($t:ty => ($direction:ident, $name:literal)),* $(,)?) => {$ (
        impl $t {
            /// Reads one bounded body, leaving the reader unchanged on error.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                let limits = r.limits;
                let input = &r.remaining()[..r.remaining().len().min(limits.max_packet)];
                let mut bounded = Reader::new(input, limits);
                let value = Self::read_body(&mut bounded, version, &mut Budget::new(limits))?;
                r.take(bounded.position())?;
                Ok(value)
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("advancement packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                let mut b = Budget::new(limits);
                self.write_body(&mut w, version, &mut b)?;
                b.check_bytes(&w)?;
                Ok(w.into_inner())
            }
            /// Appends one body; the destination is unchanged on failure.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?);
                Ok(())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::$direction, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    )*};
}
codec! {
    Advancements => (Clientbound, "advancements"),
    SelectAdvancementTab => (Clientbound, "select_advancement_tab"),
    AdvancementTab => (Serverbound, "advancement_tab"),
}

fn read_id(r: &mut Reader<'_>, version: Version) -> Result<String> {
    let value = r.string(32767)?;
    identifier::validate(value, version)?;
    Ok(value.to_owned())
}
fn string(value: &str, w: &mut Writer, maximum: usize, b: &Budget) -> Result<()> {
    let mut prefix = 1usize;
    let mut len = value.len();
    while len > 127 {
        prefix += 1;
        len >>= 7;
    }
    if value.len().saturating_add(prefix) > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
        return Err(Error::Limit("advancement string bytes"));
    }
    w.string(value, maximum.min(b.limits.max_string_chars))?;
    b.check_bytes(w)
}
fn write_id(value: &str, w: &mut Writer, version: Version, b: &Budget) -> Result<()> {
    identifier::validate(value, version)?;
    string(value, w, 32767, b)
}
fn read_text(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<ChatComponent> {
    if version.protocol() < 765 {
        Ok(ChatComponent::Json(r.string(262_144)?.to_owned()))
    } else {
        inventory::read_nbt(r, RootFormat::Anonymous, b)?
            .map(ChatComponent::Nbt)
            .ok_or(Error::Invalid("absent advancement text NBT"))
    }
}
fn write_text(
    value: &ChatComponent,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    match (value, version.protocol() < 765) {
        (ChatComponent::Json(json), true) => string(json, w, 262_144, b),
        (ChatComponent::Nbt(nbt), false) => {
            inventory::write_nbt(Some(nbt), w, RootFormat::Anonymous, b)
        }
        _ => Err(Error::Invalid(
            "advancement text representation for version",
        )),
    }
}
fn count(r: &mut Reader<'_>, b: &mut Budget, minimum_bytes: usize) -> Result<usize> {
    let n = b.count(r)?;
    if n > r.remaining().len() / minimum_bytes {
        return Err(Error::Eof);
    }
    Ok(n)
}
fn read_names(r: &mut Reader<'_>, b: &mut Budget) -> Result<Vec<String>> {
    let n = count(r, b, 1)?;
    let mut names = Vec::with_capacity(n);
    for _ in 0..n {
        names.push(r.string(32767)?.to_owned());
    }
    Ok(names)
}
fn write_names(names: &[String], w: &mut Writer, b: &mut Budget) -> Result<()> {
    b.write_count(names.len(), w)?;
    for name in names {
        string(name, w, 32767, b)?;
    }
    Ok(())
}
impl AdvancementDisplay {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let title = read_text(r, version, b)?;
        let description = read_text(r, version, b)?;
        let icon = if version.protocol() >= 775 {
            AdvancementIcon::Template(inventory::read_template(r, version, b, 0)?)
        } else {
            AdvancementIcon::Slot(inventory::read_slot(r, version, b, 0)?)
        };
        let frame = match r.var_i32()? {
            0 => AdvancementFrame::Task,
            1 => AdvancementFrame::Challenge,
            2 => AdvancementFrame::Goal,
            _ => return Err(Error::Invalid("advancement frame type")),
        };
        let flags = r.i32()? as u32;
        let background_texture = if flags & 1 != 0 {
            Some(read_id(r, version)?)
        } else {
            None
        };
        Ok(Self {
            title,
            description,
            icon,
            frame,
            flags,
            background_texture,
            x: r.f32()?,
            y: r.f32()?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        if (self.flags & 1 != 0) != self.background_texture.is_some() {
            return Err(Error::Invalid("advancement background flag"));
        }
        write_text(&self.title, w, version, b)?;
        write_text(&self.description, w, version, b)?;
        match (&self.icon, version.protocol() >= 775) {
            (AdvancementIcon::Slot(slot), false) => inventory::write_slot(slot, w, version, b, 0)?,
            (AdvancementIcon::Template(item), true) => {
                inventory::write_template(item, w, version, b, 0)?
            }
            _ => {
                return Err(Error::Invalid(
                    "advancement icon representation for version",
                ))
            }
        }
        w.var_i32(self.frame as i32);
        w.i32(self.flags as i32);
        b.check_bytes(w)?;
        if let Some(background) = &self.background_texture {
            write_id(background, w, version, b)?;
        }
        w.f32(self.x);
        w.f32(self.y);
        b.check_bytes(w)
    }
}
impl Body for Advancements {
    fn read_body(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let reset = r.bool()?;
        let n = count(r, b, if version.protocol() == 763 { 6 } else { 5 })?;
        let mut added = Vec::with_capacity(n);
        for _ in 0..n {
            let id = read_id(r, version)?;
            let parent_id = if r.bool()? {
                Some(read_id(r, version)?)
            } else {
                None
            };
            let display = if r.bool()? {
                Some(AdvancementDisplay::read(r, version, b)?)
            } else {
                None
            };
            let criteria = if version.protocol() == 763 {
                Some(read_names(r, b)?)
            } else {
                None
            };
            let n = count(r, b, 1)?;
            let mut requirements = Vec::with_capacity(n);
            for _ in 0..n {
                requirements.push(read_names(r, b)?);
            }
            added.push(Advancement {
                id,
                parent_id,
                display,
                criteria,
                requirements,
                sends_telemetry_data: r.bool()?,
            });
        }
        let n = count(r, b, 1)?;
        let mut removed = Vec::with_capacity(n);
        for _ in 0..n {
            removed.push(read_id(r, version)?);
        }
        let n = count(r, b, 2)?;
        let mut progress = Vec::with_capacity(n);
        for _ in 0..n {
            let id = read_id(r, version)?;
            let n = count(r, b, 2)?;
            let mut criteria = Vec::with_capacity(n);
            for _ in 0..n {
                criteria.push(CriterionProgress {
                    criterion: r.string(32767)?.to_owned(),
                    achieved_at: if r.bool()? { Some(r.i64()?) } else { None },
                });
            }
            progress.push(AdvancementProgress { id, criteria });
        }
        Ok(Self {
            reset,
            added,
            removed,
            progress,
            show_advancements: if version.protocol() >= 770 {
                Some(r.bool()?)
            } else {
                None
            },
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        if self.show_advancements.is_some() != (version.protocol() >= 770) {
            return Err(Error::Invalid("advancement show flag for version"));
        }
        w.bool(self.reset);
        b.write_count(self.added.len(), w)?;
        for added in &self.added {
            if added.criteria.is_some() != (version.protocol() == 763) {
                return Err(Error::Invalid("advancement criteria list for version"));
            }
            write_id(&added.id, w, version, b)?;
            w.bool(added.parent_id.is_some());
            if let Some(parent) = &added.parent_id {
                write_id(parent, w, version, b)?;
            }
            w.bool(added.display.is_some());
            if let Some(display) = &added.display {
                display.write(w, version, b)?;
            }
            if let Some(criteria) = &added.criteria {
                write_names(criteria, w, b)?;
            }
            b.write_count(added.requirements.len(), w)?;
            for group in &added.requirements {
                write_names(group, w, b)?;
            }
            w.bool(added.sends_telemetry_data);
            b.check_bytes(w)?;
        }
        b.write_count(self.removed.len(), w)?;
        for id in &self.removed {
            write_id(id, w, version, b)?;
        }
        b.write_count(self.progress.len(), w)?;
        for progress in &self.progress {
            write_id(&progress.id, w, version, b)?;
            b.write_count(progress.criteria.len(), w)?;
            for criterion in &progress.criteria {
                string(&criterion.criterion, w, 32767, b)?;
                w.bool(criterion.achieved_at.is_some());
                if let Some(timestamp) = criterion.achieved_at {
                    w.i64(timestamp);
                }
                b.check_bytes(w)?;
            }
        }
        if let Some(show) = self.show_advancements {
            w.bool(show);
        }
        Ok(())
    }
}
impl Body for SelectAdvancementTab {
    fn read_body(r: &mut Reader<'_>, version: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            id: if r.bool()? {
                Some(read_id(r, version)?)
            } else {
                None
            },
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        w.bool(self.id.is_some());
        if let Some(id) = &self.id {
            write_id(id, w, version, b)?;
        }
        Ok(())
    }
}
impl Body for AdvancementTab {
    fn read_body(r: &mut Reader<'_>, version: Version, _: &mut Budget) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::Opened(read_id(r, version)?)),
            1 => Ok(Self::Closed),
            _ => Err(Error::Invalid("advancement tab action")),
        }
    }
    fn write_body(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        match self {
            Self::Opened(id) => {
                w.var_i32(0);
                write_id(id, w, version, b)?;
            }
            Self::Closed => w.var_i32(1),
        }
        Ok(())
    }
}

/// Clientbound advancement family; serverbound notifications are deliberately
/// excluded from the clientbound typed dispatcher.
#[derive(Clone, Debug, PartialEq)]
pub enum AdvancementPacket {
    Update(Advancements),
    SelectTab(SelectAdvancementTab),
}
impl AdvancementPacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "advancements" => Self::Update(Advancements::decode(bytes, version, limits)?),
            "select_advancement_tab" => {
                Self::SelectTab(SelectAdvancementTab::decode(bytes, version, limits)?)
            }
            _ => return Err(Error::Unsupported("typed advancement packet")),
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        match self {
            Self::Update(value) => value.encode(version, limits),
            Self::SelectTab(value) => value.encode(version, limits),
        }
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        match self {
            Self::Update(value) => value.packet(version, limits),
            Self::SelectTab(value) => value.packet(version, limits),
        }
    }
}
