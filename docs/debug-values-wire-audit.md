# Debug value packets: wire and implementation audit

Scope: clientbound Play `debug_block_value`, `debug_chunk_value`,
`debug_entity_value`, and `debug_event`, protocols 773–776. Packet IDs are
respectively 0x1a–0x1d. The implementation is original Rust, with no new runtime
dependency and no upstream source, game classes, or disassembly embedded here.

## Evidence and corrections

The independent static audit inspected the registered codecs in the official
1.21.9, 1.21.11, 26.1 and 26.2 server artifacts. Sources are the official
[1.21.9 metadata](https://piston-meta.mojang.com/v1/packages/5f4990c6189ca01b97c58996bec83ebcd879f0a6/1.21.9.json),
[1.21.11 metadata](https://piston-meta.mojang.com/v1/packages/4f6bd9388f12e9d7adc2ded64acba66212d60521/1.21.11.json),
[26.1 metadata](https://piston-meta.mojang.com/v1/packages/84d0ff7bd4428695691af5a178edae22c7c83d89/26.1.json), and
[26.2 bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar).
The inspected inner JAR SHA-256 values, in that order, are:

- `8218472d58bf3aba49b51e777621f9ed19f95dbd4f0b687a45f97c2ebefa2351`
- `ec47239a8de246335e1d54f6ac319bd35641778eb4b6a6da06372840d02fcebc`
- `a7fed6f7d88379349e35ae0c6e9881d4484605132f6f620376a3868eea6cce52`
- `183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`

The public cross-check was PacketEvents commit
`1063d4e71cff72b9fe022f37b51faad9536815b7` (v2.13.0), with release-aligned
v2.10.0, v2.11.0 and v2.12.0 debug files checked for equality. The historical
pins are respectively `8a0ebb77ebda5e05918b134fcac965dbdc8a6936`,
`ebd8a66d057f6228d5a6a1503064ff23dc157670` and
`2ddba2590987d01fbc430c9c1440000f5cf4d4e7`. Its
[registration](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/debug/DebugSubscriptions.java),
[path codec](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/debug/path/DebugPath.java), and
[goal codec](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/debug/struct/DebugGoalInfo.java)
corroborate these fields. Neither schemas nor PacketEvents alone establish all
wire details. The implementation deliberately corrects these discrepancies:

1. GoalSelectors **events also contain a goal list**, rather than a single goal.
2. Path list order is **nodes, targets, open, closed**, not declaration order.
3. ID 0 (dedicated server tick time) has **no value codec**, even for a removal.
4. VillageSections is a real zero-byte Unit. Updates still encode presence;
   event `0a`, removal suffix `0a00`, present suffix `0a01` remain distinct.
5. Numeric PathType 26 exists from protocol 775. Prior ordinals retain placement
   despite renamed labels; no version-independent label semantics are invented.
6. GameEvents begins with a **direct GAME_EVENT registry VarInt** in all four
   official releases. PacketEvents' identifier string is a discrepancy.
7. Goal strings are capped at **255 UTF-16 code units**; brain strings use 32767.

## Wire contract

An update carries the kind VarInt, strict presence boolean and optional value.
An event carries the kind VarInt and value without presence. Block/chunk/entity
updates respectively prepend packed BlockPos, packed ChunkPos, or entity VarInt.
There is no extra payload length, timestamp, expiry field or per-envelope kind
whitelist. ChunkPos places signed x in the low 32 bits and z in the high 32:
x=-2,z=3 produces `00000003fffffffe`.

All fixed scalars are big endian. Lists have a VarInt count. `Option<T>` has a
separate strict bool and optional T. BlockPos uses the ordinary packed 26/26/12
x/z/y layout. Ordinary diagnostic VarInts and i32s stay signed. Registry IDs
are unresolved nonnegative i32-compatible numbers, without an ID-plus-one tag
or inline holder. Floats preserve signed zero, infinities and NaN payload bits.

| ID | Kind | Value fields in wire order |
|---|---|---|
| 1 | Bees | optional hive position; optional flower position; travel ticks VarInt; blacklisted position list |
| 2 | Brains | name string; profession string; xp i32; health/max-health f32; inventory string; wants-golem bool; anger i32; activities/behaviors/memories/gossips string lists; POI/potential-POI position lists |
| 3 | Breezes | optional attack-target VarInt; optional jump-target position |
| 4 | GoalSelectors | goal list, each priority VarInt, running bool, name string(255) |
| 5 | EntityPaths | reached bool; next-node-index i32; target position; node/target/open/closed lists; max-node-distance f32 |
| 6 | EntityBlockIntersections | VarInt 0/1/2 (block/fluid/air) |
| 7 | BeeHives | block registry VarInt; occupant count VarInt; honey level VarInt; sedated bool |
| 8 | Pois | position; POI-type registry VarInt; free-ticket count VarInt |
| 9 | RedstoneWireOrientations | table-index VarInt 0–47 |
| 10 | VillageSections | no value bytes |
| 11 | Raids | position list |
| 12 | Structures | structure list; each min/max positions and piece list; each piece min/max positions and start bool |
| 13 | GameEventListeners | radius VarInt |
| 14 | NeighborUpdates | position |
| 15 | GameEvents | game-event registry VarInt; x/y/z f64 |

Node and target elements both have x/y/z i32, walked-distance/cost-malus f32,
closed bool, PathType VarInt and f f32 (26 bytes for the supported ordinals).
No target heuristic, graph pointer, runtime reached flag or optional debug-data
flag follows. All lists preserve ordering and duplicates, including vanilla's
in-memory sets. Bounding boxes are two packed positions and are not normalized.
Rustwire encodes decoder-valid empty target lists; vanilla's Path producer
requires nonempty target debug data.

## Bounds and failure contract

`Limits.max_collection` is one **per-body aggregate element budget**, shared by
all sibling/nested lists, including structure entries and their pieces. All
counts also fit i32. `Limits.max_packet` caps the body and cumulative owned
string bytes. Individual strings enforce the lesser of their audited UTF-16
limit and `Limits.max_string_chars`, plus the 3-bytes-per-code-unit wire ceiling.

Before reserving each vector the decoder checks count times minimum wire size
plus mandatory suffix bytes: position 8, goal 3, string 1, node 26, piece 17,
structure 17. Nested piece preflight includes remaining sibling structures.
All arithmetic is checked; owned strings and vectors reserve fallibly. Encoding
traverses first without output, validating every size/field against the body
budget before reserving a single exactly sized buffer. A second checked pass
emits it. Reader cursor advancement and writer append occur only after success.
There is fixed nesting, so NBT/graph recursion limits do not apply.

These are library resource policies, not claims about small official list caps.
The audited official list codecs permit up to INT_MAX elements. Unknown kinds,
ID 0, unsupported versions and unknown PathType/intersection kinds return
`Unsupported` so the existing connection layer can preserve the whole packet.
Negative kinds/registry IDs, invalid strict booleans, invalid orientation indices,
malformed/truncated data, trailing bytes and budget failures remain errors;
they do **not** silently become raw success events. Unknown nested bodies cannot
be skipped because they have no local byte-length delimiter.

`DebugValue` boxes its large brain/path members; semantic dispatch boxes the
rare debug packet family to avoid enlarging unrelated events. No subscription,
reply, renderer, expiry simulation, registry lookup or world action is performed.

## Validation boundaries

588 independent Python-encoded fixtures exercise all 15 serializable values across
all four envelopes and supported protocols. Rust tests exercise those bytes,
all 23,780 strict prefixes, trailing bytes, transactional reads/writes, packet IDs,
version boundaries, budgets, list-order sentinels, string limits and float bits.
In-memory connection tests verify raw preservation only for unsupported layouts,
malformed/limit errors, subsequent-frame alignment and the absence of automatic outbound responses.
The alignment assertion does not change the connection API recommendation to end
a connection after malformed selected typed data.

These are original synthetic wire fixtures, not live packet captures. No game
or plugin was run for this codec audit. Same-protocol patch releases (1.21.10,
26.1.1/26.1.2) were not separately inspected. Runtime producer reachability,
subscription permissions, registries, rendering and temporal behavior are outside
this bounded wire implementation.
