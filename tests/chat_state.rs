use rustwire_mc::{
    packet::chat::{
        state::*, ChatComponent, ChatFilter, ChatType, PlayerChat, PreviousMessage,
        UnsignedChatMessage, UnsignedChatPolicy,
    },
    Version,
};
fn signature(id: u8) -> MessageSignature {
    let mut s = [0; 256];
    s[255] = id;
    s
}
fn ids(signatures: &[MessageSignature]) -> Vec<u8> {
    signatures.iter().map(|s| s[255]).collect()
}

// Independent outputs of tools/paper/ChatStateOracle.java against official
// cached release jars: empty, one, ignore, wrap, repeated update and zero hash.
#[test]
fn official_tracker_golden_sequence_all_protocols() {
    for &version in Version::ALL {
        let mut t = LastSeenTracker::default();
        let empty = t.generate_update(version);
        assert_eq!(empty.update.offset, 0);
        assert_eq!(empty.update.acknowledged, [0; 3]);
        assert_eq!(
            empty.update.checksum,
            (version.protocol() >= 770).then_some(1)
        );
        assert!(t.track(&signature(1), true).unwrap());
        assert!(!t.track(&signature(1), true).unwrap());
        let one = t.generate_update(version);
        assert_eq!(one.update.offset, 1);
        assert_eq!(one.update.acknowledged, [0, 0, 8]);
        assert_eq!(
            one.update.checksum,
            (version.protocol() >= 770).then_some(33)
        );
        assert_eq!(ids(&one.last_seen), [1]);
        t.track(&signature(2), false).unwrap();
        t.track(&signature(3), true).unwrap();
        assert!(t.ignore_pending(&signature(3)));
        let ignored = t.generate_update(version);
        assert_eq!(ignored.update.offset, 2);
        assert_eq!(ignored.update.acknowledged, [0, 0, 2]);
        assert_eq!(ids(&ignored.last_seen), [1]);
        assert!(!t.ignore_pending(&signature(1)));
        assert_eq!(t.generate_update(version).update.acknowledged, [0, 0, 2]);
        for id in 4..=25 {
            t.track(&signature(id), true).unwrap();
        }
        let wrapped = t.generate_update(version);
        assert_eq!(wrapped.update.offset, 22);
        assert_eq!(wrapped.update.acknowledged, [255, 255, 15]);
        assert_eq!(
            wrapped.update.checksum,
            (version.protocol() >= 770).then_some(75)
        );
        assert_eq!(ids(&wrapped.last_seen), (6..=25).collect::<Vec<_>>());
        let repeated = t.generate_update(version);
        assert_eq!(repeated.update.offset, 0);
        assert_eq!(repeated.last_seen, wrapped.last_seen);
        assert_eq!(repeated.update.acknowledged, wrapped.update.acknowledged);
    }
}
#[test]
fn checksum_zero_becomes_one_and_signed_bytes_preserved() {
    let mut t = LastSeenTracker::default();
    t.track(&signature(224), true).unwrap();
    assert_eq!(t.generate_update(Version::V1_21_5).update.checksum, Some(1));
    let mut t = LastSeenTracker::default();
    t.track(&signature(255), true).unwrap();
    assert_eq!(
        t.generate_update(Version::V1_21_5).update.checksum,
        Some(31)
    );
}
#[test]
fn standalone_ack_preserves_pending_display_history() {
    let mut t = LastSeenTracker::default();
    t.track(&signature(9), true).unwrap();
    assert_eq!(t.pending_offset(), 1);
    assert_eq!(t.take_offset(), 1);
    assert_eq!(t.take_offset(), 0);
    assert!(t.ignore_pending(&signature(9)));
    assert!(t.generate_update(Version::V1_21_5).last_seen.is_empty());
    // A ignored entry still suppresses the immediately repeated signature.
    assert!(!t.track(&signature(9), true).unwrap());
    t.track(&signature(10), false).unwrap();
    assert!(t.track(&signature(9), true).unwrap());
    let preview = t.clone().generate_update(Version::V1_21_5);
    assert_eq!(t.pending_offset(), 2);
    assert_eq!(preview.last_seen, vec![signature(9)]);
}
#[test]
fn generated_payload_writes_exact_bitset_and_checksum_boundary() {
    for &version in Version::ALL {
        let mut t = LastSeenTracker::default();
        t.track(&signature(1), true).unwrap();
        let p = UnsignedChatMessage {
            message: "x".into(),
            timestamp: 0,
            salt: 0,
            last_seen: t.generate_update(version).update,
        }
        .encode(version, UnsignedChatPolicy::AllowedByServer)
        .unwrap();
        let mut expected = vec![1, b'x'];
        expected.extend([0; 17]);
        expected.extend([1, 0, 0, 8]);
        if version.protocol() >= 770 {
            expected.push(33);
        }
        assert_eq!(p.data, expected);
    }
}
#[test]
fn official_cache_order_duplicate_and_unsigned_update_fixtures() {
    let mut cache = SignatureCache::default();
    cache
        .push(&[signature(1), signature(2)], Some(&signature(3)))
        .unwrap();
    assert_eq!(
        (0..3)
            .map(|i| cache.get(i).unwrap()[255])
            .collect::<Vec<_>>(),
        [3, 2, 1]
    );
    cache
        .push(
            &[signature(2), signature(1), signature(2)],
            Some(&signature(3)),
        )
        .unwrap();
    assert_eq!(
        (0..4)
            .map(|i| cache.get(i).unwrap()[255])
            .collect::<Vec<_>>(),
        [3, 2, 1, 2]
    );
    cache.push(&[signature(3)], None).unwrap();
    assert_eq!(
        (0..4)
            .map(|i| cache.get(i).unwrap()[255])
            .collect::<Vec<_>>(),
        [3, 2, 1, 2]
    );
    assert_eq!(cache.index_of(&signature(2)), Some(1));
    assert_eq!(cache.index_of(&signature(4)), None);
    assert_eq!(
        cache
            .resolve(&[
                PreviousMessage::Cached(2),
                PreviousMessage::Signature(Box::new(signature(5)))
            ])
            .unwrap(),
        [signature(1), signature(5)]
    );
}
#[test]
fn cache_capacity_and_invalid_references_are_bounded() {
    let mut cache = SignatureCache::default();
    assert!(cache.get(0).is_err());
    assert!(cache.get(u32::MAX).is_err());
    for id in 0..=200 {
        cache.push(&[], Some(&signature(id))).unwrap();
    }
    assert_eq!(cache.get(0).unwrap()[255], 200);
    assert_eq!(cache.get(127).unwrap()[255], 73);
    assert!(cache.get(128).is_err());
    assert_eq!(cache.index_of(&signature(72)), None);
    assert!(cache
        .push(&vec![signature(0); 21], Some(&signature(0)))
        .is_err());
    assert!(cache
        .resolve(&vec![PreviousMessage::Cached(0); 21])
        .is_err());
    assert_eq!(cache.get(0).unwrap()[255], 200);
    cache.push(&[], None).unwrap();
    assert_eq!(cache.get(127).unwrap()[255], 73);
}
fn message(previous: Vec<PreviousMessage>) -> PlayerChat {
    PlayerChat {
        global_index: None,
        sender: [0; 16],
        index: 0,
        signature: Some(Box::new(signature(8))),
        message: "fixture".into(),
        timestamp: 0,
        salt: 0,
        previous_messages: previous,
        unsigned_content: None,
        filter: ChatFilter::PassThrough,
        chat_type: ChatType::Registry(0),
        name: ChatComponent::Json("\"fixture\"".into()),
        target: None,
    }
}
#[test]
fn ingestion_resolves_before_cache_mutation_and_is_transactional() {
    let mut cache = SignatureCache::default();
    cache.push(&[], Some(&signature(7))).unwrap();
    let invalid = message(vec![
        PreviousMessage::Cached(0),
        PreviousMessage::Cached(128),
    ]);
    assert!(cache.ingest(&invalid).is_err());
    assert_eq!(cache.get(0).unwrap()[255], 7);
    let valid = message(vec![PreviousMessage::Cached(0)]);
    assert_eq!(cache.ingest(&valid).unwrap(), [signature(7)]);
    assert_eq!(cache.get(0).unwrap()[255], 8);
    assert_eq!(cache.get(1).unwrap()[255], 7);
}
