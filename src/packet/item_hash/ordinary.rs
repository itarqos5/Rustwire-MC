//! Registry-independent persistent item CODECs. See the original Java oracle.
use super::*;
use crate::packet::entity_metadata::holders::RegistryHolder;
use inventory::{
    ConsumeEffect, Filterable, FireworkExplosion, FireworkShape, SoundHolder, SwingAnimationType,
    UseAnimation,
};

fn invalid() -> Error {
    Error::Invalid("component persistent codec range")
}
fn range(value: f32, min: f32, max: f32) -> Result<()> {
    // Vanilla's codecs use Float.compare: negative zero sorts below zero.
    if !value.is_finite() || value.total_cmp(&min).is_lt() || value.total_cmp(&max).is_gt() {
        return Err(invalid());
    }
    Ok(())
}
fn positive(value: f32) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(invalid());
    }
    Ok(())
}
fn filterable<T>(value: &Filterable<T>, hash: impl Fn(&T) -> Result<u32>) -> Result<u32> {
    let mut out = vec![("raw", hash(&value.raw)?)];
    if let Some(filtered) = &value.filtered {
        out.push(("filtered", hash(filtered)?));
    }
    Ok(fields(out))
}
fn explosion(value: &FireworkExplosion) -> u32 {
    let mut out = vec![(
        "shape",
        string(match value.shape {
            FireworkShape::SmallBall => "small_ball",
            FireworkShape::LargeBall => "large_ball",
            FireworkShape::Star => "star",
            FireworkShape::Creeper => "creeper",
            FireworkShape::Burst => "burst",
        }),
    )];
    if !value.colors.is_empty() {
        out.push(("colors", list(value.colors.iter().copied().map(integer))));
    }
    if !value.fade_colors.is_empty() {
        out.push((
            "fade_colors",
            list(value.fade_colors.iter().copied().map(integer)),
        ));
    }
    if value.has_trail {
        out.push(("has_trail", boolean(true)));
    }
    if value.has_twinkle {
        out.push(("has_twinkle", boolean(true)));
    }
    fields(out)
}
fn sound(value: &SoundHolder) -> Result<u32> {
    let RegistryHolder::Inline(value) = value else {
        return Err(Error::Unsupported("registry sound component hash"));
    };
    let mut out = vec![("sound_id", identifier(&value.name)?)];
    if let Some(range) = value.fixed_range {
        out.push(("range", float(range)));
    }
    Ok(fields(out))
}
fn effect(value: &ConsumeEffect) -> Result<u32> {
    let mut out = vec![];
    let kind = match value {
        ConsumeEffect::ClearAllEffects => "clear_all_effects",
        ConsumeEffect::TeleportRandomly { diameter } => {
            positive(*diameter)?;
            if *diameter != 16.0 {
                out.push(("diameter", float(*diameter)));
            }
            "teleport_randomly"
        }
        ConsumeEffect::PlaySound(value) => {
            out.push(("sound", sound(value)?));
            "play_sound"
        }
        _ => return Err(Error::Unsupported("registry consume effect hash")),
    };
    out.push(("type", identifier(kind)?));
    Ok(fields(out))
}
pub(super) fn hash(component: &Component, version: Version) -> Result<u32> {
    Ok(match (component.name, &component.value) {
        ("custom_name" | "item_name", V::Nbt(value)) => text::hash(&value.root)?,
        ("lore", V::Lore(lines)) => {
            if lines.len() > 256 {
                return Err(invalid());
            }
            list(
                lines
                    .iter()
                    .map(|line| {
                        text::hash(
                            &line
                                .as_ref()
                                .ok_or(Error::Invalid("missing lore text"))?
                                .root,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?,
            )
        }
        ("food", V::Food(value)) => {
            if value.nutrition < 0 {
                return Err(invalid());
            }
            let mut out = vec![
                ("nutrition", integer(value.nutrition)),
                ("saturation", float(value.saturation_modifier)),
            ];
            if value.can_always_eat {
                out.push(("can_always_eat", boolean(true)));
            }
            fields(out)
        }
        ("use_cooldown", V::UseCooldown(value)) => {
            positive(value.seconds)?;
            let mut out = vec![("seconds", float(value.seconds))];
            if let Some(group) = &value.group {
                out.push(("cooldown_group", identifier(group)?));
            }
            fields(out)
        }
        ("weapon", V::Weapon(value)) => {
            if value.item_damage_per_attack < 0 {
                return Err(invalid());
            }
            range(value.disable_blocking_seconds, 0.0, f32::MAX)?;
            let mut out = vec![];
            if value.item_damage_per_attack != 1 {
                out.push((
                    "item_damage_per_attack",
                    integer(value.item_damage_per_attack),
                ));
            }
            if value.disable_blocking_seconds != 0.0 {
                out.push((
                    "disable_blocking_for_seconds",
                    float(value.disable_blocking_seconds),
                ));
            }
            fields(out)
        }
        ("use_effects", V::UseEffects(value)) => {
            range(value.speed_multiplier, 0.0, 1.0)?;
            let mut out = vec![];
            if value.can_sprint {
                out.push(("can_sprint", boolean(true)));
            }
            if !value.interact_vibrations {
                out.push(("interact_vibrations", boolean(false)));
            }
            if value.speed_multiplier != 0.2 {
                out.push(("speed_multiplier", float(value.speed_multiplier)));
            }
            fields(out)
        }
        ("attack_range", V::AttackRange(value)) => {
            let mut out = vec![];
            for (name, number, default, maximum) in [
                ("min_reach", value.min_range, 0.0, 64.0),
                ("max_reach", value.max_range, 3.0, 64.0),
                ("min_creative_reach", value.min_creative_range, 0.0, 64.0),
                ("max_creative_reach", value.max_creative_range, 5.0, 64.0),
                ("hitbox_margin", value.hitbox_margin, 0.3, 1.0),
                ("mob_factor", value.mob_factor, 1.0, 2.0),
            ] {
                range(number, 0.0, maximum)?;
                if number != default {
                    out.push((name, float(number)));
                }
            }
            fields(out)
        }
        ("swing_animation", V::SwingAnimation(value)) => {
            if value.duration <= 0 {
                return Err(invalid());
            }
            let mut out = vec![];
            if value.kind != SwingAnimationType::Whack {
                out.push((
                    "type",
                    string(match value.kind {
                        SwingAnimationType::None => "none",
                        SwingAnimationType::Whack => "whack",
                        SwingAnimationType::Stab => "stab",
                    }),
                ));
            }
            if value.duration != 6 {
                out.push(("duration", integer(value.duration)));
            }
            fields(out)
        }
        ("firework_explosion", V::FireworkExplosion(value)) => explosion(value),
        ("fireworks", V::Fireworks(value)) => {
            // ExtraCodecs.UNSIGNED_BYTE only rejects the upper bound when encoding.
            if value.flight_duration > 255 {
                return Err(invalid());
            }
            let mut out = vec![];
            if value.flight_duration != 0 {
                out.push((
                    "flight_duration",
                    primitive(6, &(value.flight_duration as i8).to_le_bytes()),
                ));
            }
            if !value.explosions.is_empty() {
                out.push(("explosions", list(value.explosions.iter().map(explosion))));
            }
            fields(out)
        }
        ("lodestone_tracker", V::LodestoneTracker(value)) => {
            let mut out = vec![];
            if let Some(target) = &value.target {
                let p = target.position;
                out.push((
                    "target",
                    fields(vec![
                        ("dimension", identifier(&target.dimension)?),
                        ("pos", nbt_hash(&Tag::IntArray(vec![p.x, p.y, p.z]))),
                    ]),
                ));
            }
            if !value.tracked {
                out.push(("tracked", boolean(false)));
            }
            fields(out)
        }
        ("writable_book_content", V::WritableBook(pages)) => {
            if pages.is_empty() {
                fields(vec![])
            } else {
                fields(vec![(
                    "pages",
                    list(
                        pages
                            .iter()
                            .map(|page| filterable(page, |s| Ok(string(s))))
                            .collect::<Result<Vec<_>>>()?,
                    ),
                )])
            }
        }
        ("written_book_content", V::WrittenBook(value)) => {
            if !(0..=3).contains(&value.generation) {
                return Err(invalid());
            }
            let mut out = vec![
                ("title", filterable(&value.title, |s| Ok(string(s)))?),
                ("author", string(&value.author)),
            ];
            if value.generation != 0 {
                out.push(("generation", integer(value.generation)));
            }
            if !value.pages.is_empty() {
                out.push((
                    "pages",
                    list(
                        value
                            .pages
                            .iter()
                            .map(|page| filterable(page, |n| text::hash(&n.root)))
                            .collect::<Result<Vec<_>>>()?,
                    ),
                ));
            }
            if value.resolved {
                out.push(("resolved", boolean(true)));
            }
            fields(out)
        }
        ("break_sound", V::Sound(value)) => sound(value)?,
        ("death_protection", V::DeathProtection(effects)) => {
            if effects.is_empty() {
                fields(vec![])
            } else {
                fields(vec![(
                    "death_effects",
                    list(effects.iter().map(effect).collect::<Result<Vec<_>>>()?),
                )])
            }
        }
        ("consumable", V::Consumable(value)) => {
            range(value.consume_seconds, 0.0, f32::MAX)?;
            let mut out = vec![("sound", sound(&value.sound)?)];
            if value.consume_seconds != 1.6 {
                out.push(("consume_seconds", float(value.consume_seconds)));
            }
            if value.animation != UseAnimation::Eat {
                out.push((
                    "animation",
                    string(match value.animation {
                        UseAnimation::None => "none",
                        UseAnimation::Eat => "eat",
                        UseAnimation::Drink => "drink",
                        UseAnimation::Block => "block",
                        UseAnimation::Bow => "bow",
                        UseAnimation::Trident if version.protocol() < 774 => "spear",
                        UseAnimation::Trident => "trident",
                        UseAnimation::Crossbow => "crossbow",
                        UseAnimation::Spyglass => "spyglass",
                        UseAnimation::TootHorn => "toot_horn",
                        UseAnimation::Brush => "brush",
                        UseAnimation::Bundle => "bundle",
                        UseAnimation::Spear => "spear",
                    }),
                ));
            }
            if !value.makes_particles {
                out.push(("has_consume_particles", boolean(false)));
            }
            if !value.effects.is_empty() {
                out.push((
                    "on_consume_effects",
                    list(
                        value
                            .effects
                            .iter()
                            .map(effect)
                            .collect::<Result<Vec<_>>>()?,
                    ),
                ));
            }
            fields(out)
        }
        _ => return Err(Error::Unsupported("component persistent hash codec")),
    })
}
