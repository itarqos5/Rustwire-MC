# Boss-bar and player-list overlay wire audit

`overlay::BossBar` and `overlay::PlayerListHeaderFooter` cover clientbound play
packets `boss_bar` and `playerlist_header` in all release protocols 763–776.
The only component boundary in these envelopes is JSON through 764, anonymous
NBT from 765. Components remain untrusted wire data; this is not a renderer.

`tools/paper/OverlayOracle.java` is an original synthetic fixture harness. It
invokes the prepared official release packet APIs, decodes and re-encodes the
fixtures, and asserts byte equality. It contains no game implementation and
starts no server or network connection. `validation/overlay-wire-oracle.json`
records the source hash, prepared-jar hashes, verified pinned schema hashes,
and 896 successful checks (64 in each of 14 releases):

- Six boss operations and a header/footer fixture
- All 35 color/division combinations
- Negative zero, negative progress, positive/negative infinity and a NaN payload
- Eight flag bytes, including reserved high bits
- Nine invalid operation/color/division ordinals, including negative IDs

Boss operation IDs are 0–5, color IDs 0–6, and division IDs 0–4. Their closed
enum domains reject unknown IDs. Health/progress is a raw IEEE-754 `f32`; the
wire API does not enforce the nominal 0–1 gameplay range or finiteness.
Official decoding accepts high flag bits and its writer keeps only bits 0–2.
Rustwire intentionally retains the entire incoming byte when re-encoding, as a
lossless wire envelope. Applications can use `BossBarFlags` accessors for the
three defined meanings.

## Reproduce

Use an independently prepared validation workspace containing the release jars,
libraries and JDK described by the existing Paper tooling:

```sh
python3 tools/paper/validate_overlays.py \
  --root ../rustwire-server-validation \
  --schemas research/protocols \
  --output /tmp/rustwire-overlay-wire-oracle.json
```

The runner does not download software or accept agreements. Optional repeated
`--release` selects individual release representatives. `tests/overlay.rs`
separately checks Rust byte fixtures and IDs, all prefixes/trailing bytes,
invalid enums/representations, exact float bits, full flag bytes, mutation
resilience, UTF-16/byte limits, NBT depth/aggregate node budgets, and transactional
body read/write behavior across all 14 families.

## Limits of this evidence

No live server interaction or UI rendering is claimed. The API fixtures use
literal text components; arbitrary component/style semantics and dynamic
registry content are outside this packet-envelope implementation. The nested
component representation is intentionally preserved using `ChatComponent`.
