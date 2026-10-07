# Modern mixed-list NBT component hashes

Status: tested component-hash correction. No live inventory-click replay is claimed.

## Defect and correction

Modern `ListTag` loading interprets an immediate compound list entry whose only
key is the empty string as a wrapper and unwraps one layer. The binary NBT
format remains homogeneous: mixed logical lists use compound wrappers on the
wire. A real single-empty-key compound is double-wrapped so its map survives.
Duplicate keys collapse last-write-wins before the wrapper test.

Rustwire's lossless NBT decoder correctly retained that wire representation,
but `hash_nbt` hashed wrappers as real maps. The supported `custom_data` and
`bucket_entity_data` component-hash paths inherited the wrong result. Official
release APIs return `220565617` for the mixed byte/string fixture, while the
previous implementation returned `1940058576`.

The correction is confined to hashing: immediate list-entry compounds are
normalized exactly once, then their logical values are hashed normally. Empty
compounds, root/ordinary-field empty-key compounds and compounds with another
distinct key remain maps. Nested lists normalize at their own entry boundaries.
Raw NBT decoding, encoding, duplicate preservation and roundtrip bytes are
unchanged. Raw input byte, node, collection and depth limits are still checked
before hashing; normalization cannot hide over-budget wrappers.

## Evidence

[Machine-readable oracle](validation/mixed-nbt-hash-oracle.json) records 14
independently encoded fixtures and executed public API results for pinned Paper
1.21.5, 26.1.2 and 26.2. Both official component CODECs agree with logical
`NbtOps` to `HashOps` conversion on every fixture. Six fixtures exposed the old
bug; eight controls prevent over-unwrapping or unrelated hash changes.

The [official 1.21.5 release notes](https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-5)
describe heterogeneous lists and their unchanged binary storage representation.
Read-only exact-jar inspection also verified the network `ListTag.addAndUnwrap`
path and the ordinary `CustomData.CODEC`/`NbtOps` hashing path. No decompiled game
implementation is included.

The four Rust regressions cover the golden hashes and byte-exact raw roundtrips,
both component types and `HashedItemStack::from_slot` across 770–776, malformed
wire-list construction, strict raw budgets and all 267 strict fixture prefixes.
Golden regressions across seven families are distinct from the three release
APIs actually executed. Live clicks, other persistent codecs, and broader NBT
semantic changes are outside this fix.

## Reproduce the independent API oracle

Use an already prepared, checksum-verified official Paper installation and the
pinned Java 25 runtime. Compile the authored `tools/paper/MixedNbtHashOracle.java`
against its extracted server jar plus all runtime library jars. Feed it the
first two columns (`name|wire_hex`) of `tests/fixtures/mixed-nbt-hash.tsv` on stdin.
The `ORACLE` rows contain logical/direct-codec hashes, normalized binary bytes
and logical SNBT; `COMPONENTS` rows separately execute both registered component
CODECs. The program does not start a server, listener or account sign-in.

The Rust check is:

```sh
cargo test --locked --test item_hash_mixed_nbt
```

## Original local validation status, 2026-10-05

The final local tree passes 603 no-default, 607 default and 642 all-feature Rust
tests, 20 example tests in each of the no-default/all-feature modes, strict
Clippy/rustdoc/formatting, and 19 focused hash tests on Rust 1.88. The machine
report retains log hashes. These were local Linux results; no GitHub CI run or live server click
result was included in the original checkpoint.

## Recovery and network-path regression checkpoint, 2026-10-07

The original local fix (`601bd5663549a3e785181ff2fdde50de76001da6`) was restored
from its checksum-verified backup. Public main was independently checked
and remained at `d7bfeedc0d99942192e0e0ceb60d6cce0a3b1166`. This checkpoint
contains only the NBT correction and its regression evidence.

`tests/item_hash_mixed_nbt_wire.rs` extends coverage from standalone NBT values
to the inventory network path:

- 196 cases cover all 14 official-API NBT fixtures, both supported components,
  and protocols 770–776. Independently assembled incoming `set_slot` bytes pass
  through typed dispatch, retain their raw representation, and produce the exact
  expected hashed click payload and release-specific packet ID
- Fourteen in-memory framed `Connection` flows exercise login/configuration,
  incoming inventory dispatch, and sending the prediction. Each send must emit
  exactly one expected frame, including 26.2's login-session UUID boundary
- All 6,090 strict incoming-packet prefixes, trailing-byte variants, and tested
  byte/node over-budget cases fail as malformed/limited input, never unsupported
  raw fallback. A malformed known frame is also checked through `Connection`
- Discarded duplicate-empty-key values still consume raw string, depth and node
  budgets before last-write-wins normalization, through all three hash APIs and
  both component types. These are new relational regressions, not additional
  executed official API fixtures

The packet/component IDs and envelope layouts were independently checked
against all seven checksum-pinned research schemas. Expected NBT hashes retain
the original three-release API provenance. The window-130, slot-36/45 packet is
a synthetic swap-shaped wire fixture; no valid server menu operation, gameplay
acceptance, or live server replay is claimed.

As a negative control, the same wire tests were run against public main without
the fix. Both the typed prediction and framed-send regressions failed on the
incorrect hashes; the malformed-input test passed. All three pass with the fix.

Fresh Linux validation passes 607 no-default, 611 default and 646 all-feature
Rust tests; 20 example tests in each of the no-default/all-feature modes; all
23 focused hash tests on Rust 1.88; strict Clippy, rustdoc and formatting; all
three generated metadata checks; and 131 Python validation-harness tests.
The [machine-readable checkpoint](validation/mixed-nbt-wire-checkpoint-20261007.json)
records source, schema and log digests from the local checkpoint before
publication. Live inventory-click replay remains unperformed.
