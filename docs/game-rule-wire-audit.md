# Game-rule values/updates and low-disk notifications

`packet::game_rules` implements clientbound `GameRuleValues`, serverbound
`SetGameRules`, and the empty `LowDiskSpaceWarning` notification for protocols
775–776 (26.1 through 26.2). Packet helpers use exact Play-state catalogs and
reject older versions. Typed clientbound dispatch exposes `GameRules` and
`LowDiskSpaceWarning`; the serverbound setter is separate.

Both rule messages carry a VarInt count followed by resource-identifier/string
pairs. Protocol 775 names the shared alias `GameRule`; 776 expands the same
layout inline with different field names. These are not different wire formats.
Values remain ordinary UTF-8 text, not inferred booleans/integers or parsed
commands. Ordered vectors retain duplicate key spellings and values rather than
applying a map replacement policy or querying a game-rule registry.

The low-disk notification has no body in either family. Its schema is an empty
container at 775 and `void` at 776. A zero-byte body is valid even at a zero-byte
body limit; any trailing payload is rejected. It is a received notification, not
an assessment of the application machine's disk state.

## Evidence

The [fixture checker](../tools/check_game_rule_fixtures.py) verifies all fourteen
source hashes, six present/36 absent packet bodies, exact field order/counts,
primitive/GameRule aliases, directions and packet-ID/name dispatch. Four isolated
negative-regression tests use constructed schemas and no downloaded inputs.

Immutable independent PacketEvents v2.13.0 sources corroborate identifiers,
strings and direction-specific map/list semantics. The
[source manifest](validation/game-rule-source-audit.json) records inspected URLs
and hashes; source text is not copied into the implementation:

- [GameRuleValues](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerGameRuleValues.java)
- [SetGameRule](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientSetGameRule.java)
- [LowDiskSpaceWarning](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerLowDiskSpaceWarning.java)

Narrow static official 26.2 inspection of `ClientboundGameRuleValuesPacket`
confirms a game-rule `ResourceKey`/STRING_UTF8 map codec. The official
[bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)
SHA-256 is `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`,
and its inner `META-INF/versions/26.2/server-26.2.jar` is
`183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
This is static endpoint evidence, not an executed serializer or a historical
release matrix. Use `javap -c -p -classpath <inner.jar>` on the fully qualified
`net.minecraft.network.protocol.game.ClientboundGameRuleValuesPacket` to inspect
the binding. No game artifact or disassembly is redistributed.

## Bounds and tests

Whole-packet byte limits are applied before decoding. Counts are checked against
the caller collection limit and minimum remaining bytes before growing the
record vector; records are added only after both strings decode. Identifier
syntax, per-string UTF-16 limits and the 32,767-unit wire ceiling are checked.
Writes preflight count minima and full string prefixes/payloads against remaining
space. Single-body reads/writes are transactional. No internal list or string
resets an aggregate whole-body byte budget.

Four Rust regressions cover **14 original Python fixtures**, all **1,860 strict
prefixes**, exact IDs/bytes and typed direction/state scope, duplicate entries,
Unicode/multibyte strings, absent version boundaries, count/string/byte caps,
malformed IDs/UTF-8/counts, empty warning bodies, transactional I/O and bounded
mutations. Fixtures are independently encoded, not Rust self-generated goldens,
network captures or release-runtime outputs.

```sh
python3 tools/check_game_rule_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_game_rule_fixtures.py'
cargo test --locked --offline --no-default-features --test game_rules
```

No rule values are applied, permissions inferred, server rules queried, disk
probes run or live setting changes tested by this wire-codec increment.
