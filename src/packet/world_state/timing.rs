//! Versioned time updates, including the 775+ world-clock registry map.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockUpdate {
    /// Direct nonnegative world-clock registry ID, without an offset or lookup.
    pub clock_id: i32,
    pub total_ticks: i64,
    pub partial_tick: f32,
    pub rate: f32,
}
#[derive(Debug, Clone, PartialEq)]
pub enum TimeData {
    /// Protocols 763–767 use the sign of this raw i64 for daylight-cycle state.
    /// Negative values (including i64::MIN) are preserved without normalization.
    Legacy { day_time: i64 },
    /// Protocols 768–774 carry a separate ticking flag.
    DayTime { day_time: i64, tick_day_time: bool },
    /// Protocols 775–776. Entries retain wire order and repeated keys; release
    /// readers collect a map and the last value for a repeated key wins.
    Clocks(Vec<ClockUpdate>),
}
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateTime {
    pub world_age: i64,
    pub time: TimeData,
}
fn clock_id(id: i32) -> Result<i32> {
    if id < 0 {
        Err(Error::Invalid("negative world-clock registry ID"))
    } else {
        Ok(id)
    }
}
impl Body for UpdateTime {
    fn read(r: &mut Reader<'_>, version: Version) -> Result<Self> {
        let world_age = r.i64()?;
        let time = match version.protocol() {
            763..=767 => TimeData::Legacy { day_time: r.i64()? },
            768..=774 => TimeData::DayTime {
                day_time: r.i64()?,
                tick_day_time: r.bool()?,
            },
            _ => {
                let count = r.count(r.limits.max_collection)?;
                // Each entry needs at least an ID byte, a tick byte and two f32s.
                if count > r.remaining().len() / 10 {
                    return Err(Error::Eof);
                }
                let mut clocks = Vec::with_capacity(count);
                for _ in 0..count {
                    clocks.push(ClockUpdate {
                        clock_id: clock_id(r.var_i32()?)?,
                        total_ticks: r.var_i64()?,
                        partial_tick: r.f32()?,
                        rate: r.f32()?,
                    });
                }
                TimeData::Clocks(clocks)
            }
        };
        Ok(Self { world_age, time })
    }
    fn write(&self, w: &mut Writer, version: Version, limits: Limits) -> Result<()> {
        w.i64(self.world_age);
        match (&self.time, version.protocol()) {
            (TimeData::Legacy { day_time }, 763..=767) => w.i64(*day_time),
            (
                TimeData::DayTime {
                    day_time,
                    tick_day_time,
                },
                768..=774,
            ) => {
                w.i64(*day_time);
                w.bool(*tick_day_time);
            }
            (TimeData::Clocks(clocks), 775..=776) => {
                if clocks.len() > limits.max_collection.min(i32::MAX as usize)
                    || clocks.len() > limits.max_packet.saturating_sub(9) / 10
                {
                    return Err(Error::Limit("world-clock update count"));
                }
                w.var_i32(clocks.len() as i32);
                for clock in clocks {
                    w.var_i32(clock_id(clock.clock_id)?);
                    w.var_i64(clock.total_ticks);
                    w.f32(clock.partial_tick);
                    w.f32(clock.rate);
                    if w.as_slice().len() > limits.max_packet {
                        return Err(Error::Limit("world-state packet bytes"));
                    }
                }
            }
            _ => return Err(Error::Invalid("time-update version fields")),
        }
        Ok(())
    }
}
codec!(UpdateTime => "update_time");
