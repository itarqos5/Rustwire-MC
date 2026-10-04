# HUD and player-feedback wire audit

## Subsequent bounded live supplement

The original increment below used synthetic/source-level evidence. A later
2026-10-04 [live replay](hud-control-live-validation.md) supplements it for an
explicit subset on Paper 1.20.1, 1.21.1 and 26.2; see the
[exact results](validation/hud-control-live-results.json). This does not upgrade
unexercised packet families or semantics to live-validated status.

## Evidence and scope

The `packet::hud` module covers ten clientbound play packet families in every
supported protocol, 763–776: `clear_titles`, `action_bar`, `set_title_text`,
`set_title_subtitle`, `set_title_time`, `open_book`, `experience`,
`enter_combat_event`, `end_combat_event`, and `death_combat_event`.

Before this increment those catalog names had no typed codec or typed dispatch.
The existing boss-bar/player-list overlay, chat, health, inventory, movement,
world-state and player-roster codecs remain separate.

Evidence is the fourteen SHA-256-pinned files identified by
`research/schema-hashes.json` and their public provenance in `PROVENANCE.md`.
The original read-only `tools/verify_hud_schemas.py` verifies each complete body
structure, clientbound play name-to-type dispatch, and fixture packet ID against
its exact schema. It intentionally checks the protocol-775 hand mapper as a
mapper, rather than flattening away that schema difference. Neither neighboring
releases nor packet catalog presence stand in for a layout verification.

`tests/fixtures/hud.tsv` contains 140 original fixtures: the body bytes were
written independently by hand, and packet IDs were transcribed from the pinned
schemas. The fixtures do not come from Rustwire's Writer/encoder, game captures,
or upstream executable code. This is schema-backed wire coverage, with no new
release-API serializer or live-server validation. Receiving-client acceptance,
rendering, title timing behavior, book contents, experience calculations and
combat state transitions are not established by these tests.

## Exact layouts

All integers below preserve the signed wire domain unless stated otherwise.

| Packet | Body across 763–776 |
| --- | --- |
| `clear_titles` | Strict boolean `reset` |
| `action_bar` | Component `text` |
| `set_title_text` | Component `text` |
| `set_title_subtitle` | Component `text` |
| `set_title_time` | Big-endian i32 `fadeIn`, `stay`, `fadeOut` |
| `open_book` | VarInt `hand` |
| `experience` | f32 `experienceBar`, VarInt `level`, VarInt `totalExperience` |
| `enter_combat_event` | Empty body |
| `end_combat_event` | VarInt `duration` |
| `death_combat_event` | VarInt `playerId`, component `message` |

Components use the existing `ChatComponent` representation: a length-prefixed
UTF-8 JSON string through protocol 764, and a required anonymous NBT root from
765. Encoding a mismatched representation is an error; there is no conversion.
JSON syntax/component meaning and NBT component semantics are not validated.
The existing NBT machinery preserves duplicate compound keys, order, Java
modified UTF-8 and unpaired UTF-16 surrogates. TAG_End cannot stand in for the
required component. Named-root versus anonymous-root handling is not confused
with the separate general network-NBT boundary at 764.

`OpenBook::hand_id` retains an opaque signed VarInt in all releases. Only 775's
pinned schema supplies a mapper with 0 = main hand and 1 = off hand; the other
thirteen schemas call this field `varint`. `known_hand()` exposes those known
values using the existing `interact::Hand`, while unknown IDs remain typed raw
scalars. This is an explicit envelope policy, not evidence that any release's
client accepts unknown hands, and not a claim of semantic validation of 775's
mapper. `for_hand()` builds either known value.

Experience bars preserve signed zero, infinities and NaN payload bits. They are
not constrained to 0–1. Negative levels, total experience, title times, combat
durations and entity IDs are preserved rather than interpreted as allocation
counts or rejected by guessed gameplay rules.

## Resource and error boundaries

Each body is limited by `max_packet`; fixed-size and empty bodies follow the
same boundary. JSON byte requirements are checked before copying into the
output allocation. NBT uses the established bounded parser/writer and inventory
NBT budget helpers, including the bytes already used by the death packet's
player-ID prefix. String/UTF-16, collection, NBT node/array-element and depth
limits all apply. There are no unbounded packet collections in this slice.

Individual packet types have bounded transactional `read` and `write` methods:
failed reads do not consume the caller's reader, and failed writes do not append
partial data. `read` permits subsequent stream bytes; `decode` requires the
complete body and rejects trailing bytes. A zero-byte Enter Combat body remains
valid even with `max_packet = 0`.

`DecodedPacket::Hud` dispatches these names only in play state. Packet helpers
select each exact release's clientbound play ID; no serverbound alias is added.
The top-level dispatcher returns `None` for an unimplemented name, whereas the
family-specific `HudPacket::decode` returns `Error::Unsupported` for a name
outside this family. Malformed known fields, absent NBT, truncation, trailing
bytes and exhausted resource budgets remain errors. Unknown book-hand scalars
are retained within a successfully typed packet. An unknown packet ID still
travels through `Connection` as its complete raw packet; malformed known HUD
packets do not silently fall back. No new unframed/unknown nested layout is
present in this slice.

## Reproduction

```sh
python3 tools/verify_hud_schemas.py
python3 -m unittest discover -s tools -p 'test_verify_hud_schemas.py'
cargo test --offline --locked --no-default-features --test hud --test hud_typed
cargo clippy --offline --locked --no-default-features --lib \
  --test hud --test hud_typed -- -D warnings
cargo fmt --all -- --check
```

Tests check independently authored bodies and exact IDs in all fourteen
families; all strict truncations and trailing bytes; malformed booleans,
VarInts, UTF-8, lengths and NBT; signed scalar extremes and f32 bit patterns;
764/765 representation boundaries; packet, string, collection, NBT-node and depth
budgets; transactional stream reads/writes; all non-play states, directions,
and connection-level raw versus typed/error behavior. The schema verifier's
negative controls cover drift, hash mismatch, dispatch/ID changes and missing
or duplicate fixture keys. No runtime dependency is introduced.
