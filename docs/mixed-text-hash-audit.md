# Modern mixed-list text component hashes

Status: executed release-API oracle and Rust network-path regressions. No live
inventory-click replay or broader text-codec support is claimed.

## Defect and bounded correction

For `{"text":"x","extra":["a",{"text":"b","bold":true}]}`, the pinned
release stream codecs serialize a compound list containing an empty-key wrapper
around `"a"` and an ordinary styled text compound. The expected custom/item-name
hash is **-1619342070**. Rustwire preserved those bytes correctly but rejected
the wrapped sibling as a nonliteral text component when predicting a stack hash.

The existing modern NBT list-entry normalization is now shared with the literal
text parser. It unwraps one layer at each root-list or `extra`-list entry, after
duplicate empty keys collapse last-write-wins. Nested lists have their own entry
boundaries. No root/ordinary-field wrapper is unwrapped, and a double wrapper
still leaves a real empty-key map. Raw NBT bytes and duplicate entries remain
untouched by the inventory codec. All raw input budgets run before normalization.

The change applies to already-supported literal custom names, item names, lore,
and raw/filtered written-book pages. Translation, click/hover events and unknown
style/content fields remain unsupported. Four oracle controls contain an empty
key plus a `text` key: the release codec ignores the unknown field, while
Rustwire deliberately keeps its existing fail-closed subset. These controls are
not described as invalid Minecraft text.

## Independent evidence

The [complete machine-readable report](validation/mixed-text-hash-oracle.json)
records 47 cases executed on each checksum-pinned Paper runtime:

- 1.21.5 build 114
- 26.1.2 build 74
- 26.2 build 129

All three releases produce identical complete rows: 39 accepted and eight
rejected. Every accepted row is encoded and decoded through the registered
`DataComponentType.streamCodec()` and rehashed through the persistent component
codec with `HashOps.CRC32C_INSTANCE`. Name values are also read through
`NbtIo.readAnyTag` and the persistent codec with matching hashes.

The report distinguishes two input sources:

- JSON rows use bytes actually emitted by the release stream serializer.
- WIRE rows use independently authored binary inputs, including root/nested
  lists and duplicate keys, then execute the release stream decoder and hasher.
  The report separately records canonical re-encoding.

Logical SNBT is diagnostic only; it is never substituted for network bytes.
Lore fixtures include their count and individual NBT roots. Written-book
fixtures include title/filter, author, generation, page/filter and resolved
fields, rather than substituting their persistent NBT representation.

These are release Minecraft API calls inside Paper distributions. No live
server, network capture, inventory acceptance, account login or unpatched
vanilla-process result is implied. No game binaries or implementation code are
included in the repository.

## Rust regressions and negative control

`tests/item_hash_mixed_text.rs` checks:

- 35 supported fixtures across all seven protocols 770–776: 245 independent
  component payloads inside synthetic Slot envelopes. Slot decode/re-encode
  must preserve all bytes, and `HashedItemStack::from_slot` must match the oracle
- Eight official invalid-text and four intentionally out-of-subset controls
  across all seven protocols, without altering lossless Slot decoding
- All 13,118 strict supported-Slot prefixes and every trailing-byte variant
- Duplicate-empty-key last-wins behavior, one-layer-only normalization, empty
  lists/compounds, and unchanged unsupported text/style boundaries
- Raw byte, depth, node and collection limits, including discarded duplicate
  values that would otherwise hide excess string/depth/node budgets

The Slot component IDs are independently checked against the seven unchanged
checksum-pinned research schemas. Testing seven protocol families with common
goldens does not claim that seven separate release APIs were executed.

As a negative control, the same eight Rust tests were run against the previous
published NBT-branch source (`5b5fc03`). The official supported payload test and
two duplicate-wrapper tests failed with `Unsupported`; five rejection/budget
controls passed. All eight pass with this correction.

## Reproduce

Use a separately obtained, checksum-verified prepared Paper installation from
the report and a compatible Java runtime. Compile the original
`tools/paper/TextMixedWireOracle.java` against that installation's server jar and
runtime libraries, then run it with `tools/paper/text-mixed-inputs.tsv` on stdin.
Do not mix libraries between releases. The report records exact artifact URLs,
runtime/library hashes, oracle source and input hashes, and process exits.

`tools/paper/generate_text_mixed_inputs.py` independently creates `inputs.tsv`
in the current directory. Running it in an empty temporary directory must
reproduce the committed input file byte-for-byte. It does not import Rustwire.

```sh
cargo test --locked --test item_hash_mixed_text
cargo test --locked --test item_hash_mixed_nbt --test item_hash_mixed_nbt_wire
```

No new local Rust 1.88, Clippy or rustfmt result is claimed: those tools were not
available locally. The exact published commit's CI runs the minimum Rust
version, strict Clippy/formatting, and the Linux/Windows/macOS feature matrix.

Local Linux Rust 1.99 checks pass 615 no-default, 619 default and 654 all-feature
Rust tests; 20 example tests in each minimal/all-feature mode; all-feature
example compilation; strict rustdoc; 131 Paper-tool and 166 root-tool Python
tests; packet/component coverage checks; and 14 historical connection acceptance
controls. The unchanged entity-metadata generator check could not run because
it requires the unavailable local rustfmt. These local counts do not replace
exact-head CI or add new live-server evidence.
