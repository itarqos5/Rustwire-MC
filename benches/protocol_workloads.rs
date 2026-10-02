//! Synthetic allocation-inclusive codec baselines, not network throughput claims.
use rustwire_mc::{
    chunk::{
        ChunkData, ChunkSection, ContainerKind, Heightmaps, LightData, Palette, PaletteContainer,
    },
    nbt::{Nbt, Tag, TagType},
    packet::{
        chunk_updates::{ChunkBiomeData, ChunkBiomes, UpdateLight},
        inventory::{Component, ComponentValue},
        item_hash::hash_component,
    },
    Limits, Version,
};
use std::{hint::black_box, time::Instant};

fn measure<T>(
    name: &str,
    iterations: usize,
    input_bytes: Option<usize>,
    mut run: impl FnMut() -> T,
) {
    for _ in 0..8 {
        black_box(run());
    }
    let mut samples = [0.0; 5];
    for sample in &mut samples {
        let start = Instant::now();
        for _ in 0..iterations {
            // Construction and destruction of the decoded value are both timed.
            black_box(run());
        }
        *sample = start.elapsed().as_nanos() as f64 / iterations as f64;
    }
    samples.sort_by(f64::total_cmp);
    println!("{name}: iterations={iterations} samples=5 input_bytes={input_bytes:?} median_ns={:.2} min_ns={:.2} max_ns={:.2}", samples[2], samples[0], samples[4]);
}

fn biomes() -> PaletteContainer {
    PaletteContainer {
        kind: ContainerKind::Biomes,
        bits_per_entry: 1,
        palette: Palette::Indirect(vec![1, 7]),
        data: vec![0xaaaaaaaaaaaaaaaa_u64],
    }
}
fn main() {
    let version = Version::V26_2;
    let limits = Limits::default();
    let section = ChunkSection {
        non_air_count: 2048,
        fluid_count: Some(0),
        blocks: PaletteContainer {
            kind: ContainerKind::Blocks,
            bits_per_entry: 4,
            palette: Palette::Indirect(vec![0, 1]),
            data: vec![0x1010101010101010; 256],
        },
        biomes: biomes(),
    };
    let light = LightData {
        sky_mask: vec![(1 << 26) - 1],
        block_mask: vec![1 << 17],
        empty_sky_mask: vec![0],
        empty_block_mask: vec![((1 << 26) - 1) ^ (1 << 17)],
        sky_arrays: vec![vec![255; 2048]; 26],
        block_arrays: vec![vec![0x21; 2048]],
    };
    let chunk = ChunkData {
        x: -17,
        z: 32,
        heightmaps: Heightmaps::Typed(vec![]),
        sections: vec![section; 24],
        block_entities: vec![],
        light: light.clone(),
    };
    let chunk_bytes = chunk.encode(version, limits).unwrap();
    assert_eq!(
        ChunkData::decode(&chunk_bytes, version, 24, limits).unwrap(),
        chunk
    );
    let lighting = UpdateLight {
        chunk_x: -17,
        chunk_z: 32,
        light,
    };
    let light_bytes = lighting.encode(version, 24, limits).unwrap();
    assert_eq!(
        UpdateLight::decode(&light_bytes, version, 24, limits).unwrap(),
        lighting
    );
    let biome_update = ChunkBiomes {
        chunks: (0..9)
            .map(|i| ChunkBiomeData {
                chunk_x: i % 3,
                chunk_z: i / 3,
                sections: vec![biomes(); 24],
            })
            .collect(),
    };
    let biome_bytes = biome_update.encode(version, 24, limits).unwrap();
    assert_eq!(
        ChunkBiomes::decode(&biome_bytes, version, 24, limits).unwrap(),
        biome_update
    );
    let name = Component {
        name: "custom_name",
        value: ComponentValue::Nbt(Nbt::anonymous(Tag::Compound(vec![
            ("text".into(), Tag::String("Rustwire 🚀".into())),
            ("italic".into(), Tag::Byte(1)),
            ("color".into(), Tag::String("red".into())),
            (
                "extra".into(),
                Tag::List {
                    element_type: TagType::String,
                    elements: vec![Tag::String("example".into())],
                },
            ),
        ]))),
    };
    hash_component(&name, version, limits).unwrap();
    println!("protocol={} fixture=synthetic_mixed_palettes sections=24; compile-time defaults and active features are selected by Cargo", version.protocol());
    measure("chunk_decode", 500, Some(chunk_bytes.len()), || {
        ChunkData::decode(black_box(&chunk_bytes), version, 24, limits).unwrap()
    });
    measure("chunk_encode", 500, None, || {
        black_box(&chunk).encode(version, limits).unwrap()
    });
    measure("light_decode", 5_000, Some(light_bytes.len()), || {
        UpdateLight::decode(black_box(&light_bytes), version, 24, limits).unwrap()
    });
    measure(
        "biome_decode_nine_chunks",
        1_000,
        Some(biome_bytes.len()),
        || ChunkBiomes::decode(black_box(&biome_bytes), version, 24, limits).unwrap(),
    );
    measure("styled_name_hash", 20_000, None, || {
        hash_component(black_box(&name), version, limits).unwrap()
    });
}
