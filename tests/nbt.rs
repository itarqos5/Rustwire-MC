use rustwire_mc::{
    codec::{Reader, Writer},
    nbt::{Nbt, NbtString, RootFormat, Tag, TagType},
    Limits, Version,
};
fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(entries.into_iter().map(|(n, v)| (n.into(), v)).collect())
}
#[test]
fn golden_named_anonymous_roots() {
    let named = [10, 0, 0, 3, 0, 1, b'x', 0, 0, 0, 42, 0];
    let anonymous = [10, 3, 0, 1, b'x', 0, 0, 0, 42, 0];
    let n = Nbt::decode(&named, RootFormat::Named, Limits::default()).unwrap();
    assert_eq!(n.root.get("x"), Some(&Tag::Int(42)));
    assert_eq!(
        n.encode(RootFormat::Named, Limits::default()).unwrap(),
        named
    );
    assert_eq!(
        n.encode(RootFormat::Anonymous, Limits::default()).unwrap(),
        anonymous
    );
    assert_eq!(RootFormat::for_version(Version::V1_20), RootFormat::Named);
    assert_eq!(
        RootFormat::for_version(Version::V1_20_2),
        RootFormat::Anonymous
    );
}
#[test]
fn all_payload_types_roundtrip() {
    let n = Nbt::anonymous(compound(vec![
        ("b", Tag::Byte(-128)),
        ("s", Tag::Short(i16::MIN)),
        ("i", Tag::Int(i32::MIN)),
        ("l", Tag::Long(i64::MIN)),
        ("f", Tag::Float(1.25)),
        ("d", Tag::Double(-2.5)),
        ("ba", Tag::ByteArray(vec![-128, -1, 0, 127])),
        ("text", Tag::String("a\0💎".into())),
        ("ia", Tag::IntArray(vec![i32::MIN, 42, i32::MAX])),
        ("la", Tag::LongArray(vec![i64::MIN, 42, i64::MAX])),
        (
            "list",
            Tag::List {
                element_type: TagType::Compound,
                elements: vec![compound(vec![("nested", Tag::Byte(1))])],
            },
        ),
        (
            "empty",
            Tag::List {
                element_type: TagType::End,
                elements: vec![],
            },
        ),
    ]));
    let bytes = n.encode(RootFormat::Anonymous, Limits::default()).unwrap();
    assert_eq!(
        Nbt::decode(&bytes, RootFormat::Anonymous, Limits::default()).unwrap(),
        n
    );
}
#[test]
fn modified_utf8_golden_null_supplementary_and_lone_surrogate() {
    // U+0000 is C0 80; U+1F600 is the modified UTF-8 encoding of D83D DE00.
    let bytes = [
        8, 0, 11, 0xc0, 0x80, 0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80, 0xed, 0xa0, 0x80,
    ];
    let n = Nbt::decode(&bytes, RootFormat::Anonymous, Limits::default()).unwrap();
    assert_eq!(
        n.root,
        Tag::String(NbtString(vec![0, 0xd83d, 0xde00, 0xd800]))
    );
    assert_eq!(
        n.encode(RootFormat::Anonymous, Limits::default()).unwrap(),
        bytes
    );
    let Tag::String(s) = n.root else { panic!() };
    assert!(s.to_string().is_err());
}
#[test]
fn malformed_nbt_rejected_without_panics() {
    for bytes in [
        vec![],
        vec![13],
        vec![8, 0, 1, 0],
        vec![8, 0, 2, 0xc1, 0x80],
        vec![8, 0, 4, 0xf0, 0x9f, 0x98, 0x80],
        vec![8, 0, 1, 0xc2],
        vec![7, 255, 255, 255, 255],
        vec![9, 0, 0, 0, 0, 1],
        vec![10, 1, 0, 2, b'x'],
    ] {
        assert!(
            Nbt::decode(&bytes, RootFormat::Anonymous, Limits::default()).is_err(),
            "{bytes:?}"
        );
    }
    let invalid = Nbt::anonymous(Tag::List {
        element_type: TagType::Int,
        elements: vec![Tag::Byte(1)],
    });
    assert!(invalid
        .encode(RootFormat::Anonymous, Limits::default())
        .is_err());
}
#[test]
fn depth_node_array_and_string_budgets() {
    let mut tag = Tag::Byte(1);
    for _ in 0..5 {
        tag = compound(vec![("n", tag)]);
    }
    let bytes = Nbt::anonymous(tag)
        .encode(RootFormat::Anonymous, Limits::default())
        .unwrap();
    assert!(Nbt::decode(
        &bytes,
        RootFormat::Anonymous,
        Limits {
            max_nbt_depth: 2,
            ..Limits::default()
        }
    )
    .is_err());
    let limits = Limits {
        max_nbt_nodes: 3,
        ..Limits::default()
    };
    assert!(Nbt::anonymous(Tag::LongArray(vec![1, 2, 3]))
        .encode(RootFormat::Anonymous, limits)
        .is_err());
    assert!(Nbt::decode(
        &[11, 0, 0, 0, 10],
        RootFormat::Anonymous,
        Limits {
            max_collection: 3,
            ..Limits::default()
        }
    )
    .is_err());
    assert!(Nbt::anonymous(Tag::String("ab".into()))
        .encode(
            RootFormat::Anonymous,
            Limits {
                max_string_chars: 1,
                ..Limits::default()
            }
        )
        .is_err());
    assert!(Nbt::decode(
        &[8, 0, 2, b'a', b'b'],
        RootFormat::Anonymous,
        Limits {
            max_packet: 4,
            ..Limits::default()
        }
    )
    .is_err());
}
#[test]
fn optional_root_and_embedded_reader_position() {
    let mut r = Reader::new(&[0, 42], Limits::default());
    assert!(Nbt::read(&mut r, RootFormat::Named).unwrap().is_none());
    assert_eq!(r.u8().unwrap(), 42);
    let bytes = [1, 127, 42];
    let mut r = Reader::new(&bytes, Limits::default());
    assert_eq!(
        Nbt::read(&mut r, RootFormat::Anonymous)
            .unwrap()
            .unwrap()
            .root,
        Tag::Byte(127)
    );
    assert_eq!(r.u8().unwrap(), 42);
    let mut writer = Writer::new();
    writer.u8(42);
    let invalid = Nbt::anonymous(Tag::List {
        element_type: TagType::Byte,
        elements: vec![Tag::Int(1)],
    });
    assert!(invalid
        .write(&mut writer, RootFormat::Anonymous, Limits::default())
        .is_err());
    assert_eq!(writer.as_slice(), [42]);
}
#[test]
fn each_truncation_of_compound_fails() {
    let n = Nbt::anonymous(compound(vec![("a", Tag::LongArray(vec![0, 1, i64::MIN]))]));
    let bytes = n.encode(RootFormat::Anonymous, Limits::default()).unwrap();
    for n in 0..bytes.len() {
        assert!(Nbt::decode(&bytes[..n], RootFormat::Anonymous, Limits::default()).is_err());
    }
}
