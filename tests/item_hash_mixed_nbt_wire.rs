//! Independent network envelopes around the public-release-API NBT fixtures.
//! Packet/component IDs and field layouts come from the pinned research schemas;
//! expected hashes come from mixed-nbt-hash.tsv, never Rustwire's own encoder.
use rustwire_mc::{
    connection::{Connection, Event, TypedEvent},
    frame::{FrameCodec, RawPacket},
    packet::{
        inventory::{ClickHeader, ClickMode, HashedContainerClick, HashedItemStack},
        typed::{DecodedPacket, InventoryPacket},
    },
    version::{Direction, State},
    Error, Limits, Version,
};
use std::{
    cell::RefCell,
    io::{Cursor, Read, Write},
    rc::Rc,
};

// protocol, custom_data ID, bucket_entity_data ID, serverbound window_click ID.
const FAMILIES: [(i32, u8, u8, i32); 7] = [
    (770, 0, 50, 16),
    (771, 0, 50, 17),
    (772, 0, 50, 17),
    (773, 0, 50, 17),
    (774, 0, 57, 17),
    (775, 0, 59, 18),
    (776, 0, 59, 18),
];

fn fixtures() -> Vec<(&'static str, Vec<u8>, i32)> {
    include_str!("fixtures/mixed-nbt-hash.tsv")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let fields: Vec<_> = line.split('|').collect();
            assert_eq!(fields.len(), 3);
            let (pairs, remainder) = fields[1].as_bytes().as_chunks::<2>();
            assert!(remainder.is_empty());
            let bytes = pairs
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            (fields[0], bytes, fields[2].parse().unwrap())
        })
        .collect()
}

fn incoming(component: u8, nbt: &[u8]) -> Vec<u8> {
    // window=130, state=300, slot=36; ordinary stack count=3, item=1,
    // added=1, removed=1, then component ID + anonymous NBT, removed max_damage=2.
    let mut bytes = vec![0x82, 1, 0xac, 2, 0, 36, 3, 1, 1, 1, component];
    bytes.extend_from_slice(nbt);
    bytes.push(2);
    bytes
}

fn expected_click(component: u8, hash: i32) -> Vec<u8> {
    // Same synthetic window/state; swap-shaped slot 36/45 fixture, button=40, mode=2.
    // This asserts wire layout, not server menu/offhand semantics for window 130.
    // Two changes: source empty, target present with item ID THEN count.
    let mut bytes = vec![
        0x82, 1, 0xac, 2, 0, 36, 40, 2, 2, 0, 36, 0, 0, 45, 1, 1, 3, 1, component,
    ];
    bytes.extend_from_slice(&hash.to_be_bytes());
    bytes.extend_from_slice(&[1, 2, 0]); // one removal (max_damage), empty cursor
    bytes
}

fn prediction(packet: InventoryPacket, version: Version) -> HashedContainerClick {
    let InventoryPacket::Slot(slot) = packet else {
        panic!("set_slot lost typed dispatch")
    };
    assert_eq!((slot.window_id, slot.state_id, slot.slot), (130, 300, 36));
    let hashed = HashedItemStack::from_slot(&slot.item, version, Limits::default()).unwrap();
    HashedContainerClick {
        header: ClickHeader {
            window_id: slot.window_id,
            state_id: slot.state_id,
            slot: slot.slot,
            button: 40,
            mode: ClickMode::Swap,
        },
        changed_slots: vec![(36, None), (45, hashed)],
        carried_item: None,
    }
}

#[test]
fn incoming_typed_slots_produce_official_hash_click_bytes_in_all_seven_families() {
    let mut checked = 0;
    for (protocol, custom, bucket, click_id) in FAMILIES {
        let version = Version::from_protocol(protocol).unwrap();
        assert_eq!(
            version
                .packet_id(State::Play, Direction::Clientbound, "set_slot")
                .unwrap(),
            20
        );
        for (component, name) in [(custom, "custom_data"), (bucket, "bucket_entity_data")] {
            for (case, nbt, hash) in fixtures() {
                let bytes = incoming(component, &nbt);
                let Some(DecodedPacket::Inventory(packet)) = DecodedPacket::decode(
                    State::Play,
                    "set_slot",
                    &bytes,
                    version,
                    Limits::default(),
                )
                .unwrap() else {
                    panic!("missing typed inventory")
                };
                let InventoryPacket::Slot(ref slot) = packet else {
                    unreachable!()
                };
                assert_eq!(
                    slot.encode(version, Limits::default()).unwrap(),
                    bytes,
                    "raw representation changed: {protocol} {name} {case}"
                );
                let click = prediction(packet, version);
                let stack = click.changed_slots[1].1.as_ref().unwrap();
                assert_eq!(stack.components, vec![(name, hash)]);
                assert_eq!(stack.removed_components, vec!["max_damage"]);
                let wire = click.packet(version, Limits::default()).unwrap();
                assert_eq!(wire.id, click_id);
                assert_eq!(
                    wire.data,
                    expected_click(component, hash),
                    "{protocol} {name} {case}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 196);
}

#[test]
fn truncated_trailing_and_over_budget_slots_never_become_unsupported_fallbacks() {
    for (protocol, custom, bucket, _) in FAMILIES {
        let version = Version::from_protocol(protocol).unwrap();
        for component in [custom, bucket] {
            for (case, nbt, _) in fixtures() {
                let bytes = incoming(component, &nbt);
                let check = |bytes: &[u8], limits| {
                    let result =
                        DecodedPacket::decode(State::Play, "set_slot", bytes, version, limits);
                    assert!(result.is_err(), "{protocol} {component} {case}");
                    assert!(!matches!(result, Err(Error::Unsupported(_))));
                };
                for end in 0..bytes.len() {
                    check(&bytes[..end], Limits::default());
                }
                let mut trailing = bytes.clone();
                trailing.push(0);
                check(&trailing, Limits::default());
                check(
                    &bytes,
                    Limits {
                        max_packet: bytes.len() - 1,
                        ..Limits::default()
                    },
                );
                check(
                    &bytes,
                    Limits {
                        max_nbt_nodes: 1,
                        ..Limits::default()
                    },
                );
            }
        }
    }
}

struct Memory {
    input: Cursor<Vec<u8>>,
    output: Rc<RefCell<Vec<u8>>>,
}
impl Read for Memory {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(bytes)
    }
}
impl Write for Memory {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.output.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
type CapturedConnection = (Connection<Memory>, Rc<RefCell<Vec<u8>>>);
fn connection(version: Version, body: Vec<u8>) -> CapturedConnection {
    // Independent Login Success body, with 26.2's extra session UUID.
    let mut login = vec![0; 16];
    login.extend_from_slice(b"\x08Rustwire\x00");
    if version.protocol() == 776 {
        login.extend_from_slice(&[1; 16]);
    }
    let codec = FrameCodec::default();
    let mut input = Vec::new();
    for packet in [
        RawPacket::new(2, login),
        RawPacket::new(3, vec![]),
        RawPacket::new(20, body),
    ] {
        input.extend(codec.encode(&packet).unwrap());
    }
    let output = Rc::new(RefCell::new(vec![]));
    let mut connection = Connection::new(
        Memory {
            input: Cursor::new(input),
            output: output.clone(),
        },
        version,
        Limits::default(),
    );
    connection
        .start_login("localhost", 25565, "Rustwire", [0; 16])
        .unwrap();
    assert!(matches!(
        connection.next_typed_event().unwrap(),
        TypedEvent::Control(Event::LoginSuccess(_))
    ));
    assert!(matches!(
        connection.next_typed_event().unwrap(),
        TypedEvent::Control(Event::Ready)
    ));
    (connection, output)
}

#[test]
fn framed_connection_dispatch_and_send_keep_the_corrected_hash_and_packet_id() {
    let (_, nbt, hash) = fixtures()
        .into_iter()
        .find(|row| row.0 == "mixed_wrappers")
        .unwrap();
    for (protocol, custom, bucket, click_id) in FAMILIES {
        let version = Version::from_protocol(protocol).unwrap();
        for component in [custom, bucket] {
            let body = incoming(component, &nbt);
            let (mut stream, output) = connection(version, body.clone());
            let TypedEvent::Decoded(DecodedPacket::Inventory(packet)) =
                stream.next_typed_event().unwrap()
            else {
                panic!("known inventory frame was not decoded")
            };
            let before_send = output.borrow().len();
            stream
                .send(
                    &prediction(packet, version)
                        .packet(version, Limits::default())
                        .unwrap(),
                )
                .unwrap();
            // Exactly one outgoing click frame follows login/config acknowledgements.
            let mut expected = vec![
                (expected_click(component, hash).len() + 1) as u8,
                click_id as u8,
            ];
            expected.extend(expected_click(component, hash));
            assert_eq!(&output.borrow()[before_send..], expected);
            let mut malformed = body;
            malformed.pop();
            let result = connection(version, malformed).0.next_typed_event();
            assert!(result.is_err());
            assert!(!matches!(result, Err(Error::Unsupported(_))));
        }
    }
}
