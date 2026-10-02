//! Bounded big-endian Java NBT, including Java's modified UTF-8.
//!
//! Strings retain UTF-16 code units, so even unpaired Java surrogates round-trip.
//! Network roots are named through protocol 763 and anonymous from 764 onward.
//! A node budget charges tags and primitive-array elements; collection, string,
//! byte-size and recursion limits are also enforced. Compound order/duplicates
//! and empty-list element types are preserved.
use crate::{
    codec::{Reader, Writer},
    Error, Limits, Result, Version,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootFormat {
    Named,
    Anonymous,
}
impl RootFormat {
    pub fn for_version(version: Version) -> Self {
        if version.protocol() < 764 {
            Self::Named
        } else {
            Self::Anonymous
        }
    }
}

/// Lossless Java string. Use `to_string()` for a checked Rust UTF-8 conversion.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NbtString(pub Vec<u16>);
impl NbtString {
    pub fn to_string(&self) -> std::result::Result<String, std::string::FromUtf16Error> {
        String::from_utf16(&self.0)
    }
    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.0)
    }
    pub fn equals(&self, s: &str) -> bool {
        self.0.iter().copied().eq(s.encode_utf16())
    }
}
impl From<&str> for NbtString {
    fn from(s: &str) -> Self {
        Self(s.encode_utf16().collect())
    }
}
impl From<String> for NbtString {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TagType {
    End = 0,
    Byte = 1,
    Short = 2,
    Int = 3,
    Long = 4,
    Float = 5,
    Double = 6,
    ByteArray = 7,
    String = 8,
    List = 9,
    Compound = 10,
    IntArray = 11,
    LongArray = 12,
}
impl TryFrom<u8> for TagType {
    type Error = Error;
    fn try_from(id: u8) -> Result<Self> {
        Ok(match id {
            0 => Self::End,
            1 => Self::Byte,
            2 => Self::Short,
            3 => Self::Int,
            4 => Self::Long,
            5 => Self::Float,
            6 => Self::Double,
            7 => Self::ByteArray,
            8 => Self::String,
            9 => Self::List,
            10 => Self::Compound,
            11 => Self::IntArray,
            12 => Self::LongArray,
            _ => return Err(Error::Invalid("NBT tag type")),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(NbtString),
    List {
        element_type: TagType,
        elements: Vec<Tag>,
    },
    Compound(Vec<(NbtString, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}
impl Tag {
    pub fn tag_type(&self) -> TagType {
        match self {
            Self::Byte(_) => TagType::Byte,
            Self::Short(_) => TagType::Short,
            Self::Int(_) => TagType::Int,
            Self::Long(_) => TagType::Long,
            Self::Float(_) => TagType::Float,
            Self::Double(_) => TagType::Double,
            Self::ByteArray(_) => TagType::ByteArray,
            Self::String(_) => TagType::String,
            Self::List { .. } => TagType::List,
            Self::Compound(_) => TagType::Compound,
            Self::IntArray(_) => TagType::IntArray,
            Self::LongArray(_) => TagType::LongArray,
        }
    }
    /// Last matching entry, consistent with Java compound-map replacement.
    pub fn get(&self, key: &str) -> Option<&Tag> {
        if let Self::Compound(entries) = self {
            entries
                .iter()
                .rev()
                .find(|(k, _)| k.equals(key))
                .map(|(_, v)| v)
        } else {
            None
        }
    }
    pub fn as_i32(&self) -> Option<i32> {
        if let Self::Int(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    pub fn as_str(&self) -> Option<&NbtString> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Nbt {
    pub name: Option<NbtString>,
    pub root: Tag,
}
impl Nbt {
    pub fn anonymous(root: Tag) -> Self {
        Self { name: None, root }
    }
    /// Reads one root. TAG_End denotes an absent optional root.
    pub fn read(reader: &mut Reader<'_>, format: RootFormat) -> Result<Option<Self>> {
        // Restrict the view before parsing, so nested allocations cannot exceed
        // the byte budget even when the parent Reader covers a larger stream.
        let available = reader.remaining();
        let mut bounded = Reader::new(
            &available[..available.len().min(reader.limits.max_packet)],
            reader.limits,
        );
        let kind = TagType::try_from(bounded.u8()?)?;
        if kind == TagType::End {
            reader.take(1)?;
            return Ok(None);
        }
        let mut budget = Budget::new(reader.limits);
        let name = if format == RootFormat::Named {
            Some(read_string(&mut bounded)?)
        } else {
            None
        };
        let root = read_tag(&mut bounded, kind, 0, &mut budget)?;
        let consumed = bounded.position();
        reader.take(consumed)?;
        Ok(Some(Self { name, root }))
    }
    pub fn decode(bytes: &[u8], format: RootFormat, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("NBT byte size"));
        }
        let mut r = Reader::new(bytes, limits);
        let n = Self::read(&mut r, format)?.ok_or(Error::Invalid("absent NBT root"))?;
        r.finish()?;
        Ok(n)
    }
    pub fn write(&self, writer: &mut Writer, format: RootFormat, limits: Limits) -> Result<()> {
        // Encode separately: an invalid value cannot leave a partial root in a packet.
        let mut out = Writer::new();
        let mut budget = Budget::new(limits);
        out.u8(self.root.tag_type() as u8);
        if format == RootFormat::Named {
            write_string(
                &mut out,
                self.name.as_ref().unwrap_or(&NbtString::default()),
                limits,
            )?;
        }
        write_tag(&mut out, &self.root, 0, &mut budget)?;
        if out.as_slice().len() > limits.max_packet {
            return Err(Error::Limit("NBT byte size"));
        }
        writer.raw(out.as_slice());
        Ok(())
    }
    pub fn encode(&self, format: RootFormat, limits: Limits) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        self.write(&mut w, format, limits)?;
        Ok(w.into_inner())
    }
}

struct Budget {
    limits: Limits,
    used: usize,
}
impl Budget {
    fn new(limits: Limits) -> Self {
        Self { limits, used: 0 }
    }
    fn charge(&mut self, n: usize) -> Result<()> {
        self.used = self.used.checked_add(n).ok_or(Error::Limit("NBT nodes"))?;
        if self.used > self.limits.max_nbt_nodes {
            Err(Error::Limit("NBT nodes"))
        } else {
            Ok(())
        }
    }
    fn depth(&self, depth: usize) -> Result<()> {
        if depth > self.limits.max_nbt_depth.min(512) {
            Err(Error::Limit("NBT depth"))
        } else {
            Ok(())
        }
    }
    fn collection(&self, n: usize) -> Result<()> {
        if n > self.limits.max_collection || n > i32::MAX as usize {
            Err(Error::Limit("NBT collection"))
        } else {
            Ok(())
        }
    }
}
fn length(r: &mut Reader<'_>, budget: &Budget) -> Result<usize> {
    let n = r.i32()?;
    if n < 0 {
        return Err(Error::Invalid("negative NBT array length"));
    }
    budget.collection(n as usize)?;
    Ok(n as usize)
}
fn read_string(r: &mut Reader<'_>) -> Result<NbtString> {
    let len = r.u16()? as usize;
    let bytes = r.take(len)?;
    let mut units = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let a = bytes[i];
        i += 1;
        let unit = match a {
            0x01..=0x7f => u16::from(a),
            0xc0..=0xdf => {
                let b = *bytes
                    .get(i)
                    .ok_or(Error::Invalid("truncated NBT modified UTF-8"))?;
                i += 1;
                if b & 0xc0 != 0x80 {
                    return Err(Error::Invalid("NBT modified UTF-8 continuation"));
                }
                let value = (u16::from(a & 0x1f) << 6) | u16::from(b & 0x3f);
                if value < 0x80 && !(a == 0xc0 && b == 0x80) {
                    return Err(Error::Invalid("overlong NBT modified UTF-8"));
                }
                value
            }
            0xe0..=0xef => {
                let b = *bytes
                    .get(i)
                    .ok_or(Error::Invalid("truncated NBT modified UTF-8"))?;
                let c = *bytes
                    .get(i + 1)
                    .ok_or(Error::Invalid("truncated NBT modified UTF-8"))?;
                i += 2;
                if b & 0xc0 != 0x80 || c & 0xc0 != 0x80 {
                    return Err(Error::Invalid("NBT modified UTF-8 continuation"));
                }
                let value =
                    (u16::from(a & 15) << 12) | (u16::from(b & 63) << 6) | u16::from(c & 63);
                if value < 0x800 {
                    return Err(Error::Invalid("overlong NBT modified UTF-8"));
                }
                value
            }
            _ => return Err(Error::Invalid("NBT modified UTF-8 leading byte")),
        };
        if units.len() >= r.limits.max_string_chars {
            return Err(Error::Limit("NBT string length"));
        }
        units.push(unit);
    }
    Ok(NbtString(units))
}
fn write_string(w: &mut Writer, s: &NbtString, limits: Limits) -> Result<()> {
    if s.0.len() > limits.max_string_chars {
        return Err(Error::Limit("NBT string length"));
    }
    let len =
        s.0.iter()
            .try_fold(0usize, |n, &u| {
                n.checked_add(if u == 0 {
                    2
                } else if u < 0x80 {
                    1
                } else if u < 0x800 {
                    2
                } else {
                    3
                })
            })
            .ok_or(Error::Limit("NBT string length"))?;
    if len > u16::MAX as usize {
        return Err(Error::Limit("NBT modified UTF-8 byte length"));
    }
    w.u16(len as u16);
    for &u in &s.0 {
        if (1..0x80).contains(&u) {
            w.u8(u as u8);
        } else if u < 0x800 {
            w.u8(0xc0 | (u >> 6) as u8);
            w.u8(0x80 | (u & 63) as u8);
        } else {
            w.u8(0xe0 | (u >> 12) as u8);
            w.u8(0x80 | ((u >> 6) & 63) as u8);
            w.u8(0x80 | (u & 63) as u8);
        }
    }
    Ok(())
}
fn read_tag(r: &mut Reader<'_>, kind: TagType, depth: usize, b: &mut Budget) -> Result<Tag> {
    b.depth(depth)?;
    b.charge(1)?;
    Ok(match kind {
        TagType::End => return Err(Error::Invalid("NBT End payload")),
        TagType::Byte => Tag::Byte(r.u8()? as i8),
        TagType::Short => Tag::Short(r.i16()?),
        TagType::Int => Tag::Int(r.i32()?),
        TagType::Long => Tag::Long(r.i64()?),
        TagType::Float => Tag::Float(r.f32()?),
        TagType::Double => Tag::Double(r.f64()?),
        TagType::String => Tag::String(read_string(r)?),
        TagType::ByteArray => {
            let n = length(r, b)?;
            b.charge(n)?;
            Tag::ByteArray(r.take(n)?.iter().map(|x| *x as i8).collect())
        }
        TagType::IntArray => {
            let n = length(r, b)?;
            b.charge(n)?;
            let raw = r.take(n.checked_mul(4).ok_or(Error::Limit("NBT array bytes"))?)?;
            Tag::IntArray(
                raw.as_chunks::<4>()
                    .0
                    .iter()
                    .map(|x| i32::from_be_bytes(*x))
                    .collect(),
            )
        }
        TagType::LongArray => {
            let n = length(r, b)?;
            b.charge(n)?;
            let raw = r.take(n.checked_mul(8).ok_or(Error::Limit("NBT array bytes"))?)?;
            Tag::LongArray(
                raw.as_chunks::<8>()
                    .0
                    .iter()
                    .map(|x| i64::from_be_bytes(*x))
                    .collect(),
            )
        }
        TagType::List => {
            let element_type = TagType::try_from(r.u8()?)?;
            let n = length(r, b)?;
            if n > 0 && element_type == TagType::End {
                return Err(Error::Invalid("nonempty NBT End list"));
            }
            if n > b.limits.max_nbt_nodes.saturating_sub(b.used) {
                return Err(Error::Limit("NBT nodes"));
            }
            let mut elements = Vec::new();
            for _ in 0..n {
                elements.push(read_tag(r, element_type, depth + 1, b)?);
            }
            Tag::List {
                element_type,
                elements,
            }
        }
        TagType::Compound => {
            let mut entries = Vec::new();
            loop {
                let kind = TagType::try_from(r.u8()?)?;
                if kind == TagType::End {
                    break;
                }
                b.collection(entries.len() + 1)?;
                let name = read_string(r)?;
                entries.push((name, read_tag(r, kind, depth + 1, b)?));
            }
            Tag::Compound(entries)
        }
    })
}
fn write_tag(w: &mut Writer, tag: &Tag, depth: usize, b: &mut Budget) -> Result<()> {
    b.depth(depth)?;
    b.charge(1)?;
    match tag {
        Tag::Byte(v) => w.u8(*v as u8),
        Tag::Short(v) => w.i16(*v),
        Tag::Int(v) => w.i32(*v),
        Tag::Long(v) => w.i64(*v),
        Tag::Float(v) => w.f32(*v),
        Tag::Double(v) => w.f64(*v),
        Tag::String(v) => write_string(w, v, b.limits)?,
        Tag::ByteArray(v) => {
            b.collection(v.len())?;
            b.charge(v.len())?;
            w.i32(v.len() as i32);
            for x in v {
                w.u8(*x as u8);
            }
        }
        Tag::IntArray(v) => {
            b.collection(v.len())?;
            b.charge(v.len())?;
            w.i32(v.len() as i32);
            for x in v {
                w.i32(*x);
            }
        }
        Tag::LongArray(v) => {
            b.collection(v.len())?;
            b.charge(v.len())?;
            w.i32(v.len() as i32);
            for x in v {
                w.i64(*x);
            }
        }
        Tag::List {
            element_type,
            elements,
        } => {
            b.collection(elements.len())?;
            if elements.iter().any(|t| t.tag_type() != *element_type) {
                return Err(Error::Invalid("heterogeneous NBT list"));
            }
            w.u8(*element_type as u8);
            w.i32(elements.len() as i32);
            for item in elements {
                write_tag(w, item, depth + 1, b)?;
            }
        }
        Tag::Compound(entries) => {
            b.collection(entries.len())?;
            for (name, item) in entries {
                w.u8(item.tag_type() as u8);
                write_string(w, name, b.limits)?;
                write_tag(w, item, depth + 1, b)?;
            }
            w.u8(0);
        }
    }
    if w.as_slice().len() > b.limits.max_packet {
        return Err(Error::Limit("NBT byte size"));
    }
    Ok(())
}
