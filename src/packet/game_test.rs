//! Game-test editor and status wire envelopes. No test, world edit, export,
//! file operation, rendering or permission decision is performed by these codecs.
pub use super::world_edit::StructureRotation;
use crate::{
    codec::{identifier, BlockPosition, Reader, Writer},
    frame::RawPacket,
    packet::chat::ChatComponent,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

macro_rules! id_enum {
    ($name:ident, $error:literal, {$($variant:ident=$id:literal),* $(,)?}) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(i32)]
        pub enum $name { $($variant=$id),* }
        impl $name {
            pub fn from_id(id: i32) -> Result<Self> {
                match id { $($id=>Ok(Self::$variant),)* _=>Err(Error::Invalid($error)) }
            }
        }
    };
}
id_enum!(TestBlockMode,"test-block mode",{Start=0,Log=1,Fail=2,Accept=3});
id_enum!(TestInstanceActionKind,"test-instance action",{Init=0,Query=1,Set=2,Reset=3,Save=4,Export=5,Run=6});
id_enum!(TestInstanceRunStatus,"test-instance status",{Cleared=0,Running=1,Finished=2});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameTestHighlightPos {
    pub absolute_position: BlockPosition,
    pub relative_position: BlockPosition,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TestInstanceBlockStatus {
    pub status: ChatComponent,
    /// Three signed VarInts, not three fixed-width integers or a packed position.
    pub size: Option<[i32; 3]>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTestBlock {
    pub position: BlockPosition,
    pub mode: TestBlockMode,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TestInstanceData {
    /// An optional test-instance registry key; no registry lookup occurs.
    pub test: Option<String>,
    pub size: [i32; 3],
    pub rotation: StructureRotation,
    pub ignore_entities: bool,
    pub status: TestInstanceRunStatus,
    pub error_message: Option<ChatComponent>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TestInstanceBlockAction {
    pub position: BlockPosition,
    pub action: TestInstanceActionKind,
    pub data: TestInstanceData,
}
fn available(w: &Writer, n: usize, limits: Limits) -> Result<()> {
    if w.as_slice().len() > limits.max_packet
        || n > limits.max_packet.saturating_sub(w.as_slice().len())
    {
        return Err(Error::Limit("game-test packet bytes"));
    }
    Ok(())
}
fn prefix_len(mut n: u32) -> usize {
    let mut size = 1;
    while n > 127 {
        size += 1;
        n >>= 7;
    }
    size
}
fn varint(w: &mut Writer, n: i32, limits: Limits) -> Result<()> {
    available(w, prefix_len(n as u32), limits)?;
    w.var_i32(n);
    Ok(())
}
fn boolean(w: &mut Writer, value: bool, limits: Limits) -> Result<()> {
    available(w, 1, limits)?;
    w.bool(value);
    Ok(())
}
fn position(w: &mut Writer, p: BlockPosition, limits: Limits) -> Result<()> {
    let packed = p.pack()?;
    available(w, 8, limits)?;
    w.i64(packed);
    Ok(())
}
fn string(w: &mut Writer, s: &str, limits: Limits) -> Result<()> {
    let cap = 32767.min(limits.max_string_chars);
    if s.len() > cap.saturating_mul(3) || s.encode_utf16().count() > cap {
        return Err(Error::Limit("game-test string length"));
    }
    available(w, s.len() + prefix_len(s.len() as u32), limits)?;
    w.string(s, cap)
}
fn component(
    w: &mut Writer,
    value: &ChatComponent,
    version: Version,
    limits: Limits,
) -> Result<()> {
    available(w, 0, limits)?;
    value.write(
        w,
        version,
        Limits {
            max_packet: limits.max_packet - w.as_slice().len(),
            ..limits
        },
    )
}
fn vector(r: &mut Reader<'_>) -> Result<[i32; 3]> {
    Ok([r.var_i32()?, r.var_i32()?, r.var_i32()?])
}
fn write_vector(w: &mut Writer, v: [i32; 3], limits: Limits) -> Result<()> {
    for n in v {
        varint(w, n, limits)?;
    }
    Ok(())
}
trait Body: Sized {
    fn read_body(r: &mut Reader<'_>, version: Version) -> Result<Self>;
    fn write_body(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()>;
}
macro_rules! packets {
    ($($variant:ident($ty:ty)=>($name:literal,$direction:ident)),* $(,)?) => {
        $(impl $ty {
            /// Read one body without advancing the reader on failure.
            pub fn read(r: &mut Reader<'_>,version: Version)->Result<Self> {
                version.packet_id(State::Play,Direction::$direction,$name)?;
                let mut body=Reader::new(&r.remaining()[..r.remaining().len().min(r.limits.max_packet)],r.limits);
                let value=Self::read_body(&mut body,version)?;r.take(body.position())?;Ok(value)
            }
            pub fn decode(bytes: &[u8],version: Version,limits: Limits)->Result<Self> {
                if bytes.len()>limits.max_packet { return Err(Error::Limit("game-test packet bytes")); }
                let mut r=Reader::new(bytes,limits);let value=Self::read(&mut r,version)?;r.finish()?;Ok(value)
            }
            pub fn encode(&self,version: Version,limits: Limits)->Result<Vec<u8>> {
                version.packet_id(State::Play,Direction::$direction,$name)?;
                let mut w=Writer::new();self.write_body(&mut w,version,limits)?;available(&w,0,limits)?;Ok(w.into_inner())
            }
            /// Append only a complete body; failure leaves the writer unchanged.
            pub fn write(&self,w: &mut Writer,version: Version,limits: Limits)->Result<()> { w.raw(&self.encode(version,limits)?);Ok(()) }
            pub fn packet(&self,version: Version,limits: Limits)->Result<RawPacket> {
                Ok(RawPacket::new(version.packet_id(State::Play,Direction::$direction,$name)?,self.encode(version,limits)?))
            }
        })*
        #[derive(Debug,Clone,PartialEq)]pub enum GameTestPacket { $($variant($ty)),* }
        impl GameTestPacket {
            pub fn decode(name: &str,bytes: &[u8],version: Version,limits: Limits)->Result<Self> {
                Ok(match name { $($name=>Self::$variant(<$ty>::decode(bytes,version,limits)?),)* _=>return Err(Error::Unsupported("game-test packet")) })
            }
            pub fn encode(&self,version: Version,limits: Limits)->Result<Vec<u8>> { match self { $(Self::$variant(p)=>p.encode(version,limits)),* } }
            pub fn packet(&self,version: Version,limits: Limits)->Result<RawPacket> { match self { $(Self::$variant(p)=>p.packet(version,limits)),* } }
        }
    };
}
packets! {
    Highlight(GameTestHighlightPos)=>("game_test_highlight_pos",Clientbound),
    Status(TestInstanceBlockStatus)=>("test_instance_block_status",Clientbound),
    SetBlock(SetTestBlock)=>("set_test_block",Serverbound),
    Action(TestInstanceBlockAction)=>("test_instance_block_action",Serverbound),
}
impl Body for GameTestHighlightPos {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            absolute_position: BlockPosition::unpack(r.i64()?),
            relative_position: BlockPosition::unpack(r.i64()?),
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        position(w, self.absolute_position, limits)?;
        position(w, self.relative_position, limits)
    }
}
impl Body for TestInstanceBlockStatus {
    fn read_body(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            status: ChatComponent::read(r, version)?,
            size: if r.bool()? { Some(vector(r)?) } else { None },
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        component(w, &self.status, version, limits)?;
        boolean(w, self.size.is_some(), limits)?;
        if let Some(size) = self.size {
            write_vector(w, size, limits)?;
        }
        Ok(())
    }
}
impl Body for SetTestBlock {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            mode: TestBlockMode::from_id(r.var_i32()?)?,
            message: r.string(32767)?.into(),
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version, limits: Limits) -> Result<()> {
        position(w, self.position, limits)?;
        varint(w, self.mode as i32, limits)?;
        string(w, &self.message, limits)
    }
}
impl TestInstanceData {
    fn read_body(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let test = if r.bool()? {
            let s = r.string(32767)?;
            identifier::validate(s, version)?;
            Some(s.to_owned())
        } else {
            None
        };
        Ok(Self {
            test,
            size: vector(r)?,
            rotation: StructureRotation::from_id(r.var_i32()?)?,
            ignore_entities: r.bool()?,
            status: TestInstanceRunStatus::from_id(r.var_i32()?)?,
            error_message: if r.bool()? {
                Some(ChatComponent::read(r, version)?)
            } else {
                None
            },
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        boolean(w, self.test.is_some(), limits)?;
        if let Some(test) = &self.test {
            identifier::validate(test, version)?;
            string(w, test, limits)?;
        }
        write_vector(w, self.size, limits)?;
        varint(w, self.rotation as i32, limits)?;
        boolean(w, self.ignore_entities, limits)?;
        varint(w, self.status as i32, limits)?;
        boolean(w, self.error_message.is_some(), limits)?;
        if let Some(error) = &self.error_message {
            component(w, error, version, limits)?;
        }
        Ok(())
    }
}
impl Body for TestInstanceBlockAction {
    fn read_body(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            action: TestInstanceActionKind::from_id(r.var_i32()?)?,
            data: TestInstanceData::read_body(r, version)?,
        })
    }
    fn write_body(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        position(w, self.position, limits)?;
        varint(w, self.action as i32, limits)?;
        self.data.write_body(w, version, limits)
    }
}
