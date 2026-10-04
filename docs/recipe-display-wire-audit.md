# Modern recipe display wire audit

## Scope and evidence levels

`packet::recipe_display` implements SlotDisplay, RecipeDisplay, clientbound
`recipe_book_add`, and modern `craft_recipe_response` for exactly protocols
768–776. Existing legacy recipe controls remain in `packet::recipe`; their
public types and legacy response are preserved. Modern typed dispatch uses
`DecodedPacket::RecipeDisplay` with a boxed `RecipeDisplayPacket`.

These models describe wire payloads only. They do not register or execute
recipes, resolve item/tag/component/display IDs, validate crafting grids, or
apply book state. Modern `declare_recipes` is implemented separately by
[`packet::recipe_properties`](recipe-properties-wire-audit.md).

Evidence is deliberately separated:

1. `tools/check_recipe_display_fixtures.py` checks SHA-256 for all fourteen
   existing schema inputs, complete display-union shapes and discriminator
   maps for the nine modern families, ordered packet bodies, direction,
   presence/absence, unique packet IDs, packet-name dispatch, and reachable
   scalar/holder/slot/template/trim-pattern aliases. It checks 23 present
   outer bodies (including five legacy ghost-response envelopes), 18 display
   unions, and 18 complete modern packet/family combinations. Fixture-specific
   custom-name component ID/layout is checked; arbitrary nested component
   layouts retain the existing inventory codec's separate coverage boundary.
2. Pinned, independently authored PacketEvents and Minestom sources were read,
   not executed. Their relevant confirmations and disagreement are recorded
   below. Source text is not included in this repository.
3. Static inspection of the hash-verified official 26.2 release artifact
   confirms three narrow codec-composition facts. No game/server runtime or
   official packet serializer was executed. This is **not release-API output**.
4. An original Python encoder emits 256 synthetic fixtures: all known display
   variants, all five recipe kinds, every modern family, additions, ghost
   responses, signed groups, unknown framed categories, and reserved flag bits.
   These are neither copied upstream fixtures nor live traffic. Rust checks
   all 9,893 strict prefixes, trailing bytes, exact/undersized packet budgets and
   byte-for-byte re-encoding. Self-roundtrips are not the primary fixture oracle.

The immutable URLs and inspected-file SHA-256 values are in
[the source manifest](validation/recipe-display-source-audit.json). The schema
pins remain in `research/schema-hashes.json`; the ignored schema cache is only
needed for the separate pinned audit, never for `tools/test_*.py` unit tests.

## Exact layouts

All modern ghost responses contain a signed window VarInt followed by one
RecipeDisplay. Additions are a VarInt-counted entry list followed by a strict
replace boolean. Each entry is ordered as display-ID VarInt, RecipeDisplay,
shifted optional group VarInt, category VarInt, boolean-prefixed optional
VarInt-counted ingredient list, then one flags byte. Every ingredient is an
item holder set: marker zero plus tag identifier, or marker N+1 plus N item IDs.
Empty explicit sets and absent/present-empty requirement lists remain distinct.

Recipe display kinds have unchanged IDs for 768–776:

- 0: shapeless, ingredient SlotDisplay list, result, station
- 1: shaped, width VarInt, height VarInt, ingredient list, result, station
- 2: furnace, ingredient, fuel, result, station, duration VarInt, experience f32
- 3: stonecutter, ingredient, result, station
- 4: smithing, template, base, addition, result, station

The codec preserves width, height, duration and floating-point bit patterns;
it does not infer recipe usability or enforce a grid product. Lists preserve
order, duplicates, and empty lists.

Slot-display registries are explicitly versioned:

| Protocol | Discriminators in numeric order |
| --- | --- |
| 768–774 | empty, any fuel, item ID, stack, tag, smithing trim, with remainder, composite |
| 775–776 | empty, any fuel, with any potion, only with component, item ID, template, tag, dyed, smithing trim, with remainder, composite |

`OnlyWithComponent` carries one registry-ID scalar, without a component payload;
unknown nonnegative IDs are safe to retain. The 775 schema's `dyed_slot_demo`
and 776 `dyed` labels describe the same two-display wire body. The 775
`with_any_potion.base` versus 776 `.display` labels likewise do not alter bytes.

`DisplayTrimPattern::Display` is the recursive third display at 768–769.
`DisplayTrimPattern::Holder` is the shifted trim-pattern holder at 770–776.
An inline holder contains asset identifier, anonymous description NBT, and decal
boolean; there is no template-item ID at 770+. Numeric holder IDs are unshifted
in the Rust model, with zero on the wire reserved for inline data.

`SlotDisplay::ItemStack` uses the existing count-before-ID Slot format only at
768–774. `SlotDisplay::ItemTemplate` uses ID-before-count plus component patch
at **both 775 and 776**. A zero template count still carries ID and patch and
is distinct from the earlier empty Slot sentinel. Wrong-family representations
are rejected, including old recursive trim patterns, new slot kinds, and old
stack versus new template variants.

## The protocol-776 schema correction

The pinned 776 schema labels `SlotDisplay.item_stack` as `Slot`, whereas the
775 schema labels it `ItemStackTemplate`. The verifier deliberately checks the
actual pinned `Slot` spelling at 776; it does not rewrite the schema to conceal
the disagreement. The implementation follows the following exact 776 evidence:

- PacketEvents v2.13.0 commit `1063d4e71cff72b9fe022f37b51faad9536815b7`
  explicitly lists `V_26_2(776)`. Its
  [ItemStackSlotDisplay](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/recipe/display/slot/ItemStackSlotDisplay.java)
  invokes the template codec. The pinned
  [ItemStackSerialization](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/item/ItemStackSerialization.java)
  uses ID, count, component patch for 26.1 and later, with no 26.2 exception.
- Minestom tag `2026.09.12-26.2`, commit
  `aba93bdb5096179bd66dc35c9849d9f0bacdc17e`, explicitly selects
  `ItemStackTemplate.NETWORK_TYPE` for
  [SlotDisplay.ItemStack](https://github.com/Minestom/Minestom/blob/aba93bdb5096179bd66dc35c9849d9f0bacdc17e/src/main/java/net/minestom/server/recipe/display/SlotDisplay.java).
  This is narrowly supporting evidence: its smithing-trim model is still
  recursive and disagrees with the verified 770+ holder layout, so no blanket
  Minestom conformance is inferred.
- The [official 26.2 server bundler](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar) SHA-256 is
  `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`;
  its nested server JAR SHA-256 is
  `183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
  Static class metadata shows `SlotDisplay.ItemStackSlotDisplay` stores an
  `ItemStackTemplate` and uses that type's stream codec. The latter composes
  item, VarInt count, and component patch in that order. Static metadata also
  confirms smithing trim uses two SlotDisplay codecs followed by
  `TrimPattern.STREAM_CODEC`.

No neighboring-version fallback is used to decide this discrepancy. The 776
fixture explicitly distinguishes ID 300 from count 2; zero count is also tested.

## Scalar identities and safe unknowns

Group marker zero means absent. Otherwise Java-style wrapping subtraction by
one is preserved, including negative markers and signed boundaries. Encoding
`Some(-1)` is rejected because its shifted marker would collide with absence.
The pinned PacketEvents `PacketWrapper.readNullableVarInt` and
`writeNullableVarInt` independently establish this operation; the recipe entry
calls those methods. Group IDs are never resolved or allocated here.

Categories 0–12 follow the pinned schema mapping; unknown category scalars are
retained in `RecipeBookCategory::Unknown`. All eight flag bits are retained;
notification is bit 0 and highlight bit 1. Display/window IDs remain signed
VarInts. Item, component-reference and holder IDs are nonnegative wire registry
identities, without registry lookup. Identifier validation and string limits
are shared with the rest of Rustwire.

Unknown recipe/slot-display discriminators and unknown unframed item-component
payloads return `Error::Unsupported`. `Connection::next_typed_event` preserves
the full RawPacket, including unread suffixes; it does not pretend that such
suffixes were validated. Known malformed/truncated/oversized bodies return errors,
not raw fallback. Unknown framed categories and component-reference scalars do
not need an opaque fallback because their boundaries are known.

## Bounds and validation

One inventory Budget spans the whole packet: entry/list elements, display nodes,
item/template/component collections and ingredient IDs share `max_collection`;
all description/item text NBT roots share `max_nbt_nodes`. Recursive slot
displays have `max_nbt_depth`, hard-capped at 64, and the display depth is deducted
from nested item/template and NBT allowances. Array counts are checked against
remaining input before vector allocation; recursive/large entries grow only as
their bodies decode rather than bulk-reserving an attacker-declared capacity.
Encode paths preflight
strings/count minima against the remaining packet budget, return no partial
body, and use existing bounded slot/template codecs. The subsequent shared NBT
writer hardening preflights modified-UTF-8 strings and primitive arrays before
temporary output growth. Its separate regressions cover both atomic public
writes and early rejection before the variable payload is appended.

Tests additionally cover invalid booleans, negative/overflow counts and IDs,
unknown nested components, impossible counts, aggregation across multiple
entries and different leaf kinds, hard-depth limits even with permissive caller
limits, wrong-version representations, state-scoped typed dispatch, and complete
raw retention. The new verifier's synthetic tests mutate hashes, aliases,
discriminators, nested fields, packet presence/IDs and dispatch without using
`research/protocols` or any network access.

The evidence does not establish vanilla recipe acceptance, rendering behavior,
registry resolution, gameplay interoperability, or modern declaration support.
MSRV validation belongs to the repository's stated Rust-1.88 CI gate.
