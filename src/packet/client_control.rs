//! Bounded serverbound player/UI controls. These serialize intent, not permissions
//! or game-state changes. Version-specific wire identities remain explicit.
use crate::{
    codec::{BlockPosition, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};
trait Body: Sized {
    fn read_body(r: &mut Reader<'_>, v: Version) -> Result<Self>;
    fn write_body(&self, w: &mut Writer, v: Version) -> Result<()>;
}
macro_rules! packets{
    ($($variant:ident($ty:ty)=>$name:literal),* $(,)?)=>{
        $(impl $ty{
            /// Read one body without advancing the reader on failure.
            pub fn read(r:&mut Reader<'_>,v:Version)->Result<Self>{
                v.packet_id(State::Play,Direction::Serverbound,$name)?;
                let mut body=Reader::new(&r.remaining()[..r.remaining().len().min(r.limits.max_packet)],r.limits);
                let value=Self::read_body(&mut body,v)?;r.take(body.position())?;Ok(value)
            }
            pub fn decode(bytes:&[u8],v:Version,limits:Limits)->Result<Self>{
                if bytes.len()>limits.max_packet{return Err(Error::Limit("client-control packet bytes"));}
                let mut r=Reader::new(bytes,limits);let value=Self::read(&mut r,v)?;r.finish()?;Ok(value)
            }
            pub fn encode(&self,v:Version,limits:Limits)->Result<Vec<u8>>{
                v.packet_id(State::Play,Direction::Serverbound,$name)?;
                let mut w=Writer::new();self.write_body(&mut w,v)?;
                if w.as_slice().len()>limits.max_packet{return Err(Error::Limit("client-control packet bytes"));}Ok(w.into_inner())
            }
            /// Appends only complete bodies, leaving the destination unchanged on error.
            pub fn write(&self,w:&mut Writer,v:Version,limits:Limits)->Result<()>{w.raw(&self.encode(v,limits)?);Ok(())}
            pub fn packet(&self,v:Version,limits:Limits)->Result<RawPacket>{Ok(RawPacket::new(v.packet_id(State::Play,Direction::Serverbound,$name)?,self.encode(v,limits)?))}
        })*
        #[derive(Debug,Clone,PartialEq,Eq)]
        pub enum ClientControlPacket{$($variant($ty)),*}
        impl ClientControlPacket{
            pub fn decode(name:&str,bytes:&[u8],v:Version,limits:Limits)->Result<Self>{Ok(match name{$($name=>Self::$variant(<$ty>::decode(bytes,v,limits)?),)*_=>return Err(Error::Unsupported("client-control packet"))})}
            pub fn encode(&self,v:Version,limits:Limits)->Result<Vec<u8>>{match self{$(Self::$variant(p)=>p.encode(v,limits)),*}}
            pub fn packet(&self,v:Version,limits:Limits)->Result<RawPacket>{match self{$(Self::$variant(p)=>p.packet(v,limits)),*}}
        }
    }
}
packets! {
    Boat(SteerBoat)=>"steer_boat",
    SpectateUuid(Spectate)=>"spectate",
    SpectateEntity(SpectateEntity)=>"spectate_entity",
    SpectatorAction(SpectatorAction)=>"spectator_action",
    PickInventory(PickItem)=>"pick_item",
    PickBlock(PickItemFromBlock)=>"pick_item_from_block",
    PickEntity(PickItemFromEntity)=>"pick_item_from_entity",
    Bundle(SelectBundleItem)=>"select_bundle_item",
    SlotState(SetSlotState)=>"set_slot_state",
    Difficulty(SetDifficulty)=>"set_difficulty",
    DifficultyLock(LockDifficulty)=>"lock_difficulty",
    GameMode(ChangeGameMode)=>"change_gamemode",
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SteerBoat {
    pub left_paddle: bool,
    pub right_paddle: bool,
}
impl Body for SteerBoat {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            left_paddle: r.bool()?,
            right_paddle: r.bool()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.bool(self.left_paddle);
        w.bool(self.right_paddle);
        Ok(())
    }
}
/// UUID-targeted spectator request across all supported releases. At 775 it
/// coexists with `SpectateEntity`, and at 776 with `SpectatorAction`. The pinned
/// 776 schema omits this packet; the independently corrected catalog retains it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spectate {
    pub target_uuid: [u8; 16],
}
impl Body for Spectate {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            target_uuid: r.uuid()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.raw(&self.target_uuid);
        Ok(())
    }
}
/// Protocol 775 only. This request has no optional/absent wire form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpectateEntity {
    pub entity_id: i32,
}
impl Body for SpectateEntity {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.var_i32(self.entity_id);
        Ok(())
    }
}
/// Protocol 776 only: zero marker means absent, other markers subtract one with
/// Java-style wrapping. `Some(-1)` collides with absence and cannot be encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpectatorAction {
    pub entity_id: Option<i32>,
}
impl Body for SpectatorAction {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let n = r.var_i32()?;
        Ok(Self {
            entity_id: (n != 0).then(|| n.wrapping_sub(1)),
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.var_i32(match self.entity_id {
            None => 0,
            Some(-1) => return Err(Error::Invalid("unrepresentable spectator entity")),
            Some(n) => n.wrapping_add(1),
        });
        Ok(())
    }
}
/// Legacy inventory-slot pick request through 768; it does not use a block/entity target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickItem {
    pub slot: i32,
}
impl Body for PickItem {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self { slot: r.var_i32()? })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.var_i32(self.slot);
        Ok(())
    }
}
/// Block-target pick request from 769, including the explicit data-copy flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickItemFromBlock {
    pub position: BlockPosition,
    pub include_data: bool,
}
impl Body for PickItemFromBlock {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            position: BlockPosition::unpack(r.i64()?),
            include_data: r.bool()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.i64(self.position.pack()?);
        w.bool(self.include_data);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickItemFromEntity {
    pub entity_id: i32,
    pub include_data: bool,
}
impl Body for PickItemFromEntity {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            entity_id: r.var_i32()?,
            include_data: r.bool()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.var_i32(self.entity_id);
        w.bool(self.include_data);
        Ok(())
    }
}
/// Bundle selection from 768. Index -1 clears selection; indices below -1 are
/// malformed on the wire. No upper bound is inferred from inventory state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectBundleItem {
    pub slot_id: i32,
    pub selected_index: i32,
}
impl Body for SelectBundleItem {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        let value = Self {
            slot_id: r.var_i32()?,
            selected_index: r.var_i32()?,
        };
        if value.selected_index < -1 {
            return Err(Error::Invalid("bundle selected index"));
        }
        Ok(value)
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        if self.selected_index < -1 {
            return Err(Error::Invalid("bundle selected index"));
        }
        w.var_i32(self.slot_id);
        w.var_i32(self.selected_index);
        Ok(())
    }
}
/// Crafter-slot state from 765. Both IDs are VarInts in every supported family,
/// even where other container packets still used a legacy byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetSlotState {
    pub slot_id: i32,
    pub window_id: i32,
    pub enabled: bool,
}
impl Body for SetSlotState {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            slot_id: r.var_i32()?,
            window_id: r.var_i32()?,
            enabled: r.bool()?,
        })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.var_i32(self.slot_id);
        w.var_i32(self.window_id);
        w.bool(self.enabled);
        Ok(())
    }
}
/// Wire difficulty ID, retained without applying the game's by-ID normalization.
/// The field is an unsigned byte through 770 and a signed VarInt from 771.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetDifficulty {
    pub difficulty: i32,
}
impl Body for SetDifficulty {
    fn read_body(r: &mut Reader<'_>, v: Version) -> Result<Self> {
        Ok(Self {
            difficulty: if v.protocol() < 771 {
                i32::from(r.u8()?)
            } else {
                r.var_i32()?
            },
        })
    }
    fn write_body(&self, w: &mut Writer, v: Version) -> Result<()> {
        if v.protocol() < 771 {
            w.u8(u8::try_from(self.difficulty)
                .map_err(|_| Error::Invalid("legacy difficulty byte"))?);
        } else {
            w.var_i32(self.difficulty);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockDifficulty {
    pub locked: bool,
}
impl Body for LockDifficulty {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self { locked: r.bool()? })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.bool(self.locked);
        Ok(())
    }
}
/// Raw signed mode ID from 771, without normalization or permission decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeGameMode {
    pub mode: i32,
}
impl Body for ChangeGameMode {
    fn read_body(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self { mode: r.var_i32()? })
    }
    fn write_body(&self, w: &mut Writer, _: Version) -> Result<()> {
        w.var_i32(self.mode);
        Ok(())
    }
}
