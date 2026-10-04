//! Clientbound tracked waypoints from protocol 771. This preserves wire data,
//! without rendering, identity/style lookup or client tracking state.
use crate::{
    codec::{identifier, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WaypointOperation {
    Track = 0,
    Untrack = 1,
    Update = 2,
}
impl WaypointOperation {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        match r.var_i32()? {
            0 => Ok(Self::Track),
            1 => Ok(Self::Untrack),
            2 => Ok(Self::Update),
            _ => Err(Error::Invalid("waypoint operation")),
        }
    }
}
/// Named identities are ordinary strings, not resource identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaypointIdentity {
    Uuid([u8; 16]),
    Name(String),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaypointIcon {
    /// Resource identifier spelling is preserved, without asset lookup.
    pub style: String,
    /// Exactly RGB bytes, with no alpha or packed integer normalization.
    pub color: Option<[u8; 3]>,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaypointLocation {
    Empty,
    /// Three signed VarInts, not a packed block position or fixed-width ints.
    Position([i32; 3]),
    Chunk {
        x: i32,
        z: i32,
    },
    /// Raw f32; signed zero, infinities and NaN bits remain unchanged.
    Azimuth(f32),
}
/// Every operation, including Untrack, carries a complete waypoint body.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedWaypoint {
    pub operation: WaypointOperation,
    pub identity: WaypointIdentity,
    pub icon: WaypointIcon,
    pub location: WaypointLocation,
}
fn string(w: &mut Writer, text: &str, limits: Limits) -> Result<()> {
    let cap = limits.max_string_chars.min(32767);
    if text.encode_utf16().count() > cap || text.len() > cap.saturating_mul(3) {
        return Err(Error::Limit("waypoint string length"));
    }
    let mut n = text.len();
    let mut prefix = 1;
    while n > 127 {
        prefix += 1;
        n >>= 7;
    }
    let needed = text
        .len()
        .checked_add(prefix)
        .ok_or(Error::Limit("waypoint string bytes"))?;
    if w.as_slice().len() > limits.max_packet
        || needed > limits.max_packet.saturating_sub(w.as_slice().len())
    {
        return Err(Error::Limit("waypoint packet bytes"));
    }
    w.string(text, cap)
}
impl TrackedWaypoint {
    /// Reads one body transactionally, leaving the reader unchanged on error.
    pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        version.packet_id(State::Play, Direction::Clientbound, "tracked_waypoint")?;
        let limits = r.limits;
        let mut body = Reader::new(
            &r.remaining()[..r.remaining().len().min(limits.max_packet)],
            limits,
        );
        let operation = WaypointOperation::read(&mut body)?;
        let identity = if body.bool()? {
            WaypointIdentity::Uuid(body.uuid()?)
        } else {
            WaypointIdentity::Name(body.string(32767)?.to_owned())
        };
        let style = body.string(32767)?;
        identifier::validate(style, version)?;
        let style = style.to_owned();
        let color = if body.bool()? {
            Some([body.u8()?, body.u8()?, body.u8()?])
        } else {
            None
        };
        let kind = body.var_i32()?;
        let location = match kind {
            0 => WaypointLocation::Empty,
            1 => WaypointLocation::Position([body.var_i32()?, body.var_i32()?, body.var_i32()?]),
            2 => WaypointLocation::Chunk {
                x: body.var_i32()?,
                z: body.var_i32()?,
            },
            3 => WaypointLocation::Azimuth(body.f32()?),
            n if n < 0 => return Err(Error::Invalid("negative waypoint location kind")),
            _ => return Err(Error::Unsupported("waypoint location kind")),
        };
        r.take(body.position())?;
        Ok(Self {
            operation,
            identity,
            icon: WaypointIcon { style, color },
            location,
        })
    }
    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("waypoint packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let value = Self::read(&mut r, version)?;
        r.finish()?;
        Ok(value)
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        version.packet_id(State::Play, Direction::Clientbound, "tracked_waypoint")?;
        let mut w = Writer::new();
        w.var_i32(self.operation as i32);
        match &self.identity {
            WaypointIdentity::Uuid(uuid) => {
                w.bool(true);
                w.raw(uuid);
            }
            WaypointIdentity::Name(name) => {
                w.bool(false);
                string(&mut w, name, limits)?;
            }
        }
        identifier::validate(&self.icon.style, version)?;
        string(&mut w, &self.icon.style, limits)?;
        w.bool(self.icon.color.is_some());
        if let Some(rgb) = self.icon.color {
            w.raw(&rgb);
        }
        match self.location {
            WaypointLocation::Empty => w.var_i32(0),
            WaypointLocation::Position(position) => {
                w.var_i32(1);
                for n in position {
                    w.var_i32(n);
                }
            }
            WaypointLocation::Chunk { x, z } => {
                w.var_i32(2);
                w.var_i32(x);
                w.var_i32(z);
            }
            WaypointLocation::Azimuth(angle) => {
                w.var_i32(3);
                w.f32(angle);
            }
        }
        if w.as_slice().len() > limits.max_packet {
            return Err(Error::Limit("waypoint packet bytes"));
        }
        Ok(w.into_inner())
    }
    /// Appends only a complete body; the destination is unchanged on failure.
    pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.raw(&self.encode(version, limits)?);
        Ok(())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        Ok(RawPacket::new(
            version.packet_id(State::Play, Direction::Clientbound, "tracked_waypoint")?,
            self.encode(version, limits)?,
        ))
    }
}
