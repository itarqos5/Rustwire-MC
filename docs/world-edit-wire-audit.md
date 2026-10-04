# Command-block, jigsaw and structure-edit requests

`packet::world_edit` implements five serverbound Play request families across
protocols 763–776: structure generation, command-block updates, command-minecart
updates, jigsaw updates and structure-block updates. Each has bounded decode/
encode, transactional read/write and exact-version packet helpers. The separate
`WorldEditPacket` enum is not placed in clientbound dispatch.

These codecs serialize intent. They do not execute commands, generate/edit a
world, resolve block-state expressions, load structures or grant permissions.

## Exact layouts and boundaries

- `generate_structure`: packed position, signed level VarInt, keep-jigsaws boolean
- `update_command_block`: position, command string, mode VarInt, raw flags byte
- `update_command_block_minecart`: signed entity VarInt, command string, output boolean
- `update_jigsaw_block`: position; name/target/pool resource identifiers; final-state
  and joint-type strings; then selection/placement VarInts from protocol 765
- `update_structure_block`: position; action/mode VarInts; name string; three signed
  offset bytes and three signed size bytes; mirror/rotation VarInts; metadata
  string; integrity f32; **signed VarLong seed**; raw flags byte

Jigsaw priority presence is explicit: `None` through 764, required `Some` from
765. The wrong representation is rejected on encode; no default priorities are
silently inserted. Joint-type strings remain raw instead of being normalized to
`aligned` when a game implementation does not recognize the name. Final-state
expressions remain unparsed. Only the three resource-key fields use identifier
syntax validation. Structure names and command/metadata text are ordinary strings.

Command-block mode IDs are sequence=0, auto=1, redstone=2. Structure action/mode
and rotation IDs are 0–3; mirror IDs are 0–2. Unknown enum IDs are malformed.
Command flags retain all bits, with output/conditional/automatic at 1/2/4.
Structure flags are **bit masks**, not mutually exclusive labels: ignore-entities,
show-air and show-box are 1/2/4; strict is 8 from 770. Reserved bits are preserved.
The [official 1.21.5 release notes](https://www.minecraft.net/sv-se/article/minecraft-java-edition-1-21-5)
confirm the introduction of the Strict Placement option.

Structure offset/size bytes and integrity bits are preserved without applying
the receiver's coordinate, extent or 0–1 clamping. This preserves wire values,
not a simulated post-processing result. NaNs, infinities and signed zero survive.
Metadata is limited to 128 UTF-16 units; other strings have a 32,767-unit ceiling.
Caller string/whole-body limits can be stricter. Packed-position range validation
is inherited from the shared codec.

## Two explicit schema discrepancies

All fourteen pinned schemas label the structure seed `varint`. Independent
release-aligned sources and official static bindings establish **VarLong**:

- [MCProtocolLib's exact 1.20 implementation](https://github.com/GeyserMC/MCProtocolLib/blob/caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c/src/main/java/com/github/steveice10/mc/protocol/packet/ingame/serverbound/inventory/ServerboundSetStructureBlockPacket.java)
  uses a long field and VarLong read/write; its matching
  [MinecraftCodec](https://github.com/GeyserMC/MCProtocolLib/blob/caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c/src/main/java/com/github/steveice10/mc/protocol/codec/MinecraftCodec.java)
  explicitly declares protocol 763 / 1.20
- [PacketEvents v2.13.0](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientSetStructureBlock.java)
  reads/writes a VarLong without a boundary change for the supported families;
  it also establishes bitwise flags and the 128-unit metadata bound
- Official 1.20.6 `ServerboundSetStructureBlockPacket` maps to `aio`. Its seed
  reader calls `wm.m:()J`, mapped to `readVarLong`, and its writer calls the
  mapped `writeVarLong(long)` method. Official 26.2 uses named VarLong bindings
  directly in the corresponding packet class

From 770, the pinned schema also describes structure flags with an ordinal-like
mapper. The implementation preserves the byte and exposes actual bit constants
instead of treating values 0/1/2/3 as exclusive options. The checker verifies both
discrepant source annotations **unchanged**; it does not silently repair inputs
or claim those annotations establish the corrected wire interpretation.

The [source manifest](validation/world-edit-source-audit.json) records immutable
independent files/hashes, including jigsaw's explicit 1.20.3 priority branch and
mirror/rotation ordinals. No third-party implementation text is vendored.
The official 1.20.6 artifact/mappings hashes and acquisition links are in the
[slot-count audit](slot-count-wire-audit.md). The official
[26.2 bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)
SHA-256 is `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`;
its inner JAR is `183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
Static checks use `javap -c -p -classpath <inner.jar>` on `aio` (1.20.6) or
`net.minecraft.network.protocol.game.ServerboundSetStructureBlockPacket` (26.2),
with official mappings for obfuscated member names. These are static endpoint
checks, not executed serializers or an all-release runtime matrix. No binaries,
mapping text or disassembly are published.

## Verification

`tools/check_world_edit_fixtures.py` checks all fourteen input hashes, 70 complete
packet bodies and their aliases, directions and exact packet mapping/dispatch.
Its **196 original Python fixtures** include seeds outside the i32 domain, both
i64 bounds, raw signed bytes/flags, float bit patterns, Unicode and both jigsaw
layouts. Rust rejects all **7,912 strict prefixes** and trailing bytes and checks
exact packet IDs/output, independent meanings, version mismatches, resource
limits, invalid enums/identifiers, transactional I/O and bounded mutations.

Four Python negative-regression tests use synthetic schemas and committed fixture
IDs, without requiring ignored research inputs or network access. No Rustwire
encoder is used to generate the fixture bytes. No command or world edit is run.

```sh
python3 tools/check_world_edit_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_world_edit_fixtures.py'
cargo test --locked --offline --no-default-features --test world_edit
```
