//! Bounded, loss-preserving debug values for protocols 773–776.
//!
//! Events and present updates share the same value codec. Lists retain order
//! and duplicates, registry references remain unresolved, and diagnostic floats
//! are not normalized. These codecs do not subscribe, render, or expire values.
//! See `docs/debug-values-wire-audit.md` for sources and schema corrections.
use crate::{
    codec::{BlockPosition, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

/// Serializable subscription kinds. ID 0 has no registered value codec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DebugValueKind {
    Bees = 1,
    Brains = 2,
    Breezes = 3,
    GoalSelectors = 4,
    EntityPaths = 5,
    EntityBlockIntersections = 6,
    BeeHives = 7,
    Pois = 8,
    RedstoneWireOrientations = 9,
    VillageSections = 10,
    Raids = 11,
    Structures = 12,
    GameEventListeners = 13,
    NeighborUpdates = 14,
    GameEvents = 15,
}
impl DebugValueKind {
    pub fn from_id(id: i32) -> Result<Self> {
        Ok(match id {
            1 => Self::Bees,
            2 => Self::Brains,
            3 => Self::Breezes,
            4 => Self::GoalSelectors,
            5 => Self::EntityPaths,
            6 => Self::EntityBlockIntersections,
            7 => Self::BeeHives,
            8 => Self::Pois,
            9 => Self::RedstoneWireOrientations,
            10 => Self::VillageSections,
            11 => Self::Raids,
            12 => Self::Structures,
            13 => Self::GameEventListeners,
            14 => Self::NeighborUpdates,
            15 => Self::GameEvents,
            n if n < 0 => return Err(Error::Invalid("negative debug value kind")),
            _ => return Err(Error::Unsupported("debug value kind")),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugBee {
    pub hive: Option<BlockPosition>,
    pub flower: Option<BlockPosition>,
    pub travel_ticks: i32,
    pub blacklisted_hives: Vec<BlockPosition>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugBrain {
    pub name: String,
    pub profession: String,
    pub xp: i32,
    pub health: f32,
    pub max_health: f32,
    pub inventory: String,
    pub wants_golem: bool,
    pub anger_level: i32,
    pub activities: Vec<String>,
    pub behaviors: Vec<String>,
    pub memories: Vec<String>,
    pub gossips: Vec<String>,
    pub pois: Vec<BlockPosition>,
    pub potential_pois: Vec<BlockPosition>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugBreeze {
    pub attack_target: Option<i32>,
    pub jump_target: Option<BlockPosition>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugGoal {
    pub priority: i32,
    pub running: bool,
    /// At most 255 UTF-16 code units (not Unicode scalar values).
    pub name: String,
}
/// Numeric ordinal, avoiding version-dependent names. IDs 0–25 are valid from
/// 773; ID 26 is valid from 775. Unknown ordinals are unsupported layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebugPathType(u8);
impl DebugPathType {
    pub fn new(id: i32, version: Version) -> Result<Self> {
        supported(version)?;
        if id < 0 {
            return Err(Error::Invalid("negative debug path type"));
        }
        let max = if version.protocol() >= 775 { 26 } else { 25 };
        if id > max {
            return Err(Error::Unsupported("debug path type"));
        }
        Ok(Self(id as u8))
    }
    pub const fn id(self) -> u8 {
        self.0
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugPathNode {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub walked_distance: f32,
    pub cost_malus: f32,
    pub closed: bool,
    pub path_type: DebugPathType,
    pub f: f32,
}
/// Nodes and targets use identical wire fields. Target runtime heuristic state
/// is not transmitted. Empty targets are decoder-valid and may be encoded here;
/// vanilla's producer enforces a stronger nonempty-target/debug-data invariant.
#[derive(Clone, Debug, PartialEq)]
pub struct DebugPath {
    pub reached: bool,
    pub next_node_index: i32,
    pub target: BlockPosition,
    pub nodes: Vec<DebugPathNode>,
    pub target_nodes: Vec<DebugPathNode>,
    pub open_set: Vec<DebugPathNode>,
    pub closed_set: Vec<DebugPathNode>,
    pub max_node_distance: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum DebugIntersection {
    InBlock = 0,
    InFluid = 1,
    InAir = 2,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugHive {
    /// Direct block registry ID, not a block-state ID.
    pub block_id: u32,
    pub occupant_count: i32,
    pub honey_level: i32,
    pub sedated: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugPoi {
    pub position: BlockPosition,
    /// Direct POI-type registry ID, not an inline holder or ID-plus-one.
    pub poi_type_id: u32,
    pub free_ticket_count: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugStructurePiece {
    pub min: BlockPosition,
    pub max: BlockPosition,
    pub start: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugStructure {
    pub min: BlockPosition,
    pub max: BlockPosition,
    pub pieces: Vec<DebugStructurePiece>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugGameEvent {
    /// Direct GAME_EVENT registry ID, not a string or ID-plus-one.
    pub game_event_id: u32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DebugValue {
    Bees(DebugBee),
    Brains(Box<DebugBrain>),
    Breezes(DebugBreeze),
    GoalSelectors(Vec<DebugGoal>),
    EntityPaths(Box<DebugPath>),
    EntityBlockIntersections(DebugIntersection),
    BeeHives(DebugHive),
    Pois(DebugPoi),
    /// Official orientation table index, 0–47.
    RedstoneWireOrientations(u8),
    VillageSections,
    Raids(Vec<BlockPosition>),
    Structures(Vec<DebugStructure>),
    GameEventListeners {
        listener_radius: i32,
    },
    NeighborUpdates(BlockPosition),
    GameEvents(DebugGameEvent),
}
impl DebugValue {
    pub const fn kind(&self) -> DebugValueKind {
        match self {
            Self::Bees(_) => DebugValueKind::Bees,
            Self::Brains(_) => DebugValueKind::Brains,
            Self::Breezes(_) => DebugValueKind::Breezes,
            Self::GoalSelectors(_) => DebugValueKind::GoalSelectors,
            Self::EntityPaths(_) => DebugValueKind::EntityPaths,
            Self::EntityBlockIntersections(_) => DebugValueKind::EntityBlockIntersections,
            Self::BeeHives(_) => DebugValueKind::BeeHives,
            Self::Pois(_) => DebugValueKind::Pois,
            Self::RedstoneWireOrientations(_) => DebugValueKind::RedstoneWireOrientations,
            Self::VillageSections => DebugValueKind::VillageSections,
            Self::Raids(_) => DebugValueKind::Raids,
            Self::Structures(_) => DebugValueKind::Structures,
            Self::GameEventListeners { .. } => DebugValueKind::GameEventListeners,
            Self::NeighborUpdates(_) => DebugValueKind::NeighborUpdates,
            Self::GameEvents(_) => DebugValueKind::GameEvents,
        }
    }
}
/// Removal retains its subscription kind; present values cannot mismatch it.
#[derive(Clone, Debug, PartialEq)]
pub enum DebugUpdate {
    Removed(DebugValueKind),
    Value(DebugValue),
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugBlockValue {
    pub position: BlockPosition,
    pub update: DebugUpdate,
}
/// Chunk coordinates pack x in the low 32 bits and z in the high 32 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebugChunkPosition {
    pub x: i32,
    pub z: i32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugChunkValue {
    pub position: DebugChunkPosition,
    pub update: DebugUpdate,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugEntityValue {
    pub entity_id: i32,
    pub update: DebugUpdate,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DebugEvent {
    pub value: DebugValue,
}
#[derive(Clone, Debug, PartialEq)]
pub enum DebugValuePacket {
    Block(DebugBlockValue),
    Chunk(DebugChunkValue),
    Entity(DebugEntityValue),
    Event(DebugEvent),
}
impl DebugValuePacket {
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        Ok(match name {
            "debug_block_value" => Self::Block(DebugBlockValue::decode(bytes, version, limits)?),
            "debug_chunk_value" => Self::Chunk(DebugChunkValue::decode(bytes, version, limits)?),
            "debug_entity_value" => Self::Entity(DebugEntityValue::decode(bytes, version, limits)?),
            "debug_event" => Self::Event(DebugEvent::decode(bytes, version, limits)?),
            _ => return Err(Error::Unsupported("debug value packet")),
        })
    }
}
fn supported(v: Version) -> Result<()> {
    if !(773..=776).contains(&v.protocol()) {
        return Err(Error::Unsupported("debug value version"));
    }
    Ok(())
}
fn prefix_len(n: i32) -> usize {
    let mut n = n as u32;
    let mut size = 1;
    while n > 127 {
        n >>= 7;
        size += 1;
    }
    size
}
/// One aggregate budget for all sibling/nested collections and owned strings.
struct Budget {
    elements: usize,
    strings: usize,
    limits: Limits,
}
impl Budget {
    fn new(limits: Limits) -> Self {
        Self {
            elements: 0,
            strings: 0,
            limits,
        }
    }
    fn collection(&mut self, n: usize) -> Result<()> {
        self.elements = self
            .elements
            .checked_add(n)
            .ok_or(Error::Limit("debug aggregate elements"))?;
        if n > i32::MAX as usize || self.elements > self.limits.max_collection {
            return Err(Error::Limit("debug aggregate elements"));
        }
        Ok(())
    }
    fn string(&mut self, n: usize) -> Result<()> {
        self.strings = self
            .strings
            .checked_add(n)
            .ok_or(Error::Limit("debug string bytes"))?;
        if self.strings > self.limits.max_packet {
            return Err(Error::Limit("debug string bytes"));
        }
        Ok(())
    }
}
struct Input<'a> {
    r: Reader<'a>,
    budget: Budget,
    version: Version,
}
impl Input<'_> {
    fn position(&mut self) -> Result<BlockPosition> {
        Ok(BlockPosition::unpack(self.r.i64()?))
    }
    fn registry(&mut self) -> Result<u32> {
        let n = self.r.var_i32()?;
        if n < 0 {
            return Err(Error::Invalid("negative debug registry ID"));
        }
        Ok(n as u32)
    }
    fn string(&mut self, max: usize) -> Result<String> {
        let s = self.r.string(max)?;
        self.budget.string(s.len())?;
        let mut value = String::new();
        value
            .try_reserve_exact(s.len())
            .map_err(|_| Error::Limit("debug string allocation"))?;
        value.push_str(s);
        Ok(value)
    }
    fn optional<T>(&mut self, read: impl FnOnce(&mut Self) -> Result<T>) -> Result<Option<T>> {
        if self.r.bool()? {
            Ok(Some(read(self)?))
        } else {
            Ok(None)
        }
    }
    /// Check wire minima plus mandatory suffix before any Vec reservation.
    fn list<T>(
        &mut self,
        minimum: usize,
        suffix: usize,
        mut read: impl FnMut(&mut Self, usize) -> Result<T>,
    ) -> Result<Vec<T>> {
        let n = self
            .r
            .count(self.budget.limits.max_collection.min(i32::MAX as usize))?;
        self.budget.collection(n)?;
        let needed = n
            .checked_mul(minimum)
            .and_then(|n| n.checked_add(suffix))
            .ok_or(Error::Limit("debug list wire bytes"))?;
        if needed > self.r.remaining().len() {
            return Err(Error::Eof);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(n)
            .map_err(|_| Error::Limit("debug list allocation"))?;
        for i in 0..n {
            values.push(read(self, suffix + (n - i - 1) * minimum)?);
        }
        Ok(values)
    }
    fn node(&mut self) -> Result<DebugPathNode> {
        Ok(DebugPathNode {
            x: self.r.i32()?,
            y: self.r.i32()?,
            z: self.r.i32()?,
            walked_distance: self.r.f32()?,
            cost_malus: self.r.f32()?,
            closed: self.r.bool()?,
            path_type: DebugPathType::new(self.r.var_i32()?, self.version)?,
            f: self.r.f32()?,
        })
    }
    fn value(&mut self, kind: DebugValueKind) -> Result<DebugValue> {
        Ok(match kind {
            DebugValueKind::Bees => DebugValue::Bees(DebugBee {
                hive: self.optional(Self::position)?,
                flower: self.optional(Self::position)?,
                travel_ticks: self.r.var_i32()?,
                blacklisted_hives: self.list(8, 0, |s, _| s.position())?,
            }),
            DebugValueKind::Brains => DebugValue::Brains(Box::new(DebugBrain {
                name: self.string(32767)?,
                profession: self.string(32767)?,
                xp: self.r.i32()?,
                health: self.r.f32()?,
                max_health: self.r.f32()?,
                inventory: self.string(32767)?,
                wants_golem: self.r.bool()?,
                anger_level: self.r.i32()?,
                activities: self.list(1, 5, |s, _| s.string(32767))?,
                behaviors: self.list(1, 4, |s, _| s.string(32767))?,
                memories: self.list(1, 3, |s, _| s.string(32767))?,
                gossips: self.list(1, 2, |s, _| s.string(32767))?,
                pois: self.list(8, 1, |s, _| s.position())?,
                potential_pois: self.list(8, 0, |s, _| s.position())?,
            })),
            DebugValueKind::Breezes => DebugValue::Breezes(DebugBreeze {
                attack_target: self.optional(|s| s.r.var_i32())?,
                jump_target: self.optional(Self::position)?,
            }),
            DebugValueKind::GoalSelectors => {
                DebugValue::GoalSelectors(self.list(3, 0, |s, _| {
                    Ok(DebugGoal {
                        priority: s.r.var_i32()?,
                        running: s.r.bool()?,
                        name: s.string(255)?,
                    })
                })?)
            }
            DebugValueKind::EntityPaths => DebugValue::EntityPaths(Box::new(DebugPath {
                reached: self.r.bool()?,
                next_node_index: self.r.i32()?,
                target: self.position()?,
                nodes: self.list(26, 7, |s, _| s.node())?,
                target_nodes: self.list(26, 6, |s, _| s.node())?,
                open_set: self.list(26, 5, |s, _| s.node())?,
                closed_set: self.list(26, 4, |s, _| s.node())?,
                max_node_distance: self.r.f32()?,
            })),
            DebugValueKind::EntityBlockIntersections => {
                DebugValue::EntityBlockIntersections(match self.r.var_i32()? {
                    0 => DebugIntersection::InBlock,
                    1 => DebugIntersection::InFluid,
                    2 => DebugIntersection::InAir,
                    n if n < 0 => return Err(Error::Invalid("negative debug intersection kind")),
                    _ => return Err(Error::Unsupported("debug intersection kind")),
                })
            }
            DebugValueKind::BeeHives => DebugValue::BeeHives(DebugHive {
                block_id: self.registry()?,
                occupant_count: self.r.var_i32()?,
                honey_level: self.r.var_i32()?,
                sedated: self.r.bool()?,
            }),
            DebugValueKind::Pois => DebugValue::Pois(DebugPoi {
                position: self.position()?,
                poi_type_id: self.registry()?,
                free_ticket_count: self.r.var_i32()?,
            }),
            DebugValueKind::RedstoneWireOrientations => {
                let n = self.r.var_i32()?;
                if !(0..48).contains(&n) {
                    return Err(Error::Invalid("debug orientation index"));
                }
                DebugValue::RedstoneWireOrientations(n as u8)
            }
            DebugValueKind::VillageSections => DebugValue::VillageSections,
            DebugValueKind::Raids => DebugValue::Raids(self.list(8, 0, |s, _| s.position())?),
            DebugValueKind::Structures => {
                DebugValue::Structures(self.list(17, 0, |s, suffix| {
                    Ok(DebugStructure {
                        min: s.position()?,
                        max: s.position()?,
                        pieces: s.list(17, suffix, |s, _| {
                            Ok(DebugStructurePiece {
                                min: s.position()?,
                                max: s.position()?,
                                start: s.r.bool()?,
                            })
                        })?,
                    })
                })?)
            }
            DebugValueKind::GameEventListeners => DebugValue::GameEventListeners {
                listener_radius: self.r.var_i32()?,
            },
            DebugValueKind::NeighborUpdates => DebugValue::NeighborUpdates(self.position()?),
            DebugValueKind::GameEvents => DebugValue::GameEvents(DebugGameEvent {
                game_event_id: self.registry()?,
                x: self.r.f64()?,
                y: self.r.f64()?,
                z: self.r.f64()?,
            }),
        })
    }
    fn update(&mut self) -> Result<DebugUpdate> {
        let kind = DebugValueKind::from_id(self.r.var_i32()?)?;
        Ok(if self.r.bool()? {
            DebugUpdate::Value(self.value(kind)?)
        } else {
            DebugUpdate::Removed(kind)
        })
    }
}
/// The identical traversal measures/validates first, then emits into an exactly
/// sized local buffer. Every addition is checked before reservation or growth.
struct Output<'a> {
    writer: Option<&'a mut Writer>,
    size: usize,
    budget: Budget,
    version: Version,
}
impl Output<'_> {
    fn add(&mut self, n: usize) -> Result<()> {
        self.size = self
            .size
            .checked_add(n)
            .ok_or(Error::Limit("debug packet bytes"))?;
        if self.size > self.budget.limits.max_packet {
            return Err(Error::Limit("debug packet bytes"));
        }
        Ok(())
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<()> {
        self.add(bytes.len())?;
        if let Some(w) = &mut self.writer {
            w.raw(bytes);
        }
        Ok(())
    }
    fn boolean(&mut self, b: bool) -> Result<()> {
        self.raw(&[u8::from(b)])
    }
    fn i32(&mut self, n: i32) -> Result<()> {
        self.raw(&n.to_be_bytes())
    }
    fn f32(&mut self, n: f32) -> Result<()> {
        self.raw(&n.to_bits().to_be_bytes())
    }
    fn f64(&mut self, n: f64) -> Result<()> {
        self.raw(&n.to_bits().to_be_bytes())
    }
    fn var(&mut self, n: i32) -> Result<()> {
        self.add(prefix_len(n))?;
        if let Some(w) = &mut self.writer {
            w.var_i32(n);
        }
        Ok(())
    }
    fn registry(&mut self, n: u32) -> Result<()> {
        if n > i32::MAX as u32 {
            return Err(Error::Invalid("debug registry ID"));
        }
        self.var(n as i32)
    }
    fn position(&mut self, p: &BlockPosition) -> Result<()> {
        self.raw(&p.pack()?.to_be_bytes())
    }
    fn string(&mut self, s: &str, max: usize) -> Result<()> {
        let max = max.min(self.budget.limits.max_string_chars);
        if s.len() > i32::MAX as usize
            || s.len() > max.saturating_mul(3)
            || s.encode_utf16().count() > max
        {
            return Err(Error::Limit("debug string length"));
        }
        self.budget.string(s.len())?;
        self.var(s.len() as i32)?;
        self.raw(s.as_bytes())
    }
    fn optional<T>(
        &mut self,
        v: &Option<T>,
        write: impl FnOnce(&mut Self, &T) -> Result<()>,
    ) -> Result<()> {
        self.boolean(v.is_some())?;
        if let Some(v) = v {
            write(self, v)?;
        }
        Ok(())
    }
    fn list<T>(
        &mut self,
        values: &[T],
        mut write: impl FnMut(&mut Self, &T) -> Result<()>,
    ) -> Result<()> {
        self.budget.collection(values.len())?;
        self.var(values.len() as i32)?;
        for value in values {
            write(self, value)?;
        }
        Ok(())
    }
    fn node(&mut self, n: &DebugPathNode) -> Result<()> {
        DebugPathType::new(n.path_type.id() as i32, self.version)?;
        self.i32(n.x)?;
        self.i32(n.y)?;
        self.i32(n.z)?;
        self.f32(n.walked_distance)?;
        self.f32(n.cost_malus)?;
        self.boolean(n.closed)?;
        self.var(n.path_type.id() as i32)?;
        self.f32(n.f)
    }
    fn value(&mut self, v: &DebugValue) -> Result<()> {
        match v {
            DebugValue::Bees(v) => {
                self.optional(&v.hive, Self::position)?;
                self.optional(&v.flower, Self::position)?;
                self.var(v.travel_ticks)?;
                self.list(&v.blacklisted_hives, Self::position)?;
            }
            DebugValue::Brains(v) => {
                self.string(&v.name, 32767)?;
                self.string(&v.profession, 32767)?;
                self.i32(v.xp)?;
                self.f32(v.health)?;
                self.f32(v.max_health)?;
                self.string(&v.inventory, 32767)?;
                self.boolean(v.wants_golem)?;
                self.i32(v.anger_level)?;
                for list in [&v.activities, &v.behaviors, &v.memories, &v.gossips] {
                    self.list(list, |s, v| s.string(v, 32767))?;
                }
                self.list(&v.pois, Self::position)?;
                self.list(&v.potential_pois, Self::position)?;
            }
            DebugValue::Breezes(v) => {
                self.optional(&v.attack_target, |s, n| s.var(*n))?;
                self.optional(&v.jump_target, Self::position)?;
            }
            DebugValue::GoalSelectors(v) => self.list(v, |s, g| {
                s.var(g.priority)?;
                s.boolean(g.running)?;
                s.string(&g.name, 255)
            })?,
            DebugValue::EntityPaths(v) => {
                self.boolean(v.reached)?;
                self.i32(v.next_node_index)?;
                self.position(&v.target)?;
                for list in [&v.nodes, &v.target_nodes, &v.open_set, &v.closed_set] {
                    self.list(list, Self::node)?;
                }
                self.f32(v.max_node_distance)?;
            }
            DebugValue::EntityBlockIntersections(v) => self.var(*v as i32)?,
            DebugValue::BeeHives(v) => {
                self.registry(v.block_id)?;
                self.var(v.occupant_count)?;
                self.var(v.honey_level)?;
                self.boolean(v.sedated)?;
            }
            DebugValue::Pois(v) => {
                self.position(&v.position)?;
                self.registry(v.poi_type_id)?;
                self.var(v.free_ticket_count)?;
            }
            DebugValue::RedstoneWireOrientations(n) => {
                if *n > 47 {
                    return Err(Error::Invalid("debug orientation index"));
                }
                self.var(*n as i32)?;
            }
            DebugValue::VillageSections => {}
            DebugValue::Raids(v) => self.list(v, Self::position)?,
            DebugValue::Structures(v) => self.list(v, |s, v| {
                s.position(&v.min)?;
                s.position(&v.max)?;
                s.list(&v.pieces, |s, p| {
                    s.position(&p.min)?;
                    s.position(&p.max)?;
                    s.boolean(p.start)
                })
            })?,
            DebugValue::GameEventListeners { listener_radius } => self.var(*listener_radius)?,
            DebugValue::NeighborUpdates(p) => self.position(p)?,
            DebugValue::GameEvents(v) => {
                self.registry(v.game_event_id)?;
                self.f64(v.x)?;
                self.f64(v.y)?;
                self.f64(v.z)?;
            }
        }
        Ok(())
    }
    fn update(&mut self, update: &DebugUpdate) -> Result<()> {
        match update {
            DebugUpdate::Removed(kind) => {
                self.var(*kind as i32)?;
                self.boolean(false)
            }
            DebugUpdate::Value(v) => {
                self.var(v.kind() as i32)?;
                self.boolean(true)?;
                self.value(v)
            }
        }
    }
}
trait Body: Sized {
    fn read_body(input: &mut Input<'_>) -> Result<Self>;
    fn write_body(&self, output: &mut Output<'_>) -> Result<()>;
}
macro_rules! body_codec {
    ($ty:ty, $name:literal) => {
        impl $ty {
            /// Read one body transactionally; suffix bytes remain unread.
            pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
                supported(version)?;
                version.packet_id(State::Play, Direction::Clientbound, $name)?;
                let mut input = Input {
                    r: Reader::new(
                        &r.remaining()[..r.remaining().len().min(r.limits.max_packet)],
                        r.limits,
                    ),
                    budget: Budget::new(r.limits),
                    version,
                };
                let value = Self::read_body(&mut input)?;
                r.take(input.r.position())?;
                Ok(value)
            }
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet {
                    return Err(Error::Limit("debug packet bytes"));
                }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version)?;
                r.finish()?;
                Ok(value)
            }
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                supported(version)?;
                version.packet_id(State::Play, Direction::Clientbound, $name)?;
                let mut measured = Output {
                    writer: None,
                    size: 0,
                    budget: Budget::new(limits),
                    version,
                };
                self.write_body(&mut measured)?;
                let mut w = Writer::with_capacity(measured.size);
                self.write_body(&mut Output {
                    writer: Some(&mut w),
                    size: 0,
                    budget: Budget::new(limits),
                    version,
                })?;
                Ok(w.into_inner())
            }
            /// Append atomically, with limits applying to this body only.
            pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
                w.raw(&self.encode(version, limits)?);
                Ok(())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(
                    version.packet_id(State::Play, Direction::Clientbound, $name)?,
                    self.encode(version, limits)?,
                ))
            }
        }
    };
}
impl Body for DebugBlockValue {
    fn read_body(s: &mut Input<'_>) -> Result<Self> {
        Ok(Self {
            position: s.position()?,
            update: s.update()?,
        })
    }
    fn write_body(&self, s: &mut Output<'_>) -> Result<()> {
        s.position(&self.position)?;
        s.update(&self.update)
    }
}
impl Body for DebugChunkValue {
    fn read_body(s: &mut Input<'_>) -> Result<Self> {
        let bits = s.r.i64()?;
        Ok(Self {
            position: DebugChunkPosition {
                x: bits as i32,
                z: (bits >> 32) as i32,
            },
            update: s.update()?,
        })
    }
    fn write_body(&self, s: &mut Output<'_>) -> Result<()> {
        let bits = ((self.position.z as u32 as u64) << 32) | self.position.x as u32 as u64;
        s.raw(&bits.to_be_bytes())?;
        s.update(&self.update)
    }
}
impl Body for DebugEntityValue {
    fn read_body(s: &mut Input<'_>) -> Result<Self> {
        Ok(Self {
            entity_id: s.r.var_i32()?,
            update: s.update()?,
        })
    }
    fn write_body(&self, s: &mut Output<'_>) -> Result<()> {
        s.var(self.entity_id)?;
        s.update(&self.update)
    }
}
impl Body for DebugEvent {
    fn read_body(s: &mut Input<'_>) -> Result<Self> {
        let kind = DebugValueKind::from_id(s.r.var_i32()?)?;
        Ok(Self {
            value: s.value(kind)?,
        })
    }
    fn write_body(&self, s: &mut Output<'_>) -> Result<()> {
        s.var(self.value.kind() as i32)?;
        s.value(&self.value)
    }
}
body_codec!(DebugBlockValue, "debug_block_value");
body_codec!(DebugChunkValue, "debug_chunk_value");
body_codec!(DebugEntityValue, "debug_entity_value");
body_codec!(DebugEvent, "debug_event");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_string_bytes_are_cumulative_and_checked_before_copy() {
        let limits = Limits {
            max_packet: 5,
            ..Limits::default()
        };
        let mut budget = Budget::new(limits);
        budget.string(3).unwrap();
        budget.string(2).unwrap();
        assert!(matches!(budget.string(1), Err(Error::Limit(_))));
        let mut budget = Budget::new(Limits {
            max_packet: usize::MAX,
            ..limits
        });
        budget.string(usize::MAX).unwrap();
        assert!(matches!(budget.string(1), Err(Error::Limit(_))));
    }
    #[test]
    fn element_sum_and_output_size_cannot_overflow() {
        let mut budget = Budget::new(Limits {
            max_collection: usize::MAX,
            ..Limits::default()
        });
        budget.elements = usize::MAX;
        assert!(matches!(budget.collection(1), Err(Error::Limit(_))));
        let mut output = Output {
            writer: None,
            size: usize::MAX,
            budget: Budget::new(Limits {
                max_packet: usize::MAX,
                ..Limits::default()
            }),
            version: Version::V26_2,
        };
        assert!(matches!(output.add(1), Err(Error::Limit(_))));
    }
}
