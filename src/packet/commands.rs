//! Bounded command graphs and suggestions for production protocols 763–776.
//!
//! Parser IDs and layouts were checked against the pinned protocol schemas and
//! release-server ArgumentTypeInfos/ArgumentTypeInfo serializers. The registry
//! corrects the schemas' `nbt` spelling and protocol-776 `team_color` rename.
//! Restriction bit 0x20 exists from 771. Numeric bounds are lossless wire values:
//! the official serializers do not require finite or ordered bounds.
//!
//! Graph validation follows ClientboundCommandsPacket's two independent
//! dependency relations: child cycles and redirect-only cycles are invalid,
//! but mixed child/redirect cycles are normal (for example `execute ... run`).
//! This codec never recursively expands the graph or executes commands.
use super::{
    chat::ChatComponent,
    inventory::{self, Budget},
};
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    nbt::RootFormat,
    version::State,
    Error, Limits, Result, Version,
};
mod registry;
pub use registry::{parser_registry, ParserKind};

/// Whether a Brigadier string consumes one word, a quoted phrase, or the tail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringMode {
    SingleWord,
    QuotablePhrase,
    GreedyPhrase,
}

/// Properties are paired with a [`ParserKind`]; encoding rejects mismatches.
/// An absent numeric bound is distinct from an explicitly transmitted default.
/// IEEE infinities/NaNs and inverted bounds are retained, as in the official
/// serializers. Callers decide whether such parsers are useful or acceptable.
#[derive(Clone, Debug, PartialEq)]
pub enum ParserProperties {
    None,
    Float {
        min: Option<f32>,
        max: Option<f32>,
    },
    Double {
        min: Option<f64>,
        max: Option<f64>,
    },
    Integer {
        min: Option<i32>,
        max: Option<i32>,
    },
    Long {
        min: Option<i64>,
        max: Option<i64>,
    },
    String(StringMode),
    /// Bit 0 means a single target, not "entities only" as some schemas label it.
    Entity {
        single: bool,
        players_only: bool,
    },
    ScoreHolder {
        multiple: bool,
    },
    Time {
        minimum: i32,
    },
    /// resource, resource_key, resource_or_tag, resource_or_tag_key, or
    /// resource_selector. The identifier is retained without registry lookup.
    Registry(String),
}
#[derive(Clone, Debug, PartialEq)]
pub struct CommandParser {
    pub kind: ParserKind,
    pub properties: ParserProperties,
}

/// Built-in providers and opaque, syntax-checked custom provider identifiers.
/// Custom names are not executed or resolved; unqualified names stay verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuggestionProvider {
    AskServer,
    AvailableSounds,
    SummonableEntities,
    Custom(String),
}
impl SuggestionProvider {
    pub fn name(&self) -> &str {
        match self {
            Self::AskServer => "minecraft:ask_server",
            Self::AvailableSounds => "minecraft:available_sounds",
            Self::SummonableEntities => "minecraft:summonable_entities",
            Self::Custom(name) => name,
        }
    }
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let name = read_identifier(r, version)?;
        Ok(match name.as_str() {
            "minecraft:ask_server" => Self::AskServer,
            "minecraft:available_sounds" => Self::AvailableSounds,
            "minecraft:summonable_entities" => Self::SummonableEntities,
            _ => Self::Custom(name),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum CommandNodeKind {
    Root,
    Literal {
        name: String,
    },
    Argument {
        name: String,
        parser: CommandParser,
        suggestions: Option<SuggestionProvider>,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct CommandNode {
    pub kind: CommandNodeKind,
    pub executable: bool,
    /// Protocol 771+ only. This is the wire "restricted" flag, not permission
    /// to run a command; applications still enforce their own security policy.
    pub restricted: bool,
    pub children: Vec<u32>,
    pub redirect: Option<u32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CommandTree {
    /// Original node order and indices are preserved, including shared nodes.
    pub nodes: Vec<CommandNode>,
    pub root_index: u32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CommandSuggestion {
    pub text: String,
    pub tooltip: Option<ChatComponent>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CommandSuggestions {
    pub transaction_id: i32,
    /// Java UTF-16 indices, retained as signed VarInts like the release codecs.
    /// No request text is available here to validate or apply the replacement.
    pub start: i32,
    pub length: i32,
    pub matches: Vec<CommandSuggestion>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSuggestionRequest {
    pub transaction_id: i32,
    pub text: String,
}

fn reader(bytes: &[u8], limits: Limits) -> Result<Reader<'_>> {
    if bytes.len() > limits.max_packet {
        return Err(Error::Limit("command packet bytes"));
    }
    Ok(Reader::new(bytes, limits))
}
fn string(w: &mut Writer, value: &str, max: usize, b: &Budget) -> Result<()> {
    let prefix = varint_len(value.len());
    if value
        .len()
        .checked_add(prefix)
        .is_none_or(|n| n > b.limits.max_packet.saturating_sub(w.as_slice().len()))
    {
        return Err(Error::Limit("command string bytes"));
    }
    w.string(value, max.min(b.limits.max_string_chars))?;
    b.check_bytes(w)
}
fn varint_len(mut n: usize) -> usize {
    let mut len = 1;
    while n > 127 {
        n >>= 7;
        len += 1;
    }
    len
}
fn identifier(value: &str, version: Version) -> Result<()> {
    let (namespace, path) = value.split_once(':').unwrap_or(("minecraft", value));
    let namespace = if namespace.is_empty() {
        "minecraft"
    } else {
        namespace
    };
    let valid = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b);
    if (version.protocol() >= 775 && namespace == "..")
        || !namespace.bytes().all(valid)
        || !path.bytes().all(|b| valid(b) || b == b'/')
    {
        return Err(Error::Invalid("command resource identifier"));
    }
    Ok(())
}
fn read_identifier(r: &mut Reader<'_>, version: Version) -> Result<String> {
    let value = r.string(32767)?;
    identifier(value, version)?;
    Ok(value.to_owned())
}
fn write_identifier(w: &mut Writer, value: &str, version: Version, b: &Budget) -> Result<()> {
    identifier(value, version)?;
    string(w, value, 32767, b)
}
fn flags(r: &mut Reader<'_>, allowed: u8) -> Result<u8> {
    let flags = r.u8()?;
    if flags & !allowed != 0 {
        return Err(Error::Invalid("command flags"));
    }
    Ok(flags)
}
fn index(r: &mut Reader<'_>, count: usize) -> Result<u32> {
    let index = r.var_i32()?;
    if index < 0 || index as usize >= count {
        return Err(Error::Invalid("command node reference"));
    }
    Ok(index as u32)
}
fn numeric_flags<T>(min: &Option<T>, max: &Option<T>) -> u8 {
    u8::from(min.is_some()) | (u8::from(max.is_some()) << 1)
}

impl CommandParser {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        // Parser payloads are not length-framed. Unknown IDs must fail before
        // reading anything that could instead be the next node or root index.
        let kind = ParserKind::from_id(r.var_i32()?, version)?;
        let properties = match kind {
            ParserKind::Float => {
                let f = flags(r, 3)?;
                ParserProperties::Float {
                    min: if f & 1 != 0 { Some(r.f32()?) } else { None },
                    max: if f & 2 != 0 { Some(r.f32()?) } else { None },
                }
            }
            ParserKind::Double => {
                let f = flags(r, 3)?;
                ParserProperties::Double {
                    min: if f & 1 != 0 { Some(r.f64()?) } else { None },
                    max: if f & 2 != 0 { Some(r.f64()?) } else { None },
                }
            }
            ParserKind::Integer => {
                let f = flags(r, 3)?;
                ParserProperties::Integer {
                    min: if f & 1 != 0 { Some(r.i32()?) } else { None },
                    max: if f & 2 != 0 { Some(r.i32()?) } else { None },
                }
            }
            ParserKind::Long => {
                let f = flags(r, 3)?;
                ParserProperties::Long {
                    min: if f & 1 != 0 { Some(r.i64()?) } else { None },
                    max: if f & 2 != 0 { Some(r.i64()?) } else { None },
                }
            }
            ParserKind::String => ParserProperties::String(match r.var_i32()? {
                0 => StringMode::SingleWord,
                1 => StringMode::QuotablePhrase,
                2 => StringMode::GreedyPhrase,
                _ => return Err(Error::Invalid("command string mode")),
            }),
            ParserKind::Entity => {
                let f = flags(r, 3)?;
                ParserProperties::Entity {
                    single: f & 1 != 0,
                    players_only: f & 2 != 0,
                }
            }
            ParserKind::ScoreHolder => ParserProperties::ScoreHolder {
                multiple: flags(r, 1)? != 0,
            },
            ParserKind::Time => ParserProperties::Time { minimum: r.i32()? },
            ParserKind::Resource
            | ParserKind::ResourceKey
            | ParserKind::ResourceOrTag
            | ParserKind::ResourceOrTagKey
            | ParserKind::ResourceSelector => {
                ParserProperties::Registry(read_identifier(r, version)?)
            }
            _ => ParserProperties::None,
        };
        Ok(Self { kind, properties })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &Budget) -> Result<()> {
        w.var_i32(self.kind.id(version)?);
        match (self.kind, &self.properties) {
            (ParserKind::Float, ParserProperties::Float { min, max }) => {
                w.u8(numeric_flags(min, max));
                if let Some(x) = min {
                    w.f32(*x);
                }
                if let Some(x) = max {
                    w.f32(*x);
                }
            }
            (ParserKind::Double, ParserProperties::Double { min, max }) => {
                w.u8(numeric_flags(min, max));
                if let Some(x) = min {
                    w.f64(*x);
                }
                if let Some(x) = max {
                    w.f64(*x);
                }
            }
            (ParserKind::Integer, ParserProperties::Integer { min, max }) => {
                w.u8(numeric_flags(min, max));
                if let Some(x) = min {
                    w.i32(*x);
                }
                if let Some(x) = max {
                    w.i32(*x);
                }
            }
            (ParserKind::Long, ParserProperties::Long { min, max }) => {
                w.u8(numeric_flags(min, max));
                if let Some(x) = min {
                    w.i64(*x);
                }
                if let Some(x) = max {
                    w.i64(*x);
                }
            }
            (ParserKind::String, ParserProperties::String(mode)) => w.var_i32(match mode {
                StringMode::SingleWord => 0,
                StringMode::QuotablePhrase => 1,
                StringMode::GreedyPhrase => 2,
            }),
            (
                ParserKind::Entity,
                ParserProperties::Entity {
                    single,
                    players_only,
                },
            ) => w.u8(u8::from(*single) | (u8::from(*players_only) << 1)),
            (ParserKind::ScoreHolder, ParserProperties::ScoreHolder { multiple }) => {
                w.bool(*multiple)
            }
            (ParserKind::Time, ParserProperties::Time { minimum }) => w.i32(*minimum),
            (
                ParserKind::Resource
                | ParserKind::ResourceKey
                | ParserKind::ResourceOrTag
                | ParserKind::ResourceOrTagKey
                | ParserKind::ResourceSelector,
                ParserProperties::Registry(name),
            ) => write_identifier(w, name, version, b)?,
            (kind, ParserProperties::None)
                if !matches!(
                    kind,
                    ParserKind::Float
                        | ParserKind::Double
                        | ParserKind::Integer
                        | ParserKind::Long
                        | ParserKind::String
                        | ParserKind::Entity
                        | ParserKind::ScoreHolder
                        | ParserKind::Time
                        | ParserKind::Resource
                        | ParserKind::ResourceKey
                        | ParserKind::ResourceOrTag
                        | ParserKind::ResourceOrTagKey
                        | ParserKind::ResourceSelector
                ) => {}
            _ => return Err(Error::Invalid("command parser properties")),
        }
        b.check_bytes(w)
    }
}

impl CommandTree {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let mut b = Budget::new(limits);
        let count = b.count(&mut r)?;
        // Even a root node needs a flag byte and an empty children count.
        if count > r.remaining().len() / 2 {
            return Err(Error::Eof);
        }
        let mut nodes = Vec::with_capacity(count);
        for _ in 0..count {
            let f = flags(
                &mut r,
                if version.protocol() >= 771 {
                    0x3f
                } else {
                    0x1f
                },
            )?;
            if f & 3 == 3 || f & 16 != 0 && f & 3 != 2 {
                return Err(Error::Invalid("command node flags/type"));
            }
            let child_count = b.count(&mut r)?;
            if child_count > r.remaining().len() {
                return Err(Error::Eof);
            }
            let mut children = Vec::with_capacity(child_count);
            for _ in 0..child_count {
                children.push(index(&mut r, count)?);
            }
            let redirect = if f & 8 != 0 {
                b.charge(1)?;
                Some(index(&mut r, count)?)
            } else {
                None
            };
            let kind = match f & 3 {
                0 => CommandNodeKind::Root,
                1 => CommandNodeKind::Literal {
                    name: r.string(32767)?.to_owned(),
                },
                2 => CommandNodeKind::Argument {
                    name: r.string(32767)?.to_owned(),
                    parser: CommandParser::read(&mut r, version)?,
                    suggestions: if f & 16 != 0 {
                        Some(SuggestionProvider::read(&mut r, version)?)
                    } else {
                        None
                    },
                },
                _ => unreachable!(),
            };
            nodes.push(CommandNode {
                kind,
                executable: f & 4 != 0,
                restricted: f & 32 != 0,
                children,
                redirect,
            });
        }
        let value = Self {
            nodes,
            root_index: index(&mut r, count)?,
        };
        r.finish()?;
        value.validate_topology()?;
        Ok(value)
    }
    /// Preserves graph indices and numeric bit patterns; no partial packet is
    /// returned on error. `max_collection` covers nodes plus all references.
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut b = Budget::new(limits);
        b.charge(self.nodes.len())?;
        if self.nodes.len() > i32::MAX as usize || self.nodes.len() > limits.max_packet / 2 {
            return Err(Error::Limit("command node count"));
        }
        for node in &self.nodes {
            b.charge(node.children.len())?;
            b.charge(usize::from(node.redirect.is_some()))?;
            if node.children.len() > i32::MAX as usize {
                return Err(Error::Limit("command children"));
            }
            if node.restricted && version.protocol() < 771 {
                return Err(Error::Invalid("restricted command for protocol"));
            }
        }
        self.validate_topology()?;
        let mut w = Writer::new();
        w.var_i32(self.nodes.len() as i32);
        for node in &self.nodes {
            let (tag, suggestions) = match &node.kind {
                CommandNodeKind::Root => (0, None),
                CommandNodeKind::Literal { .. } => (1, None),
                CommandNodeKind::Argument { suggestions, .. } => (2, suggestions.as_ref()),
            };
            w.u8(tag
                | (u8::from(node.executable) << 2)
                | (u8::from(node.redirect.is_some()) << 3)
                | (u8::from(suggestions.is_some()) << 4)
                | (u8::from(node.restricted) << 5));
            w.var_i32(node.children.len() as i32);
            for child in &node.children {
                w.var_i32(*child as i32);
                b.check_bytes(&w)?;
            }
            if let Some(redirect) = node.redirect {
                w.var_i32(redirect as i32);
            }
            match &node.kind {
                CommandNodeKind::Root => {}
                CommandNodeKind::Literal { name } => string(&mut w, name, 32767, &b)?,
                CommandNodeKind::Argument { name, parser, .. } => {
                    string(&mut w, name, 32767, &b)?;
                    parser.write(&mut w, version, &b)?;
                }
            }
            if let Some(suggestions) = suggestions {
                write_identifier(&mut w, suggestions.name(), version, &b)?;
            }
            b.check_bytes(&w)?;
        }
        w.var_i32(self.root_index as i32);
        b.check_bytes(&w)?;
        Ok(w.into_inner())
    }
    fn validate_topology(&self) -> Result<()> {
        if !matches!(
            self.nodes
                .get(self.root_index as usize)
                .map(|node| &node.kind),
            Some(CommandNodeKind::Root)
        ) {
            return Err(Error::Invalid("command root index/type"));
        }
        for node in &self.nodes {
            for target in node.children.iter().copied().chain(node.redirect) {
                if target as usize >= self.nodes.len() {
                    return Err(Error::Invalid("command node reference"));
                }
            }
        }
        // Iterative DFS keeps work O(nodes + references) and the call stack
        // constant. Do not combine the two edge kinds when checking cycles.
        let mut colors = vec![0u8; self.nodes.len()];
        let mut stack = Vec::new();
        for redirects in [false, true] {
            colors.fill(0);
            for start in 0..self.nodes.len() {
                if colors[start] != 0 {
                    continue;
                }
                colors[start] = 1;
                stack.push((start, 0usize));
                while let Some((at, edge)) = stack.last_mut() {
                    let node = &self.nodes[*at];
                    let target = if redirects {
                        if *edge == 0 {
                            node.redirect
                        } else {
                            None
                        }
                    } else {
                        node.children.get(*edge).copied()
                    };
                    if let Some(target) = target {
                        *edge += 1;
                        match colors[target as usize] {
                            0 => {
                                colors[target as usize] = 1;
                                stack.push((target as usize, 0));
                            }
                            1 => return Err(Error::Invalid("cyclic command dependency")),
                            _ => {}
                        }
                    } else {
                        colors[*at] = 2;
                        stack.pop();
                    }
                }
            }
        }
        Ok(())
    }
}

impl CommandSuggestions {
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let mut b = Budget::new(limits);
        let (transaction_id, start, length) = (r.var_i32()?, r.var_i32()?, r.var_i32()?);
        let count = b.count(&mut r)?;
        if count > r.remaining().len() / 2 {
            return Err(Error::Eof);
        }
        let mut matches = Vec::with_capacity(count);
        for _ in 0..count {
            let text = r.string(32767)?.to_owned();
            let tooltip = if r.bool()? {
                Some(if version.protocol() < 765 {
                    ChatComponent::read(&mut r, version)?
                } else {
                    ChatComponent::Nbt(
                        inventory::read_nbt(&mut r, RootFormat::Anonymous, &mut b)?
                            .ok_or(Error::Invalid("absent command tooltip NBT"))?,
                    )
                })
            } else {
                None
            };
            matches.push(CommandSuggestion { text, tooltip });
        }
        r.finish()?;
        Ok(Self {
            transaction_id,
            start,
            length,
            matches,
        })
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        let mut b = Budget::new(limits);
        w.var_i32(self.transaction_id);
        w.var_i32(self.start);
        w.var_i32(self.length);
        b.write_count(self.matches.len(), &mut w)?;
        for suggestion in &self.matches {
            string(&mut w, &suggestion.text, 32767, &b)?;
            w.bool(suggestion.tooltip.is_some());
            if let Some(tooltip) = &suggestion.tooltip {
                match (tooltip, version.protocol() >= 765) {
                    (ChatComponent::Json(json), false) => string(&mut w, json, 262144, &b)?,
                    (ChatComponent::Nbt(nbt), true) => {
                        inventory::write_nbt(Some(nbt), &mut w, RootFormat::Anonymous, &mut b)?
                    }
                    _ => {
                        return Err(Error::Invalid(
                            "command tooltip representation for protocol",
                        ))
                    }
                }
            }
            b.check_bytes(&w)?;
        }
        b.check_bytes(&w)?;
        Ok(w.into_inner())
    }
}
impl CommandSuggestionRequest {
    /// Vanilla's limit is 32,500 UTF-16 units; server implementations may impose
    /// a smaller policy limit (for example Paper's 2,048-unit receive limit).
    pub fn decode(bytes: &[u8], _version: Version, limits: Limits) -> Result<Self> {
        let mut r = reader(bytes, limits)?;
        let value = Self {
            transaction_id: r.var_i32()?,
            text: r.string(32500)?.to_owned(),
        };
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, _version: Version, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        w.var_i32(self.transaction_id);
        string(&mut w, &self.text, 32500, &Budget::new(limits))?;
        Ok(w.into_inner())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::named(
            version,
            State::Play,
            "tab_complete",
            self.encode(version, limits)?,
        )
    }
}
