# World/session wire audit

This focused addition provides body decode, encode and packet helpers in
`packet::world_state` for eleven clientbound packet names and serverbound
`player_loaded`. It does not implement a simulation, registry database, clock
scheduler or automatic acknowledgement policy. The aggregate clientbound API is
`WorldStatePacket`, with `encode` returning body bytes and `packet` returning a
`RawPacket`. The broader typed dispatcher exposes `DecodedPacket::WorldState`;
unknown game reasons retain its raw fallback after fixed-body validation.

## Release-verified boundaries

| Envelope | Verified representation |
|---|---|
| Game-state change | u8 reason followed by raw f32 parameter; reasons 0–11 in 763, 0–12 in 764, 0–13 in 765–776 |
| Time update, 763–767 | i64 world age, i64 raw day time (including its legacy sign encoding) |
| Time update, 768–774 | i64 world age, i64 day time, boolean tick-day-time |
| Time update, 775–776 | i64 world age, VarInt-sized clock-update map; each entry is direct clock registry VarInt ID, VarLong ticks, f32 partial tick, f32 rate |
| Spawn position, 763–772 | packed block position, f32 yaw |
| Spawn position, 773–776 | dimension resource identifier, packed block position, f32 yaw, f32 pitch |
| Difficulty, 763–770 | unsigned byte difficulty, boolean locked |
| Difficulty, 771–776 | signed VarInt difficulty, boolean locked |
| Border initialization | four f64s, **VarLong** duration (milliseconds through 773; ticks from 774), three signed VarInts |
| Border center/size | two/one f64s |
| Border interpolation | two f64s, **VarLong** duration (milliseconds through 773; ticks from 774) |
| Border warning distance/time | signed VarInt scalar |
| Block-change acknowledgement | signed VarInt sequence; catalog name `acknowledge_player_digging` |
| Player loaded | empty serverbound body, present exactly from 769 |

The duration fields are VarLong in all fourteen inspected release serializers,
although all fourteen pinned schemas call them VarInt. Long durations above
32 bits and negative ten-byte VarLongs are included in the independent fixtures.
No 26.x body layout is inferred from a neighboring version.

## Domains and deliberate behavior

- Floating-point bits are preserved, including infinities, NaNs and signed zero.
  Scalar borders, time fields and acknowledgement sequences are not clamped to
  gameplay ranges. Only true counts, byte budgets and field representations are
  bounded.
- Difficulty readers wrap any wire integer modulo four. The typed envelope
  retains the original raw value; `level()` exposes that release-compatible
  interpretation. Encoding preserves the raw integer instead of normalizing it
  as the release object serializer does. Legacy encoding still requires a u8.
- Unknown game-event IDs deserialize to a null event in the release APIs and
  cannot be re-encoded there. This library returns `Unsupported` after verifying
  the complete fixed-size body, allowing the outer raw fallback. Truncation and
  trailing bytes remain malformed errors, including for unknown reasons.
- Clock IDs are direct nonnegative registry IDs. Their identities and availability
  remain caller registry context. The original API oracle registers three
  synthetic clocks in memory; no game server is started. Negative IDs fail in
  the release registry decoder. Wire order and duplicate clock entries are
  retained; release map readers apply last-value-wins and may reorder entries.
- Spawn identifiers reuse the existing verified identifier syntax: empty paths
  are accepted, and namespace `..` becomes invalid in 775. The envelope preserves
  source spelling, whereas release serializers normalize omitted namespaces.
- Booleans follow the library's existing canonical 0/1 reader convention. This
  does not attempt to preserve noncanonical truthy byte spellings.
- Caller-supplied limits cover packet bytes, dimension-string UTF-16 length and
  clock collection size. Clock allocation is also preflighted against the minimum
  remaining ten bytes per entry. Version-specific field shapes must match exactly.

## Independent evidence and reproduction

`tests/fixtures/world-state.tsv` contains 162 synthetic release-serializer golden
bodies across all fourteen protocols. Packet IDs were extracted separately from
the hash-verified pinned schemas. `docs/validation/world-state-wire-oracle.json`
records 586 actual API cases, input/output bytes, expected failed domains, schema
hashes, oracle-source hash and the hash of each cached release jar. API oracle
code is original; no game binaries or decompiled implementation are included.

The runtime inputs are the prepared Paper release jars and dependencies used by
the existing validation project. These expose the release packet serializers.
This is serializer interoperability evidence, not live-server gameplay or
connection/dispatch acceptance evidence.

```sh
python3 tools/paper/validate_world_state.py \
  --runtime-root /path/to/rustwire-server-validation \
  --schemas /path/to/hash-pinned/protocols \
  --output /path/to/new/world-state-evidence
(cd tools/paper && python3 -m unittest test_validate_world_state -v)
cargo test --locked --no-default-features --test world_state
```

The Java oracle boots only registries and invokes packet APIs. Its validator
fails on missing/extra cases, unconsumed bytes, changed accepted domains,
difficulty fallback drift, clock-map semantic drift or fixture drift. Eight
Python mutation/regression tests verify these failure conditions.

Eight Rust test functions cover the 162 independent fixtures, every one of their
2,269 strict body prefixes, 6,807 single-byte mutations, trailing data, exact and
undersized packet budgets, collection/string limits, version boundaries,
identifier rules, signed extrema, malformed VarInts/VarLongs and raw float bits.

Verified on the isolated baseline `01bf0a0739199949cd4aa584738aaba0e49fadc3` plus
this addition: stable no-default (309 tests), stable default (313), stable
all-features (347), Rust 1.88 no-default (309), Rust 1.88 all-features (347), strict
all-target/all-feature Clippy, strict all-feature rustdoc and formatting. Broad
connection-dispatch coverage is left to integration with the parent change.

## Border-duration semantic correction

A bounded 26.2 smoke replay exposed a misleading `duration_ms` API label even
though all serializer bytes round-tripped. Cached command and moving-border
APIs independently establish the unit switch at protocol 774 (1.21.11): older
releases use wall-clock milliseconds; newer releases track game ticks. The
[all-release semantic facts](validation/world-border-duration-units.json) contain
only API observations and jar hashes, with the verifier hash for reproduction:

```sh
python3 tools/paper/verify_border_units.py --runtime-root /path/to/rustwire-server-validation \
  --output /path/to/new/border-units.json
```

`InitializeWorldBorder::duration` and `WorldBorderLerpSize::duration` now carry
`BorderDuration::Milliseconds(i64)` or `BorderDuration::Ticks(i64)`. Decoding
selects the exact release unit and encoding rejects a mismatched variant. No
20-TPS assumption or lossy conversion is applied. The new regression tests both
packet types, every supported release, signed extrema and values above 32 bits.
The initial failed smoke remains preserved outside this evidence package; a
corrected frozen-source live matrix is still pending at this checkpoint.
