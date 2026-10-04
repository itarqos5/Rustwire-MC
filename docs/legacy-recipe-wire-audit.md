# Legacy recipe declaration wire audit

## Scope and evidence

`packet::recipe_declarations::LegacyDeclareRecipes` encodes and decodes the
clientbound Play `declare_recipes` packet in **protocols 763–767 only**
(1.20–1.21.1). It includes all **23 independently audited vanilla serializer
kinds**, with ingredient alternatives, result item stacks, classic NBT and
component patches. It does not register recipes, resolve item IDs, execute
crafting, simulate a menu, or maintain a recipe book. Modern declaration packets
at **768+ use a separate [modern codec](recipe-properties-wire-audit.md)**;
this legacy type rejects those versions rather than inferring modern framing.

Evidence is deliberately separated:

1. `tools/check_legacy_recipe_fixtures.py` hashes all five complete pinned schema
   files against `research/schema-hashes.json`, checks each complete declaration
   layout, its serializer mapping and switches, direct ingredient/slot/cooking/
   simple-recipe aliases, primitive aliases, packet direction/ID/dispatch, and
   the component aliases used by the original fixtures. Existing item-component
   generation and coverage checks separately audit their complete component
   registries. Two schema discrepancies are retained and checked explicitly,
   rather than silently editing downloaded schemas.
2. Independently authored MCProtocolLib, PacketEvents and ViaVersion sources
   were read at immutable release/version-specific commits. The
   [source manifest](validation/legacy-recipe-source-audit.json) records each
   inspected file's URL and SHA-256. Source text is not vendored. This is source
   inspection, not execution of those libraries or of a vanilla release API.
3. An original Python encoder, independent of Rustwire's encoder, emits **144
   fixture rows**: **131 supported bodies** and **13 unknown-serializer cases**.
   Rust tests reject all **14,337 strict truncation prefixes** of the supported
   bodies, reject trailing bytes, and independently inspect decoded values.
   These bytes are synthetic, not network captures or official API outputs.

No new dependency, game binary, account, private server, or gameplay session is
needed. Minimum-Rust-version execution remains a separate CI gate.

## Versioned envelope and payloads

All packets begin with a nonnegative VarInt recipe count.

| Protocol | Entry header | Shaped prefix | Item representation |
| --- | --- | --- | --- |
| 763 | serializer identifier, recipe identifier | width, height, group, crafting category | present bool, item ID VarInt, positive signed-byte count, named NBT |
| 764 | serializer identifier, recipe identifier | width, height, group, crafting category | same classic slot, anonymous NBT |
| 765 | serializer identifier, recipe identifier | group, crafting category, width, height | classic slot, anonymous NBT |
| 766–767 | recipe identifier, serializer VarInt ID | group, crafting category, width, height | **VarInt count**, item ID, added/removed component counts, component payloads/removals |

The shared inventory codec now uses VarInt counts from **766**, correcting an
existing schema-derived boundary. Empty slots are a false present flag for
763–765, or count zero from 766. Recipe ingredients are always count-prefixed
ordered lists of slots for the implemented families; ingredient lists may be
empty and alternatives may include empty slots. Order and duplicates survive.
Result slots use the same version-aware slot codec and shared limits.

- Shaped: versioned prefix above; exactly width × height ingredients in flat
  wire order; result slot; strict show-notification boolean. Width and height
  are nonnegative, independently bounded counts, with checked multiplication.
  Zero-area grids are preserved. No 3×3-menu or recipe-validity rule is inferred.
- Shapeless: group, crafting category, ingredient count/list, result slot
- Special and decorated pot: crafting category only
- Smelting/blasting/smoking/campfire: group, cooking category, one ingredient,
  result slot, f32 experience, signed VarInt cook time
- Stonecutting: group, one ingredient, result slot
- Smithing transform: template, base, addition ingredients, result slot
- Smithing trim: template, base, addition ingredients, with **no result slot**

Crafting categories are building=0, redstone=1, equipment=2, misc=3. Cooking
categories are food=0, blocks=1, misc=2. Unknown category ordinals are malformed
known layouts, not opaque fallback. Experience and cook time are wire scalars;
no gameplay clamp is imposed. Recipe keys and named serializers use the shared
resource-location validator; unqualified/empty-namespace spellings are preserved
exactly. Serializer representations cannot be silently carried across the
name-to-ID boundary, and a serializer/data-shape mismatch is an encoding error.

## Corrected serializer registry

The audited 766/767 IDs are:

| IDs | Kinds |
| --- | --- |
| 0–1 | crafting_shaped, crafting_shapeless |
| 2–5 | crafting_special_armordye, bookcloning, mapcloning, mapextending |
| 6–8 | crafting_special_firework_rocket, firework_star, firework_star_fade |
| 9–14 | crafting_special_tippedarrow, bannerduplicate, shielddecoration, shulkerboxcoloring, suspiciousstew, repairitem |
| 15–18 | smelting, blasting, smoking, campfire_cooking |
| 19–22 | stonecutting, smithing_transform, smithing_trim, crafting_decorated_pot |

For compactness the `crafting_special_` prefix is shown once per table row's
first entry where applicable. The Rust kind names and complete fixture registry
spell out every identifier. Legacy named serializers use these same full names
in the `minecraft` namespace.

The pinned schemas include obsolete `minecraft:crafting_special_banneraddpattern`
at numeric ID 11, shifting all later IDs by one and putting decorated pot at
23. That disagrees with the independent
[MCProtocolLib 1.20.6 registry](https://github.com/GeyserMC/MCProtocolLib/blob/8dbdffc0004746122efd1fbe63d1c1620a8336db/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/data/game/recipe/RecipeType.java)
and [1.21 registry](https://github.com/GeyserMC/MCProtocolLib/blob/e71d7df8c127b7196fd81dd392034d7d97acbb4e/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/data/game/recipe/RecipeType.java),
along with the pinned PacketEvents legacy serializer registry in the manifest.
Rustwire uses the independently corroborated 23-kind registry, not the shifted
schema IDs. Obsolete banner-add-pattern names and unknown IDs remain
`Unsupported`. There is no guessed serializer body or partial skip. The
supported pot name is `crafting_decorated_pot`, not
`crafting_special_decoratedpot`; the old plain `smithing` kind is also excluded.

The 765 ordering boundary is independently corroborated by
[MCProtocolLib's protocol-765 packet](https://github.com/GeyserMC/MCProtocolLib/blob/723fbb3d54fd4e6ec01633dd346d5f677c93be6e/src/main/java/com/github/steveice10/mc/protocol/packet/ingame/clientbound/ClientboundUpdateRecipesPacket.java),
whose matching `MinecraftCodec` declares protocol 765 / 1.20.4, and by
PacketEvents' explicit 1.20.3 branch. The manifest records both.

## Official protocol-766 static cross-check

Read-only inspection of the official 1.20.6 archive and mappings independently
confirms the 23 serializer registration names/order above, including shield
decoration at 11, smelting at 15 and decorated pot at 22. There is no obsolete
banner-add-pattern entry. The [slot-count audit](slot-count-wire-audit.md)
records the exact release URLs, archive/mapping hashes and inspection method.
The official mappings identify RecipeSerializer as `czb`; inspect its static
initializer with `javap -c -p -classpath server-1.20.6.jar czb` to check the
registration order. This static cross-check applies to protocol 766 only; it is
not execution of the game's serializer or new live-server evidence.

## Slot-count and category discrepancies

The pinned 766 schema says `Slot.itemCount` is i8, and the inspected
MCProtocolLib 1.20.6 and 1.21 helpers both use a byte. Independent release-era
implementations contradict that boundary:

- [ViaVersion 4.10.2 ItemType1_20_5](https://github.com/ViaVersion/ViaVersion/blob/113bf1e2751935ae2ddbc72a9f67c5df00e958cd/api/src/main/java/com/viaversion/viaversion/api/type/types/item/ItemType1_20_5.java)
  reads/writes the amount with `Type.VAR_INT`
- ViaVersion 5.0.0 and PacketEvents v2.4.0/v2.5.0 independently retain the
  VarInt amount for the 1.20.5 item format; exact source pins are in the manifest

The official protocol-766 ItemStack stream bindings independently confirm the
VarInt width; see the [primary static audit](slot-count-wire-audit.md).
Rustwire therefore uses VarInt counts at both 766 and 767. Existing 766
count-one fixtures cannot distinguish the two formats. New independent
count-128/count-300 inventory checks and count-300 recipe outputs do. This
correction is shared by every packet using the inventory slot helper. The
schema checker still checks the pinned i8 discrepancy; it does not pretend the
schema itself supplies the corrected wire count.

MCProtocolLib's inspected cooked-recipe parser reuses its crafting category
enum. PacketEvents explicitly distinguishes
[CookingCategory](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/recipe/CookingCategory.java)
from crafting categories. Rustwire follows the separate 0–2 cooking domain.

## Bounds and raw/error behavior

A single shared inventory budget spans recipes, ingredient containers,
slot-alternative list entries, every ingredient/result/nested slot and component
patch/list entries. NBT roots across the entire declaration packet share that
budget object's `max_nbt_nodes`; nested item/component recursion shares the
`max_nbt_depth` limit with a hard cap of 64. Limits are not reset per recipe,
ingredient or result. Array entries and slots both consume conservative aggregate
collection units, matching the existing inventory packet codecs.

Every count is checked against limits and remaining-input minimum size before
vector reservation. Shaped-grid multiplication is checked before allocation.
String writes preflight their full prefix and byte length against remaining
packet space. Decode and encode enforce whole-packet byte limits; failed encode
returns no partial output. Legacy NBT naming and modern component layout checks
are delegated to the existing bounded slot codec.

Known malformed values, including negative counts, negative serializer IDs,
invalid booleans, category ordinals, UTF-8, identifiers, dimensions or slot fields,
VarInt overflow, truncated/trailing data and resource limits, remain errors.
An unknown unframed serializer or component returns `Unsupported` immediately;
its payload boundary cannot be determined safely. `Connection::next_typed_event`
then retains the **entire original RawPacket**, including previously parsed
recipes and all remaining bytes. Such unknown bodies cannot have a meaningful
prefix/trailing-data completeness claim. No opaque tail is presented as a
partially typed recipe.

Clientbound typed dispatch is Play-only. Serverbound recipe controls are not
routed into this type. Protocol 768+ `declare_recipes` returns no typed decoder
and stays raw without claimed validation of its arbitrary body.

## Reproduction

```sh
python3 tools/check_legacy_recipe_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_legacy_recipe_fixtures.py'
cargo test --locked --offline --no-default-features --test legacy_recipes --test legacy_recipes_typed --test inventory
cargo test --locked --offline --all-features --test legacy_recipes --test legacy_recipes_typed --test inventory
cargo clippy --locked --offline --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --all-features --no-deps
cargo fmt --all -- --check
```

The real schema audit uses the existing pinned schema cache. The eight Python
regression tests instead create synthetic schema inputs and use committed
fixtures; they require no downloaded schema or network. Mutations test hash,
full-layout, kind mapping, direct aliases, component dispatch and fixture aliases,
packet direction, unique IDs, dispatch and version boundaries. Synthetic checks
test verifier behavior; they are not substitutes for the separate pinned audit.

Final staging verification: the complete no-default suite passed 445 tests,
the default-feature suite passed 449, and the all-feature suite passed
484. All commands used `--locked --offline`. Strict all-target/all-feature
Clippy, strict all-feature rustdoc, formatting, the five-schema legacy audit,
the existing item-component generation/coverage checks and all 41 Python
regressions passed. The Python suite also passed from an isolated copy containing
only tools and committed fixtures, without `research/protocols` or network.
The twelve focused legacy tests include the unknown-name and hard-depth-ceiling
regressions. MSRV remains CI; no live game/server or official release-API result
is claimed by these staging checks.
