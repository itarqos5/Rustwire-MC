# Serverbound editing and NBT-query envelopes

`packet::editing` adds seven packet families across protocols 763–776:
`update_sign`, `edit_book`, `name_item`, `set_beacon_effect`, `query_block_nbt`,
`query_entity_nbt`, and `set_creative_slot`. Each has bounded decode/encode,
transactional one-body read/write, and exact-version serverbound Play packet
helpers. `EditingPacket` is a separate serverbound enum; these requests are not
inserted into the clientbound `DecodedPacket` dispatcher.

## Evidence

The offline checker verifies all **98 outer packet layouts** against all fourteen
SHA-256-pinned schema inputs. It also checks primitive/position/string aliases and
the complete `UntrustedSlot`/framed-component aliases from 770. Classic and
ordinary component slots reuse the existing inventory codecs and their separate
component audit; this checker does not re-establish every nested ordinary slot.
The protocol-766 count correction remains documented in the
[official slot-count audit](slot-count-wire-audit.md).

The [independent source manifest](validation/editing-source-audit.json) records
immutable PacketEvents v2.13.0 URLs and hashes. It corroborates field ordering,
the book limit boundary, and creative component length framing. Sources were
inspected, not executed or copied into the implementation. In particular:

- [EditBook](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientEditBook.java)
  explicitly changes the page/count/title limits at 1.21.2
- [ItemStackSerialization](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/item/ItemStackSerialization.java)
  and [PatchableComponentMap](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/component/PatchableComponentMap.java)
  distinguish the length-prefixed untrusted representation

Original Python code emits **217 synthetic fixtures** independently of Rustwire.
Rust tests check exact packet IDs/bytes, decoded semantics, every one of their
**2,139 strict prefixes**, trailing bytes, exact/undersized packet caps, bounded
mutations, aggregate collection limits, and read/write rollback. These are not
captured packets or executed release-serializer outputs. Six Python negative
regressions construct their own small schema documents and need no downloads.

## Fields and version boundaries

| Packet | Wire fields |
|---|---|
| Sign | packed block position, front-side boolean, exactly four strings |
| Book | signed slot VarInt, counted page strings, optional title string |
| Rename | one string |
| Beacon | separately boolean-prefixed optional effect VarInts |
| Block NBT query | signed transaction VarInt, packed block position |
| Entity NBT query | signed transaction and entity VarInts |
| Creative slot | signed i16 slot, versioned item representation |

Sign lines are bounded at 384 UTF-16 units. Book pages/count/title limits are
8192/200/128 through 767, and 1024/100/32 from 768. Caller limits can tighten them.
The historical book schema labels the first field `hand`; the actual value is
an inventory slot, so the API uses `slot: i32`, not a two-value hand enum.
None and a present empty title remain distinct. Rename uses the ordinary 32767
UTF-16 wire-string limit; no anvil gameplay/name policy is inferred.

Beacon presence remains explicit, including a present negative VarInt, rather
than collapsing it into an absent-value sentinel. Query transaction/entity IDs
remain signed wire scalars. No permission, entity/effect lookup or query-state
tracking is performed. Position packing retains the existing coordinate checks.

### Creative item framing

`CreativeItem::Unframed(Slot)` is required through protocol 769. It uses ordinary
classic NBT/component payloads and the existing shared collection/NBT/depth
budget. Unknown unframed component layouts return `Unsupported` without guessing
their end; standalone decoding does not return a partial packet.

`CreativeItem::Framed(UntrustedSlot)` is required from 770. Count zero is empty;
a positive count is followed by item ID, added count, removed count, then added
components and removed IDs. Each added component has a type ID, byte length and
that many payload bytes. Added and removed entries share an aggregate collection
budget with the item itself; payload lengths consume the whole-packet byte
budget, not a separate per-byte collection count. All lengths are checked before
allocation/output growth. Wrong-family representations are rejected on encode.

Framed component bytes are **opaque even for known IDs**. Unknown nonnegative
IDs and empty/arbitrary framed bytes are safely retained, along with order and
duplicates; the codec does not claim their payloads are valid component values.
No NBT tree is constructed inside those opaque bytes. Item counts must be
positive for a nonempty item, and registry IDs nonnegative, consistent with the
ordinary slot codec. No maximum-stack-size or creative-permission gameplay rule
is imposed.

The signed-short API preserves every 16-bit wire pattern, including -1. The
pinned PacketEvents wrapper interprets this field unsigned at 766+, but schemas
and narrow official static evidence use signed SHORT. It is supporting evidence
for byte width/framing, not authority for that signedness interpretation.

## Narrow official static cross-checks

The official 1.20.6 bundle, inner JAR, mappings and hashes are recorded in the
[slot-count audit](slot-count-wire-audit.md). Static inspection of mapped
`ServerboundEditBookPacket` (`ahp`) confirms limits 8192/200/128; mapped
`ServerboundSignUpdatePacket` (`aip`) confirms 384-unit line reads. Mapped
`ServerboundSetCreativeModeSlotPacket` (`aim`) stores a Java short and uses
`ByteBufCodecs.SHORT` (`zl.d`), independently settling the signed wire type at 766.

For protocol 776, the [official 26.2 server bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)
has SHA-256 `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`;
its inner JAR has SHA-256
`183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
Static `ServerboundEditBookPacket` codec composition confirms 1024/100/32 limits.
`ServerboundSetCreativeModeSlotPacket` composes SHORT and the validated
OPTIONAL_UNTRUSTED_STREAM_CODEC. This confirms two endpoint snapshots, not
execution or acceptance of every historical version.

Reproduction uses `javap -c -p -classpath <inner.jar> <class>` on the named
classes, checking obfuscated members against the linked official mappings for
1.20.6. The JDK module launcher described in the slot audit is also supported.
No game code is loaded/executed, no server is started, and no game binary,
mapping text or disassembly is redistributed in this repository.

## Reproduction and limits

```sh
python3 tools/check_editing_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_editing_fixtures.py'
cargo test --locked --offline --no-default-features --test editing
cargo test --locked --offline --all-features --test editing
```

This slice serializes requests. It does not edit a world/book/inventory on its
own, grant permissions, validate server menu state, execute commands, resolve
NBT queries or establish receiving-server acceptance. No new live gameplay or
account-backed authentication evidence is claimed.
