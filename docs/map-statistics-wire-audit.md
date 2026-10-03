# Map data and statistics wire envelopes

## Scope and evidence

The clientbound play names `map` and `statistics` now have typed, bounded
encode/decode, single-body read/write, and packet-ID helpers for protocols
763–776. Dispatch returns `DecodedPacket::Map` or `DecodedPacket::Statistics`.
This is wire-envelope coverage, not map rendering, statistics state management,
or registry resolution.

All fourteen inputs were SHA-256 checked against
[`research/schema-hashes.json`](../research/schema-hashes.json). The immutable
upstream source commits are recorded by
[`tools/fetch_schemas.py`](../tools/fetch_schemas.py):

- PrismarineJS/minecraft-data, commit
  `f5d7d74604d8c6153fd086bfe035e0630a5207cc`, protocols 763–775
- Complexity-ML/minecraft-data-26.2, commit
  `2a6a5fd9ebb0d964a73312d11beef596b7ec029b`, protocol 776

[`tools/verify_map_statistics_schemas.py`](../tools/verify_map_statistics_schemas.py)
checks every field name, order, primitive, count prefix, option and switch in
all 28 packet layouts. It is read-only, downloads nothing and fails on hash or
layout drift. This is schema evidence only. No new release-API, Mojang serializer
or live-server validation was performed for these packets. No cached server JAR,
game source, binary or packet capture was used.

## Layouts and deliberate limits

| Protocols | Map label | Remaining map envelope | Statistics envelope |
|---|---|---|---|
| 763–764 | Optional JSON string | VarInt ID; signed-byte scale; locked boolean; optional VarInt-counted decorations; unsigned-byte columns; nonzero-column patch suffix | VarInt count; category ID, statistic ID, value, each a VarInt |
| 765–776 | Optional anonymous NBT | Same field order and primitive widths | Same layout |

Each decoration carries a VarInt kind, signed-byte x/z, unsigned-byte rotation,
and optional label. The patch suffix carries unsigned-byte rows, x/z offset,
and a VarInt-length byte array. `None` decorations and `Some(empty)` remain
separate; zero columns means no patch. Encoding `Some(patch)` with zero columns
is rejected because it would lose the patch fields.

IDs remain signed wire scalars. No map-decoration enum table, statistic-category
range, registry membership, name resolution or cross-release ID stability is
assumed. Retaining an ID does not establish that a game registry accepts it.
Statistics preserve order and duplicate entries. Signed values, map scales and
full rotation bytes are retained instead of imposing unverified gameplay clamps.

Map patches preserve dimensions, offsets and the received color-array length.
`MapPatch::validate_canvas()` separately checks nonzero dimensions, overflow-safe
bounds within a 128×128 canvas, and exactly `columns * rows` color bytes. This is
application-side validation, not a claim about official release-reader rejection.
It does not interpret colors, validate palette membership or apply the update.

Labels reuse the established opaque `ChatComponent` representation. JSON syntax
and semantic text-component validity are not interpreted here; anonymous NBT
supports the crate's complete NBT tag envelope. There is no partially decoded
nested component or guessed opaque tail. Unknown registry IDs need no new layout
and remain numeric. Existing `Connection` behavior still preserves full raw
packets on genuinely unsupported typed layouts; malformed known bodies are
errors rather than silently converted to raw packets.

## Resource and failure guarantees

- Entire decode inputs and each streamed body are bounded by `max_packet`
- A count must fit the remaining minimum wire bytes before vector reservation
- Decorations and pixel bytes share a packet-level `max_collection` budget
- All NBT labels share `max_nbt_nodes`, including primitive-array elements;
  per-NBT collection, depth, string and byte limits also apply
- Legacy label strings use the existing component cap and caller string limit;
  string and color bytes are checked before copying into output buffers
- Strict booleans, negative lengths, overflowing VarInts, invalid UTF-8/NBT,
  missing required label NBT, truncation and trailing bytes are rejected
- Stream reads leave the reader unchanged on failure; appending writes are
  transactional because the entire body is encoded before touching the target

## Tests and evidence boundary

The fixtures in [`tests/map.rs`](../tests/map.rs),
[`tests/statistics.rs`](../tests/statistics.rs), and
[`tests/map_statistics_typed.rs`](../tests/map_statistics_typed.rs) are original
hand-authored bytes, independent of the Rust encoder. They check all fourteen
families and the exact 764/765 label boundary, no-update/clear/update variants,
signed and multibyte scalars, duplicate statistics, every fixture truncation,
trailing bytes, malformed flags/counts/VarInts/labels, aggregate budgets, exact
byte limits, single-body reads, transactional failure and typed play dispatch.
Canvas checks include a full map, the far edge, zero dimensions, out-of-range
edges and short/long pixel arrays.

These fixtures independently exercise this implementation against the documented
schema interpretation. They are not an independent upstream protocol source or
proof that arbitrary retained wire values are accepted by a vanilla client.
Release-API and live traffic checks remain future validation work.
