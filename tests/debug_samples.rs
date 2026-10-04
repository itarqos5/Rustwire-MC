use rustwire_mc::{
    codec::{Reader, Writer},
    packet::{
        debug::{DebugSample, DebugSampleKind, DebugSampleSubscription, DebugSubscriptionRequest},
        typed::DecodedPacket,
    },
    version::{Direction, State},
    Limits, Version,
};
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(), 16).unwrap())
        .collect()
}
fn samples(case: &str) -> Vec<i64> {
    match case {
        "empty" => vec![],
        "zero" => vec![0],
        "signed_edges" => vec![-1, i64::MIN, i64::MAX],
        "wide_count" => (0..128).collect(),
        _ => panic!("case"),
    }
}
fn subscriptions(case: &str) -> Vec<u32> {
    match case {
        "empty" => vec![],
        "tick_time" => vec![0],
        "all_builtin" => (0..16).collect(),
        "duplicates" => vec![2, 2, 0],
        "unresolved_references" => vec![127, 128, 16383, 16384, i32::MAX as u32],
        "maximum_count" => vec![0; 32],
        _ => panic!("case"),
    }
}
macro_rules! verify {
    ($ty:ty,$value:expr,$v:expr,$body:expr,$id:expr) => {{
        let p = $value;
        let v = $v;
        let bytes: &[u8] = $body;
        let limits = Limits::default();
        assert_eq!(<$ty>::decode(bytes, v, limits).unwrap(), p);
        assert_eq!(p.encode(v, limits).unwrap(), bytes);
        assert_eq!(p.packet(v, limits).unwrap().id, $id);
        assert_eq!(p.packet(v, limits).unwrap().data, bytes);
        for end in 0..bytes.len() {
            assert!(<$ty>::decode(&bytes[..end], v, limits).is_err());
            let mut r = Reader::new(&bytes[..end], limits);
            assert!(<$ty>::read(&mut r, v).is_err());
            assert_eq!(r.position(), 0);
        }
        let mut extra = bytes.to_vec();
        extra.push(0xa5);
        assert!(<$ty>::decode(&extra, v, limits).is_err());
        let mut r = Reader::new(&extra, limits);
        assert_eq!(<$ty>::read(&mut r, v).unwrap(), p);
        assert_eq!(r.remaining(), &[0xa5]);
        let mut prefixed = vec![0x55];
        prefixed.extend(bytes);
        prefixed.push(0x66);
        let mut r = Reader::new(&prefixed, limits);
        assert_eq!(r.u8().unwrap(), 0x55);
        assert_eq!(<$ty>::read(&mut r, v).unwrap(), p);
        assert_eq!(r.remaining(), &[0x66]);
        let exact = Limits {
            max_packet: bytes.len(),
            ..limits
        };
        assert_eq!(p.encode(v, exact).unwrap(), bytes);
        assert_eq!(<$ty>::decode(bytes, v, exact).unwrap(), p);
        let small = Limits {
            max_packet: bytes.len() - 1,
            ..limits
        };
        assert!(p.encode(v, small).is_err());
        assert!(<$ty>::decode(bytes, v, small).is_err());
        let mut w = Writer::new();
        w.u8(0x55);
        assert!(p.write(&mut w, v, small).is_err());
        assert_eq!(w.as_slice(), &[0x55]);
        p.write(&mut w, v, limits).unwrap();
        assert_eq!(&w.as_slice()[1..], bytes);
    }};
}
#[test]
fn all_independent_fixtures_exact_ids_semantics_prefixes_and_atomic_io() {
    let mut total = 0;
    let mut prefixes = 0;
    for line in include_str!("fixtures/debug-samples.tsv")
        .lines()
        .filter(|s| !s.starts_with('#'))
    {
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 6);
        let v = Version::from_protocol(f[0].parse().unwrap()).unwrap();
        let body = hex(f[5]);
        let id = f[4].parse::<i32>().unwrap();
        let direction = if f[2] == "toClient" {
            Direction::Clientbound
        } else {
            Direction::Serverbound
        };
        assert_eq!(v.packet_id(State::Play, direction, f[1]).unwrap(), id);
        match f[1] {
            "debug_sample" => verify!(
                DebugSample,
                DebugSample {
                    samples: samples(f[3]),
                    kind: DebugSampleKind::TickTime
                },
                v,
                &body,
                id
            ),
            "debug_sample_subscription" => verify!(
                DebugSampleSubscription,
                DebugSampleSubscription {
                    kind: DebugSampleKind::TickTime
                },
                v,
                &body,
                id
            ),
            "debug_subscription_request" => verify!(
                DebugSubscriptionRequest,
                DebugSubscriptionRequest {
                    subscriptions: subscriptions(f[3])
                },
                v,
                &body,
                id
            ),
            _ => panic!("fixture name"),
        }
        let typed = DecodedPacket::decode(State::Play, f[1], &body, v, Limits::default()).unwrap();
        if f[1] == "debug_sample" {
            assert!(
                matches!(typed,Some(DecodedPacket::DebugSample(p)) if p.samples == samples(f[3]))
            );
        } else {
            assert!(typed.is_none());
        }
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
        ] {
            assert!(v.packet_id(state, direction, f[1]).is_err());
            assert!(
                DecodedPacket::decode(state, f[1], &body, v, Limits::default())
                    .unwrap()
                    .is_none()
            );
        }
        total += 1;
        prefixes += body.len();
    }
    assert_eq!(total, 75);
    assert_eq!(prefixes, 12006);
}
#[test]
fn exact_family_replacement_gates() {
    for &v in Version::ALL {
        let limits = Limits::default();
        let sample = DebugSample {
            samples: vec![],
            kind: DebugSampleKind::TickTime,
        };
        let old = DebugSampleSubscription {
            kind: DebugSampleKind::TickTime,
        };
        let new = DebugSubscriptionRequest {
            subscriptions: vec![],
        };
        assert_eq!(sample.encode(v, limits).is_ok(), v.protocol() >= 766);
        assert_eq!(
            DebugSample::decode(&[0, 0], v, limits).is_ok(),
            v.protocol() >= 766
        );
        assert_eq!(
            old.encode(v, limits).is_ok(),
            (766..=772).contains(&v.protocol())
        );
        assert_eq!(
            DebugSampleSubscription::decode(&[0], v, limits).is_ok(),
            (766..=772).contains(&v.protocol())
        );
        assert_eq!(new.encode(v, limits).is_ok(), v.protocol() >= 773);
        assert_eq!(
            DebugSubscriptionRequest::decode(&[0], v, limits).is_ok(),
            v.protocol() >= 773
        );
    }
}
#[test]
fn collection_caps_unresolved_ids_and_missing_bytes_preflight() {
    let v = Version::V26_2;
    let limits = Limits::default();
    let small = Limits {
        max_collection: 0,
        ..limits
    };
    let empty = DebugSample {
        samples: vec![],
        kind: DebugSampleKind::TickTime,
    };
    assert_eq!(empty.encode(v, small).unwrap(), [0, 0]);
    let one = DebugSample {
        samples: vec![i64::MIN],
        kind: DebugSampleKind::TickTime,
    };
    let body = one.encode(v, limits).unwrap();
    assert!(one.encode(v, small).is_err());
    assert!(DebugSample::decode(&body, v, small).is_err());
    for maximum in [limits.max_collection, usize::MAX] {
        assert!(DebugSample::decode(
            &[0xff, 0xff, 0xff, 0xff, 0x07, 0],
            v,
            Limits {
                max_collection: maximum,
                ..limits
            }
        )
        .is_err());
    }
    let mut request = DebugSubscriptionRequest {
        subscriptions: vec![i32::MAX as u32; 32],
    };
    let bytes = request.encode(v, limits).unwrap();
    assert_eq!(bytes.len(), 161);
    assert_eq!(
        DebugSubscriptionRequest::decode(&bytes, v, limits).unwrap(),
        request
    );
    assert!(request
        .encode(
            v,
            Limits {
                max_collection: 31,
                ..limits
            }
        )
        .is_err());
    request.subscriptions.push(0);
    assert!(request.encode(v, limits).is_err());
    assert!(DebugSubscriptionRequest::decode(&[33], v, limits).is_err());
    assert!(DebugSubscriptionRequest::decode(&[32], v, limits).is_err());
    assert_eq!(
        DebugSubscriptionRequest {
            subscriptions: vec![]
        }
        .encode(v, small)
        .unwrap(),
        [0]
    );
    let mut w = Writer::new();
    w.u8(7);
    for bad in [i32::MAX as u32 + 1, u32::MAX] {
        assert!(DebugSubscriptionRequest {
            subscriptions: vec![bad]
        }
        .write(&mut w, v, limits)
        .is_err());
        assert_eq!(w.as_slice(), &[7]);
    }
}
#[test]
fn malformed_types_varints_and_negative_references_fail_atomically() {
    let limits = Limits::default();
    let v = Version::V26_2;
    for id in [-1, i32::MIN, 1, 127, i32::MAX] {
        let mut w = Writer::new();
        w.u8(0);
        w.var_i32(id);
        assert!(DebugSample::decode(w.as_slice(), v, limits).is_err());
        let mut w = Writer::new();
        w.var_i32(id);
        assert!(DebugSampleSubscription::decode(w.as_slice(), Version::V1_20_5, limits).is_err());
    }
    for bad in [
        &[0xff, 0xff, 0xff, 0xff, 0x0f][..],
        &[0x80, 0x80, 0x80, 0x80, 0x80, 0][..],
    ] {
        assert!(DebugSample::decode(bad, v, limits).is_err());
        assert!(DebugSubscriptionRequest::decode(bad, v, limits).is_err());
        let mut r = Reader::new(bad, limits);
        assert!(DebugSample::read(&mut r, v).is_err());
        assert_eq!(r.position(), 0);
    }
    let negative_id = [1, 0xff, 0xff, 0xff, 0xff, 0x0f];
    assert!(DebugSubscriptionRequest::decode(&negative_id, v, limits).is_err());
    let mut r = Reader::new(&negative_id, limits);
    assert!(DebugSubscriptionRequest::read(&mut r, v).is_err());
    assert_eq!(r.position(), 0);
    let overlong = DebugSample::decode(&[0x80, 0, 0x80, 0], v, limits).unwrap();
    assert_eq!(overlong.encode(v, limits).unwrap(), [0, 0]);
    let overlong = DebugSubscriptionRequest::decode(&[0x81, 0, 0x80, 0], v, limits).unwrap();
    assert_eq!(overlong.encode(v, limits).unwrap(), [1, 0]);
}
