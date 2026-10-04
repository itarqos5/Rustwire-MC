# Server and chat metadata wire audit

`packet::server_metadata` provides six bounded body codecs, transactional reader
and writer helpers, and exact-version packet wrappers. Clientbound dispatch adds
`DecodedPacket::ServerData`, `DecodedPacket::PingResponse`,
`ChatPacket::{Suggestions, Hide}` and `CommonPacket::CustomReportDetails`.
`CustomReportDetails` and `ReportDetail` are also available through
`packet::common`. `PingRequest` is an explicit serverbound play packet builder.

## Exact schema boundaries

| Packet | Direction/state | Protocols | Body |
|---|---|---|---|
| `server_data` | clientbound play | 763–776 | component MOTD, optional VarInt-length icon byte array; secure-chat boolean only through 765 |
| `custom_report_details` | clientbound configuration and play | 767–776 | VarInt count and ordered string key/value pairs |
| `chat_suggestions` | clientbound play | 763–776 | VarInt action and VarInt-length string list |
| `hide_message` | clientbound play | 763–776 | packed signature: zero followed by 256 bytes, or positive cache index plus one |
| `ping_response` | clientbound play | 764–776 | fixed signed i64 identifier |
| `ping_request` | serverbound play | 764–776 | fixed signed i64 identifier |

The MOTD component uses JSON through protocol 764 and anonymous NBT from 765.
Encoding requires the matching `ChatComponent` variant and does not translate
formats. `enforces_secure_chat` is `Some(bool)` through 765 and must be `None`
from 766, when the field is no longer transmitted. Its absence does not indicate
that unsigned chat is permitted. Existing explicit unsigned-chat policy remains
unchanged.

The common report layout is a schema-global type referenced by both supported
clientbound states. The pinned catalogs additionally advertise serverbound
configuration `custom_report_details` at protocols 767–770, removing that entry
from 771. The audit verifies this boundary, but this increment deliberately
provides only the clientbound report wrapper and dispatch. That outbound catalog
entry remains raw-only. There is no global catalog modification.

The play ping pair is distinct from status ping and the configuration/play
fixed-i32 `ping`/`pong` connection-control pair. Identifiers preserve the full i64
bit domain; there is no interpretation as time, no automatic ping request, and
no automatic response to a received `ping_response`.

## Interpretation and safety boundary

- Icon bytes are opaque. No PNG validation, decompression or rendering occurs
- Report metadata preserves entry order and duplicate keys. It neither submits a
  report nor claims that a receiving implementation preserves duplicate entries
- Suggestion actions are the closed `Add=0`, `Remove=1`, `Set=2` wire enum; unknown
  action IDs are malformed rather than silently applied or remapped
- Message deletion retains the existing `PreviousMessage` representation.
  A full signature is exactly 256 bytes, without cryptographic verification;
  cached references are unshifted nonnegative indices. Index+1 must fit the
  positive i32 wire domain, matching the existing player-chat decoder policy
- Cache presence and the fixed 128-entry cache range are checked only when the
  caller explicitly invokes `SignatureCache` resolution. Out-of-cache positive
  references are retained as envelopes, not mistaken for verified signatures
- Parsing never changes a signature cache, acknowledgement tracker, chat display,
  authentication, consent, resource-pack, URL-navigation or server-transfer policy

Booleans follow the shared strict 0/1 convention. Overlong but nonoverflowing
VarInts accepted by the shared reader are canonicalized on re-encoding. Negative
counts and packed-signature indices, overflowing VarInts, malformed UTF-8,
missing required NBT and trailing bytes are errors. These are stated Rustwire
codec choices, not an assertion of identical permissiveness in every client.

## Resource budgets

Decode checks `max_packet` before parsing the body. Reader-based entry points
restrict their view to that budget and advance the original reader only on
success. Count checks precede collection allocation; impossible suggestion and
report counts are also checked against the minimum remaining bytes per entry.

Report details are bounded to 32 entries, 128 UTF-16 units per key and 4096 per
value, additionally restricted by caller `max_collection` and
`max_string_chars`. Suggestion strings use 32767 UTF-16 units and the caller's
string bound. Icon bytes and suggestion/report entry counts obey
`max_collection`. The collection limit counts icon bytes or list entries; it
is not an aggregate string-character limit. Aggregate strings, icon bytes and
all framing fields share the complete `max_packet` body budget.

The single MOTD reuses `ChatComponent` and the existing bounded NBT reader/writer,
including depth, per-collection, string and total-node checks. Encode checks
string/icon capacity before copying and shares remaining body bytes with NBT.
Writes are transactional: a failed body encode never partially appends to the
caller's writer.

## Evidence and limits

All fourteen cached schema files are hash-verified against
`research/schema-hashes.json`. The original
`tools/verify_server_metadata_schemas.py` checks exact body shapes, common-type
references, ByteArray expansion, packet name/type dispatch, state/direction
presence and packet IDs. Its independent Python-standard-library encoder writes
310 synthetic cases to `tests/fixtures/server-metadata.tsv`. Those cases are
not produced by Rustwire's encoders.

Schema sources:

- [PrismarineJS/minecraft-data at f5d7d746](https://github.com/PrismarineJS/minecraft-data/tree/f5d7d74604d8c6153fd086bfe035e0630a5207cc/data/pc), protocols 763–775
- [Complexity-ML/minecraft-data-26.2 at 2a6a5fd9](https://github.com/Complexity-ML/minecraft-data-26.2/tree/2a6a5fd9ebb0d964a73312d11beef596b7ec029b/data/pc/26.2), protocol 776

Independent public implementation source inspection supplements facts omitted
from the schemas:

- [Azalea suggestion actions at 153c90aa](https://github.com/azalea-rs/azalea/blob/153c90aa5570b3a94b909e82c1c0b60a90fff5db/azalea-protocol/src/packets/game/c_custom_chat_completions.rs) defines the three operation values
- [Azalea packed signatures at 153c90aa](https://github.com/azalea-rs/azalea/blob/153c90aa5570b3a94b909e82c1c0b60a90fff5db/azalea-protocol/src/packets/game/c_player_chat.rs) corroborates zero/full-signature and index+1 packing; [delete chat](https://github.com/azalea-rs/azalea/blob/153c90aa5570b3a94b909e82c1c0b60a90fff5db/azalea-protocol/src/packets/game/c_delete_chat.rs) reuses that type
- [Azalea report details at 153c90aa](https://github.com/azalea-rs/azalea/blob/153c90aa5570b3a94b909e82c1c0b60a90fff5db/azalea-protocol/src/packets/game/c_custom_report_details.rs) documents the 32/128/4096 limits, while explicitly noting it does not enforce those limits
- [Minestom report serializer at 64381b6f](https://github.com/Minestom/Minestom/blob/64381b6ff9aefc976bb8bf9c01c13ee097213cb3/src/main/java/net/minestom/server/network/packet/server/common/CustomReportDetailsPacket.java) independently constrains the report map to 32 entries in configuration and play
- [MCProtocolLib report decoder at 19783c29](https://github.com/GeyserMC/MCProtocolLib/blob/19783c29ece24bc3f07f8ff08628549527e3de20/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/common/clientbound/ClientboundCustomReportDetailsPacket.java) reads keys/values with limits 128/4096; its [string reader](https://github.com/GeyserMC/MCProtocolLib/blob/19783c29ece24bc3f07f8ff08628549527e3de20/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/codec/MinecraftTypes.java) checks Java String length (UTF-16 units) and the three-bytes-per-unit encoded bound

These are pinned source cross-checks, not executed implementation oracles. They
corroborate modern semantics; earlier release boundaries come from each exact
hash-pinned schema. No claim is made of release-API, receiving-client acceptance
or live-server interoperability testing for these packets. No game binaries,
implementation code or private packet captures are included.

## Reproduction

```sh
python3 tools/verify_server_metadata_schemas.py
python3 -m unittest discover -s tools -p 'test_verify_server_metadata_schemas.py' -v
cargo test --locked --offline --no-default-features \
  --test server_metadata --test server_metadata_connection \
  --test common --test chat --test chat_state --test typed
cargo test --locked --offline --all-features \
  --test server_metadata --test server_metadata_connection \
  --test common --test chat --test chat_state --test typed
cargo clippy --locked --offline --no-default-features \
  --lib --test server_metadata --test server_metadata_connection -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --no-default-features --no-deps
cargo fmt --all -- --check
```

The new tests cover all 310 fixtures, all 5,601 strict body prefixes, trailing bytes,
exact/undersized packet budgets, transactional reader/writer behavior, version
and state gates, packet IDs/directions, malformed primitives, NBT depth/node
limits, UTF-16 boundaries, aggregate string-byte limits, duplicate report keys,
opaque icons, packed-signature/cache separation and full i64 extrema. Two
in-memory connection tests check configuration/play dispatch without automatic
writes and verify that malformed metadata remains an error. Eight Python
negative controls check hash, layout, alias, dispatch, state/direction, presence
and version-boundary drift.
