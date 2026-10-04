use rustwire_mc::{
    packet,
    version::{Direction, State},
    Version,
};

#[test]
fn report_metadata_is_never_an_outbound_configuration_packet() {
    for &v in Version::ALL {
        for name in ["custom_report_details", "server_links"] {
            assert!(v
                .packet_id(State::Configuration, Direction::Serverbound, name)
                .is_err());
            assert!(packet::named(v, State::Configuration, name, vec![]).is_err());
            assert_eq!(
                v.packet_id(State::Configuration, Direction::Clientbound, name)
                    .is_ok(),
                v.protocol() >= 767
            );
            assert_eq!(
                v.packet_id(State::Play, Direction::Clientbound, name)
                    .is_ok(),
                v.protocol() >= 767
            );
        }
        if (767..=770).contains(&v.protocol()) {
            let packets = v.packets(State::Configuration, Direction::Serverbound);
            assert_eq!(packets.len(), 8);
            for (i, name) in [
                "settings",
                "cookie_response",
                "custom_payload",
                "finish_configuration",
                "keep_alive",
                "pong",
                "resource_pack_receive",
                "select_known_packs",
            ]
            .iter()
            .enumerate()
            {
                assert_eq!(packets[i].id, i as i32);
                assert_eq!(packets[i].name, *name);
            }
            assert!(v
                .packet(State::Configuration, Direction::Serverbound, 8)
                .is_none());
            assert!(v
                .packet(State::Configuration, Direction::Serverbound, 9)
                .is_none());
        }
    }
}
