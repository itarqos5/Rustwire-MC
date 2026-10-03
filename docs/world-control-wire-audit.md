# World and player control wire audit

## Scope and evidence

`packet::world_control` implements eleven ordinary clientbound play envelopes,
with named typed dispatch and explicit version gating. It does not update a
world, resolve registry/entity IDs, run a tick clock, simulate projectiles, or
apply look/rotation effects.

This increment uses three distinct evidence levels:

1. All fourteen cached `research/protocols` files are verified against the
   existing SHA-256 manifest. The verifier checks complete outer layouts,
   packet presence/IDs, packed positions, the f64 vector alias and optional-NBT
   aliases. There are 142 present packet/family combinations.
2. Version-pinned independently authored public implementations resolve a known
   early `face_player` schema defect and corroborate optional compound NBT and
   the later rotation boundary. This is source-level inspection, not execution
   of Minecraft's release serializer.
3. An original Python encoder, independent of the Rust implementation, produces
   198 synthetic fixture rows. 184 are accepted bodies; 14 intentionally contain
   unknown anchor ordinals. These are not captured traffic or release-API golden
   fixtures. No new live-server or release-API interoperability run is claimed.

## Layouts and presence

All multibyte fixed scalars use big-endian order. VarInts retain signed
32-bit scalar values unless they denote an explicitly closed enum.

| Packet | Protocols | Ordered body |
| --- | --- | --- |
| `block_break_animation` | 763–776 | entity VarInt, packed position, signed stage byte |
| `block_action` | 763–776 | packed position, unsigned action byte, unsigned parameter byte, block registry VarInt |
| `open_sign_entity` | 763–776 | packed position, front-text boolean |
| `nbt_query_response` | 763–776 | transaction VarInt, optional compound NBT |
| `collect` | 763–776 | collected entity VarInt, collector entity VarInt, item-count VarInt |
| `vehicle_move` | 763–776 | X/Y/Z f64, yaw/pitch f32 |
| `face_player` | 763–776 | source-anchor VarInt, X/Y/Z f64, entity-presence boolean, optional entity VarInt + target-anchor VarInt |
| `player_rotation` | 768–772 | yaw f32, pitch f32 |
| `player_rotation` | 773–776 | yaw f32, relative-yaw boolean, pitch f32, relative-pitch boolean |
| `set_projectile_power` | 766 | entity VarInt, X/Y/Z power f64 |
| `set_projectile_power` | 767–776 | entity VarInt, acceleration-power f64 |
| `set_ticking_state` | 765–776 | tick-rate f32, frozen boolean |
| `step_tick` | 765–776 | signed tick-step VarInt |

The NBT root is named only at 763 and anonymous from 764. TAG_End is the absent
value in every family. A non-absent root must be a compound; wrong known root
types return `Invalid`. Complete root parsing and its existing string, collection,
node, depth and byte limits happen before the semantic root check.
Encoding deliberately follows the shared `Nbt::write` framing contract: a
missing root name becomes empty at 763, while a supplied root name is omitted
at 764+. An explicit test covers this cross-version framing adaptation; the
compound payload itself is retained.

`player_rotation`, `set_projectile_power`, `set_ticking_state` and `step_tick`
fail with `Unsupported` before their introduction, including through direct
body decode/encode, packet helpers and named typed dispatch. Encoding rejects
incompatible projectile variants and missing/extra rotation relative flags.

### Explicit early schema correction

The pinned 763/764 schemas label the optional target anchor `string`. The wire
field is a VarInt, confirmed at exact historical release refs:

- MCProtocolLib 1.20-1, commit
  [`caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c`](https://github.com/GeyserMC/MCProtocolLib/blob/caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c/src/main/java/com/github/steveice10/mc/protocol/packet/ingame/clientbound/entity/player/ClientboundPlayerLookAtPacket.java),
  with [protocol 763 declaration](https://github.com/GeyserMC/MCProtocolLib/blob/caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c/src/main/java/com/github/steveice10/mc/protocol/codec/MinecraftCodec.java)
- MCProtocolLib 1.20.2-1, commit
  [`a0cd1e1cef2e4ef3987381376ddaaacc8e20cab8`](https://github.com/GeyserMC/MCProtocolLib/blob/a0cd1e1cef2e4ef3987381376ddaaacc8e20cab8/src/main/java/com/github/steveice10/mc/protocol/packet/ingame/clientbound/entity/player/ClientboundPlayerLookAtPacket.java),
  with [protocol 764 declaration](https://github.com/GeyserMC/MCProtocolLib/blob/a0cd1e1cef2e4ef3987381376ddaaacc8e20cab8/src/main/java/com/github/steveice10/mc/protocol/codec/MinecraftCodec.java)
- PacketEvents v2.2.1, commit
  [`5bb90aa0de54e8b8277a90beb7c58c82e9ddc1f1`](https://github.com/retrooper/packetevents/blob/5bb90aa0de54e8b8277a90beb7c58c82e9ddc1f1/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerFacePlayer.java),
  with [763/764 version entries](https://github.com/retrooper/packetevents/blob/5bb90aa0de54e8b8277a90beb7c58c82e9ddc1f1/api/src/main/java/com/github/retrooper/packetevents/protocol/player/ClientVersion.java)

The verifier preserves and checks the known historical schema defect rather than
rewriting pinned files or pretending that the wire implementation agrees with
it. Its independently encoded fixtures deliberately use VarInt anchors.

These external implementations have separate issues that are not reproduced:
MCProtocolLib's historical encoder writes the source ordinal in the final target
field, while PacketEvents reverses the Feet/Eyes labels. The fixture with source
Eyes and target Feet checks that distinct anchors stay distinct. The semantic
ordinals follow MCProtocolLib's
[`RotationOrigin` (Feet=0, Eyes=1)](https://github.com/GeyserMC/MCProtocolLib/blob/a0cd1e1cef2e4ef3987381376ddaaacc8e20cab8/src/main/java/com/github/steveice10/mc/protocol/data/game/entity/RotationOrigin.java).
Both independent readers restrict anchors to two values. Unknown signed ordinals
therefore return `Unsupported` only after the complete body, strict booleans,
VarInts and trailing-data check succeed. `Connection::next_typed_event` retains
the complete raw packet for that unsupported enum boundary. Truncated or trailing
bodies remain errors.

### NBT and rotation cross-checks

Historical MCProtocolLib helpers default to nullable CompoundTag for both
[763 named-root reading](https://github.com/GeyserMC/MCProtocolLib/blob/caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c/src/main/java/com/github/steveice10/mc/protocol/codec/MinecraftCodecHelper.java)
and [764 anonymous-root reading](https://github.com/GeyserMC/MCProtocolLib/blob/a0cd1e1cef2e4ef3987381376ddaaacc8e20cab8/src/main/java/com/github/steveice10/mc/protocol/codec/MinecraftCodecHelper.java).
The [1.21.7-1 tag-query implementation](https://github.com/GeyserMC/MCProtocolLib/blob/486f59d2784f5818e80ab2632fc98e30ea7e6522/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/level/ClientboundTagQueryPacket.java)
explicitly uses a nullable NbtMap and `readCompoundTag`. These corroborate the
compound-only domain; the schema's generic optional-NBT alias alone does not.

[PacketEvents v2.13.0 player rotation](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerPlayerRotation.java)
independently gates each interleaved relative boolean at 1.21.9 / protocol 773,
matching all pinned schemas. No external implementation source was copied.

## Domains and budgets

- Packed positions use signed 26/12/26-bit X/Y/Z domains. All six out-of-range
  directions reject on encode; both extrema round-trip.
- Stage bytes retain all signed values, including values outside 0–9. Block
  action and parameter retain the complete unsigned byte domain.
- Entity references, transaction IDs, block registry references, collection
  item-counts and tick steps retain the complete signed VarInt domain. They do
  not consume an allocation-count budget. No reference is resolved or applied.
- Floating fields retain NaN payload bits, infinities, signed zero and negative
  finite values. The codecs do not clamp tick rates, normalize rotations, or
  implement any game-side transform.
- NBT reuses the existing bounded codec. One node budget spans the root, sibling
  compounds, list elements and primitive-array elements. Collection, Java
  modified-UTF-8 string, depth and packet-byte budgets remain enforced.
- Packet bytes are bounded before decode. NBT encode receives only the remaining
  enclosing packet budget after the transaction ID. Encodes return no partial
  body on error.

## Tests and reproducibility

The focused suite has 15 new Rust tests plus 16 existing typed/world-state
regressions. It checks all 198 original fixture rows, all 3,627 strict prefixes
through both direct and typed paths, trailing bytes, exact packet-size limits,
absent versions, incompatible version representations, packed bounds, raw
signed/floating domains, NBT root framing and budgets, malformed booleans,
VarInts and array lengths, and unknown-anchor raw fallback. Every fixture's
packet ID is checked against the Rust catalog.

Ten Python verifier regressions test reproducibility, hash mismatch, repinned
layout drift, duplicate packet mappings, unexpectedly introduced packets,
position/vector/NBT-alias drift and the explicit historical schema correction.

```sh
python3 tools/check_world_control_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_world_control_fixtures.py' -v
cargo test --locked --offline --no-default-features \
  --test world_control --test world_control_typed --test typed \
  --test world_state --test world_state_typed
cargo clippy --locked --offline --no-default-features --lib \
  --test world_control --test world_control_typed --test typed -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --no-default-features --no-deps
cargo fmt --all -- --check
```

These focused stable-toolchain checks passed. Full integrated feature/MSRV gates
are separate from this packet increment. No game-side behavior or live server
interoperability is established by these synthetic fixtures.

## Exact schema packet IDs

A dash means the packet is absent, not an alias to a neighboring version.

| Protocol | `block_break_animation` | `block_action` | `open_sign_entity` | `nbt_query_response` | `collect` | `vehicle_move` | `face_player` | `player_rotation` | `set_projectile_power` | `set_ticking_state` | `step_tick` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 763 | 0x07 | 0x09 | 0x31 | 0x66 | 0x67 | 0x2e | 0x3b | — | — | — | — |
| 764 | 0x06 | 0x08 | 0x32 | 0x69 | 0x6a | 0x2f | 0x3d | — | — | — | — |
| 765 | 0x06 | 0x08 | 0x32 | 0x6b | 0x6c | 0x2f | 0x3d | — | — | 0x6e | 0x6f |
| 766 | 0x06 | 0x08 | 0x34 | 0x6e | 0x6f | 0x31 | 0x3f | — | 0x79 | 0x71 | 0x72 |
| 767 | 0x06 | 0x08 | 0x34 | 0x6e | 0x6f | 0x31 | 0x3f | — | 0x79 | 0x71 | 0x72 |
| 768 | 0x06 | 0x08 | 0x36 | 0x75 | 0x76 | 0x33 | 0x41 | 0x43 | 0x80 | 0x78 | 0x79 |
| 769 | 0x06 | 0x08 | 0x36 | 0x75 | 0x76 | 0x33 | 0x41 | 0x43 | 0x80 | 0x78 | 0x79 |
| 770 | 0x05 | 0x07 | 0x35 | 0x74 | 0x75 | 0x32 | 0x40 | 0x42 | 0x80 | 0x78 | 0x79 |
| 771 | 0x05 | 0x07 | 0x35 | 0x74 | 0x75 | 0x32 | 0x40 | 0x42 | 0x80 | 0x78 | 0x79 |
| 772 | 0x05 | 0x07 | 0x35 | 0x74 | 0x75 | 0x32 | 0x40 | 0x42 | 0x80 | 0x78 | 0x79 |
| 773 | 0x05 | 0x07 | 0x3a | 0x79 | 0x7a | 0x37 | 0x45 | 0x47 | 0x85 | 0x7d | 0x7e |
| 774 | 0x05 | 0x07 | 0x3a | 0x79 | 0x7a | 0x37 | 0x45 | 0x47 | 0x85 | 0x7d | 0x7e |
| 775 | 0x05 | 0x07 | 0x3c | 0x7b | 0x7c | 0x39 | 0x47 | 0x49 | 0x87 | 0x7f | 0x80 |
| 776 | 0x05 | 0x07 | 0x3c | 0x7b | 0x7c | 0x39 | 0x47 | 0x49 | 0x87 | 0x7f | 0x80 |
