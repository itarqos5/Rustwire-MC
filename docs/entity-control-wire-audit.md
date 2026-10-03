# Ordinary entity-control wire audit

`packet::entity_control` adds seven clientbound play body codecs, packet helpers,
`EntityControlPacket` and `DecodedPacket::EntityControl`. Existing entity movement,
metadata/equipment, effects and serverbound interaction codecs are unchanged.
There is no entity simulation, registry lookup or automatic action policy.

## Schema-verified scope

All seven packet names exist in each exact supported protocol, 763–776. The
hash-pinned schemas show identical bodies throughout this range; packet IDs
remain release-specific and are resolved through the existing exact catalog.

| Packet | Wire body |
|---|---|
| `set_passengers` | signed VarInt entity reference, VarInt count, ordered signed VarInt passenger references |
| `attach_entity` | two fixed-width signed i32 references |
| `entity_head_rotation` | signed VarInt entity reference, one byte angle |
| `camera` | signed VarInt camera entity reference |
| `animation` | signed VarInt entity reference, unsigned u8 animation code |
| `damage_event` | four signed VarInts, canonical boolean, optional three f64 coordinates |
| `hurt_animation` | signed VarInt entity reference, f32 yaw |

The schema names the attachment's second field `vehicleId`; the API calls it
`holder_id` and retains it verbatim, without inferring passenger/riding state.
The signed schema `headYaw` byte is exposed through the existing lossless
`Angle(u8)` representation, interpreting rotation modulo a full turn.

Damage source-type IDs remain unresolved registry references. Cause/direct IDs
retain their raw optional-ID wire offsets: they are not automatically converted
into entity IDs. Zero, signed extrema and all other VarInt values are retained.
This is wire representation support, not a claim that arbitrary registry/entity
references identify valid game objects. No gameplay-range validation is guessed.

Animation codes use all 256 byte values instead of an unverified closed enum.
Unknown byte codes are preserved in the typed envelope; they do not introduce
an unknown nested layout. Unknown packet names still return `Unsupported` from
`EntityControlPacket::decode`; broader dispatch leaves unimplemented packets raw.
Malformed known bodies remain errors, not raw-fallback successes.

Float bits, including signed zero, NaN payloads and infinities, are retained.
Booleans follow Rustwire's canonical 0/1 convention; overlong VarInts follow the
shared reader's existing behavior and are canonicalized on re-encoding. These
are explicit codec choices, not claims about every release reader's permissive
or gameplay behavior.

## Budgets

- Decode rejects an oversized body before parsing or allocation
- Passenger counts obey `max_collection`, reject negative counts and are checked
  against the minimum remaining one byte per reference before reserving memory
- Encode checks passenger count and exact signed-VarInt byte size before growing
  the output buffer for the list; all bodies also obey `max_packet`
- Strict body completion rejects trailing data; incomplete and overflowing
  primitives remain errors through the shared codec
- Encoding is transactional: no partial body is returned on failure

## Evidence and its limits

`tools/check_entity_control_fixtures.py` verifies SHA-256 against
`research/schema-hashes.json`, exact presence/shape of all 98 packet layouts and
the nested `vec3f64` type. It separately obtains per-version packet IDs from the
same verified schema inputs. An original Python-standard-library encoder
creates 224 synthetic fixture bodies in `tests/fixtures/entity-control.tsv`;
it does not call the Rust codec under test.

The pinned source repositories and commits are:

- [PrismarineJS/minecraft-data at f5d7d746](https://github.com/PrismarineJS/minecraft-data/tree/f5d7d74604d8c6153fd086bfe035e0630a5207cc/data/pc)
- [Complexity-ML/minecraft-data-26.2 at 2a6a5fd9](https://github.com/Complexity-ML/minecraft-data-26.2/tree/2a6a5fd9ebb0d964a73312d11beef596b7ec029b/data/pc/26.2)

As a limited independent implementation cross-check, public
[Azalea protocol 776 packet declarations at 153c90aa](https://github.com/azalea-rs/azalea/tree/153c90aa5570b3a94b909e82c1c0b60a90fff5db/azalea-protocol/src/packets/game)
were inspected from an existing immutable checkout. Its packet declarations
agree with the seven outer layouts; its animation implementation documents that
it substitutes a VarInt enum for known small u8 action codes. Rustwire retains
an actual u8 and explicitly tests codes 128–255. This source inspection is not
an executed Azalea oracle and does not verify earlier release implementations.
No implementation code was copied.

These new fixtures are **schema-derived synthetic evidence**, independently
encoded from Rustwire, not official release-serializer outputs, live-server
captures or independent all-release interoperability proof. No game jars, copied game implementations or private captures were added.
Original release-API/live verification remains future work;
existing evidence for other packet families must not be attributed to this one.

## Reproduction and focused validation

```sh
python3 tools/check_entity_control_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_entity_control_fixtures.py' -v
cargo test --locked --offline --no-default-features \
  --test entity_control --test entity_control_typed --test entities --test typed
cargo clippy --locked --offline --no-default-features \
  --lib --test entity_control --test entity_control_typed -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --no-default-features --no-deps
cargo fmt --all -- --check
```

The nine body tests and two typed-dispatch tests cover 224 goldens and all 2,478
strict body prefixes, each with trailing data and exact/undersized packet
budgets. Representative golden assertions establish named field order. Further
cases cover all 256 animation/head-angle bytes in every protocol, signed scalar
extrema, attachment `-1`, raw damage offsets, float bit patterns, impossible
counts, collection boundaries, malformed options/VarInts and exact version/state
boundaries. Existing four entity and four typed/raw-fallback tests also pass.
Six standalone Python regression tests cover fixture cardinality, signed VarInt
extrema, hash mismatches, missing protocol rows and direct/nested schema drift.

Focused checks use Rust 1.99 stable, zero default dependencies, two build jobs
and a separate staging target. Full integrated feature suites and exact Rust
1.88 minimum-version verification are owned by integration; this focused result
does not assert that they ran here.
