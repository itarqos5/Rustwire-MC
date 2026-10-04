use rustwire_mc::{
    codec::Writer,
    packet::{common::CommonPacket, typed::DecodedPacket},
    version::{Direction, State},
    Limits, Version,
};

#[test]
fn direct_typed_dispatch_does_not_invent_common_state_tuples() {
    for (version, state, name, body) in [
        (
            Version::V26_2,
            State::Configuration,
            "bundle_delimiter",
            &[][..],
        ),
        (
            Version::V26_2,
            State::Configuration,
            "chunk_batch_start",
            &[][..],
        ),
        (Version::V26_2, State::Play, "code_of_conduct", &[0][..]),
        (Version::V1_20_2, State::Play, "feature_flags", &[0][..]),
        (Version::V1_20, State::Play, "chunk_batch_start", &[][..]),
    ] {
        assert!(
            DecodedPacket::decode(state, name, body, version, Limits::default())
                .unwrap()
                .is_none(),
            "{} {state:?} {name}",
            version.protocol()
        );
    }
}

fn present(protocol: i32, state: State, name: &str) -> bool {
    if !matches!(state, State::Configuration | State::Play)
        || (state == State::Configuration && protocol == 763)
    {
        return false;
    }
    match name {
        "reset_chat" => state == State::Configuration && protocol >= 766,
        "custom_report_details" | "server_links" => protocol >= 767,
        "resource_pack_send" => protocol < 765,
        "add_resource_pack" | "remove_resource_pack" => protocol >= 765,
        "transfer" | "store_cookie" => protocol >= 766,
        "tags" => true,
        "feature_flags" => {
            (protocol == 763 && state == State::Play)
                || (protocol >= 764 && state == State::Configuration)
        }
        "code_of_conduct" => state == State::Configuration && protocol >= 773,
        "chunk_batch_start" | "chunk_batch_finished" => state == State::Play && protocol >= 764,
        "bundle_delimiter" => state == State::Play,
        _ => false,
    }
}
fn body(protocol: i32, name: &str) -> Vec<u8> {
    let mut w = Writer::new();
    match name {
        "resource_pack_send" | "add_resource_pack" => {
            if protocol >= 765 {
                w.raw(&[0; 16]);
            }
            w.string("x", 32767).unwrap();
            w.string("", 40).unwrap();
            w.bool(false);
            w.bool(false);
        }
        "transfer" => {
            w.string("x", 32767).unwrap();
            w.var_i32(1);
        }
        "store_cookie" => {
            w.string("x", 32767).unwrap();
            w.bytes(&[]).unwrap();
        }
        "reset_chat" | "chunk_batch_start" | "bundle_delimiter" => (),
        _ => w.u8(0),
    }
    w.into_inner()
}
#[test]
fn common_catalog_state_matrix_matches_independent_version_boundaries() {
    let names = [
        "reset_chat",
        "custom_report_details",
        "resource_pack_send",
        "add_resource_pack",
        "remove_resource_pack",
        "transfer",
        "store_cookie",
        "tags",
        "feature_flags",
        "code_of_conduct",
        "server_links",
        "chunk_batch_start",
        "chunk_batch_finished",
        "bundle_delimiter",
    ];
    let limits = Limits::default();
    let mut combinations = 0;
    for &v in Version::ALL {
        for state in [
            State::Handshake,
            State::Status,
            State::Login,
            State::Configuration,
            State::Play,
        ] {
            for name in names {
                let expected = present(v.protocol(), state, name);
                let bytes = body(v.protocol(), name);
                assert_eq!(
                    v.packet_id(state, Direction::Clientbound, name).is_ok(),
                    expected,
                    "catalog {} {state:?} {name}",
                    v.protocol()
                );
                assert_eq!(
                    CommonPacket::decode_in_state(state, name, &bytes, v, limits)
                        .unwrap()
                        .is_some(),
                    expected,
                    "common {} {state:?} {name}",
                    v.protocol()
                );
                assert_eq!(
                    DecodedPacket::decode(state, name, &bytes, v, limits)
                        .unwrap()
                        .is_some(),
                    expected,
                    "typed {} {state:?} {name}",
                    v.protocol()
                );
                if expected {
                    let mut trailing = bytes;
                    trailing.push(0);
                    assert!(
                        CommonPacket::decode_in_state(state, name, &trailing, v, limits).is_err()
                    );
                } else {
                    // Wrong tuples must not try parsing an untrusted body.
                    assert!(CommonPacket::decode_in_state(
                        state,
                        name,
                        &[255; 8],
                        v,
                        Limits {
                            max_packet: 0,
                            ..limits
                        }
                    )
                    .unwrap()
                    .is_none());
                }
                combinations += 1;
            }
        }
    }
    assert_eq!(combinations, 980);
    assert!(
        CommonPacket::decode_in_state(State::Play, "unknown", &[], Version::V26_2, limits)
            .unwrap()
            .is_none()
    );
}
#[test]
fn name_only_batch_helpers_enforce_the_introduction_boundary() {
    for (name, bytes) in [
        ("chunk_batch_start", &[][..]),
        ("chunk_batch_finished", &[0][..]),
    ] {
        assert!(CommonPacket::decode(name, bytes, Version::V1_20, Limits::default()).is_err());
        for &v in Version::ALL.iter().skip(1) {
            assert!(CommonPacket::decode(name, bytes, v, Limits::default())
                .unwrap()
                .is_some());
        }
    }
}
