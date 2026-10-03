use rustwire_mc::{
    codec::{Reader, Writer},
    packet::statistics::{Statistic, Statistics},
    Error, Limits, Version,
};

// Hand-authored bytes independent of Rustwire's encoder. Duplicate keys and
// signed counters are retained as wire entries, not interpreted as client state.
const FIXTURE: &[u8] = &[
    3, 0, 1, 0xac, 2, 8, 0x80, 1, 0xff, 0xff, 0xff, 0xff, 7, 8, 0x80, 1, 0xff, 0xff, 0xff, 0xff, 15,
];
fn expected() -> Statistics {
    Statistics {
        entries: vec![
            Statistic {
                category_id: 0,
                statistic_id: 1,
                value: 300,
            },
            Statistic {
                category_id: 8,
                statistic_id: 128,
                value: i32::MAX,
            },
            Statistic {
                category_id: 8,
                statistic_id: 128,
                value: -1,
            },
        ],
    }
}

#[test]
fn independent_statistics_fixtures_all_families() {
    for &v in Version::ALL {
        assert_eq!(
            Statistics::decode(FIXTURE, v, Limits::default()).unwrap(),
            expected()
        );
        assert_eq!(expected().encode(v, Limits::default()).unwrap(), FIXTURE);
        assert_eq!(
            Statistics::decode(&[0], v, Limits::default()).unwrap(),
            Statistics::default()
        );
        assert_eq!(
            Statistics::default().encode(v, Limits::default()).unwrap(),
            [0]
        );
    }
}

#[test]
fn statistics_reject_every_truncation_and_trailing_byte() {
    for &v in Version::ALL {
        for end in 0..FIXTURE.len() {
            assert!(Statistics::decode(&FIXTURE[..end], v, Limits::default()).is_err());
        }
        let mut trailing = FIXTURE.to_vec();
        trailing.push(0);
        assert!(Statistics::decode(&trailing, v, Limits::default()).is_err());
    }
}

#[test]
fn statistics_reject_bad_counts_and_varints_before_large_allocation() {
    for &v in Version::ALL {
        for bytes in [
            vec![255, 255, 255, 255, 15],          // negative count
            vec![255, 255, 255, 255, 7],           // max count, no entries
            vec![255, 255, 255, 255, 16],          // overflowing count
            vec![1, 255, 255, 255, 255, 16, 0, 0], // overflowing category
            vec![1, 0, 255, 255, 255, 255, 16, 0], // overflowing statistic
            vec![1, 0, 0, 255, 255, 255, 255, 16], // overflowing value
        ] {
            assert!(Statistics::decode(
                &bytes,
                v,
                Limits {
                    max_collection: usize::MAX,
                    ..Limits::default()
                }
            )
            .is_err());
        }
    }
}

#[test]
fn statistics_collection_and_packet_limits_apply_both_ways() {
    for &v in Version::ALL {
        for max_collection in 0..3 {
            let limits = Limits {
                max_collection,
                ..Limits::default()
            };
            assert!(matches!(
                Statistics::decode(FIXTURE, v, limits),
                Err(Error::Limit(_))
            ));
            assert!(matches!(expected().encode(v, limits), Err(Error::Limit(_))));
        }
        for max_packet in 0..FIXTURE.len() {
            let limits = Limits {
                max_packet,
                ..Limits::default()
            };
            assert!(Statistics::decode(FIXTURE, v, limits).is_err());
            assert!(expected().encode(v, limits).is_err());
            let mut r = Reader::new(FIXTURE, limits);
            assert!(Statistics::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
        }
        let limits = Limits {
            max_collection: 3,
            max_packet: FIXTURE.len(),
            ..Limits::default()
        };
        assert_eq!(Statistics::decode(FIXTURE, v, limits).unwrap(), expected());
        assert_eq!(expected().encode(v, limits).unwrap(), FIXTURE);
    }
}

#[test]
fn statistics_read_one_body_and_write_failures_are_transactional() {
    for &v in Version::ALL {
        let bytes = [FIXTURE, &[42]].concat();
        let mut r = Reader::new(&bytes, Limits::default());
        assert_eq!(Statistics::read(&mut r, v).unwrap(), expected());
        assert_eq!(r.remaining(), &[42]);
        let mut r = Reader::new(&FIXTURE[..FIXTURE.len() - 1], Limits::default());
        assert!(Statistics::read(&mut r, v).is_err());
        assert_eq!(r.position(), 0);
        let mut w = Writer::new();
        w.u8(42);
        for limits in [
            Limits {
                max_packet: FIXTURE.len() - 1,
                ..Limits::default()
            },
            Limits {
                max_collection: 2,
                ..Limits::default()
            },
        ] {
            assert!(expected().write(&mut w, v, limits).is_err());
            assert_eq!(w.as_slice(), &[42]);
        }
        expected().write(&mut w, v, Limits::default()).unwrap();
        assert_eq!(w.as_slice(), [&[42], FIXTURE].concat());
    }
}

#[test]
fn statistics_do_not_guess_registry_membership_or_counter_domain() {
    for &v in Version::ALL {
        let bytes = [
            1, 255, 255, 255, 255, 15, 255, 255, 255, 255, 7, 128, 128, 128, 128, 8,
        ];
        let value = Statistics::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(
            value.entries,
            [Statistic {
                category_id: -1,
                statistic_id: i32::MAX,
                value: i32::MIN
            }]
        );
        assert_eq!(value.encode(v, Limits::default()).unwrap(), bytes);
    }
}
