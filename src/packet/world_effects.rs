//! World particles, explosions, sounds and level events for protocols 763–776.
//!
//! These are semantic wire values, not a renderer or world simulation. Sound
//! registry IDs and level-event IDs/data remain unresolved numeric values.
//! Sound positions retain their exact signed, one-eighth-block wire integers.
//!
//! Release packet/stream codecs were inspected in all fourteen cached release
//! artifacts. Important schema corrections: explosion sound is an untagged
//! inline event in 765, then a holder in 766; optional explosion knockback is
//! three f64 values starting in 768 (including 768); weighted block particles
//! start in 773 and keep the same outer layout through 776. World particles
//! move their ID to the suffix in 766 and add always-show in 769. UI sound
//! source starts in 771. Nested particles reuse the metadata particle codecs.
use super::{
    entity_metadata::{
        holders::RegistryHolder,
        particles::{self, Particle},
    },
    inventory::{self, Budget, SoundEvent, SoundHolder},
};
use crate::{
    codec::{BlockPosition, Reader, Writer},
    frame::RawPacket,
    version::{Direction, State},
    Error, Limits, Result, Version,
};

fn nonnegative(n: i32) -> Result<i32> {
    if n < 0 {
        Err(Error::Invalid("negative world-effect integer"))
    } else {
        Ok(n)
    }
}
fn finite(n: f32) -> Result<f32> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(Error::Invalid("non-finite world-effect float"))
    }
}
fn finite64(n: f64) -> Result<f64> {
    if n.is_finite() {
        Ok(n)
    } else {
        Err(Error::Invalid("non-finite world-effect coordinate"))
    }
}
fn read_vec3(r: &mut Reader<'_>) -> Result<[f64; 3]> {
    Ok([
        finite64(r.f64()?)?,
        finite64(r.f64()?)?,
        finite64(r.f64()?)?,
    ])
}
fn write_vec3(v: &[f64; 3], w: &mut Writer) -> Result<()> {
    for n in v {
        w.f64(finite64(*n)?);
    }
    Ok(())
}
fn read_vec3f(r: &mut Reader<'_>) -> Result<[f32; 3]> {
    Ok([finite(r.f32()?)?, finite(r.f32()?)?, finite(r.f32()?)?])
}
fn write_vec3f(v: &[f32; 3], w: &mut Writer) -> Result<()> {
    for n in v {
        w.f32(finite(*n)?);
    }
    Ok(())
}
fn identifier(s: &str, version: Version) -> Result<()> {
    let (namespace, path) = s.split_once(':').unwrap_or(("minecraft", s));
    let valid = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b);
    if (version.protocol() >= 775 && namespace == "..")
        || !namespace.bytes().all(valid)
        || !path.bytes().all(|b| valid(b) || b == b'/')
    {
        return Err(Error::Invalid("sound resource identifier"));
    }
    Ok(())
}
fn validate_sound_event(sound: &SoundEvent, version: Version) -> Result<()> {
    identifier(&sound.name, version)?;
    if let Some(range) = sound.fixed_range {
        finite(range)?;
    }
    Ok(())
}
fn validate_sound(sound: &SoundHolder, version: Version) -> Result<()> {
    if let RegistryHolder::Inline(sound) = sound {
        validate_sound_event(sound, version)?;
    }
    Ok(())
}
fn read_sound(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<SoundHolder> {
    let sound = inventory::read_sound(r, b)?;
    validate_sound(&sound, version)?;
    Ok(sound)
}
fn write_sound(
    sound: &SoundHolder,
    w: &mut Writer,
    version: Version,
    b: &mut Budget,
) -> Result<()> {
    validate_sound(sound, version)?;
    inventory::write_sound(sound, w, b)
}
trait Body: Sized {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self>;
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()>;
}
macro_rules! codec {
    ($($t:ty => $name:literal),* $(,)?) => {$ (
        impl $t {
            pub fn decode(bytes: &[u8], version: Version, limits: Limits) -> Result<Self> {
                if bytes.len() > limits.max_packet { return Err(Error::Limit("world-effect packet bytes")); }
                let mut r = Reader::new(bytes, limits);
                let value = Self::read(&mut r, version, &mut Budget::new(limits))?;
                r.finish()?;
                Ok(value)
            }
            /// Encoding is transactional: a failure never returns a partial body.
            pub fn encode(&self, version: Version, limits: Limits) -> Result<Vec<u8>> {
                let mut w = Writer::new();
                let mut b = Budget::new(limits);
                self.write(&mut w, version, &mut b)?;
                b.check_bytes(&w)?;
                Ok(w.into_inner())
            }
            pub fn packet(&self, version: Version, limits: Limits) -> Result<RawPacket> {
                Ok(RawPacket::new(version.packet_id(State::Play, Direction::Clientbound, $name)?, self.encode(version, limits)?))
            }
        }
    )*};
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorldParticles {
    pub long_distance: bool,
    /// Present exactly in protocols 769 and later, including when false.
    pub always_show: Option<bool>,
    pub position: [f64; 3],
    pub offset: [f32; 3],
    pub speed: f32,
    /// Zero requests the special single-particle behavior. This is a scalar,
    /// not an allocation count; no client particles are instantiated here.
    pub count: i32,
    pub particle: Particle,
}
impl Body for WorldParticles {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let id = if version.protocol() <= 765 {
            Some(r.var_i32()?)
        } else {
            None
        };
        let long_distance = r.bool()?;
        let always_show = if version.protocol() >= 769 {
            Some(r.bool()?)
        } else {
            None
        };
        let position = read_vec3(r)?;
        let offset = read_vec3f(r)?;
        let speed = finite(r.f32()?)?;
        let count = nonnegative(r.i32()?)?;
        let particle = if let Some(id) = id {
            particles::read_payload(r, version, b, id)?
        } else {
            particles::read(r, version, b)?
        };
        Ok(Self {
            long_distance,
            always_show,
            position,
            offset,
            speed,
            count,
            particle,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        if self.always_show.is_some() != (version.protocol() >= 769) {
            return Err(Error::Invalid("world-particle always-show release"));
        }
        if version.protocol() <= 765 {
            w.var_i32(particles::particle_id(version, self.particle.kind)?);
        }
        w.bool(self.long_distance);
        if let Some(show) = self.always_show {
            w.bool(show);
        }
        write_vec3(&self.position, w)?;
        write_vec3f(&self.offset, w)?;
        w.f32(finite(self.speed)?);
        w.i32(nonnegative(self.count)?);
        if version.protocol() <= 765 {
            particles::write_payload(&self.particle, w, version, b)
        } else {
            particles::write(&self.particle, w, version, b)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SoundSource {
    Master = 0,
    Music = 1,
    Records = 2,
    Weather = 3,
    Blocks = 4,
    Hostile = 5,
    Neutral = 6,
    Players = 7,
    Ambient = 8,
    Voice = 9,
    /// Available starting with protocol 771.
    Ui = 10,
}
impl SoundSource {
    pub fn from_id(id: i32, version: Version) -> Result<Self> {
        Ok(match id {
            0 => Self::Master,
            1 => Self::Music,
            2 => Self::Records,
            3 => Self::Weather,
            4 => Self::Blocks,
            5 => Self::Hostile,
            6 => Self::Neutral,
            7 => Self::Players,
            8 => Self::Ambient,
            9 => Self::Voice,
            10 if version.protocol() >= 771 => Self::Ui,
            _ => return Err(Error::Invalid("sound source for selected release")),
        })
    }
    pub fn id(self, version: Version) -> Result<i32> {
        Self::from_id(self as i32, version)?;
        Ok(self as i32)
    }
}

/// Exact sound coordinates in eighths of a block. Keeping integer values avoids
/// loss or rounding for negative positions and the full signed i32 range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl SoundPosition {
    pub fn blocks(self) -> [f64; 3] {
        [
            self.x as f64 / 8.0,
            self.y as f64 / 8.0,
            self.z as f64 / 8.0,
        ]
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SoundEffect {
    pub sound: SoundHolder,
    pub source: SoundSource,
    pub position: SoundPosition,
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,
}
impl Body for SoundEffect {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        Ok(Self {
            sound: read_sound(r, version, b)?,
            source: SoundSource::from_id(r.var_i32()?, version)?,
            position: SoundPosition {
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
            },
            volume: finite(r.f32()?)?,
            pitch: finite(r.f32()?)?,
            seed: r.i64()?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        write_sound(&self.sound, w, version, b)?;
        w.var_i32(self.source.id(version)?);
        w.i32(self.position.x);
        w.i32(self.position.y);
        w.i32(self.position.z);
        w.f32(finite(self.volume)?);
        w.f32(finite(self.pitch)?);
        w.i64(self.seed);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySoundEffect {
    pub sound: SoundHolder,
    pub source: SoundSource,
    pub entity_id: i32,
    pub volume: f32,
    pub pitch: f32,
    pub seed: i64,
}
impl Body for EntitySoundEffect {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        Ok(Self {
            sound: read_sound(r, version, b)?,
            source: SoundSource::from_id(r.var_i32()?, version)?,
            entity_id: nonnegative(r.var_i32()?)?,
            volume: finite(r.f32()?)?,
            pitch: finite(r.f32()?)?,
            seed: r.i64()?,
        })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        write_sound(&self.sound, w, version, b)?;
        w.var_i32(self.source.id(version)?);
        w.var_i32(nonnegative(self.entity_id)?);
        w.f32(finite(self.volume)?);
        w.f32(finite(self.pitch)?);
        w.i64(self.seed);
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopSound {
    pub source: Option<SoundSource>,
    /// Resource identifier; absent together with source means stop all sounds.
    pub sound: Option<String>,
}
impl Body for StopSound {
    fn read(r: &mut Reader<'_>, version: Version, _: &mut Budget) -> Result<Self> {
        let flags = r.u8()?;
        if flags & !3 != 0 {
            return Err(Error::Invalid("stop-sound flags"));
        }
        let source = if flags & 1 != 0 {
            Some(SoundSource::from_id(r.var_i32()?, version)?)
        } else {
            None
        };
        let sound = if flags & 2 != 0 {
            let s = r.string(32767)?;
            identifier(s, version)?;
            Some(s.to_owned())
        } else {
            None
        };
        Ok(Self { source, sound })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        w.u8(u8::from(self.source.is_some()) | (u8::from(self.sound.is_some()) << 1));
        if let Some(source) = self.source {
            w.var_i32(source.id(version)?);
        }
        if let Some(sound) = &self.sound {
            identifier(sound, version)?;
            if sound.len() > b.limits.max_packet.saturating_sub(w.as_slice().len()) {
                return Err(Error::Limit("stop-sound identifier bytes"));
            }
            w.string(sound, b.limits.max_string_chars.min(32767))?;
        }
        Ok(())
    }
}
/// Numeric level event and event-specific data. Neither integer is a payload
/// discriminator: unknown event IDs can be retained without guessing a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldEvent {
    pub event_id: i32,
    pub position: BlockPosition,
    pub data: i32,
    pub global: bool,
}
impl Body for WorldEvent {
    fn read(r: &mut Reader<'_>, _: Version, _: &mut Budget) -> Result<Self> {
        Ok(Self {
            event_id: r.i32()?,
            position: BlockPosition::unpack(r.i64()?),
            data: r.i32()?,
            global: r.bool()?,
        })
    }
    fn write(&self, w: &mut Writer, _: Version, _: &mut Budget) -> Result<()> {
        w.i32(self.event_id);
        w.i64(self.position.pack()?);
        w.i32(self.data);
        w.bool(self.global);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum BlockInteraction {
    Keep = 0,
    Destroy = 1,
    DestroyWithDecay = 2,
    TriggerBlock = 3,
}
impl BlockInteraction {
    fn from_id(id: i32) -> Result<Self> {
        Ok(match id {
            0 => Self::Keep,
            1 => Self::Destroy,
            2 => Self::DestroyWithDecay,
            3 => Self::TriggerBlock,
            _ => return Err(Error::Invalid("explosion block interaction")),
        })
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyExplosionEffects {
    pub block_interaction: BlockInteraction,
    pub small_particle: Particle,
    pub large_particle: Particle,
    /// Protocol 765 permits only Inline and writes it without a holder marker.
    pub sound: SoundHolder,
}
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyExplosion {
    pub radius: f32,
    /// Signed byte offsets from floor(center), preserved without wrapping.
    pub affected_block_offsets: Vec<[i8; 3]>,
    pub player_knockback: [f32; 3],
    /// Absent in 763–764; required in 765–767.
    pub effects: Option<Box<LegacyExplosionEffects>>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedExplosionParticle {
    pub particle: Particle,
    pub scaling: f32,
    pub speed: f32,
    /// Nonnegative weight. The total of all weights must fit a signed i32.
    pub weight: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ExplosionBlockEffects {
    pub radius: f32,
    /// A scalar count of affected blocks, not a transmitted position list.
    pub block_count: i32,
    pub particles: Vec<WeightedExplosionParticle>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ModernExplosion {
    pub player_knockback: Option<[f64; 3]>,
    pub particle: Particle,
    pub sound: SoundHolder,
    /// Absent in 768–772; required in 773–776.
    pub block_effects: Option<ExplosionBlockEffects>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum ExplosionData {
    /// Protocols 763–767.
    Legacy(LegacyExplosion),
    /// Protocols 768–776.
    Modern(ModernExplosion),
}
#[derive(Debug, Clone, PartialEq)]
pub struct Explosion {
    pub center: [f64; 3],
    pub data: ExplosionData,
}
impl Body for Explosion {
    fn read(r: &mut Reader<'_>, version: Version, b: &mut Budget) -> Result<Self> {
        let center = read_vec3(r)?;
        let data = if version.protocol() < 768 {
            let radius = finite(r.f32()?)?;
            let n = b.count(r)?;
            if n > r.remaining().len() / 3 {
                return Err(Error::Eof);
            }
            let mut affected_block_offsets = Vec::with_capacity(n);
            for _ in 0..n {
                affected_block_offsets.push([r.u8()? as i8, r.u8()? as i8, r.u8()? as i8]);
            }
            let player_knockback = read_vec3f(r)?;
            let effects = if version.protocol() >= 765 {
                let block_interaction = BlockInteraction::from_id(r.var_i32()?)?;
                let small_particle = particles::read(r, version, b)?;
                let large_particle = particles::read(r, version, b)?;
                let sound = if version.protocol() == 765 {
                    let sound = inventory::read_sound_event(r)?;
                    validate_sound_event(&sound, version)?;
                    RegistryHolder::Inline(sound)
                } else {
                    read_sound(r, version, b)?
                };
                Some(Box::new(LegacyExplosionEffects {
                    block_interaction,
                    small_particle,
                    large_particle,
                    sound,
                }))
            } else {
                None
            };
            ExplosionData::Legacy(LegacyExplosion {
                radius,
                affected_block_offsets,
                player_knockback,
                effects,
            })
        } else {
            let block_info = if version.protocol() >= 773 {
                Some((finite(r.f32()?)?, nonnegative(r.i32()?)?))
            } else {
                None
            };
            let player_knockback = if r.bool()? { Some(read_vec3(r)?) } else { None };
            let particle = particles::read(r, version, b)?;
            let sound = read_sound(r, version, b)?;
            let block_effects = if let Some((radius, block_count)) = block_info {
                let n = b.count(r)?;
                // Each entry has at least an ID, two f32s, and a weight VarInt.
                if n > r.remaining().len() / 10 {
                    return Err(Error::Eof);
                }
                let mut entries = Vec::with_capacity(n);
                let mut total_weight = 0i32;
                for _ in 0..n {
                    let particle = particles::read(r, version, b)?;
                    let scaling = finite(r.f32()?)?;
                    let speed = finite(r.f32()?)?;
                    let weight = nonnegative(r.var_i32()?)?;
                    total_weight = total_weight
                        .checked_add(weight)
                        .ok_or(Error::Invalid("explosion weight total overflow"))?;
                    entries.push(WeightedExplosionParticle {
                        particle,
                        scaling,
                        speed,
                        weight,
                    });
                }
                Some(ExplosionBlockEffects {
                    radius,
                    block_count,
                    particles: entries,
                })
            } else {
                None
            };
            ExplosionData::Modern(ModernExplosion {
                player_knockback,
                particle,
                sound,
                block_effects,
            })
        };
        Ok(Self { center, data })
    }
    fn write(&self, w: &mut Writer, version: Version, b: &mut Budget) -> Result<()> {
        write_vec3(&self.center, w)?;
        match &self.data {
            ExplosionData::Legacy(data) if version.protocol() < 768 => {
                if data.effects.is_some() != (version.protocol() >= 765) {
                    return Err(Error::Invalid("legacy explosion effects release"));
                }
                w.f32(finite(data.radius)?);
                b.write_count(data.affected_block_offsets.len(), w)?;
                if data.affected_block_offsets.len()
                    > b.limits.max_packet.saturating_sub(w.as_slice().len()) / 3
                {
                    return Err(Error::Limit("explosion offset bytes"));
                }
                for p in &data.affected_block_offsets {
                    for n in p {
                        w.u8(*n as u8);
                    }
                }
                write_vec3f(&data.player_knockback, w)?;
                if let Some(effects) = &data.effects {
                    w.var_i32(effects.block_interaction as i32);
                    particles::write(&effects.small_particle, w, version, b)?;
                    particles::write(&effects.large_particle, w, version, b)?;
                    if version.protocol() == 765 {
                        let RegistryHolder::Inline(sound) = &effects.sound else {
                            return Err(Error::Invalid(
                                "protocol 765 explosion requires inline sound",
                            ));
                        };
                        validate_sound_event(sound, version)?;
                        inventory::write_sound_event(sound, w, b)?;
                    } else {
                        write_sound(&effects.sound, w, version, b)?;
                    }
                }
            }
            ExplosionData::Modern(data) if version.protocol() >= 768 => {
                if data.block_effects.is_some() != (version.protocol() >= 773) {
                    return Err(Error::Invalid("explosion block effects release"));
                }
                if let Some(block) = &data.block_effects {
                    w.f32(finite(block.radius)?);
                    w.i32(nonnegative(block.block_count)?);
                }
                w.bool(data.player_knockback.is_some());
                if let Some(knockback) = &data.player_knockback {
                    write_vec3(knockback, w)?;
                }
                particles::write(&data.particle, w, version, b)?;
                write_sound(&data.sound, w, version, b)?;
                if let Some(block) = &data.block_effects {
                    b.write_count(block.particles.len(), w)?;
                    if block.particles.len()
                        > b.limits.max_packet.saturating_sub(w.as_slice().len()) / 10
                    {
                        return Err(Error::Limit("explosion particle bytes"));
                    }
                    let mut total_weight = 0i32;
                    for entry in &block.particles {
                        particles::write(&entry.particle, w, version, b)?;
                        w.f32(finite(entry.scaling)?);
                        w.f32(finite(entry.speed)?);
                        let weight = nonnegative(entry.weight)?;
                        total_weight = total_weight
                            .checked_add(weight)
                            .ok_or(Error::Invalid("explosion weight total overflow"))?;
                        w.var_i32(weight);
                        b.check_bytes(w)?;
                    }
                }
            }
            _ => return Err(Error::Invalid("explosion layout release")),
        }
        Ok(())
    }
}
codec! {
    WorldParticles => "world_particles",
    Explosion => "explosion",
    SoundEffect => "sound_effect",
    EntitySoundEffect => "entity_sound_effect",
    StopSound => "stop_sound",
    WorldEvent => "world_event",
}
