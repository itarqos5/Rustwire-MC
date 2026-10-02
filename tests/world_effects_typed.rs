use rustwire_mc::{
    packet::typed::{DecodedPacket, WorldEffectPacket},
    version::State,
    Limits, Version,
};
#[test]
fn world_effect_dispatch_all_families() {
    for &version in Version::ALL {
        let stop =
            DecodedPacket::decode(State::Play, "stop_sound", &[0], version, Limits::default())
                .unwrap();
        assert!(matches!(
            stop,
            Some(DecodedPacket::WorldEffect(WorldEffectPacket::StopSound(_)))
        ));
        let mut bytes = vec![0, 0, 7, 209];
        bytes.extend([0; 13]);
        let event = DecodedPacket::decode(
            State::Play,
            "world_event",
            &bytes,
            version,
            Limits::default(),
        )
        .unwrap();
        assert!(matches!(
            event,
            Some(DecodedPacket::WorldEffect(WorldEffectPacket::Event(_)))
        ));
        assert!(DecodedPacket::decode(
            State::Configuration,
            "stop_sound",
            &[0],
            version,
            Limits::default()
        )
        .unwrap()
        .is_none());
    }
}
#[test]
fn malformed_known_effect_payload_is_an_error() {
    for name in [
        "world_particles",
        "explosion",
        "sound_effect",
        "entity_sound_effect",
        "stop_sound",
        "world_event",
    ] {
        assert!(
            DecodedPacket::decode(State::Play, name, &[], Version::V26_2, Limits::default())
                .is_err()
        );
    }
}
