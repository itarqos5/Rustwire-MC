use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

/// Serverbound vehicle position/rotation report, separate from the clientbound
/// [`super::super::world_control::VehicleMove`] correction.
///
/// Protocols 763–768 carry 32 bytes; 769–776 append an on-ground boolean.
/// Position and angle bits are preserved, including non-finite values; this is
/// a wire envelope, not validation that a server will accept the reported move.
/// Callers own movement simulation, mounting state and server-side constraints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleMovement {
    pub position: [f64; 3],
    /// Yaw in degrees, not a packed entity angle.
    pub yaw: f32,
    /// Pitch in degrees, not a packed entity angle.
    pub pitch: f32,
    /// Must be `None` through protocol 768 and `Some` beginning with 769.
    /// This is version presence, not an optional field on the wire.
    pub on_ground: Option<bool>,
}

impl VehicleMovement {
    fn body_len(version: Version) -> usize {
        32 + usize::from(version.protocol() >= 769)
    }

    /// Reads one body, leaving the reader unchanged on any error. A following
    /// body may remain in the reader; use [`Self::decode`] to reject trailing data.
    pub fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        version.packet_id(State::Play, Direction::Serverbound, "vehicle_move")?;
        let length = Self::body_len(version);
        if length > r.limits.max_packet {
            return Err(Error::Limit("vehicle movement packet bytes"));
        }
        let bytes = r.remaining().get(..length).ok_or(Error::Eof)?;
        let mut body = Reader::new(bytes, r.limits);
        let value = Self {
            position: [body.f64()?, body.f64()?, body.f64()?],
            yaw: body.f32()?,
            pitch: body.f32()?,
            on_ground: if version.protocol() >= 769 {
                Some(body.bool()?)
            } else {
                None
            },
        };
        r.take(length)?;
        Ok(value)
    }

    pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("vehicle movement packet bytes"));
        }
        let mut r = Reader::new(bytes, limits);
        let value = Self::read(&mut r, version)?;
        r.finish()?;
        Ok(value)
    }

    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        version.packet_id(State::Play, Direction::Serverbound, "vehicle_move")?;
        if self.on_ground.is_some() != (version.protocol() >= 769) {
            return Err(Error::Unsupported("vehicle on-ground version"));
        }
        let length = Self::body_len(version);
        if length > limits.max_packet {
            return Err(Error::Limit("vehicle movement packet bytes"));
        }
        let mut w = Writer::with_capacity(length);
        for value in self.position {
            w.f64(value);
        }
        w.f32(self.yaw);
        w.f32(self.pitch);
        if let Some(on_ground) = self.on_ground {
            w.bool(on_ground);
        }
        Ok(w.into_inner())
    }

    /// Appends one complete body; the writer stays unchanged on error.
    pub fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.raw(&self.encode(version, limits)?);
        Ok(())
    }

    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::super::named(
            version,
            State::Play,
            "vehicle_move",
            self.encode(version, limits)?,
        )
    }
}
