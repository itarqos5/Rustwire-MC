# Recipe-book control wire audit

## Scope and evidence

`packet::recipe` provides three serverbound controls and four clientbound
control types, including a deliberately legacy-only ghost-recipe response.
It does not manage a recipe registry, resolve display IDs, update a recipe book,
execute crafting, validate a menu, or decode recipe declarations/displays.

The evidence has separate levels:

- `tools/check_recipe_control_fixtures.py` checks all fourteen existing
  SHA-256-pinned schema files, complete outer layouts, relevant primitive and
  settings aliases, presence/absence, direction, unique packet IDs and
  name-to-body dispatch. It checks **79 present outer layouts**. Of these,
  **70 packet/family combinations** have complete typed bodies; nine modern
  ghost-response outer envelopes are checked only to establish their excluded
  `RecipeDisplay` boundary. No nested `RecipeDisplay` audit is claimed.
- Pinned, independently authored MCProtocolLib and PacketEvents sources were
  inspected for container widths, old/new recipe-reference boundaries and
  modern signed scalar domains. Their disagreement about legacy byte signedness
  is recorded below. Source inspection is not execution of either library or
  Minecraft's release serializer.
- An original Python encoder produces **306 synthetic fixture rows**, without
  invoking Rustwire's encoder. These include 287 accepted bodies and 19 complete
  unsupported-enum bodies. Every one of their 2,955 strict truncation prefixes
  is rejected. These are not captured traffic or official release-API outputs.

[Source audit manifest](validation/recipe-control-source-audit.json) records
immutable source URLs and SHA-256 hashes of inspected files. Upstream source
text is not vendored. No new dependency, game binary, account, server process,
network gameplay test or copied external implementation is involved.

## Exact version boundaries

| Direction / packet | Protocols | Ordered body |
| --- | --- | --- |
| Serverbound `craft_recipe_request` | 763–767 | raw window byte, recipe registry-key string, make-all boolean |
| Serverbound `craft_recipe_request` | 768–776 | window VarInt, recipe display-ID VarInt, make-all boolean |
| Serverbound `displayed_recipe` | 763–767 | recipe registry-key string |
| Serverbound `displayed_recipe` | 768–776 | recipe display-ID VarInt |
| Serverbound `recipe_book` | 763–776 | book-kind VarInt, open boolean, filtering boolean |
| Clientbound `craft_recipe_response` | 763–767 | raw window byte, recipe registry-key string |
| Clientbound `craft_recipe_response` | 768–776 | **not implemented**: window VarInt and nested `RecipeDisplay` |
| Clientbound `unlock_recipes` | 763–767 | action VarInt, eight settings booleans, primary key list, initialization-only second key list |
| Clientbound `recipe_book_settings` | 768–776 | eight settings booleans |
| Clientbound `recipe_book_remove` | 768–776 | count-prefixed display-ID VarInt list |

The settings order is crafting, furnace, blast furnace, smoker; each pair is
open then filtering. The schema introduces a `RecipeBookSetting` container alias
at 771, without changing the eight-byte body. Book-kind ordinals are 0 through 3
in that same order. Legacy unlock ordinals are initialization=0, add=1, remove=2.
The second list exists only for initialization, after the primary list. Lists
retain order and duplicates; the codec does not apply highlights or unlocks.

`RecipeReference::RegistryKey` and `RecipeReference::DisplayId` cannot be silently
encoded in the wrong family. `RecipeWindowId` similarly requires an explicit
choice between the legacy byte and modern VarInt. Standalone settings/removal
packets are catalog-gated at 768; legacy unlock is catalog-gated through 767.
`LegacyCraftRecipeResponse` is explicitly rejected from 768 onward.

## Legacy window-byte ambiguity, without loss

All inspected sources agree on the **one-byte width** through 767, then VarInt
from 768. They do not uniformly agree on the signed numeric interpretation of
the old byte:

- MCProtocolLib's exact 763, 764, 766 and 767 release refs use signed-byte reads
  for both request and ghost response. Representative pinned files:
  [763 request](https://github.com/GeyserMC/MCProtocolLib/blob/caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c/src/main/java/com/github/steveice10/mc/protocol/packet/ingame/serverbound/inventory/ServerboundPlaceRecipePacket.java),
  [766 request](https://github.com/GeyserMC/MCProtocolLib/blob/8dbdffc0004746122efd1fbe63d1c1620a8336db/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/serverbound/inventory/ServerboundPlaceRecipePacket.java),
  [767 ghost response](https://github.com/GeyserMC/MCProtocolLib/blob/e71d7df8c127b7196fd81dd392034d7d97acbb4e/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/inventory/ClientboundPlaceGhostRecipePacket.java).
  The matching `MinecraftCodec.java` version declarations are included in the
  manifest; there is no exact 765 source-execution claim.
- PacketEvents v2.13.0's
  [legacy response](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerCraftRecipeResponse.java)
  also reads a signed byte. Its
  [request](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/client/WrapperPlayClientCraftRecipeRequest.java)
  uses the generic
  [`readContainerId` helper](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/PacketWrapper.java),
  which reads an unsigned byte in legacy versions.
- The pinned schemas use `i8` initially, then an unsigned `ContainerID` alias for
  the response at 766 and request at 767. Those schemas are checked unchanged.

This is a documented representation disagreement, **not a proven vanilla
signedness defect**. Rustwire preserves every byte as
`RecipeWindowId::LegacyByte(u8)` (and a raw `u8` on the legacy-only response).
The wire values `0x80` and `0xff` therefore appear as 128 and 255. The explicit
`legacy_signed()` view gives -128 and -1; `from_legacy_signed()` converts back
without loss. The ordinary 0–127 domain is unambiguous. A modern VarInt cannot
be silently truncated into a legacy byte, including 128, 255 or 300.

Fixtures cover raw byte values 0, 127, 128 and 255, plus tests for the signed
-1/-128 views. Modern fixtures include -1, 127, 128, 255, 300 and both i32 bounds.
No vanilla receiving-menu acceptance is inferred from these byte-level tests.

## Modern signed scalar evidence

Pinned MCProtocolLib
[768 request](https://github.com/GeyserMC/MCProtocolLib/blob/667e02d38cd4a79d46640312cd599123609633ac/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/serverbound/inventory/ServerboundPlaceRecipePacket.java),
[769 request](https://github.com/GeyserMC/MCProtocolLib/blob/7cc247026c6bff700c644d545942f7733ebb6ad0/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/serverbound/inventory/ServerboundPlaceRecipePacket.java), and
[772 removal](https://github.com/GeyserMC/MCProtocolLib/blob/486f59d2784f5818e80ab2632fc98e30ea7e6522/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/ClientboundRecipeBookRemovePacket.java)
read/write signed Java int VarInts without nonnegative validation of the window
or display values. PacketEvents'
[`RecipeDisplayId`](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/recipe/RecipeDisplayId.java)
and [removal reader](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerRecipeBookRemove.java)
also preserve signed int values. The library retains this wire domain. It does
not claim that negative IDs resolve to a real menu or registered display.
Later families are schema-checked, not individually executed against upstream
release implementations.

## Bounds and raw/error distinctions

- Packet byte budgets apply to both direct decode and encode; string/list growth
  is checked against the remaining output budget. No partial body is returned.
- Counts are nonnegative, bounded by `max_collection`, and checked against the
  remaining input before vector allocation. Legacy initialization shares one
  aggregate collection budget across its two lists.
- Registry-key strings use the shared resource-location validator, the wire
  32,767-character ceiling and `max_string_chars`. No registry lookup or string
  normalization is performed. The shared validator's accepted unqualified and
  empty-path forms remain unchanged.
- All booleans are strict; VarInts must fit i32. Bad UTF-8, malformed identifiers,
  negative counts, truncated/trailing bodies and exceeded limits are errors.
- Unknown book-kind and legacy action ordinals return `Unsupported` only after
  the complete currently defined body, strict fields and trailing-data check.
  Clientbound typed dispatch thus preserves a complete unknown-action packet as
  `TypedEvent::Raw` with the unsupported reason, while malformed known envelopes
  still fail.
- `declare_recipes`, modern `recipe_book_add`, and modern `craft_recipe_response`
  remain unimplemented whole packets. Named dispatch returns `None`; connection
  dispatch keeps their raw bytes without claiming to validate their internals.
  Modern `RecipeDisplay`/slot-display/item payloads are not opaque "typed" tails.
- Serverbound controls have explicit directional `packet()` helpers. They are
  not accidentally included in the clientbound `DecodedPacket` dispatcher.

## Reproduction and limits

```sh
python3 tools/check_recipe_control_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_recipe_control_fixtures.py'
cargo test --locked --offline --no-default-features --test recipe_control --test recipe_control_typed
cargo test --locked --offline --all-features --test recipe_control --test recipe_control_typed
cargo clippy --locked --offline --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --all-features --no-deps
cargo fmt --all -- --check
```

The real schema audit requires the already hash-verified schema cache (populate
it with the repository's existing fetcher when authorized). The nine Python
verifier regressions instead construct a small self-contained synthetic schema
corpus with IDs from committed fixtures. They pass in an isolated copy containing
only the two Python files and fixture TSV, with no `research/protocols` directory
or network access. Synthetic mutation tests establish checker behavior; they do
not replace the independent pinned-schema audit above.

Eleven focused Rust tests passed in both feature modes on the installed stable
compiler. The complete staging no-default suite passed 433 tests. Strict
all-target/all-feature Clippy, strict all-feature rustdoc, formatting, the real
79-layout audit and nine self-contained Python regressions passed. MSRV execution
remains a separate CI gate. No new live server, receiving-client acceptance,
release-API oracle or crafting gameplay validation is claimed.

## Subsequent legacy declaration increment

The earlier declaration exclusion describes the control-only increment.
`packet::recipe_declarations` now separately implements legacy `declare_recipes`
for protocols 763–767. Its [separate audit](legacy-recipe-wire-audit.md) covers
23 vanilla kinds, shared bounded slots and the serializer-ID/766-slot-count
discrepancies. Modern declarations remain explicitly unimplemented. This does
not change the control packet layouts or claim crafting/registry simulation.

## Subsequent modern recipe envelopes

Modern ghost responses/additions now use the separate
[display codecs](recipe-display-wire-audit.md), and modern declarations use
[recipe property/stonecutter codecs](recipe-properties-wire-audit.md). Earlier
exclusion notes describe the historical control-only scope. Legacy and modern
public control representations remain distinct and unchanged.
