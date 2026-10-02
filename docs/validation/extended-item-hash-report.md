# Live extended persistent item-hash validation

## Result

All seven modern protocol families passed 149 ordinary survival-inventory click cases: 142 correct predictions and seven deliberately wrong synchronization-hash controls. Correct predictions produced zero inventory corrections. Every wrong hash produced exactly one authoritative offhand-slot correction matching the complete expected item; a following correct prediction passed. Every case also passed an explicit server-side inventory-state predicate.

| Paper release | Protocol | Build | Correct predictions | Negative controls | Server/client exit |
|---|---:|---:|---:|---:|---|
| 1.21.5 | 770 | 114 | 19 | 1 | 0/0 |
| 1.21.6 | 771 | 48 | 19 | 1 | 0/0 |
| 1.21.8 | 772 | 60 | 19 | 1 | 0/0 |
| 1.21.10 | 773 | 130 | 19 | 1 | 0/0 |
| 1.21.11 | 774 | 132 | 22 | 1 | 0/0 |
| 26.1.2 | 775 | 74 | 22 | 1 | 0/0 |
| 26.2 | 776 | 129 | 22 | 1 | 0/0 |

The same frozen binary and complete execution-source manifest were used in both retained runs. The merge tool re-read the actual client/server logs, re-hashed the archived binary, checked recorded results against raw logs, and independently re-assessed every case. It refuses to merge differing source manifests or binaries.

## What was exercised

- Literal and styled custom names, hex-colored item names, shadow color, and styled Unicode lore
- Food, use cooldown, weapon data, firework explosions and multi-explosion fireworks
- Lodestone target/dimension/position, writable books, and written books with filtered and styled pages
- Inline-sound consumables, clear/teleport/play-sound death-protection effects, and inline break sounds
- Use effects, attack range, and swing animation in the three releases where these components exist

Each item was supplied by the isolated server as a stone stack with one selected component plus a unique custom-data case marker. The client decoded it, computed its persistent-codec hash, compared that hash with the independently obtained official public-CODEC/HashOps oracle, and sent a normal `HashedContainerClick` swap (`window_id=0`, source slot `36`, button `40`, mode `Swap`, predicted target slot `45`). Latest received container state IDs were retained.

The server predicate required `equipment.offhand` to contain exactly one stone with the expected case marker and required inventory source slot `0` to be absent. Both equipment and inventory were queried into the raw server log. A protocol marker after the observation interval delimited the correction window. No success was inferred merely from the connection remaining open.

For the negative control, only the predicted `custom_name` hash was changed: `-1318576648` became `-1318576647` by XOR with `1`. The server still executed the ordinary inventory action and reconciled the mismatched prediction. This validates synchronization behavior; it is not a cryptographic or anti-cheat test.

## Isolation, permissions, and shutdown

The harness reused previously approved baseline EULA/configuration and immutable boot caches, created fresh disposable worlds, bound the actual listener exclusively to `127.0.0.1`, used unique offline usernames, and confirmed survival mode. Every generated OP list remained empty. RCON/query and external account authentication were disabled. No creative inventory actions, external game servers, real accounts, or new terms acceptance were used.

All seven disposable `eula.txt` files remain byte-for-byte identical to their already-approved baseline copies. The audit records baseline/world SHA-256 values and checks baseline stability during comparison. Every client and server exited with status `0`; every per-server post-shutdown listener check was empty. The shared `one-server.lock` serialized the server window.

## Exact provenance

- Published production source: `fe7db1fb60ff3a8ceec634c4d3b03e74cdc4bc4b`
- Client binary SHA-256: `9b432f4e37954ccfc18fa9bea25e5751492a7302100dc62bac1a38da7600235c`
- Canonical complete execution-source-manifest SHA-256: `e345d77590fc0c440966810826181ad87ff014eeb1c91fe5c424baf3843bf1c0`
- Individual source, JAR, transcript, result, EULA, and audit-source hashes: [extended-item-hash-results.json](extended-item-hash-results.json)
- Independent official oracle: [component-hash-extended-oracle.json](component-hash-extended-oracle.json)
- Endpoint run ID: `20261002T214223859694` (1.21.5 and 26.2)
- Remaining-family run ID: `20261002T215413673768` (1.21.6, 1.21.8, 1.21.10, 1.21.11, 26.1.2)

Each run's exact UTC creation timestamp, base commit, result SHA-256, and per-server transcript hashes are retained in the machine report's `merge_provenance` list. Every production/Cargo file in the recorded manifest was independently compared with the published commit and matched.

Two exploratory attempts stopped at harness evidence-gate mistakes (selector syntax and the modern offhand NBT location). Those client/server processes were shut down cleanly, the harness was corrected, and neither exploratory run is included in the passing aggregate. No library change was required.

## Reproduce

Use independently obtained, digest-matching pinned Paper artifacts and baseline worlds under `../rustwire-server-validation`. The operator must already have explicitly accepted the applicable terms; this harness never accepts them. Server binaries/worlds are not committed.

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_TARGET_DIR=/tmp/rustwire-extended-hash-target \
  cargo build --locked --features crypto --example extended_hash_probe

python3 tools/paper/validate_extended_hashes.py \
  --client /tmp/rustwire-extended-hash-target/debug/examples/extended_hash_probe \
  --validation-dir ../rustwire-server-validation

python3 -m unittest discover -s tools/paper -p "test_validate_extended_hashes*.py"
cargo test --locked --features crypto --example extended_hash_probe
cargo test --locked --no-default-features --example extended_hash_probe
cargo +1.88.0 test --locked --features crypto --example extended_hash_probe
cargo clippy --locked --features crypto --example extended_hash_probe -- -D warnings
```

Audit a complete new run, or merge same-source partial runs by passing their `hash-results.json` paths:

```sh
python3 tools/paper/summarize_extended_hashes.py \
  ../rustwire-server-validation/extended-hashes/runs/RUN_ID/hash-results.json \
  --validation-dir ../rustwire-server-validation \
  --output docs/validation/extended-item-hash-results.json
```

Checks completed here: four probe unit tests on stable with crypto and without default features; the same four on Rust 1.88.0; strict probe Clippy; and 16 Python acceptance tests. Mutation tests reject wrong cases, absent/wrong-item/wrong-slot reconciliation, fabricated summary markers, mismatched raw state markers, command echoes, duplicate results/markers, missing predicate commands, changed binary/source provenance, missing/nonzero cleanup evidence, nonempty OP lists, and changed/unapproved EULA copies.

## Scope limits

This is the explicitly supported typed persistent-hash subset, not a general inventory simulator. Numeric registry holders, translated text, click/hover events, and other unsupported codecs remain explicit `Unsupported`. The report does not claim coverage for arbitrary components, other click modes, arbitrary inventories, authentication, or third-party server/anti-cheat behavior.
