//! Serverbound command-block, jigsaw and structure-edit envelopes. These codecs
//! do not execute commands, generate structures or decide server permissions.
use crate::{
    codec::{identifier, BlockPosition, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};
macro_rules! id_enum{
 ($name:ident,$error:literal,{$($variant:ident=$id:literal),* $(,)?})=>{
  #[derive(Debug,Clone,Copy,PartialEq,Eq)]
  #[repr(i32)]pub enum $name{$($variant=$id),*}
  impl $name{pub fn from_id(id:i32)->Result<Self>{match id{$($id=>Ok(Self::$variant),)*_=>Err(Error::Invalid($error))}}}
 };
}
id_enum!(CommandBlockMode,"command-block mode",{Sequence=0,Auto=1,Redstone=2});
id_enum!(StructureAction,"structure action",{UpdateData=0,SaveArea=1,LoadArea=2,ScanArea=3});
id_enum!(StructureMode,"structure mode",{Save=0,Load=1,Corner=2,Data=3});
id_enum!(StructureMirror,"structure mirror",{None=0,LeftRight=1,FrontBack=2});
id_enum!(StructureRotation,"structure rotation",{None=0,Clockwise90=1,Clockwise180=2,Counterclockwise90=3});
trait Body: Sized {
    fn read_body(r: &mut Reader<'_>, v: Version) -> Result<Self>;
    fn write_body(&self, w: &mut Writer, v: Version, limits: Limits) -> Result<()>;
}
macro_rules! packets{
 ($($variant:ident($ty:ty)=>$name:literal),* $(,)?)=>{
  $(impl $ty{
   /// Reads one body without advancing the reader on failure.
   pub fn read(r:&mut Reader<'_>,v:Version)->Result<Self>{v.packet_id(State::Play,Direction::Serverbound,$name)?;let mut body=Reader::new(&r.remaining()[..r.remaining().len().min(r.limits.max_packet)],r.limits);let value=Self::read_body(&mut body,v)?;r.take(body.position())?;Ok(value)}
   pub fn decode(bytes:&[u8],v:Version,limits:Limits)->Result<Self>{if bytes.len()>limits.max_packet{return Err(Error::Limit("world-edit packet bytes"));}let mut r=Reader::new(bytes,limits);let value=Self::read(&mut r,v)?;r.finish()?;Ok(value)}
   pub fn encode(&self,v:Version,limits:Limits)->Result<Vec<u8>>{v.packet_id(State::Play,Direction::Serverbound,$name)?;let mut w=Writer::new();self.write_body(&mut w,v,limits)?;if w.as_slice().len()>limits.max_packet{return Err(Error::Limit("world-edit packet bytes"));}Ok(w.into_inner())}
   /// Appends only a complete body, leaving the writer unchanged on failure.
   pub fn write(&self,w:&mut Writer,v:Version,limits:Limits)->Result<()>{w.raw(&self.encode(v,limits)?);Ok(())}
   pub fn packet(&self,v:Version,limits:Limits)->Result<RawPacket>{Ok(RawPacket::new(v.packet_id(State::Play,Direction::Serverbound,$name)?,self.encode(v,limits)?))}
  })*
  #[derive(Debug,Clone,PartialEq)]pub enum WorldEditPacket{$($variant($ty)),*}
  impl WorldEditPacket{
   pub fn decode(name:&str,bytes:&[u8],v:Version,limits:Limits)->Result<Self>{Ok(match name{$($name=>Self::$variant(<$ty>::decode(bytes,v,limits)?),)*_=>return Err(Error::Unsupported("world-edit packet"))})}
   pub fn encode(&self,v:Version,limits:Limits)->Result<Vec<u8>>{match self{$(Self::$variant(p)=>p.encode(v,limits)),*}}
   pub fn packet(&self,v:Version,limits:Limits)->Result<RawPacket>{match self{$(Self::$variant(p)=>p.packet(v,limits)),*}}
  }
 };
}
packets! {
 Generate(GenerateStructure)=>"generate_structure",
 Command(UpdateCommandBlock)=>"update_command_block",
 CommandMinecart(UpdateCommandBlockMinecart)=>"update_command_block_minecart",
 Jigsaw(UpdateJigsawBlock)=>"update_jigsaw_block",
 Structure(UpdateStructureBlock)=>"update_structure_block",
}
fn string(w: &mut Writer, s: &str, cap: usize, limits: Limits) -> Result<()> {
    let cap = cap.min(limits.max_string_chars);
    if s.encode_utf16().count() > cap || s.len() > cap.saturating_mul(3) {
        return Err(Error::Limit("world-edit string length"));
    }
    let mut n = s.len();
    let mut prefix = 1;
    while n > 127 {
        prefix += 1;
        n >>= 7;
    }
    let needed = s
        .len()
        .checked_add(prefix)
        .ok_or(Error::Limit("world-edit string bytes"))?;
    if w.as_slice().len() > limits.max_packet
        || needed > limits.max_packet.saturating_sub(w.as_slice().len())
    {
        return Err(Error::Limit("world-edit packet bytes"));
    }
    w.string(s, cap)
}
fn key(r: &mut Reader<'_>, v: Version) -> Result<String> {
    let key = r.string(32767)?;
    identifier::validate(key, v)?;
    Ok(key.to_owned())
}
fn write_key(w: &mut Writer, key: &str, v: Version, limits: Limits) -> Result<()> {
    identifier::validate(key, v)?;
    string(w, key, 32767, limits)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerateStructure {
    pub position: BlockPosition,
    pub levels: i32,
    pub keep_jigsaws: bool,
}
impl Body for GenerateStructure {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            levels: r.var_i32()?,
            keep_jigsaws: r.bool()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.i64(self.position.pack()?);
        w.var_i32(self.levels);
        w.bool(self.keep_jigsaws);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCommandBlock {
    pub position: BlockPosition,
    pub command: String,
    pub mode: CommandBlockMode,
    pub flags: u8,
}
impl UpdateCommandBlock {
    pub const TRACK_OUTPUT: u8 = 1;
    pub const CONDITIONAL: u8 = 2;
    pub const AUTOMATIC: u8 = 4;
}
impl Body for UpdateCommandBlock {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            command: r.string(32767)?.to_owned(),
            mode: CommandBlockMode::from_id(r.var_i32()?)?,
            flags: r.u8()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        w.i64(self.position.pack()?);
        string(w, &self.command, 32767, limits)?;
        w.var_i32(self.mode as i32);
        w.u8(self.flags);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCommandBlockMinecart {
    pub entity_id: i32,
    pub command: String,
    pub track_output: bool,
}
impl Body for UpdateCommandBlockMinecart {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            command: r.string(32767)?.to_owned(),
            track_output: r.bool()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        w.var_i32(self.entity_id);
        string(w, &self.command, 32767, limits)?;
        w.bool(self.track_output);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JigsawPriorities {
    pub selection: i32,
    pub placement: i32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateJigsawBlock {
    pub position: BlockPosition,
    pub name: String,
    pub target: String,
    pub pool: String,
    /// Unparsed block-state expression.
    pub final_state: String,
    /// Raw wire spelling; unknown names are not normalized to a game default.
    pub joint_type: String,
    /// None through 764, required Some from 765.
    pub priorities: Option<JigsawPriorities>,
}
impl Body for UpdateJigsawBlock {
    fn read_body(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            name: key(r, v)?,
            target: key(r, v)?,
            pool: key(r, v)?,
            final_state: r.string(32767)?.to_owned(),
            joint_type: r.string(32767)?.to_owned(),
            priorities: if v.protocol() >= 765 {
                Some(JigsawPriorities {
                    selection: r.var_i32()?,
                    placement: r.var_i32()?,
                })
            } else {
                None
            },
        })
    }
    fn write_body(&self, w: &mut Writer, v: Version, limits: Limits) -> Result<()> {
        if self.priorities.is_some() != (v.protocol() >= 765) {
            return Err(Error::Invalid("jigsaw priorities version"));
        }
        w.i64(self.position.pack()?);
        for key in [&self.name, &self.target, &self.pool] {
            write_key(w, key, v, limits)?;
        }
        string(w, &self.final_state, 32767, limits)?;
        string(w, &self.joint_type, 32767, limits)?;
        if let Some(p) = self.priorities {
            w.var_i32(p.selection);
            w.var_i32(p.placement);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStructureBlock {
    pub position: BlockPosition,
    pub action: StructureAction,
    pub mode: StructureMode,
    pub name: String,
    /// Raw signed-byte coordinates/sizes, without the game's post-read clamping.
    pub offset: [i8; 3],
    pub size: [i8; 3],
    pub mirror: StructureMirror,
    pub rotation: StructureRotation,
    /// Plain wire string, at most 128 UTF-16 units.
    pub metadata: String,
    /// Raw f32, without gameplay normalization; all float bits survive.
    pub integrity: f32,
    /// Full signed VarLong in every supported family, despite the schema's varint annotation.
    pub seed: i64,
    /// Raw flag byte. Bits 0/1/2: ignore entities/show air/show box; bit 3: strict from 770.
    pub flags: u8,
}
impl UpdateStructureBlock {
    pub const IGNORE_ENTITIES: u8 = 1;
    pub const SHOW_AIR: u8 = 2;
    pub const SHOW_BOUNDING_BOX: u8 = 4;
    pub const STRICT: u8 = 8;
}
impl Body for UpdateStructureBlock {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            action: StructureAction::from_id(r.var_i32()?)?,
            mode: StructureMode::from_id(r.var_i32()?)?,
            name: r.string(32767)?.to_owned(),
            offset: [r.u8()? as i8, r.u8()? as i8, r.u8()? as i8],
            size: [r.u8()? as i8, r.u8()? as i8, r.u8()? as i8],
            mirror: StructureMirror::from_id(r.var_i32()?)?,
            rotation: StructureRotation::from_id(r.var_i32()?)?,
            metadata: r.string(128)?.to_owned(),
            integrity: r.f32()?,
            seed: r.var_i64()?,
            flags: r.u8()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        w.i64(self.position.pack()?);
        w.var_i32(self.action as i32);
        w.var_i32(self.mode as i32);
        string(w, &self.name, 32767, limits)?;
        for n in self.offset {
            w.u8(n as u8);
        }
        for n in self.size {
            w.u8(n as u8);
        }
        w.var_i32(self.mirror as i32);
        w.var_i32(self.rotation as i32);
        string(w, &self.metadata, 128, limits)?;
        w.f32(self.integrity);
        w.var_i64(self.seed);
        w.u8(self.flags);
        Ok(())
    }
}
