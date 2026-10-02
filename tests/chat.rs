use rustwire_mc::{
    nbt::{Nbt, Tag},
    packet::chat::*,
    version::State,
    Error, Limits, Version,
};

// Hand-authored fixtures, not output from the codec under test.
fn component(version: Version) -> Vec<u8> {
    if version.protocol() < 765 {
        vec![3, b'"', b'x', b'"']
    } else {
        vec![8, 0, 1, b'x']
    }
}
fn expected_component(version: Version) -> ChatComponent {
    if version.protocol() < 765 {
        ChatComponent::Json("\"x\"".into())
    } else {
        ChatComponent::Nbt(Nbt::anonymous(Tag::String("x".into())))
    }
}
fn player_fixture(version: Version) -> Vec<u8> {
    let mut bytes = Vec::new();
    if version.protocol() >= 770 {
        bytes.push(17);
    }
    bytes.extend(0..16);
    bytes.extend([7, 1]);
    bytes.extend([0xaa; 256]);
    bytes.extend([2, b'H', b'i']);
    bytes.extend(123_i64.to_be_bytes());
    bytes.extend((-7_i64).to_be_bytes());
    bytes.extend([2, 0]);
    bytes.extend([0xbb; 256]);
    bytes.extend([4, 1]); // cached index 3, unsigned component present
    bytes.extend(component(version));
    bytes.extend([2, 1]); // partial filter, one word
    bytes.extend(0x8000_0000_0000_0011_u64.to_be_bytes());
    bytes.push(if version.protocol() < 767 { 2 } else { 3 });
    bytes.extend(component(version));
    bytes.push(1);
    bytes.extend(component(version));
    bytes
}

#[test]
fn system_components_and_overlay_all_families() {
    for &version in Version::ALL {
        let mut bytes = component(version);
        bytes.push(1);
        let decoded = SystemChat::decode(&bytes, version, Limits::default()).unwrap();
        assert_eq!(decoded.content, expected_component(version));
        assert!(decoded.overlay);
        assert_eq!(
            decoded.content.encode(version, Limits::default()).unwrap(),
            component(version)
        );
        for end in 0..bytes.len() {
            assert!(SystemChat::decode(&bytes[..end], version, Limits::default()).is_err());
        }
        bytes.push(0);
        assert!(SystemChat::decode(&bytes, version, Limits::default()).is_err());
    }
}
#[test]
fn disguised_chat_registry_holder_boundary() {
    for &version in Version::ALL {
        let mut bytes = component(version);
        bytes.push(if version.protocol() < 767 { 2 } else { 3 });
        bytes.extend(component(version));
        bytes.push(0);
        let decoded = DisguisedChat::decode(&bytes, version, Limits::default()).unwrap();
        assert_eq!(decoded.chat_type, ChatType::Registry(2));
        assert_eq!(decoded.message, expected_component(version));
        assert_eq!(decoded.name, expected_component(version));
        assert_eq!(decoded.target, None);
        for end in 0..bytes.len() {
            assert!(DisguisedChat::decode(&bytes[..end], version, Limits::default()).is_err());
        }
    }
}
#[test]
fn inline_chat_type_retains_decorations() {
    for &version in Version::ALL.iter().filter(|v| v.protocol() >= 767) {
        let mut bytes = component(version);
        bytes.extend([0, 1, b'c', 2, 0, 1, 10, 0, 1, b'n', 1, 2, 10, 0]);
        bytes.extend(component(version));
        bytes.push(0);
        let decoded = DisguisedChat::decode(&bytes, version, Limits::default()).unwrap();
        match decoded.chat_type {
            ChatType::Inline { chat, narration } => {
                assert_eq!(chat.translation_key, "c");
                assert_eq!(
                    chat.parameters,
                    [ChatParameter::Content, ChatParameter::Sender]
                );
                assert_eq!(narration.translation_key, "n");
                assert_eq!(narration.parameters, [ChatParameter::Target]);
                assert_eq!(chat.style, Nbt::anonymous(Tag::Compound(vec![])));
            }
            _ => panic!("expected inline holder"),
        }
    }
}
#[test]
fn player_chat_signed_envelopes_all_families() {
    for &version in Version::ALL {
        let bytes = player_fixture(version);
        let decoded = PlayerChat::decode(&bytes, version, Limits::default()).unwrap();
        assert_eq!(
            decoded.global_index,
            (version.protocol() >= 770).then_some(17)
        );
        assert_eq!(decoded.sender, std::array::from_fn(|i| i as u8));
        assert_eq!(decoded.index, 7);
        assert_eq!(decoded.signature.as_deref(), Some(&[0xaa; 256]));
        assert_eq!(decoded.message, "Hi");
        assert_eq!(decoded.timestamp, 123);
        assert_eq!(decoded.salt, -7);
        assert_eq!(
            decoded.previous_messages,
            [
                PreviousMessage::Signature(Box::new([0xbb; 256])),
                PreviousMessage::Cached(3)
            ]
        );
        assert_eq!(decoded.unsigned_content, Some(expected_component(version)));
        assert_eq!(
            decoded.filter,
            ChatFilter::PartiallyFiltered(vec![0x8000_0000_0000_0011])
        );
        assert_eq!(decoded.chat_type, ChatType::Registry(2));
        assert_eq!(decoded.target, Some(expected_component(version)));
        for end in 0..bytes.len() {
            assert!(
                PlayerChat::decode(&bytes[..end], version, Limits::default()).is_err(),
                "{} at {end}",
                version.protocol()
            );
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(PlayerChat::decode(&trailing, version, Limits::default()).is_err());
    }
}
#[test]
fn disconnect_login_remains_json_in_modern_versions() {
    for &version in Version::ALL {
        let bytes = [3, b'"', b'x', b'"'];
        let login = Disconnect::decode(&bytes, version, State::Login, Limits::default()).unwrap();
        assert_eq!(login.reason, ChatComponent::Json("\"x\"".into()));
        let play = Disconnect::decode(&component(version), version, State::Play, Limits::default())
            .unwrap();
        assert_eq!(play.reason, expected_component(version));
        assert!(Disconnect::decode(&bytes, version, State::Status, Limits::default()).is_err());
        assert_eq!(
            Disconnect::decode(
                &component(version),
                version,
                State::Configuration,
                Limits::default()
            )
            .is_ok(),
            version.has_configuration()
        );
    }
}
#[test]
fn malformed_components_and_budgets_fail_closed() {
    assert!(SystemChat::decode(&[0, 0], Version::V1_20_3, Limits::default()).is_err());
    assert!(SystemChat::decode(&[8, 0, 1, b'x', 2], Version::V1_20_3, Limits::default()).is_err());
    assert!(ChatComponent::Json("x".into())
        .encode(Version::V1_20_3, Limits::default())
        .is_err());
    for &version in Version::ALL {
        let limits = Limits {
            max_packet: 1,
            ..Limits::default()
        };
        assert!(matches!(
            PlayerChat::decode(&player_fixture(version), version, limits),
            Err(Error::Limit(_))
        ));
        assert!(matches!(
            SystemChat::decode(&[0, 0], version, limits),
            Err(Error::Limit(_))
        ));
        assert!(DisguisedChat::decode(&[0, 0], version, limits).is_err());
        assert!(Disconnect::decode(&[0, 0], version, State::Login, limits).is_err());
        assert!(ChatComponent::decode(&component(version), version, limits).is_err());
        assert!(expected_component(version).encode(version, limits).is_err());
        let limits = Limits {
            max_collection: 1,
            ..Limits::default()
        };
        assert!(PlayerChat::decode(&player_fixture(version), version, limits).is_err());
    }
}
#[test]
fn unsigned_outgoing_chat_exact_bytes_and_explicit_policy() {
    let ids = [5, 5, 5, 6, 6, 7, 7, 7, 8, 8, 8, 8, 9, 9];
    for (&version, id) in Version::ALL.iter().zip(ids) {
        let mut message = UnsignedChatMessage {
            message: "Hi".into(),
            timestamp: 123,
            salt: -7,
            last_seen: LastSeenUpdate {
                offset: 130,
                acknowledged: [1, 128, 8],
                checksum: (version.protocol() >= 770).then_some(0x5a),
            },
        };
        let packet = message
            .encode(version, UnsignedChatPolicy::AllowedByServer)
            .unwrap();
        assert_eq!(packet.id, id);
        let mut expected = vec![2, b'H', b'i'];
        expected.extend(123_i64.to_be_bytes());
        expected.extend((-7_i64).to_be_bytes());
        expected.extend([0, 0x82, 1, 1, 128, 8]);
        if version.protocol() >= 770 {
            expected.push(0x5a);
        }
        assert_eq!(packet.data, expected);
        assert!(matches!(
            message.encode(version, UnsignedChatPolicy::SigningRequired),
            Err(Error::Unsupported(_))
        ));
        message.last_seen.acknowledged[2] = 0x10;
        assert!(message
            .encode(version, UnsignedChatPolicy::AllowedByServer)
            .is_err());
        message.last_seen.acknowledged[2] = 0;
        message.last_seen.checksum = if version.protocol() >= 770 {
            None
        } else {
            Some(0)
        };
        assert!(message
            .encode(version, UnsignedChatPolicy::AllowedByServer)
            .is_err());
    }
}
#[test]
fn unsigned_commands_and_acknowledgements() {
    let command_ids = [4, 4, 4, 4, 4, 5, 5, 5, 6, 6, 6, 6, 7, 7];
    let ack_ids = [3, 3, 3, 3, 3, 4, 4, 4, 5, 5, 5, 5, 6, 6];
    for ((&version, command_id), ack_id) in Version::ALL.iter().zip(command_ids).zip(ack_ids) {
        if version.protocol() >= 766 {
            let packet = unsigned_command(version, "help").unwrap();
            assert_eq!(packet.id, command_id);
            assert_eq!(packet.data, [4, b'h', b'e', b'l', b'p']);
        } else {
            assert!(matches!(
                unsigned_command(version, "help"),
                Err(Error::Unsupported(_))
            ));
        }
        let ack = message_acknowledgement(version, 130).unwrap();
        assert_eq!(ack.id, ack_id);
        assert_eq!(ack.data, [0x82, 1]);
        assert!(message_acknowledgement(version, u32::MAX).is_err());
        assert!(unsigned_command(version, "/help").is_err());
    }
}

fn minimal_player_prefix(version: Version) -> Vec<u8> {
    let mut bytes = Vec::new();
    if version.protocol() >= 770 {
        bytes.push(0);
    }
    bytes.extend([0; 16]);
    bytes.extend([0, 0, 0]); // index, absent signature, empty text
    bytes.extend([0; 16]); // timestamp and salt
    bytes
}
#[test]
fn invalid_chat_discriminants_counts_and_indexes_are_rejected() {
    for &version in Version::ALL {
        let mut minimal = minimal_player_prefix(version);
        minimal.extend([0, 0, 0]); // no previous signatures, unsigned content, filter
        minimal.push(if version.protocol() >= 767 { 1 } else { 0 });
        minimal.extend(component(version));
        minimal.push(0);
        let decoded = PlayerChat::decode(&minimal, version, Limits::default()).unwrap();
        assert_eq!(decoded.signature, None);
        assert_eq!(decoded.filter, ChatFilter::PassThrough);
        assert!(decoded.previous_messages.is_empty());

        let index_position = 16 + usize::from(version.protocol() >= 770);
        let mut negative_index = minimal.clone();
        negative_index.splice(
            index_position..index_position + 1,
            [0xff, 0xff, 0xff, 0xff, 0x0f],
        );
        assert!(PlayerChat::decode(&negative_index, version, Limits::default()).is_err());
        let mut invalid_presence = minimal.clone();
        invalid_presence[index_position + 1] = 2;
        assert!(PlayerChat::decode(&invalid_presence, version, Limits::default()).is_err());

        for tail in [
            vec![21],                                    // over the last-seen count limit
            vec![1, 0xff, 0xff, 0xff, 0xff, 0x0f],       // negative cache reference
            vec![0, 0, 3],                               // unknown filter discriminator
            vec![0, 0, 2, 0xff, 0xff, 0xff, 0x7f],       // impossible bitset allocation
            vec![0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0x0f], // negative chat-type holder
        ] {
            let mut bytes = minimal_player_prefix(version);
            bytes.extend(tail);
            assert!(PlayerChat::decode(&bytes, version, Limits::default()).is_err());
        }
        let mut fully_filtered = minimal.clone();
        fully_filtered[minimal_player_prefix(version).len() + 2] = 1;
        assert_eq!(
            PlayerChat::decode(&fully_filtered, version, Limits::default())
                .unwrap()
                .filter,
            ChatFilter::FullyFiltered
        );
        assert!(ChatComponent::decode(
            &component(version),
            version,
            Limits {
                max_string_chars: 0,
                ..Limits::default()
            }
        )
        .is_err());
        let too_long = UnsignedChatMessage {
            message: "😀".repeat(129),
            timestamp: 0,
            salt: 0,
            last_seen: LastSeenUpdate {
                offset: 0,
                acknowledged: [0; 3],
                checksum: (version.protocol() >= 770).then_some(0),
            },
        };
        assert!(too_long
            .encode(version, UnsignedChatPolicy::AllowedByServer)
            .is_err());
    }
}
