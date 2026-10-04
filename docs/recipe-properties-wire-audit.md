# Modern recipe property/declaration wire audit

## Scope

`packet::recipe_properties::ModernDeclareRecipes` implements clientbound
`declare_recipes` for protocols 768–776. `DecodedPacket::ModernRecipes` is
separate from `LegacyRecipes`; the old 763–767 serializer list and API remain
unchanged. Modern declarations carry named item property sets and stonecutter
input/display options. They do not contain complete crafting recipe serializers.
The codec does not execute recipes, resolve registry IDs or tags, or update
client/server recipe state.

This completes the remaining **wire declaration envelope** in the recipe slice.
It does not claim full crafting behavior or validation of item registry contents.
Unknown unframed SlotDisplay or nested item-component payloads still return
`Unsupported`, retaining the complete original packet through Connection.

## Exact shape across all nine families

Every pinned 768–776 schema has the same ordered outer body:

1. VarInt property-set count
2. For each set: property identifier string, VarInt item count, item-ID VarInts
3. VarInt stonecutter-option count
4. For each option: IDSet input, then one SlotDisplay result

The IDSet marker is zero for an identifier tag, otherwise N+1 followed by N item
IDs. This preserves explicit-empty sets separately from named tags. The option
has no recipe ID, recipe serializer, boolean, or RecipeDisplay wrapper.
SlotDisplay uses each selected release's established registry/layout, including
recursive composites, trim holders from 770, and templates/new kinds at 775.

The Rust model uses vectors, retaining wire ordering and duplicate property keys,
item IDs, and options. It does not apply map replacement or set deduplication.
This preservation does not establish that a receiving game keeps duplicates.
Item IDs are nonnegative registry identities, without lookup. Property and tag
strings use the existing identifier validator, 32,767-character wire ceiling,
and the caller's string limit. No fixed list of vanilla property names is imposed.

## Evidence levels and provenance

- **Pinned schemas:** `tools/check_recipe_property_fixtures.py` verifies complete
  body order/counts/types, unique clientbound ID mapping, name dispatch, absence
  of an erroneous serverbound mapping, and exact nested IDSet/SlotDisplay plus
  primitive, Slot, ItemStackTemplate, and trim-pattern aliases for all nine
  SHA-256-pinned inputs. Fixture custom-name component IDs/layouts are checked.
  Other nested component coverage is inherited from the separately audited
  inventory codec, not newly inferred from an outer packet name.
- **Independent implementations:** MCProtocolLib `1.21.2-1`, immutable commit
  `667e02d38cd4a79d46640312cd599123609633ac`,
  [ClientboundUpdateRecipesPacket](https://github.com/GeyserMC/MCProtocolLib/blob/667e02d38cd4a79d46640312cd599123609633ac/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/ClientboundUpdateRecipesPacket.java)
  reads a resource-location/item-ID-array map before an ingredient/SlotDisplay
  list. Its pinned MinecraftCodec identifies protocol 768. PacketEvents v2.13.0,
  commit `1063d4e71cff72b9fe022f37b51faad9536815b7`,
  [declaration wrapper](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerDeclareRecipes.java)
  explicitly chooses this format from 1.21.2, delegating to RecipePropertySet
  and SingleInputOptionDisplay. Its version table includes protocol 776.
  Source was inspected, not executed; no adjacent release was silently selected.
- **Static official 26.2 inspection:** The hash-verified release artifact's codec
  composition places the property-key/RecipePropertySet map first, then
  SingleInputSet.noRecipeCodec. RecipePropertySet uses an item-codec list.
  SingleInputSet uses a list of SingleInputEntry.noRecipeCodec, each composing
  ingredient contents with SelectableRecipe.noRecipeCodec; the latter carries
  only SlotDisplay. This confirms the absence of recipe-ID/serializer fields.
  No official serializer or game/server runtime was executed.
- **Original synthetic fixtures:** 345 Python-generated bodies, independent of
  Rustwire's encoder, exercise every known SlotDisplay kind under tag, explicit-ID
  and explicit-empty inputs, multiple property sets, duplicate wire identities,
  boundary IDs, and multiple stonecutter/NBT-bearing results. Rust rejects all
  31,954 strict prefixes and appended trailing bytes, checks exact/undersized
  packet limits and byte-for-byte re-encoding, and exercises typed dispatch.
  These fixtures are neither captured traffic nor release-API output.

The [source audit manifest](validation/recipe-properties-source-audit.json)
records immutable URLs and SHA-256 hashes of independently inspected files.
The [official 26.2 bundler](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)
has SHA-256 `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`;
the nested server JAR is
`183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
No implementation source, game artifacts, or disassembly is vendored.

The existing [display audit](recipe-display-wire-audit.md) describes the explicit
776 `Slot` versus `ItemStackTemplate` schema correction. The declaration checker
continues to verify the actual pinned `Slot` spelling, while nested decoding uses
the independently and statically corroborated template codec. This is an
existing documented correction, not a new declaration-format discrepancy.

## Resource limits, failure behavior and regression tests

One shared inventory Budget spans property entries, property item-ID lists,
stonecutter entries, IDSet IDs, recursive display nodes/arrays, and item/template
patches. All nested text/trim/item NBT roots share `max_nbt_nodes`. No per-option
public SlotDisplay/Slot encode/decode call resets the budget. Shared crate-private
helpers are used; no extra public budget API is introduced.

Counts are nonnegative and checked against both remaining collection budget and
minimum possible input bytes before allocating. Large recursive records grow
only after successful decoding. Strings and output counts/IDs are preflighted
against the remaining packet budget. Nested item/NBT codecs retain their existing
allocation preflights. Encoding returns no partial body on failure.

Recursion limits apply to each recursive path, hard-capped at 64 display levels;
the enclosing display depth is deducted from nested item/NBT allowances.
Independent entries do not cumulatively consume a depth counter, but they do
share their aggregate collection and NBT-node budgets. Tests use two depth-64
chains to distinguish these rules, reject depth 65 even with permissive caller
limits, and reject aggregate-node exhaustion on a later entry.

Known malformed input, invalid identifiers/negative IDs, impossible/overflow
counts, truncation and trailing bytes return errors. Unknown unframed display or
component kinds produce whole-raw fallback without falsely validating unread
suffixes. Tests verify all fixture packets through Connection and reconcile the
previous tests that intentionally left modern declarations opaque.

The six Python verifier regression tests construct their own synthetic schemas
and use committed fixture IDs. They run without `research/protocols` and without
network access. The actual hash-pinned schema audit is a separate command.
MSRV remains the repository's Rust-1.88 CI gate; live recipe acceptance and
executed-release interoperability are not established by these tests.
