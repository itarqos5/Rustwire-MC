# Game-test editor and status envelopes

`packet::game_test` provides four bounded Play envelopes. They construct or parse
wire data; applications control transmission, permissions, rendering and any
subsequent action. No test, world edit, file save/export or command is executed.

| Packet | Direction | Protocol families | Body |
| --- | --- | --- | --- |
| `GameTestHighlightPos` | Clientbound | 773–776 | two packed block positions |
| `TestInstanceBlockStatus` | Clientbound | 770–776 | required NBT chat component; optional three-VarInt size |
| `SetTestBlock` | Serverbound | 770–776 | packed block position; mode VarInt; message string |
| `TestInstanceBlockAction` | Serverbound | 770–776 | packed block position; action VarInt; full instance data |

Clientbound packets dispatch through `DecodedPacket::GameTest`. The family
`GameTestPacket` also supports explicit body and packet operations in either
packet's defined direction. All direct helpers check exact catalog availability.
The existing protocol-776 catalog correction is retained: test-instance action
uses ID `0x41`, while the unchanged raw schema erroneously gives `0x40`.

Instance data contains an optional test-instance identifier, three signed
VarInt dimensions, rotation, ignore-entities boolean, run status and an optional
NBT error component. It is always present, including query/reset actions. The
size vector is neither three fixed-width i32 fields nor a packed position.

## Values and limits

- Modes are Start/Log/Fail/Accept; actions are Init/Query/Set/Reset/Save/Export/Run;
  run status is Cleared/Running/Finished. Existing `StructureRotation` is reused
- Enums use closed canonical IDs and reject unknown values. This is deliberately
  stricter than vanilla's wrap/default behavior for some invalid enum IDs
- Booleans use the shared strict 0/1 policy, including option presence markers
- Packed positions obey the existing 26/12/26-bit representation bounds. The
  three signed dimension values are retained without game-valid size checks,
  clamping, volume calculations or allocation based on those dimensions
- Identifiers use the shared version-aware resource syntax and remain unresolved
  registry keys. Omitted/empty namespaces and empty paths are not normalized;
  namespace `..` is rejected from 775, consistently with the shared identifier API
- Message strings obey the 32,767 UTF-16-unit wire cap and caller string limits.
  Components use anonymous NBT and inherited node/depth/string/byte budgets;
  JSON variants and absent required NBT are rejected. Component meaning and
  receiver-specific allocation quotas are not reproduced
- Packet bytes are bounded on input and before variable output growth. Reader
  and writer helpers are transactional on error; whole-body decode rejects all
  trailing bytes, while streaming reads can leave enclosing bytes

## Independent source and static evidence

[Source facts](validation/game-test-wire-facts.json) contain eleven immutable
primary-source hashes and ten official class hashes. MCProtocolLib's exact
protocol-770 / Minecraft-1.21.5 release binding and packet/helper sources establish
the initial instance/status layouts. PacketEvents v2.13.0 independently agrees
on the outer fields and modern highlight layout. Narrow static inspection of the
official 26.2 packet/data/enum classes confirms their codec composition and that
`Vec3i.STREAM_CODEC` contains three `VAR_INT` codecs. The sample component types
remain opaque NBT, not evaluated text or actions.

All fourteen unchanged hash-pinned schemas are checked for exact presence,
direction, body/switch shape, integer-vector alias and IDs. The protocol-776
fixture-ID adjustment has an exact raw-ID guard and relies on the already
independently audited spectator-omission correction; it does not alter inputs.

`tools/check_game_test_fixtures.py` independently generates 124 original fixtures
covering every mode/action, rotations/statuses, optional fields, Unicode messages,
position extrema and signed dimension extrema. Rust tests compare independent
semantic values and bytes, every one of 2,775 strict prefixes, trailing bytes,
exact/undersized budgets, state/direction gates, malformed enums/booleans/NBT,
identifier boundaries and transactional I/O. An in-memory connection checks
that an incoming status yields a typed event without a reply. Six schema-free
Python controls catch presence, width, NBT, dispatch, ID and ordering drift.

Run `python3 tools/check_game_test_fixtures.py` after fetching pinned schemas.
Only original code/fixture bytes and factual metadata are distributed. No game
class or upstream serializer was executed, and no live game-test acceptance,
rendering, permission, export or world-modification result is claimed.
