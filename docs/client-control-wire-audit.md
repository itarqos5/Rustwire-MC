# Player-control request envelopes

`packet::client_control` provides twelve serverbound Play packet families with
bounded decode/encode, transactional read/write, and exact-version packet helpers.
The separate `ClientControlPacket` enum is not a clientbound dispatcher. These
codecs serialize requests; they do not grant permissions or change game state.

## Availability and wire fields

| Packet | Protocols | Fields |
|---|---|---|
| steer_boat | 763–776 | left and right paddle booleans |
| spectate | 763–776 | target UUID |
| spectate_entity | 775 only | signed entity-ID VarInt |
| spectator_action | 776 only | shifted optional entity-ID VarInt |
| pick_item | 763–768 | signed inventory-slot VarInt |
| pick_item_from_block | 769–776 | packed position, include-data boolean |
| pick_item_from_entity | 769–776 | signed entity-ID VarInt, include-data boolean |
| select_bundle_item | 768–776 | slot and selected-index VarInts |
| set_slot_state | 765–776 | slot and window VarInts, enabled boolean |
| set_difficulty | 763–770 | unsigned byte difficulty ID |
| set_difficulty | 771–776 | signed VarInt difficulty ID |
| lock_difficulty | 763–776 | locked boolean |
| change_gamemode | 771–776 | signed VarInt mode ID |

The UUID-target spectator request continues through 776. It coexists with the
entity-ID request at 775 and optional-entity action at 776; one must not silently
replace or reinterpret another. This uses the repository's existing
[protocol-776 catalog correction](../PROVENANCE.md#protocol-776-serverbound-catalog-correction),
which places spectator action at 0x3e and the separately omitted UUID request at
0x40. The raw pinned schema instead places spectator action at 0x3f and omits UUID
spectate. No catalog change is made by this increment.

Spectator optional marker zero is absent; any other signed marker is decoded by
wrapping subtraction of one. Encoding uses wrapping addition, rejecting only
`Some(-1)`, which would collide with absence. There is no presence boolean.
The packet does not resolve an entity or infer whether the request is permitted.

Bundle index -1 clears selection, and nonnegative indices are retained. Values
below -1 are rejected, matching the release reader's explicit validity check;
no upper bound is inferred from an inventory not held by this library. Crafter
slot/window IDs are VarInts even at 765–767, despite other legacy container
packets using byte IDs. Old/new pick requests are distinct types and catalog-gated.

Difficulty and mode IDs retain their raw wire scalars, without applying the
game's enum normalization. For difficulty, only the old 0–255 byte range limits
encoding through 770; later values use a full signed VarInt. Known difficulty and
mode IDs are 0–3. No gameplay, permission, server-state or entity lookup policy
is imposed by these envelopes. Position and boolean validation remain strict.

## Evidence and schema handling

`tools/check_client_control_fixtures.py` checks all fourteen immutable input
hashes, **106 present schema bodies**, all corresponding absences, complete field
order/types, packet IDs/name dispatch and relevant primitives/aliases. It then
applies the same guarded spectator-tail correction already established by the
catalog generator, giving **107 supported packet/family combinations**. This
keeps the schema's actual omission/misordering visible rather than modifying the
source or falsely claiming its raw ID is correct.

The [independent source manifest](validation/client-control-source-audit.json)
records the immutable PacketEvents v2.13.0 wrapper URLs and hashes. The sources
were inspected, not executed or copied into Rustwire. They independently confirm
field order, the 771 difficulty-width boundary, crafter VarInts, bundle-index
rule and the 776 optional-entity branch. Representative sources:

- [SetDifficulty](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientSetDifficulty.java)
- [SlotStateChange](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientSlotStateChange.java)
- [SelectBundleItem](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientSelectBundleItem.java)
- [SpectateEntity](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientSpectateEntity.java)

Narrow static inspection of the official 26.2 classes additionally confirms:

- `ServerboundTeleportToEntityPacket` still reads/writes a UUID
- `ServerboundSpectatorActionPacket` uses `ByteBufCodecs.OPTIONAL_VAR_INT`;
  its mapped functions implement zero/absence and Java wrapping +/-1
- `ServerboundSelectBundleItemPacket` explicitly rejects selected indices below
  -1 in its wire-reading constructor
- `ServerboundChangeDifficultyPacket` uses `Difficulty.STREAM_CODEC`, backed by
  the game's wrapping by-ID mapper. PacketEvents' enum helper is not treated as
  authority for rejecting all unknown raw difficulty values

The [official 26.2 bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)
has SHA-256 `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`;
its inner `META-INF/versions/26.2/server-26.2.jar` has SHA-256
`183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
Reproduce the narrow checks with `javap -c -p -classpath <inner.jar> <class>`;
`ByteBufCodecs` includes the optional-mapping functions. No game code was
executed, and binaries, mapping text and disassembly are not redistributed.

## Verification and limits

Original Python encoding produces **362 fixtures** independently of Rustwire.
They cover all supported packet/family pairs, including signed boundaries,
paddle flags, raw UUID bytes, byte/VarInt difficulty widths, shifted optional
values, high crafter window IDs and the bundle clear sentinel. Rust checks exact
IDs and bytes, decoded meanings, every **1,621 strict prefix**, trailing bytes,
whole-body budgets, catalog absences, malformed inputs, transactional I/O and
bounded single-byte mutations. Successful values use no heap-sized collections;
all bodies are bounded scalar/UUID/position sequences.

Six verifier regressions construct small synthetic schemas and require neither
ignored research inputs nor network access. The full checker is a separate
hash-pinned-source audit, not equivalent to those synthetic unit tests.

```sh
python3 tools/check_client_control_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_client_control_fixtures.py'
cargo test --locked --offline --no-default-features --test client_control
```

Fixtures are synthetic, not captures or release-runtime output. No new live
server acceptance, player controls, inventory state, difficulty changes or
account-backed authentication were exercised.
