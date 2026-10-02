//! World-border scalar envelopes. Durations are VarLong, not schema VarInt.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InitializeWorldBorder {
    pub x: f64,
    pub z: f64,
    pub old_diameter: f64,
    pub new_diameter: f64,
    /// Signed wire milliseconds; no gameplay duration policy is imposed.
    pub duration_ms: i64,
    pub portal_teleport_boundary: i32,
    pub warning_blocks: i32,
    pub warning_time: i32,
}
impl Body for InitializeWorldBorder {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            x: r.f64()?,
            z: r.f64()?,
            old_diameter: r.f64()?,
            new_diameter: r.f64()?,
            duration_ms: r.var_i64()?,
            portal_teleport_boundary: r.var_i32()?,
            warning_blocks: r.var_i32()?,
            warning_time: r.var_i32()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.f64(self.x);
        w.f64(self.z);
        w.f64(self.old_diameter);
        w.f64(self.new_diameter);
        w.var_i64(self.duration_ms);
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
    pub duration_ms: i64,
}
impl Body for WorldBorderLerpSize {
    fn read(r: &mut Reader<'_>, _: Version) -> Result<Self> {
        Ok(Self {
            old_diameter: r.f64()?,
            new_diameter: r.f64()?,
            duration_ms: r.var_i64()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: Limits) -> Result<()> {
        w.f64(self.old_diameter);
        w.f64(self.new_diameter);
        w.var_i64(self.duration_ms);
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
