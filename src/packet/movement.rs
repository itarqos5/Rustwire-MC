//! The four ordinary serverbound player movement forms.
//!
//! This module encodes reported positions and rotations. It does not simulate
//! gravity, collision, friction, input, or a 20 Hz client clock; callers own
//! those behaviors. Vehicle movement uses separate packets.
use crate::{
    codec::{Reader, Writer},
    frame::RawPacket,
    version::State,
    Error, Limits, Result, Version,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerMovement {
    pub position: Option<[f64; 3]>,
    /// Yaw and pitch in degrees, as floating point (not packed entity angles).
    pub rotation: Option<[f32; 2]>,
    pub on_ground: bool,
    /// Available beginning with protocol 768.
    pub horizontal_collision: bool,
}
impl PlayerMovement {
    pub fn name(&self) -> &'static str {
        match (self.position.is_some(), self.rotation.is_some()) {
            (true, true) => "position_look",
            (true, false) => "position",
            (false, true) => "look",
            (false, false) => "flying",
        }
    }
    pub fn decode(name: &str, bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_packet {
            return Err(Error::Limit("movement packet bytes"));
        }
        let (position, rotation) = match name {
            "position_look" => (true, true),
            "position" => (true, false),
            "look" => (false, true),
            "flying" => (false, false),
            _ => return Err(Error::Unsupported("player movement packet")),
        };
        let mut r = Reader::new(bytes, limits);
        let position = if position {
            Some([r.f64()?, r.f64()?, r.f64()?])
        } else {
            None
        };
        let rotation = if rotation {
            Some([r.f32()?, r.f32()?])
        } else {
            None
        };
        let flags = if version.protocol() < 768 {
            u8::from(r.bool()?)
        } else {
            r.u8()?
        };
        if flags & !3 != 0 {
            return Err(Error::Invalid("reserved player movement flags"));
        }
        r.finish()?;
        let value = Self {
            position,
            rotation,
            on_ground: flags & 1 != 0,
            horizontal_collision: flags & 2 != 0,
        };
        value.validate(version)?;
        Ok(value)
    }
    fn validate(&self, version: Version) -> Result<()> {
        if self
            .position
            .is_some_and(|v| v.iter().any(|x| !x.is_finite()))
            || self
                .rotation
                .is_some_and(|v| v.iter().any(|x| !x.is_finite()))
        {
            return Err(Error::Invalid("non-finite player movement"));
        }
        if self.horizontal_collision && version.protocol() < 768 {
            return Err(Error::Unsupported(
                "horizontal collision flag before 1.21.2",
            ));
        }
        Ok(())
    }
    pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
        self.validate(version)?;
        let length = 1
            + usize::from(self.position.is_some()) * 24
            + usize::from(self.rotation.is_some()) * 8;
        if length > limits.max_packet {
            return Err(Error::Limit("movement packet bytes"));
        }
        let mut w = Writer::with_capacity(length);
        if let Some(v) = self.position {
            for x in v {
                w.f64(x);
            }
        }
        if let Some(v) = self.rotation {
            for x in v {
                w.f32(x);
            }
        }
        w.u8(u8::from(self.on_ground) | (u8::from(self.horizontal_collision) << 1));
        Ok(w.into_inner())
    }
    pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
        super::named(
            version,
            State::Play,
            self.name(),
            self.encode(version, limits)?,
        )
    }
}
