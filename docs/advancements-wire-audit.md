# Advancement updates and tab notifications

## Scope and evidence labels

The clientbound play packets `advancements` and `select_advancement_tab`, and
serverbound `advancement_tab`, now have bounded typed codecs, transactional
single-body read/write, and version-specific packet-ID helpers for protocols
763–776. Clientbound dispatch returns `DecodedPacket::Advancement`. Serverbound
notifications remain explicitly separate from that clientbound dispatcher.

**Hash-pinned schema evidence.** All fourteen local protocol inputs match
[`research/schema-hashes.json`](../research/schema-hashes.json). The read-only,
offline [`verify_advancement_schemas.py`](../tools/verify_advancement_schemas.py)
checks complete field names, order, primitives, options, switches and count
prefixes for all 42 packet layouts. It fails on hash/layout drift and downloads
nothing. Upstream pins are:

- PrismarineJS/minecraft-data `f5d7d74604d8c6153fd086bfe035e0630a5207cc`, 763–775
- Complexity-ML/minecraft-data-26.2
  `2a6a5fd9ebb0d964a73312d11beef596b7ec029b`, 776

**Independent implementation evidence.** The immutable MCProtocolLib and
Minestom sources below clarify enums, flag bits, timestamp units, and template
ordering. They are independent implementations, not vanilla release serializers.
Their current layouts do not independently establish every historical boundary.

**Original fixture evidence.** The 84 checked-in rows in
[`tests/fixtures/advancements.tsv`](../tests/fixtures/advancements.tsv) are
hand-specified bodies assembled with Python standard-library primitives in
[`generate_advancement_fixtures.py`](../tools/generate_advancement_fixtures.py).
No Rustwire codec is used to generate their bytes. Packet IDs come from the
hash-checked schemas. Fixtures are neither captures nor runtime-oracle output.

The original advancement fixture audit performed no live-server or official
serializer validation. The later [slot count correction](slot-count-wire-audit.md)
uses read-only static inspection of the official 1.20.6 release to resolve the
stale protocol-766 schema; no game code was executed or redistributed.

## Exact layout boundaries

| Protocols | Definition criteria list | Display text | Display icon | Trailing show flag |
|---|---|---|---|---|
| 763 | Present, possibly empty | JSON string | Classic named-NBT slot | Absent |
| 764 | Absent | JSON string | Classic anonymous-NBT slot | Absent |
| 765 | Absent | Anonymous NBT | Classic anonymous-NBT slot | Absent |
| 766 | Absent | Anonymous NBT | Component slot, VarInt count | Absent |
| 767–769 | Absent | Anonymous NBT | Component slot, VarInt count | Absent |
| 770–774 | Absent | Anonymous NBT | Component slot, VarInt count | Present |
| 775–776 | Absent | Anonymous NBT | ItemStackTemplate | Present |

The update body contains reset, added definitions, removed IDs, and progress.
An added definition has an ID, optional parent and display, release-dependent
criteria list, nested requirements, and telemetry boolean. Progress carries an
advancement ID and criterion names with optional signed big-endian i64 epoch
milliseconds. Absent and present-negative timestamps remain distinct, including
present -1; no internal sentinel replaces the wire presence flag.

A display contains title, description, icon, frame VarInt, fixed-width flags,
optional background texture, and x/y f32 coordinates. Frame IDs are task=0,
challenge=1, goal=2. Flag bits 0/1/2 are background/show-toast/hidden. Unknown flag
bits are retained. The background-presence bit must agree with the optional
background field on encode. Nonfinite coordinates are retained bit-for-bit.

At 775 the template is item ID then count followed by the normal component
patch, even for count zero. A normal slot instead starts with count (or classic
presence). Some 774 and 775 bodies can both parse while swapping the meanings of
two valid VarInts; exact version selection, not heuristic detection, is required.
Encoding rejects the wrong `AdvancementIcon` variant for the selected version.

`select_advancement_tab` is one optional resource identifier. Serverbound
`advancement_tab` action 0 includes an opened/selected tab identifier; action 1
closes the screen without an identifier. There is no third "seen" wire action.
Unknown frame or action IDs are malformed typed values (`Error::Invalid`).

## Independent implementation links

- [MCProtocolLib frame enum](https://github.com/GeyserMC/MCProtocolLib/blob/39fa9e822670d6cadd826e4760835b46a17d092a/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/data/game/advancement/Advancement.java#L51-L60)
- [MCProtocolLib display flags and read order](https://github.com/GeyserMC/MCProtocolLib/blob/39fa9e822670d6cadd826e4760835b46a17d092a/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/ClientboundUpdateAdvancementsPacket.java#L24-L67)
- [MCProtocolLib item-template and patch codecs](https://github.com/GeyserMC/MCProtocolLib/blob/39fa9e822670d6cadd826e4760835b46a17d092a/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/codec/MinecraftTypes.java#L524-L567)
- [Minestom display template codec](https://github.com/Minestom/Minestom/blob/9c20510def19ba151153c9a20aff8417753a0367/src/main/java/net/minestom/server/network/packet/server/play/AdvancementsPacket.java#L131-L164)
- [Minestom optional Long progress codec](https://github.com/Minestom/Minestom/blob/9c20510def19ba151153c9a20aff8417753a0367/src/main/java/net/minestom/server/network/packet/server/play/AdvancementsPacket.java#L204-L209)
- [Minestom epoch-millisecond producer](https://github.com/Minestom/Minestom/blob/9c20510def19ba151153c9a20aff8417753a0367/src/main/java/net/minestom/server/advancements/Advancement.java#L334-L337)
- [Minestom template ordering](https://github.com/Minestom/Minestom/blob/9c20510def19ba151153c9a20aff8417753a0367/src/main/java/net/minestom/server/item/ItemStackTemplate.java#L8-L13)

## Deliberate semantic limits

Collections preserve order and duplicates instead of simulating a client's map
updates. Parent, added, removed, progress, background and tab identifiers use the
shared resource-location syntax validator without normalization or registry
lookup. Criterion and requirement names are ordinary strings and may contain
spaces, uppercase letters or punctuation. No graph validation, cycle detection,
requirement resolution, rewards, client UI, telemetry submission or progress-state
application is attempted.

Text reuses opaque `ChatComponent` JSON/NBT. JSON syntax and text-component
semantics are not interpreted. Required NBT cannot be TAG_End. Item and template
icons reuse exactly the existing inventory component coverage; no unsupported
component payload is guessed or skipped. Unsupported unframed payloads stop
immediately with `Error::Unsupported`, and connection dispatch preserves the
entire raw packet. Since their lengths are unknowable, bytes after such a payload
cannot be checked for truncation or trailing data. Known malformed tags, booleans,
frame/action enums and representation mismatches remain errors.

## Resource and failure guarantees

One packet-level `Budget` covers all added/removed/progress arrays, legacy
criteria names, requirement groups and names, icons and recursive item patches.
All display and icon NBT roots share `max_nbt_nodes`. Existing nested item depth,
NBT depth/collection/string caps and the hard recursion cap still apply. Counts
are checked against remaining minimum wire bytes before vector reservations.
Strings are byte-budget checked, including their prefix, before encoding copies.
Entire packet input and each streamed body are bounded by `max_packet`.

Reads commit the caller's position only after a complete successful body. Writes
encode into a temporary buffer, so failures leave the destination unchanged.
Decode requires complete exhaustion. Strict booleans, invalid UTF-8/NBT,
negative/overflowing/impossible counts, bad identifiers and overflowing VarInts
are rejected rather than used to size unbounded allocations.

## Validation coverage

[`tests/advancements.rs`](../tests/advancements.rs) and
[`tests/advancements_typed.rs`](../tests/advancements_typed.rs) cover all fourteen
families and packet IDs, every original fixture truncation and appended byte,
all state/direction boundaries, all changed layouts, both tab branches, absent
and empty collections, both display-presence branches, all frame values, unknown
flag retention, nonfinite coordinates, present negative timestamps, template
zero counts, aggregate collections/NBT, nested icon depth, string/byte limits,
transactional failures after prefixes, malformed known payloads and full-raw
connection fallback for unknown component IDs. The connection check is an
in-memory synthetic stream, not a live-server interoperability test.
