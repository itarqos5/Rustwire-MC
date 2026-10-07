# Modern mixed-list NBT component hashes

Status: local tested fix, not published. No live inventory-click replay is claimed.

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

## Local validation status

The final local tree passes 603 no-default, 607 default and 642 all-feature Rust
tests, 20 example tests in each of the no-default/all-feature modes, strict
Clippy/rustdoc/formatting, and 19 focused hash tests on Rust 1.88. The machine
report retains log hashes. These are local Linux results; no GitHub CI run or
live server click result exists for this unpublished fix.
