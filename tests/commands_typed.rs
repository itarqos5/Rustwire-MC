use rustwire_mc::{
    packet::typed::{CommandPacket, DecodedPacket},
    version::State,
    Limits, Version,
};
#[test]
fn command_tree_and_suggestions_dispatch_all_families() {
    for &version in Version::ALL {
        let tree = DecodedPacket::decode(
            State::Play,
            "declare_commands",
            &[1, 0, 0, 0],
            version,
            Limits::default(),
        )
        .unwrap();
        assert!(matches!(
            tree,
            Some(DecodedPacket::Command(CommandPacket::Tree(_)))
        ));
        let suggestions = DecodedPacket::decode(
            State::Play,
            "tab_complete",
            &[0, 0, 0, 1, 1, b'x', 0],
            version,
            Limits::default(),
        )
        .unwrap();
        assert!(matches!(
            suggestions,
            Some(DecodedPacket::Command(CommandPacket::Suggestions(_)))
        ));
        assert!(DecodedPacket::decode(
            State::Configuration,
            "declare_commands",
            &[1, 0, 0, 0],
            version,
            Limits::default()
        )
        .unwrap()
        .is_none());
    }
}
#[test]
fn malformed_known_command_payload_is_an_error() {
    for name in ["declare_commands", "tab_complete"] {
        assert!(
            DecodedPacket::decode(State::Play, name, &[], Version::V26_2, Limits::default())
                .is_err()
        );
    }
}
