use rustwire_mc::{
    chunk::{ContainerKind, Palette, PaletteContainer},
    codec::{Reader, Writer},
    Limits, Version,
};

fn accepts(value: &PaletteContainer, version: Version) -> bool {
    let mut w = Writer::new();
    if value.write(&mut w, version, Limits::default()).is_err() {
        return false;
    }
    let bytes = w.into_inner();
    let mut reader = Reader::new(&bytes, Limits::default());
    let decoded = PaletteContainer::read(&mut reader, version, value.kind).unwrap();
    reader.finish().unwrap();
    assert_eq!(&decoded, value);
    true
}
fn entries(kind: ContainerKind) -> usize {
    match kind {
        ContainerKind::Blocks => 4096,
        ContainerKind::Biomes => 64,
    }
}
#[test]
fn packed_word_validation_matches_independent_per_entry_access() {
    let mut rng = 0x9e3779b97f4a7c15u64;
    for kind in [ContainerKind::Blocks, ContainerKind::Biomes] {
        let widths = match kind {
            ContainerKind::Blocks => 4..=8,
            ContainerKind::Biomes => 1..=3,
        };
        for bits in widths {
            for size in [1, 2, (1usize << bits) - 1, 1usize << bits] {
                for _ in 0..16 {
                    let words = entries(kind).div_ceil(64 / bits as usize);
                    let data = (0..words)
                        .map(|_| {
                            rng ^= rng << 13;
                            rng ^= rng >> 7;
                            rng ^= rng << 17;
                            rng
                        })
                        .collect();
                    let value = PaletteContainer {
                        kind,
                        bits_per_entry: bits,
                        palette: Palette::Indirect((0..size as u32).collect()),
                        data,
                    };
                    let expected = (0..entries(kind)).all(|i| value.get(i).is_some());
                    for version in [Version::V1_20, Version::V26_2] {
                        assert_eq!(accepts(&value, version), expected);
                    }
                }
            }
        }
    }
}
#[test]
fn incomplete_last_word_ignores_padding_but_checks_last_used_index() {
    for &version in Version::ALL {
        let mut value = PaletteContainer {
            kind: ContainerKind::Biomes,
            bits_per_entry: 3,
            palette: Palette::Indirect(vec![10, 11, 12]),
            data: vec![0; 4],
        };
        value.data[3] = u64::MAX << 3; // Only index 63 is used in this word.
        assert!(accepts(&value, version));
        assert_eq!(value.get(63), Some(10));
        value.data[3] |= 3; // Out-of-range final meaningful index.
        assert!(!accepts(&value, version));
        value.data[3] &= !7;
        value.data[0] |= 3; // Also check the very first index.
        assert!(!accepts(&value, version));
    }
}
#[test]
fn direct_domains_and_full_indirect_domains_preserve_maximum_ids() {
    for &version in Version::ALL {
        for kind in [ContainerKind::Blocks, ContainerKind::Biomes] {
            for bits in [9, 15, 31] {
                let value = PaletteContainer {
                    kind,
                    bits_per_entry: bits,
                    palette: Palette::Direct,
                    data: vec![u64::MAX; entries(kind).div_ceil(64 / bits as usize)],
                };
                assert!(accepts(&value, version));
                assert_eq!(
                    value.get(entries(kind) - 1),
                    Some(((1u64 << bits) - 1) as u32)
                );
            }
        }
        let mut full = PaletteContainer {
            kind: ContainerKind::Blocks,
            bits_per_entry: 8,
            palette: Palette::Indirect((0..256).collect()),
            data: vec![u64::MAX; 512],
        };
        assert!(accepts(&full, version));
        assert_eq!(full.get(4095), Some(255));
        let Palette::Indirect(ref mut ids) = full.palette else {
            unreachable!()
        };
        ids[0] = i32::MAX as u32 + 1;
        assert!(!accepts(&full, version));
    }
}

#[test]
fn indirect_word_edges_reject_real_indices_and_ignore_word_padding() {
    for kind in [ContainerKind::Blocks, ContainerKind::Biomes] {
        let widths = match kind {
            ContainerKind::Blocks => 4..=8,
            ContainerKind::Biomes => 2..=3,
        };
        for bits in widths {
            let per_word = 64 / bits as usize;
            let len = entries(kind);
            let mut value = PaletteContainer {
                kind,
                bits_per_entry: bits,
                palette: Palette::Indirect(vec![0, 1]),
                data: vec![0; len.div_ceil(per_word)],
            };
            let used_bits = per_word * bits as usize;
            if used_bits < 64 {
                value.data.fill(u64::MAX << used_bits);
            }
            assert!(accepts(&value, Version::V26_2));
            for index in [0, per_word - 1, per_word, len - 1] {
                let shift = (index % per_word) * bits as usize;
                value.data[index / per_word] |= 2u64 << shift;
                assert!(!accepts(&value, Version::V26_2));
                // Independently assemble an invalid modern wire body as the
                // production writer correctly refuses this invalid value.
                let mut raw = vec![bits, 2, 0, 1];
                for &word in &value.data {
                    raw.extend(word.to_be_bytes());
                }
                assert!(PaletteContainer::read(
                    &mut Reader::new(&raw, Limits::default()),
                    Version::V26_2,
                    kind
                )
                .is_err());
                value.data[index / per_word] &= !(2u64 << shift);
            }
        }
    }
}
