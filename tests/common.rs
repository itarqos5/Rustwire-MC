use rustwire_mc::{
    codec::Writer,
    packet::{chat::ChatComponent, common::*},
    version::State,
    Limits, Version,
};
#[test]
fn resource_pack_offer_golden_version_boundaries() {
    for &v in Version::ALL {
        let mut w = Writer::new();
        if v.protocol() >= 765 {
            w.raw(&[9; 16]);
        }
        w.string("https://example.invalid/pack.zip", 32767).unwrap();
        w.string("", 40).unwrap();
        w.bool(true);
        w.bool(true);
        if v.protocol() < 765 {
            w.string("{\"text\":\"Pack\"}", 32767).unwrap();
        } else {
            w.raw(&[8, 0, 4, b'P', b'a', b'c', b'k']);
        }
        let bytes = w.into_inner();
        let offer = ResourcePackOffer::decode(&bytes, v, Limits::default()).unwrap();
        assert!(offer.required);
        assert_eq!(offer.uuid.is_some(), v.protocol() >= 765);
        assert_eq!(
            matches!(offer.prompt, Some(ChatComponent::Json(_))),
            v.protocol() < 765
        );
        let state = if v.has_configuration() {
            State::Configuration
        } else {
            State::Play
        };
        let response = offer
            .response(v, state, ResourcePackStatus::Declined)
            .unwrap();
        assert_eq!(response.data.last(), Some(&1));
        assert_eq!(
            response.data.len(),
            if v.protocol() >= 765 { 17 } else { 1 }
        );
        for n in 0..bytes.len() {
            assert!(ResourcePackOffer::decode(&bytes[..n], v, Limits::default()).is_err());
        }
    }
}
#[test]
fn tags_golden_and_aggregate_limits() {
    let mut w = Writer::new();
    w.var_i32(1);
    w.string("minecraft:block", 32767).unwrap();
    w.var_i32(1);
    w.string("minecraft:mineable/pickaxe", 32767).unwrap();
    w.var_i32(2);
    w.var_i32(1);
    w.var_i32(7);
    let bytes = w.into_inner();
    for &v in Version::ALL {
        let tags = UpdateTags::decode(&bytes, v, Limits::default()).unwrap();
        assert_eq!(tags.registries[0].tags[0].entries, vec![1, 7]);
        assert!(UpdateTags::decode(
            &bytes,
            v,
            Limits {
                max_collection: 3,
                ..Limits::default()
            }
        )
        .is_err());
        assert!(UpdateTags::decode(
            &bytes,
            v,
            Limits {
                max_packet: 1,
                ..Limits::default()
            }
        )
        .is_err());
    }
}
#[test]
fn transfers_cookies_conduct_and_batch_controls() {
    let mut w = Writer::new();
    w.string("localhost", 32767).unwrap();
    w.var_i32(25565);
    assert_eq!(
        Transfer::decode(w.as_slice(), Version::V1_20_5, Limits::default())
            .unwrap()
            .port,
        25565
    );
    assert!(Transfer::decode(w.as_slice(), Version::V1_20, Limits::default()).is_err());
    let mut w = Writer::new();
    w.string("minecraft:test", 32767).unwrap();
    w.bytes(&[1, 2, 3]).unwrap();
    assert_eq!(
        StoredCookie::decode(w.as_slice(), Version::V26_2, Limits::default())
            .unwrap()
            .value,
        vec![1, 2, 3]
    );
    assert!(accept_code_of_conduct(Version::V1_21).is_err());
    assert!(accept_code_of_conduct(Version::V1_21_9)
        .unwrap()
        .data
        .is_empty());
    assert!(chunk_batch_received(Version::V1_20, 1.).is_err());
    assert!(chunk_batch_received(Version::V1_20_2, f32::NAN).is_err());
    assert!(chunk_batch_received(Version::V1_20_2, 20.).is_ok());
}
#[test]
fn server_links_remain_untrusted_data() {
    let mut w = Writer::new();
    w.var_i32(1);
    w.bool(true);
    w.var_i32(0);
    w.string("https://example.invalid/report", 32767).unwrap();
    let p = CommonPacket::decode(
        "server_links",
        w.as_slice(),
        Version::V26_2,
        Limits::default(),
    )
    .unwrap()
    .unwrap();
    match p {
        CommonPacket::ServerLinks(links) => {
            assert_eq!(links.len(), 1);
            assert!(matches!(links[0].label, ServerLinkLabel::BuiltIn(0)));
        }
        _ => panic!("wrong variant"),
    };
    assert!(
        CommonPacket::decode("unhandled", &[], Version::V26_2, Limits::default())
            .unwrap()
            .is_none()
    );
}
