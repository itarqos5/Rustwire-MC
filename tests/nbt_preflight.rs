//! Regression checks for the byte-budget preflight shared by nested displays.
use rustwire_mc::{
    codec::Writer,
    nbt::{Nbt, RootFormat, Tag},
    Error, Limits,
};
#[test]
fn variable_size_nbt_writes_are_atomic_and_byte_bounded() {
    for root in [
        Tag::ByteArray(vec![7; 256]),
        Tag::IntArray(vec![7; 256]),
        Tag::LongArray(vec![7; 256]),
        Tag::String("\0\u{07ff}\u{0800}".repeat(256).into()),
    ] {
        let nbt = Nbt { name: None, root };
        let body = nbt
            .encode(RootFormat::Anonymous, Limits::default())
            .unwrap();
        let exact = Limits {
            max_packet: body.len(),
            ..Limits::default()
        };
        assert_eq!(nbt.encode(RootFormat::Anonymous, exact).unwrap(), body);
        for max_packet in [0, 1, body.len() - 1] {
            let mut writer = Writer::new();
            writer.raw(&[11, 22, 33]);
            assert!(matches!(
                nbt.write(
                    &mut writer,
                    RootFormat::Anonymous,
                    Limits {
                        max_packet,
                        ..exact
                    }
                ),
                Err(Error::Limit(_))
            ));
            assert_eq!(writer.as_slice(), [11, 22, 33]);
        }
    }
}
