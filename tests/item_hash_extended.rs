// Independent public-CODEC/HashOps fixtures from ComponentHashExtendedOracle.java.
use rustwire_mc::{
    codec::BlockPosition,
    nbt::{Nbt, NbtString, Tag, TagType},
    packet::{entity_metadata::holders::RegistryHolder, inventory::*, item_hash::hash_component},
    Error, Limits, Version,
};
use ComponentValue as V;
fn version(p: i32) -> Version {
    Version::from_protocol(p).unwrap()
}
fn hash(name: &'static str, value: V, p: i32) -> rustwire_mc::Result<i32> {
    hash_component(&Component { name, value }, version(p), Limits::default())
}
#[test]
fn official_extended_component_fixtures() {
    // name_plain
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::String("Rustwire 🚀".into()))),
                p
            )
            .unwrap(),
            1915252855
        );
    }
    // name_literal
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::Compound(vec![(
                    "text".into(),
                    Tag::String("Rustwire 🚀".into())
                )]))),
                p
            )
            .unwrap(),
            1915252855
        );
    }
    // name_empty
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::Compound(vec![(
                    "text".into(),
                    Tag::String("".into())
                )]))),
                p
            )
            .unwrap(),
            1615905556
        );
    }
    // name_styled
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::Compound(vec![
                    ("text".into(), Tag::String("Rustwire 🚀".into())),
                    ("bold".into(), Tag::Byte(0)),
                    ("italic".into(), Tag::Byte(1)),
                    ("color".into(), Tag::String("red".into())),
                    (
                        "extra".into(),
                        Tag::List {
                            element_type: TagType::Compound,
                            elements: vec![
                                Tag::Compound(vec![("text".into(), Tag::String("!".into()))]),
                                Tag::Compound(vec![("text".into(), Tag::String("ok".into()))])
                            ]
                        }
                    )
                ]))),
                p
            )
            .unwrap(),
            -1318576648
        );
    }
    // name_hex
    for p in 770..=776 {
        assert_eq!(
            hash(
                "item_name",
                V::Nbt(Nbt::anonymous(Tag::Compound(vec![
                    ("text".into(), Tag::String("color".into())),
                    ("color".into(), Tag::String("#aBcDeF".into())),
                    ("underlined".into(), Tag::Byte(1)),
                    ("strikethrough".into(), Tag::Byte(0)),
                    ("obfuscated".into(), Tag::Byte(0)),
                    ("insertion".into(), Tag::String("λ".into())),
                    ("font".into(), Tag::String("uniform".into()))
                ]))),
                p
            )
            .unwrap(),
            -1898305226
        );
    }
    // name_list
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::List {
                    element_type: TagType::String,
                    elements: vec![Tag::String("a".into()), Tag::String("b".into())]
                })),
                p
            )
            .unwrap(),
            1573042902
        );
    }
    // lore_empty
    for p in 770..=776 {
        assert_eq!(hash("lore", V::Lore(vec![]), p).unwrap(), -1978007022);
    }
    // lore_plain
    for p in 770..=776 {
        assert_eq!(
            hash(
                "lore",
                V::Lore(vec![
                    Some(Nbt::anonymous(Tag::String("first".into()))),
                    Some(Nbt::anonymous(Tag::Compound(vec![
                        ("text".into(), Tag::String("第二 🚀".into())),
                        ("italic".into(), Tag::Byte(0))
                    ])))
                ]),
                p
            )
            .unwrap(),
            -1125293080
        );
    }
    // food_default
    for p in 770..=776 {
        assert_eq!(
            hash(
                "food",
                V::Food(Food {
                    nutrition: 0,
                    saturation_modifier: 0.0,
                    can_always_eat: false,
                    legacy_consumption: None
                }),
                p
            )
            .unwrap(),
            -1537973918
        );
    }
    // food_full
    for p in 770..=776 {
        assert_eq!(
            hash(
                "food",
                V::Food(Food {
                    nutrition: 7,
                    saturation_modifier: 1.25,
                    can_always_eat: true,
                    legacy_consumption: None
                }),
                p
            )
            .unwrap(),
            -523093613
        );
    }
    // food_saturation_negative
    for p in 770..=776 {
        assert_eq!(
            hash(
                "food",
                V::Food(Food {
                    nutrition: 1,
                    saturation_modifier: -0.5,
                    can_always_eat: false,
                    legacy_consumption: None
                }),
                p
            )
            .unwrap(),
            1141885162
        );
    }
    // cooldown_plain
    for p in 770..=776 {
        assert_eq!(
            hash(
                "use_cooldown",
                V::UseCooldown(UseCooldown {
                    seconds: 2.5,
                    group: None
                }),
                p
            )
            .unwrap(),
            1607827570
        );
    }
    // cooldown_group
    for p in 770..=776 {
        assert_eq!(
            hash(
                "use_cooldown",
                V::UseCooldown(UseCooldown {
                    seconds: 0.5,
                    group: Some("tools".into())
                }),
                p
            )
            .unwrap(),
            -1556065078
        );
    }
    // weapon_default
    for p in 770..=776 {
        assert_eq!(
            hash(
                "weapon",
                V::Weapon(Weapon {
                    item_damage_per_attack: 1,
                    disable_blocking_seconds: 0.0
                }),
                p
            )
            .unwrap(),
            -982207288
        );
    }
    // weapon_full
    for p in 770..=776 {
        assert_eq!(
            hash(
                "weapon",
                V::Weapon(Weapon {
                    item_damage_per_attack: 3,
                    disable_blocking_seconds: 2.5
                }),
                p
            )
            .unwrap(),
            211448302
        );
    }
    // use_effects_default
    for p in 774..=776 {
        assert_eq!(
            hash(
                "use_effects",
                V::UseEffects(UseEffects {
                    can_sprint: false,
                    interact_vibrations: true,
                    speed_multiplier: 0.2
                }),
                p
            )
            .unwrap(),
            -982207288
        );
    }
    // use_effects_full
    for p in 774..=776 {
        assert_eq!(
            hash(
                "use_effects",
                V::UseEffects(UseEffects {
                    can_sprint: true,
                    interact_vibrations: false,
                    speed_multiplier: 0.625
                }),
                p
            )
            .unwrap(),
            1579970413
        );
    }
    // explosion_default
    for p in 770..=776 {
        assert_eq!(
            hash(
                "firework_explosion",
                V::FireworkExplosion(FireworkExplosion {
                    shape: FireworkShape::SmallBall,
                    colors: vec![],
                    fade_colors: vec![],
                    has_trail: false,
                    has_twinkle: false
                }),
                p
            )
            .unwrap(),
            1619490100
        );
    }
    // explosion_full
    for p in 770..=776 {
        assert_eq!(
            hash(
                "firework_explosion",
                V::FireworkExplosion(FireworkExplosion {
                    shape: FireworkShape::Star,
                    colors: vec![-1, 0, 16711680],
                    fade_colors: vec![255],
                    has_trail: true,
                    has_twinkle: true
                }),
                p
            )
            .unwrap(),
            -1956033140
        );
    }
    // fireworks_default
    for p in 770..=776 {
        assert_eq!(
            hash(
                "fireworks",
                V::Fireworks(Fireworks {
                    flight_duration: 0,
                    explosions: vec![]
                }),
                p
            )
            .unwrap(),
            -982207288
        );
    }
    // fireworks_full
    for p in 770..=776 {
        assert_eq!(
            hash(
                "fireworks",
                V::Fireworks(Fireworks {
                    flight_duration: 3,
                    explosions: vec![
                        FireworkExplosion {
                            shape: FireworkShape::Burst,
                            colors: vec![-2147483648, 2147483647],
                            fade_colors: vec![],
                            has_trail: false,
                            has_twinkle: false
                        },
                        FireworkExplosion {
                            shape: FireworkShape::Creeper,
                            colors: vec![],
                            fade_colors: vec![],
                            has_trail: false,
                            has_twinkle: false
                        }
                    ]
                }),
                p
            )
            .unwrap(),
            88845994
        );
    }
    // fireworks_negative
    for p in 770..=776 {
        assert_eq!(
            hash(
                "fireworks",
                V::Fireworks(Fireworks {
                    flight_duration: -1,
                    explosions: vec![]
                }),
                p
            )
            .unwrap(),
            -400630225
        );
    }
    // lodestone_default
    for p in 770..=776 {
        assert_eq!(
            hash(
                "lodestone_tracker",
                V::LodestoneTracker(LodestoneTracker {
                    target: None,
                    tracked: true
                }),
                p
            )
            .unwrap(),
            -982207288
        );
    }
    // lodestone_target
    for p in 770..=776 {
        assert_eq!(
            hash(
                "lodestone_tracker",
                V::LodestoneTracker(LodestoneTracker {
                    target: Some(LodestoneTarget {
                        dimension: "overworld".into(),
                        position: BlockPosition {
                            x: -17,
                            y: -64,
                            z: 30000000
                        }
                    }),
                    tracked: false
                }),
                p
            )
            .unwrap(),
            -674177466
        );
    }
    // writable_default
    for p in 770..=776 {
        assert_eq!(
            hash("writable_book_content", V::WritableBook(vec![]), p).unwrap(),
            -982207288
        );
    }
    // writable_pages
    for p in 770..=776 {
        assert_eq!(
            hash(
                "writable_book_content",
                V::WritableBook(vec![
                    Filterable {
                        raw: "raw 🚀".into(),
                        filtered: None
                    },
                    Filterable {
                        raw: "secret".into(),
                        filtered: Some("clean".into())
                    }
                ]),
                p
            )
            .unwrap(),
            -1454823590
        );
    }
    // written_default
    for p in 770..=776 {
        assert_eq!(
            hash(
                "written_book_content",
                V::WrittenBook(WrittenBook {
                    title: Filterable {
                        raw: "Title".into(),
                        filtered: None
                    },
                    author: "A".into(),
                    generation: 0,
                    pages: vec![],
                    resolved: false
                }),
                p
            )
            .unwrap(),
            1274368349
        );
    }
    // written_full
    for p in 770..=776 {
        assert_eq!(
            hash(
                "written_book_content",
                V::WrittenBook(WrittenBook {
                    title: Filterable {
                        raw: "Raw".into(),
                        filtered: Some("Clean".into())
                    },
                    author: "λ".into(),
                    generation: 2,
                    pages: vec![
                        Filterable {
                            raw: Nbt::anonymous(Tag::String("hello 🚀".into())),
                            filtered: None
                        },
                        Filterable {
                            raw: Nbt::anonymous(Tag::Compound(vec![
                                ("text".into(), Tag::String("secret".into())),
                                ("bold".into(), Tag::Byte(1))
                            ])),
                            filtered: Some(Nbt::anonymous(Tag::String("clean".into())))
                        }
                    ],
                    resolved: true
                }),
                p
            )
            .unwrap(),
            1593820946
        );
    }
    // swing_default
    for p in 774..=776 {
        assert_eq!(
            hash(
                "swing_animation",
                V::SwingAnimation(SwingAnimation {
                    kind: SwingAnimationType::Whack,
                    duration: 6
                }),
                p
            )
            .unwrap(),
            -982207288
        );
    }
    // swing_full
    for p in 774..=776 {
        assert_eq!(
            hash(
                "swing_animation",
                V::SwingAnimation(SwingAnimation {
                    kind: SwingAnimationType::Stab,
                    duration: 9
                }),
                p
            )
            .unwrap(),
            -1715018116
        );
    }
    // attack_range_default
    for p in 774..=776 {
        assert_eq!(
            hash(
                "attack_range",
                V::AttackRange(AttackRange {
                    min_range: 0.0,
                    max_range: 3.0,
                    min_creative_range: 0.0,
                    max_creative_range: 5.0,
                    hitbox_margin: 0.3,
                    mob_factor: 1.0
                }),
                p
            )
            .unwrap(),
            -982207288
        );
    }
    // attack_range_full
    for p in 774..=776 {
        assert_eq!(
            hash(
                "attack_range",
                V::AttackRange(AttackRange {
                    min_range: 1.0,
                    max_range: 4.0,
                    min_creative_range: 2.0,
                    max_creative_range: 6.0,
                    hitbox_margin: 0.25,
                    mob_factor: 0.5
                }),
                p
            )
            .unwrap(),
            127588791
        );
    }
    // name_shadow
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::Compound(vec![
                    ("text".into(), Tag::String("x".into())),
                    ("shadow_color".into(), Tag::Int(-123))
                ]))),
                p
            )
            .unwrap(),
            1756798463
        );
    }
    // food_negative_zero
    for p in 770..=776 {
        assert_eq!(
            hash(
                "food",
                V::Food(Food {
                    nutrition: 0,
                    saturation_modifier: -0.0,
                    can_always_eat: false,
                    legacy_consumption: None
                }),
                p
            )
            .unwrap(),
            -1201875276
        );
    }
    // attack_range_reverse
    for p in 774..=776 {
        assert_eq!(
            hash(
                "attack_range",
                V::AttackRange(AttackRange {
                    min_range: 6.0,
                    max_range: 1.0,
                    min_creative_range: 0.0,
                    max_creative_range: 5.0,
                    hitbox_margin: 0.3,
                    mob_factor: 1.0
                }),
                p
            )
            .unwrap(),
            961867028
        );
    }
    // consumable_inline
    for p in 770..=776 {
        assert_eq!(
            hash(
                "consumable",
                V::Consumable(Consumable {
                    consume_seconds: 2.0,
                    animation: UseAnimation::Drink,
                    sound: RegistryHolder::Inline(SoundEvent {
                        name: "entity.generic.drink".into(),
                        fixed_range: None
                    }),
                    makes_particles: false,
                    effects: vec![
                        ConsumeEffect::ClearAllEffects,
                        ConsumeEffect::TeleportRandomly { diameter: 8.0 }
                    ]
                }),
                p
            )
            .unwrap(),
            -1456123329
        );
    }
    // death_default
    for p in 770..=776 {
        assert_eq!(
            hash("death_protection", V::DeathProtection(vec![]), p).unwrap(),
            -982207288
        );
    }
    // death_clear
    for p in 770..=776 {
        assert_eq!(
            hash(
                "death_protection",
                V::DeathProtection(vec![ConsumeEffect::ClearAllEffects]),
                p
            )
            .unwrap(),
            1103435691
        );
    }
    // death_teleport
    for p in 770..=776 {
        assert_eq!(
            hash(
                "death_protection",
                V::DeathProtection(vec![ConsumeEffect::TeleportRandomly { diameter: 16.0 }]),
                p
            )
            .unwrap(),
            -2083279098
        );
    }
    // death_teleport8
    for p in 770..=776 {
        assert_eq!(
            hash(
                "death_protection",
                V::DeathProtection(vec![ConsumeEffect::TeleportRandomly { diameter: 8.0 }]),
                p
            )
            .unwrap(),
            783827389
        );
    }
    // death_sound
    for p in 770..=776 {
        assert_eq!(
            hash(
                "death_protection",
                V::DeathProtection(vec![ConsumeEffect::PlaySound(RegistryHolder::Inline(
                    SoundEvent {
                        name: "block.note_block.bell".into(),
                        fixed_range: Some(16.0)
                    }
                ))]),
                p
            )
            .unwrap(),
            -1461947273
        );
    }
    // sound_plain
    for p in 770..=776 {
        assert_eq!(
            hash(
                "break_sound",
                V::Sound(RegistryHolder::Inline(SoundEvent {
                    name: "block.glass.break".into(),
                    fixed_range: None
                })),
                p
            )
            .unwrap(),
            1868462409
        );
    }
    // sound_fixed
    for p in 770..=776 {
        assert_eq!(
            hash(
                "break_sound",
                V::Sound(RegistryHolder::Inline(SoundEvent {
                    name: "block.glass.break".into(),
                    fixed_range: Some(-1.25)
                })),
                p
            )
            .unwrap(),
            -1572499816
        );
    }
    // consumable_inline_default
    for p in 770..=776 {
        assert_eq!(
            hash(
                "consumable",
                V::Consumable(Consumable {
                    consume_seconds: 1.6,
                    animation: UseAnimation::Eat,
                    sound: RegistryHolder::Inline(SoundEvent {
                        name: "entity.generic.eat".into(),
                        fixed_range: None
                    }),
                    makes_particles: true,
                    effects: vec![]
                }),
                p
            )
            .unwrap(),
            -1208256609
        );
    }
    // name_named_color
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::Compound(vec![
                    ("text".into(), Tag::String("x".into())),
                    ("color".into(), Tag::String("dark_red".into()))
                ]))),
                p
            )
            .unwrap(),
            -719209703
        );
    }
    // use_effects_zero
    for p in 774..=776 {
        assert_eq!(
            hash(
                "use_effects",
                V::UseEffects(UseEffects {
                    can_sprint: false,
                    interact_vibrations: true,
                    speed_multiplier: 0.0
                }),
                p
            )
            .unwrap(),
            2013407819
        );
    }
    // weapon_zero
    for p in 770..=776 {
        assert_eq!(
            hash(
                "weapon",
                V::Weapon(Weapon {
                    item_damage_per_attack: 0,
                    disable_blocking_seconds: 0.0
                }),
                p
            )
            .unwrap(),
            -1681045059
        );
    }
    // fireworks_large_negative
    for p in 770..=776 {
        assert_eq!(
            hash(
                "fireworks",
                V::Fireworks(Fireworks {
                    flight_duration: -129,
                    explosions: vec![]
                }),
                p
            )
            .unwrap(),
            -199857671
        );
    }
    // explosion_large
    for p in 770..=776 {
        assert_eq!(
            hash(
                "firework_explosion",
                V::FireworkExplosion(FireworkExplosion {
                    shape: FireworkShape::LargeBall,
                    colors: vec![],
                    fade_colors: vec![],
                    has_trail: false,
                    has_twinkle: false
                }),
                p
            )
            .unwrap(),
            1029322617
        );
    }
    // swing_none
    for p in 774..=776 {
        assert_eq!(
            hash(
                "swing_animation",
                V::SwingAnimation(SwingAnimation {
                    kind: SwingAnimationType::None,
                    duration: 6
                }),
                p
            )
            .unwrap(),
            -178357229
        );
    }
    // attack_range_boundaries
    for p in 774..=776 {
        assert_eq!(
            hash(
                "attack_range",
                V::AttackRange(AttackRange {
                    min_range: 64.0,
                    max_range: 0.0,
                    min_creative_range: 64.0,
                    max_creative_range: 0.0,
                    hitbox_margin: 1.0,
                    mob_factor: 2.0
                }),
                p
            )
            .unwrap(),
            382992097
        );
    }
    // consumable_zero
    for p in 770..=776 {
        assert_eq!(
            hash(
                "consumable",
                V::Consumable(Consumable {
                    consume_seconds: 0.0,
                    animation: UseAnimation::Eat,
                    sound: RegistryHolder::Inline(SoundEvent {
                        name: "entity.generic.eat".into(),
                        fixed_range: None
                    }),
                    makes_particles: true,
                    effects: vec![]
                }),
                p
            )
            .unwrap(),
            1714497686
        );
    }
    // name_list_nested
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::List {
                    element_type: TagType::Compound,
                    elements: vec![
                        Tag::Compound(vec![
                            ("text".into(), Tag::String("a".into())),
                            (
                                "extra".into(),
                                Tag::List {
                                    element_type: TagType::String,
                                    elements: vec![Tag::String("b".into())]
                                }
                            )
                        ]),
                        Tag::Compound(vec![("text".into(), Tag::String("c".into()))])
                    ]
                })),
                p
            )
            .unwrap(),
            1788936969
        );
    }
    // name_unpaired
    for p in 770..=776 {
        assert_eq!(
            hash(
                "custom_name",
                V::Nbt(Nbt::anonymous(Tag::String(NbtString(vec![0xd800])))),
                p
            )
            .unwrap(),
            -423001966
        );
    }
}

#[test]
fn official_signed_widths_and_float_bits() {
    for p in 770..=776 {
        for (duration, expected) in [
            (0, -982207288),
            (127, -199857671),
            (128, 1168984005),
            (255, -400630225),
            (-1, -400630225),
            (-129, -199857671),
        ] {
            assert_eq!(
                hash(
                    "fireworks",
                    V::Fireworks(Fireworks {
                        flight_duration: duration,
                        explosions: vec![]
                    }),
                    p
                )
                .unwrap(),
                expected
            );
        }
        assert!(matches!(
            hash(
                "fireworks",
                V::Fireworks(Fireworks {
                    flight_duration: 256,
                    explosions: vec![]
                }),
                p
            ),
            Err(Error::Invalid(_))
        ));
        for (number, expected) in [
            (-0.0, -1201875276),
            (f32::NAN, -2042695418),
            (f32::INFINITY, -58067332),
            (f32::NEG_INFINITY, -528399958),
        ] {
            assert_eq!(
                hash(
                    "food",
                    V::Food(Food {
                        nutrition: 0,
                        saturation_modifier: number,
                        can_always_eat: false,
                        legacy_consumption: None
                    }),
                    p
                )
                .unwrap(),
                expected
            );
        }
    }
}

fn default_attack_range() -> AttackRange {
    AttackRange {
        min_range: 0.0,
        max_range: 3.0,
        min_creative_range: 0.0,
        max_creative_range: 5.0,
        hitbox_margin: 0.3,
        mob_factor: 1.0,
    }
}
#[test]
fn persistent_range_errors_are_fail_closed() {
    for p in 770..=776 {
        for number in [-0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(matches!(
                hash(
                    "weapon",
                    V::Weapon(Weapon {
                        item_damage_per_attack: 1,
                        disable_blocking_seconds: number
                    }),
                    p
                ),
                Err(Error::Invalid(_))
            ));
            assert!(matches!(
                hash(
                    "use_cooldown",
                    V::UseCooldown(UseCooldown {
                        seconds: number,
                        group: None
                    }),
                    p
                ),
                Err(Error::Invalid(_))
            ));
        }
        for (name, value) in [
            (
                "food",
                V::Food(Food {
                    nutrition: -1,
                    saturation_modifier: 0.0,
                    can_always_eat: false,
                    legacy_consumption: None,
                }),
            ),
            (
                "weapon",
                V::Weapon(Weapon {
                    item_damage_per_attack: -1,
                    disable_blocking_seconds: 0.0,
                }),
            ),
            (
                "use_cooldown",
                V::UseCooldown(UseCooldown {
                    seconds: 0.0,
                    group: None,
                }),
            ),
            (
                "death_protection",
                V::DeathProtection(vec![ConsumeEffect::TeleportRandomly { diameter: 0.0 }]),
            ),
            (
                "written_book_content",
                V::WrittenBook(WrittenBook {
                    title: Filterable {
                        raw: "T".into(),
                        filtered: None,
                    },
                    author: "A".into(),
                    generation: 4,
                    pages: vec![],
                    resolved: false,
                }),
            ),
        ] {
            assert!(matches!(hash(name, value, p), Err(Error::Invalid(_))));
        }
    }
    for p in 774..=776 {
        for number in [-0.0, -1.0, 65.0, f32::NAN, f32::INFINITY] {
            assert!(matches!(
                hash(
                    "attack_range",
                    V::AttackRange(AttackRange {
                        min_range: number,
                        ..default_attack_range()
                    }),
                    p
                ),
                Err(Error::Invalid(_))
            ));
        }
        for number in [-0.0, -1.0, 2.0, f32::NAN, f32::INFINITY] {
            assert!(matches!(
                hash(
                    "use_effects",
                    V::UseEffects(UseEffects {
                        can_sprint: false,
                        interact_vibrations: true,
                        speed_multiplier: number
                    }),
                    p
                ),
                Err(Error::Invalid(_))
            ));
        }
        for duration in [-1, 0] {
            assert!(matches!(
                hash(
                    "swing_animation",
                    V::SwingAnimation(SwingAnimation {
                        kind: SwingAnimationType::Whack,
                        duration
                    }),
                    p
                ),
                Err(Error::Invalid(_))
            ));
        }
    }
}

#[test]
fn component_version_boundaries_and_animation_names() {
    assert!(matches!(
        hash(
            "food",
            V::Food(Food {
                nutrition: 0,
                saturation_modifier: 0.0,
                can_always_eat: false,
                legacy_consumption: None
            }),
            769
        ),
        Err(Error::Unsupported(_))
    ));
    for p in 770..=773 {
        assert!(matches!(
            hash(
                "use_effects",
                V::UseEffects(UseEffects {
                    can_sprint: false,
                    interact_vibrations: true,
                    speed_multiplier: 0.2
                }),
                p
            ),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            hash("attack_range", V::AttackRange(default_attack_range()), p),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            hash(
                "swing_animation",
                V::SwingAnimation(SwingAnimation {
                    kind: SwingAnimationType::Whack,
                    duration: 6
                }),
                p
            ),
            Err(Error::Unsupported(_))
        ));
    }
    for p in 770..=776 {
        let mut value = Consumable {
            consume_seconds: 1.6,
            animation: UseAnimation::Trident,
            sound: RegistryHolder::Inline(SoundEvent {
                name: "entity.generic.eat".into(),
                fixed_range: None,
            }),
            makes_particles: true,
            effects: vec![],
        };
        assert_eq!(
            hash("consumable", V::Consumable(value.clone()), p).unwrap(),
            if p < 774 { -1859981411 } else { 1833287472 }
        );
        value.animation = UseAnimation::Spear;
        if p >= 774 {
            assert_eq!(
                hash("consumable", V::Consumable(value), p).unwrap(),
                -1859981411
            );
        } else {
            assert!(hash("consumable", V::Consumable(value), p).is_err());
        }
    }
}

#[test]
fn unverified_text_and_registry_payloads_stay_unsupported() {
    for tag in [
        Tag::Compound(vec![(
            "translate".into(),
            Tag::String("item.minecraft.stone".into()),
        )]),
        Tag::Compound(vec![
            ("text".into(), Tag::String("x".into())),
            ("click_event".into(), Tag::Compound(vec![])),
        ]),
        Tag::Compound(vec![
            ("text".into(), Tag::String("x".into())),
            ("font".into(), Tag::Compound(vec![])),
        ]),
        Tag::Compound(vec![
            ("text".into(), Tag::String("x".into())),
            ("bold".into(), Tag::Int(1)),
        ]),
    ] {
        assert!(matches!(
            hash("custom_name", V::Nbt(Nbt::anonymous(tag)), 776),
            Err(Error::Unsupported(_))
        ));
    }
    assert!(matches!(
        hash("break_sound", V::Sound(RegistryHolder::RegistryId(0)), 776),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        hash(
            "death_protection",
            V::DeathProtection(vec![ConsumeEffect::ApplyEffects {
                effects: vec![],
                probability: 1.0
            }]),
            776
        ),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(
        hash(
            "custom_name",
            V::Nbt(Nbt::anonymous(Tag::List {
                element_type: TagType::End,
                elements: vec![]
            })),
            776
        ),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn literal_text_normalization_and_budget_validation() {
    let plain = Nbt::anonymous(Tag::String("same".into()));
    let duplicates = Nbt::anonymous(Tag::Compound(vec![
        ("text".into(), Tag::String("old".into())),
        ("text".into(), Tag::String("same".into())),
    ]));
    assert_eq!(
        hash("custom_name", V::Nbt(plain), 776).unwrap(),
        hash("custom_name", V::Nbt(duplicates), 776).unwrap()
    );
    let text = Component {
        name: "custom_name",
        value: V::Nbt(Nbt::anonymous(Tag::Compound(vec![
            ("text".into(), Tag::String("longer text 🚀".into())),
            ("bold".into(), Tag::Byte(1)),
        ]))),
    };
    for limits in [
        Limits {
            max_packet: 4,
            ..Limits::default()
        },
        Limits {
            max_string_chars: 2,
            ..Limits::default()
        },
        Limits {
            max_nbt_nodes: 1,
            ..Limits::default()
        },
        Limits {
            max_collection: 1,
            ..Limits::default()
        },
    ] {
        assert!(hash_component(&text, version(776), limits).is_err());
    }
    let mut tag = Tag::String("bottom".into());
    for _ in 0..12 {
        tag = Tag::Compound(vec![
            ("text".into(), Tag::String("a".into())),
            (
                "extra".into(),
                Tag::List {
                    element_type: tag.tag_type(),
                    elements: vec![tag],
                },
            ),
        ]);
    }
    let recursive = Component {
        name: "custom_name",
        value: V::Nbt(Nbt::anonymous(tag)),
    };
    assert!(hash_component(
        &recursive,
        version(776),
        Limits {
            max_nbt_depth: 8,
            ..Limits::default()
        }
    )
    .is_err());
    assert!(hash_component(&recursive, version(776), Limits::default()).is_ok());
}

#[test]
fn slot_hashes_use_verified_codecs_and_aggregate_limits() {
    let slot = Slot::Item(ItemStack {
        item_id: 1,
        count: 2,
        data: ItemData::Components(ComponentPatch {
            added: vec![
                Component {
                    name: "custom_name",
                    value: V::Nbt(Nbt::anonymous(Tag::Compound(vec![(
                        "text".into(),
                        Tag::String("Rustwire 🚀".into()),
                    )]))),
                },
                Component {
                    name: "food",
                    value: V::Food(Food {
                        nutrition: 7,
                        saturation_modifier: 1.25,
                        can_always_eat: true,
                        legacy_consumption: None,
                    }),
                },
            ],
            removed: vec![],
        }),
    });
    for p in 770..=776 {
        assert_eq!(
            HashedItemStack::from_slot(&slot, version(p), Limits::default())
                .unwrap()
                .unwrap()
                .components,
            vec![("custom_name", 1915252855), ("food", -523093613)]
        );
    }
    assert!(HashedItemStack::from_slot(
        &slot,
        version(776),
        Limits {
            max_packet: 8,
            ..Limits::default()
        }
    )
    .is_err());
    assert!(HashedItemStack::from_slot(
        &slot,
        version(776),
        Limits {
            max_collection: 1,
            ..Limits::default()
        }
    )
    .is_err());
}

#[test]
fn official_lore_limit_and_written_page_boundaries() {
    for p in 770..=776 {
        let line = Some(Nbt::anonymous(Tag::String("x".into())));
        assert_eq!(
            hash("lore", V::Lore(vec![line.clone(); 256]), p).unwrap(),
            -398462815
        );
        assert!(matches!(
            hash("lore", V::Lore(vec![line; 257]), p),
            Err(Error::Invalid(_))
        ));
        for (count, expected) in [
            (100, -355901934),
            (101, 2142002770),
            (256, 1959899898),
            (257, 809719999),
        ] {
            let book = WrittenBook {
                title: Filterable {
                    raw: "T".into(),
                    filtered: None,
                },
                author: "A".into(),
                generation: 0,
                pages: vec![
                    Filterable {
                        raw: Nbt::anonymous(Tag::String("x".into())),
                        filtered: None
                    };
                    count
                ],
                resolved: false,
            };
            assert_eq!(
                hash("written_book_content", V::WrittenBook(book), p).unwrap(),
                expected
            );
        }
        for (length, expected) in [
            (32765, -383599939),
            (32767, 1713229623),
            (32768, 44411054),
            (65535, 434533676),
        ] {
            let value = V::WrittenBook(WrittenBook {
                title: Filterable {
                    raw: "T".into(),
                    filtered: None,
                },
                author: "A".into(),
                generation: 0,
                pages: vec![Filterable {
                    raw: Nbt::anonymous(Tag::String("x".repeat(length).into())),
                    filtered: None,
                }],
                resolved: false,
            });
            assert_eq!(
                hash_component(
                    &Component {
                        name: "written_book_content",
                        value
                    },
                    version(p),
                    Limits {
                        max_string_chars: 65535,
                        ..Limits::default()
                    }
                )
                .unwrap(),
                expected
            );
        }
    }
}
