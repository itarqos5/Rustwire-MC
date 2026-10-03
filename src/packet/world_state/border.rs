//! World-border scalar envelopes. Durations are VarLong, not schema VarInt.
use super::*;

/// World-border interpolation switched from wall-clock milliseconds to game
/// ticks in 1.21.11 (protocol 774). Both use a signed VarLong on the wire.
/// Values are preserved verbatim; conversion would depend on the server tick
/// rate and would lose information for negative or very large wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderDuration {
    /// Protocols 763–773 (1.20 through 1.21.10).
    Milliseconds(i64),
    /// Protocols 774–776 (1.21.11 through 26.2).
    Ticks(i64),
}
impl BorderDuration {
    /// Interpret a raw wire duration using the target release's unit.
    pub fn from_wire(value: i64, version: Version) -> Self {
        if version.protocol() >= 774 {
            Self::Ticks(value)
        } else {
            Self::Milliseconds(value)
        }
    }
    /// The signed VarLong value, without converting or dropping its unit.
    pub fn raw_value(self) -> i64 {
        match self {
            Self::Milliseconds(value) | Self::Ticks(value) => value,
        }
    }
    fn for_version(self, version: Version) -> Result<i64> {
        match self {
            Self::Milliseconds(value) if version.protocol() < 774 => Ok(value),
            Self::Ticks(value) if version.protocol() >= 774 => Ok(value),
            _ => Err(Error::Invalid("world-border duration unit for protocol")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitializeWorldBorder {
    pub x: f64,
    pub z: f64,
    pub old_diameter: f64,
    pub new_diameter: f64,
    /// Signed duration in the release-specific wire unit; no gameplay policy is imposed.
    pub duration: BorderDuration,
    pub portal_teleport_boundary: i32,
    pub warning_blocks: i32,
    pub warning_time: i32,
}
impl Body for InitializeWorldBorder {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            x: r.f64()?,
            z: r.f64()?,
            old_diameter: r.f64()?,
            new_diameter: r.f64()?,
            duration: BorderDuration::from_wire(r.var_i64()?, version),
            portal_teleport_boundary: r.var_i32()?,
            warning_blocks: r.var_i32()?,
            warning_time: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, _: Limits) -> Result<()> {
        w.f64(self.x);
        w.f64(self.z);
        w.f64(self.old_diameter);
        w.f64(self.new_diameter);
        w.var_i64(self.duration.for_version(version)?);
        w.var_i32(self.portal_teleport_boundary);
        w.var_i32(self.warning_blocks);
        w.var_i32(self.warning_time);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBorderCenter {
    pub x: f64,
    pub z: f64,
}
impl Body for WorldBorderCenter {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            x: r.f64()?,
            z: r.f64()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.f64(self.x);
        w.f64(self.z);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBorderLerpSize {
    pub old_diameter: f64,
    pub new_diameter: f64,
    pub duration: BorderDuration,
}
impl Body for WorldBorderLerpSize {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        Ok(Self {
            old_diameter: r.f64()?,
            new_diameter: r.f64()?,
            duration: BorderDuration::from_wire(r.var_i64()?, version),
        })
    }
    fn write(&self, w: &mut Writer, version: Version, _: Limits) -> Result<()> {
        w.f64(self.old_diameter);
        w.f64(self.new_diameter);
        w.var_i64(self.duration.for_version(version)?);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBorderSize {
    pub diameter: f64,
}
impl Body for WorldBorderSize {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self { diameter: r.f64()? })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.f64(self.diameter);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldBorderWarningDelay {
    pub warning_time: i32,
}
impl Body for WorldBorderWarningDelay {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            warning_time: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.warning_time);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldBorderWarningDistance {
    pub warning_blocks: i32,
}
impl Body for WorldBorderWarningDistance {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            warning_blocks: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.var_i32(self.warning_blocks);
        Ok(())
    }
}
codec!(InitializeWorldBorder => "initialize_world_border", WorldBorderCenter => "world_border_center", WorldBorderLerpSize => "world_border_lerp_size", WorldBorderSize => "world_border_size", WorldBorderWarningDelay => "world_border_warning_delay", WorldBorderWarningDistance => "world_border_warning_reach");
