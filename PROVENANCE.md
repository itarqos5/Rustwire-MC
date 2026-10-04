# Protocol provenance

Verified/retrieved 2026-10-02. Minecraft Java releases only, not Bedrock, snapshots,
pre-releases, or modded protocol extensions. The checked-in catalog is a packet
ID/name index, not a claim that every listed packet has a typed implementation.

- Release chronology: [Mojang launcher version manifest](https://piston-meta.mojang.com/mc/game/version_manifest_v2.json)
- Protocol numbers: [PrismarineJS minecraft-data protocolVersions.json](https://github.com/PrismarineJS/minecraft-data/blob/f5d7d74604d8c6153fd086bfe035e0630a5207cc/data/pc/common/protocolVersions.json)
- Packet IDs/names through 26.1: [PrismarineJS minecraft-data](https://github.com/PrismarineJS/minecraft-data/tree/f5d7d74604d8c6153fd086bfe035e0630a5207cc), commit `f5d7d74604d8c6153fd086bfe035e0630a5207cc`, MIT per its README
- 26.2 schema: [Complexity-ML minecraft-data-26.2](https://github.com/Complexity-ML/minecraft-data-26.2/tree/2a6a5fd9ebb0d964a73312d11beef596b7ec029b), `complexity-26.2.5` / commit `2a6a5fd9ebb0d964a73312d11beef596b7ec029b`. This is an independent implementation source, not Mojang documentation. Upstream PrismarineJS had the 776 mapping but no 26.2 schema in dataPaths when retrieved.
- Official release: [Minecraft Java 26.2](https://www.minecraft.net/en-us/article/minecraft-java-edition-26-2)

`research/schema-hashes.json` records exact input SHA-256 values and release
aliases. `tools/fetch_schemas.py` downloads pinned schemas; then
`python3 tools/generate_catalog.py` reproduces `src/catalog/`. No download,
Java installation, decompilation, or code generator runs during Cargo builds.

Some upstream data was extracted by its maintainers from wiki.vg and
minecraft.gamepedia.com; upstream notes its historical source-license caveat
in its README. Rustwire includes only protocol facts (IDs/names), with
attribution, and independently written Rust codecs, not Mojang implementation
source or game assets.

## Format and authentication references

- [PrismarineJS chunk implementation](https://github.com/PrismarineJS/prismarine-chunk/tree/master/src/pc): palette array length removal in 1.21.5 and fluid counts in 26.1
- [Microsoft device authorization documentation](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code)
- [Microsoft application registration](https://learn.microsoft.com/en-us/entra/identity-platform/quickstart-register-app)
- [PrismarineJS auth implementation](https://github.com/PrismarineJS/prismarine-auth): Xbox/XSTS/Minecraft service interoperability reference

HTTP authentication helpers require an application-owned, service-authorized
client ID and an eligible account. No launcher client ID is borrowed. OAuth and
Minecraft Services may reject an unapproved application even when the flow's
wire format is correct. Live account authentication is not covered by offline
tests.

## Additional typed-codec references and corrections

Entity metadata and item-component registries are generated from the same hash-pinned schemas by `generate_entity_metadata.py` and `generate_item_components.py`. `report_coverage.py` reproduces the exact supported/unsupported name lists in `docs/typed-coverage.json`.

- [PrismarineJS lpVec3 implementation](https://github.com/PrismarineJS/node-minecraft-protocol/blob/master/src/datatypes/lpVec3.js): compact mixed-endian velocity/vector wire representation
- [MCProtocolLib section block updates](https://github.com/GeyserMC/MCProtocolLib/blob/master/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/level/ClientboundSectionBlocksUpdatePacket.java): VarLong section records, correcting an upstream VarInt alias
- Cached checksum-verified Paper 1.20.1 serializers confirmed respawn keep-data flags and optional global-position fields; Paper 1.21.5 build 114 confirmed the obsolete palette-length allocation overhead. Only protocol facts and original Rust implementations are included here; no proprietary bytecode/source is distributed

Where an upstream schema differs from inspected wire behavior, the specific correction is documented and tested rather than silently treating the schema as infallible. The exact real-server artifacts are recorded in `docs/validation/matrix-download-provenance.json`.

## Modern inventory component hashes

`packet::item_hash` implements the structured CRC32C HashOps representation,
not a CRC of network bytes. `tools/paper/HashOracle.java` is an original fixture
harness that calls the cached official server's public HashOps API. It was run
against pinned Paper 1.21.5 build 114 and 26.2 build 129; both produced identical
primitive/array/list/map fixtures. `tools/paper/ComponentHashOracle.java` also calls those builds' actual
DataComponents codecs verified damage, unbreakable, rarity, block-state,
custom-model-data, tooltip defaults and enchantable fixtures. NBT numeric types
are significant: JSON conversion can narrow an integer to a byte, while SNBT
`7` and a network TAG_Int retain the integer representation.

Primary interoperability references consulted:
- [GeyserMC HashOps encoder](https://github.com/GeyserMC/Geyser/blob/63a4e2b79b12f0d138777d5fd80176a112b4bd72/core/src/main/java/org/geysermc/geyser/item/hashing/MinecraftHashEncoder.java)
- [GeyserMC persistent component-codec adapters](https://github.com/GeyserMC/Geyser/blob/63a4e2b79b12f0d138777d5fd80176a112b4bd72/core/src/main/java/org/geysermc/geyser/item/hashing/DataComponentHashers.java)

No proprietary class implementation or decompiled source is included. Run the
harness only against an independently downloaded and accepted server install:
`javac -cp "$SERVER_AND_LIBRARIES" -d /tmp tools/paper/HashOracle.java`, then
`java -cp "/tmp:$SERVER_AND_LIBRARIES" HashOracle`. The classpath must contain the
Paper version JAR and its runtime library JARs. No server or network is started
by this primitive hash harness.

## Particle and holder metadata corrections

The metadata generator corrects verified schema discrepancies using release-server
stream codecs: protocol 765 embeds the wrong particle-ID mapper; vibration entity
eye height is f32; RGB dust-transition scale moves last at 766; dust colors become
packed integers at 768 and remain packed at 776; trail duration starts at 769 and
still exists at 776. Protocol 775 replaces particle Slots with item templates.
Painting/wolf references in 766 are direct IDs; inline holders begin at 767.
Painting dimensions are VarInts and optional title/author fields start at 768.
`tests/entity_metadata_particles.rs` and `tests/entity_metadata_holders.rs` include
independent fixtures, every-prefix truncation, malformed options, unknown-ID
fallback, and shared collection/NBT/depth limits. All known outer metadata
serializer kinds are implemented; nested item components remain a stated subset.

## Ordinary item-component corrections

The generated component classifications are paired with handwritten version-aware
codecs. Official release codecs confirmed that food in 766 has no converter while
767 has a boolean-prefixed nonempty stack; both legacy versions carry full effect
instances. Potion custom names begin at 768. Written-book filtered pages use a
boolean followed by required component NBT. Bee occupants add their entity type
ID at 773. Writable-book page/string limits and the 256-explosion limit are
enforced. `tests/item_components.rs` adds independent golden/truncation fixtures,
version-boundary rejection and aggregate effect/NBT/collection budget tests.

Nested item stream layouts were checked separately: use-remainder, charged
projectiles and bundles require nonempty Slots through 774; container entries
are optional. All four switch to complete ItemStackTemplates at 775 and remain
templates at 776 despite stale schema aliases. Sulfur-cube content also carries
a template at 776. Zero-count templates retain all fields. Charged-projectile
template caps are 64 at 775 and 1024 at 776; bundle/container caps are 256. The
shared template codecs enforce aggregate budgets across component and particle
nesting. Six additional regression tests cover these verified corrections.

The 1.20.6 and 1.21.1 ClientboundContainerSetSlotPacket constructors still
read a signed byte for the container ID, verified independently from both
server artifacts. The misleading unsigned schema alias was corrected so
reserved IDs -1 and -2 survive through protocol 767. Regression fixtures cover
all five pre-768 families and the later separate-cursor boundary.

## Protocol 776 serverbound catalog correction

The pinned 26.2 schema misorders `spectator_action` after swing and omits the
separate UUID-target teleport-to-entity packet (`spectate`). Official client
and Paper 26.2 `GameProtocols` independently register 69 serverbound packets:
spectator action 0x3e, swing 0x3f, teleport-to-entity 0x40, test-instance action
0x41, use-item-on 0x42, use-item 0x43, custom-click action 0x44. The generator
applies an explicit guarded correction while retaining the immutable input hash.
A full-tail independent fixture also checks actual use-item/swing encoded frames.

The official 26.2 client SHA-1 is
`2dc72797acbc1b63fc16a11c4ac393605f453754`; its verified official version-metadata
SHA-1 is `c7868781b30aaf24be0dac894c94a34e5d6df10d`. Cached Paper 26.2 build 129
agrees with the client registration order. No game implementation is distributed.

## Headless client ordering and session hashes

The bounded Grim probe's action order was checked with the official 1.21.5
client and mappings. Verified client SHA-1:
`b88808bbb3da8d9f453694b5d8f74a3396f1a533`; mappings SHA-1:
`57669731d542f98646772e91a0d68628f9827a5c`. Actual client tick ordering places
input/UI item actions before player movement; unchanged selected slots are not
resent. Official 26.2 client inspection confirms teleport acknowledgement then
position/rotation response. A later transaction Pong must not overtake that
application handling. Original Rust/TCP regressions cover these facts; no
proprietary class, disassembly or implementation source is distributed.

The official server's Crypt.digestData uses ISO-8859-1 for the server ID.
Java-generated SHA-1/signed-BigInteger fixtures cover Latin-1 and replacement of
unrepresentable Unicode characters. Secret and public-key bytes remain verbatim.
These offline interoperability fixtures are not evidence of account authentication.

## Chat acknowledgement and signature-cache state

The original `tools/paper/ChatStateOracle.java` harness invokes the prepared
release APIs for LastSeenMessagesTracker and MessageSignatureCache. All fourteen
representative server artifacts agree on the twenty-entry ring, consecutive
duplicate suppression, ignored-message advancement, pending-only deletion and
128-entry packed-signature ordering. Protocol 770 adds the nonzero one-byte
checksum of the ordered Java signed-byte hashes. The oracle exercises both
legacy and modern API signatures without copying the underlying implementation.

[Oracle outputs and artifact hashes](docs/validation/chat-state-oracle.json)
record actual API execution, not a live signed-chat exchange. Compile the harness
with `javac -d /tmp tools/paper/ChatStateOracle.java` and run with `/tmp`, the
prepared release JAR and its libraries on the classpath. No game server, account
login or network connection is started. Java 25 emits incidental Jansi warnings
for the two oldest artifacts; their fixture outputs and exit codes still pass.
Eight original Rust tests cover these independent outputs, bitset wire bytes,
checksum zero normalization, ignored/duplicate/wrapped histories, cache capacity,
invalid-reference atomicity and offset overflow. None of these helpers verifies
RSA signatures or certifies a remote identity.

## Player roster, equipment, attributes and effects

All fourteen cached release serializers were checked alongside pinned schemas.
Official player-list actions assign list order bit 6 from 768 and hat bit 7 from
769; the pinned schemas swap those labels in newer releases. Profile property
count/name/signature limits become 16/64/1024 at 766; username, public-key and
key-signature bounds remain 16/512/4096. Equipment adds BODY at 766 and SADDLE
at 770. Attributes use registry IDs from 766 and modifier resource identifiers
from 767. Attribute operations are VarInts from 766 despite the schema's i8 label.
Effects switch from byte amplifiers and optional factor NBT to VarInt amplifiers
and the blend flag at 766. Resource identifiers retain original text while
duplicate checks normalize the default namespace; the namespace `..` is rejected
from 775. These are verified protocol facts, not copied implementations.

The original golden tests target ClientboundPlayerInfoUpdate/RemovePacket,
EquipmentSlot, attribute/effect packets, profile/session codecs and the release
ResourceLocation/Identifier rules. Public keys and signatures remain explicitly
unverified envelopes, and registry references remain unresolved numeric IDs.

## Remaining item-component stream layouts

The new inventory submodules check the actual release stream codecs instead of
assuming every historical schema alias is correct. Bundle-use animation starts
at 769 and spear-use animation at 774. Equippable gains interaction at 770 and
shearing fields at 771. Shield bypass changes from an optional tag string to an
optional holder set at 775. Profiles become discriminated complete/partial
profiles with a skin patch at 773. Trim model-index floats remain through 768,
numeric armor keys through 767, and ingredient/template IDs disappear at 770.
Instruments use ticks without descriptions through 767, then seconds with a
description. Instrument, provided-trim-material and jukebox key wrappers disappear
at 775. Chicken, zombie-nautilus and damage-type component references use older
boolean ID/key wrappers before their plain registry-ID representations at 775.

Adventure-mode block predicates have boolean-prefixed compound NBT. Protocol 770
adds typed exact component matchers and registry-aware NBT partial matchers; 774
adds the concrete/any-value selector, and 775 adds VillagerVariant. Partial
matcher NBT is the actual official network representation, not an opaque fallback.
The original codecs preserve it and enforce shared recursion/node/byte budgets;
game-specific predicate evaluation remains an application responsibility.

`tests/components_extended.rs`, `components_holders.rs` and
`components_predicates.rs` contain 32 independent tests. The code comments name
the corresponding release classes and corrected boundaries. No game class,
disassembly or proprietary implementation is distributed.

## Command registries and graphs

The parser registry and property serializers were checked against all fourteen
cached release artifacts. The official parser name is `minecraft:nbt_compound_tag`,
not the schemas' `minecraft:nbt`; protocol 776 uses `minecraft:team_color` at
parser index 16. Entity-parser flag bit 0 means a single target and bit 1 means
players only. Restricted-node flag 0x20 begins at protocol 771. Numeric bound
values are retained exactly, including unusual floating-point values transmitted
by the official serializers. Graph validation independently checks child and
redirect dependencies, retaining valid mixed-edge cycles such as `execute run`.

Suggestion requests follow vanilla's 32,500 UTF-16-unit wire limit. The cached
Paper 26.2 receiver imposes a separately patched 2,048-unit policy; applications
should respect their server's stricter limits. Provider identifiers are data,
never executed or fetched by Rustwire. The original tests cover every known
parser/version ordinal and independent malformed/limit fixtures.

## World-particle, explosion and sound boundaries

The six new clientbound codecs follow cached official release serializers.
Explosions use the legacy layout in 763–764; 765 adds interaction, two particles
and an untagged inline sound, with a sound holder replacing that direct sound in
766–767. Optional player knockback uses f64 from 768, correcting a stale f32
schema annotation. Radius, fixed-i32 block count and weighted block particles
begin at 773 and persist through 776. The outer explosion layout does not change
at 775/776, although its nested particle payloads do. World-particle IDs move
from the prefix to the suffix at 766, the always-show flag starts at 769, and
UI sound category 10 starts at 771. Fixed-point sound coordinates remain exact
integers rather than round-tripping through floating-point conversions.

Shared particle and SoundEvent helpers were factored without changing existing
metadata/item behavior. Independent tests cover all six packet families and
release boundaries, plus nested item budgets and weighted-total overflow. The
26.1.2/26.2 explosion codecs were also checked against the official artifacts
directly. No rendering, world simulation or cryptographic trust is implied.

## Canonical chat-signing input

`tools/paper/ChatSigningOracle.java` is an original reflection harness invoking
the official PlayerChatMessage signature updater on all fourteen release
artifacts. It initializes the release registries when the codec's static
initializers require them, without starting a game server. The captured bytes
confirm the fixed big-endian signature domain, sender/session UUIDs, signed
32-bit index, salt, Unix seconds, UTF-8 byte length/content and ordered previous
signatures. Packet timestamps remain milliseconds; negative values use floor
division when constructing canonical signing input.

The output fixtures are test data, not copied game implementation. Signed
commands carry at most eight argument signatures with names bounded to sixteen
UTF-16 units; chat-session envelopes retain the verified 512-byte public-key and
4096-byte key-signature bounds. The dedicated signed-command packet begins at
766 and acknowledgement checksums at 770. The cached Paper command receiver has
a separately patched input-size policy; the codecs follow vanilla wire bounds.
The original Rust helpers expose canonical bytes/provider callbacks and wire
envelopes, with no certificate acquisition, private-key storage or implicit trust.

## Live equippable slot-ID correction

The expanded Paper matrix detected that equippable item components serialize
EquipmentSlot.getId(), while entity-equipment packets serialize enum ordinals.
All applicable release constructors/stream bindings confirm MainHand=0, Feet=1,
Legs=2, Chest=3, Head=4, OffHand=5, Body=6 and Saddle=7, with Saddle introduced
at 770. The corrected component mapping is independently covered by all seventy
valid slot/version combinations and pre-770 rejection. Other enum casts were
audited against their release constructors; no collateral entity-packet change
was needed. The final fourteen-release replay verifies the Head value rather
than merely accepting successful roundtrips. Diagnostic disassemblies remain
outside published source and evidence bundles.

## Real command/effect serialization checks

The independent Paper surface matrix confirms actual target-parser flags, redirect
references, suggestion ranges, all four stop-sound filters, particle envelope and
payload boundaries, world event 2001 and explosion formats. Decoded values are
checked before requiring byte-exact re-encoding. The harness uses normal console
actions and a non-executing client suggestion request in fresh loopback worlds.
Observed field differences are recorded as fixture-expectation corrections, not
retrofitted codec changes. Entity-attached sound remains a fixture-only surface;
no plugin was added to manufacture an observation.

## Streamed chunk packets

[All-family wire facts and artifact hashes](docs/validation/chunk-update-protocol-facts.json)
cover Update Light, Chunk Biomes, standalone block-entity data and view controls.
Release readers independently confirm no trust-edges field, two extra light
boundary sections, and signed VarInt view distances. Standalone compound NBT
becomes non-null at 766, unlike optional embedded full-chunk block entities.
Paper 1.21.5 build 114 biome buffers retain precisely the removed palette-length
allocation bytes; the decoder tolerates only that version-specific zero tail.
No proprietary implementation text is included.

## Additional persistent component representations

[ComponentHashExtendedOracle.java](tools/paper/ComponentHashExtendedOracle.java)
is an original offline API test, using public DataComponents persistent codecs
and HashOps for the seven cached modern families. Its [122-case record](docs/validation/component-hash-extended-oracle.json)
checks semantic text normalization, byte widths, ordered integer lists, defaults,
filtered books and registry-independent consumption effects. It does not copy
implementation text or distribute game binaries. Numeric registry references and
unimplemented structured text remain unsupported rather than hashed as wire bytes.

## Registry/tag packet roundtrips

The original [RegistryTagsOracle.java](tools/paper/RegistryTagsOracle.java)
uses cached public packet APIs/reflection to check canonical registry/tag bodies,
last-key-wins duplicate maps, default namespace aliases, the protocol775 namespace
boundary, and modern optional scalar NBT entries. [Per-release facts and artifact
hashes](docs/validation/registry-tags-oracle.json) retain all 14 results. These
checks do not log into an account, start a server or distribute game implementation
text. Syntactic identifiers do not supply missing static registry-name data.

## Authentication request contracts

The original loopback HTTP suite uses synthetic credentials and checks the
[Microsoft device flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code),
[refresh flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow#refresh-the-access-token),
[documented Xbox request headers and token exchange](https://learn.microsoft.com/en-us/gaming/gdk/docs/services/fundamentals/s2s-auth-calls/service-authentication/live-website-authentication),
and [RFC8628 polling requirements](https://www.rfc-editor.org/rfc/rfc8628#section-3.5).
Microsoft's Xbox documentation was verified 2026-10-02. A website/confidential-client
example is used only for Xbox exchange/header facts, not to claim public-client
app approval or require a client secret in Rustwire's device flow. Minecraft
profile/session fixtures validate this library's HTTP contract without asserting
live account interoperability. No real credentials or borrowed application IDs
are used by the tests.

## Scoreboard and player overlay wire facts

Original Java API oracles and bounded Python verifiers independently check
scoreboard/team and boss-bar/header/footer serializers in all 14 cached releases.
The [scoreboard report](docs/scoreboard-wire-audit.md) and
[overlay report](docs/overlay-wire-audit.md) link per-artifact hashes, exact
version boundaries and fallback/strict-enum distinctions. Domain probes distinguish
wire representability from valid gameplay/UI choices. Only original oracle code
and protocol facts are distributed.

## World/session wire facts

The original [WorldStateOracle.java](tools/paper/WorldStateOracle.java) invokes
cached release packet APIs without distributing their implementation text. The
[wire audit](docs/world-state-wire-audit.md) and [artifact facts](docs/validation/world-state-wire-oracle.json)
record all 14 releases, including clock-registry setup for 26.x. Both valid and
non-gameplay scalar domains are tested; names, field types and version boundaries
are reported as protocol facts rather than inferred from adjacent releases.

## Registry lookup identifier aliases

Registry lookup, duplicate detection and replacement now reuse the shared
resource-location identity rules. The existing
[registry/tag oracle record](docs/validation/registry-tags-oracle.json) establishes
default namespaces and empty paths through release APIs in all 14 families; this
increment reuses that evidence and does not claim a new Java or live-server run.

The nine original Rust regressions include hand-authored modern wire bytes,
every truncation of that valid fixture, all-family wire roundtrips, legacy
explicit IDs and transactional duplicate/replacement checks. Wire identifiers
and stored entry spellings are retained. Rejecting duplicate semantic identities
is `RegistryStore`'s existing uniqueness policy extended to aliases, not a claim
that the official packet reader rejects duplicate wire entries. No game source,
binary or private-server capture was added.
## Auxiliary container and merchant packets

`tools/paper/ContainerAuxiliaryOracle.java` is an original reflection-based
fixture harness for six packet families across protocols 763–776. The
reproducible checked report is
`docs/validation/container-auxiliary-wire-oracle.json`; the mechanically
extracted Rust fixtures are `tests/fixtures/container-auxiliary.txt`. Prepared
Paper artifact hashes, the oracle source hash and all 14 pinned schema hashes
are recorded in the report. No server binaries or implementation code are
included.

The actual release APIs correct two historical schema boundaries: container
button fields become signed VarInts at protocol 766, and merchant container IDs
remain signed VarInts even at 766–767 (their schema incorrectly uses the byte
ContainerID alias). Property/mount IDs are unsigned bytes through 767 and signed
VarInts from 768. Mount inventory size changes meaning from slots to columns at
767; the public codec retains the wire value without deriving a layout.
Cooldown targets change from numeric item IDs to resource identifiers at 768.

Merchant inputs use ordinary Slots through 765, then item ID, signed count and
ordered exact component values from 766. Exact costs have no empty sentinel or
removed-component count. The second cost has a separate boolean presence flag; modern result Slots must
be nonempty.
Tests retain nonpositive modern cost counts, signed scalar fields, negative
zero, infinities and NaN payload bits. Existing Slot validation still applies to
legacy cost and result stacks; this is not a simulator of merchant game rules.

Merchant bodies are decoded and re-encoded through release APIs through 768.
For 769+ the full body is decoded and its numeric fields inspected; each modern
cost is independently re-encoded. Full nonempty merchant re-encoding is not
claimed for 769+, because Paper's outbound ItemStack sanitizer requires a live
server. It is neither replaced nor disabled. For 775–776 the isolated stone
fixture holder is given an explicit empty default-component map via the public
holder API; this does not load or validate data-pack defaults. The five other
packet families and empty merchant bodies are decoded and re-encoded across
all 14 families. No server, world or network connection is started.

## Map and statistics schema-backed envelopes

The new [map/statistics wire audit](docs/map-statistics-wire-audit.md) records
bounded outer codecs, unresolved numeric IDs and the 764/765 label representation
boundary from all fourteen hash-pinned schemas. The original read-only
`tools/verify_map_statistics_schemas.py` checks complete structural equality for
both packet bodies in every family. Original hand-authored Rust fixtures do not
use the library encoder to construct expected wire bytes. This increment used
no game artifacts or private project material and makes no new release-API or
live-server interoperability claim. Optional map-canvas validation is explicitly
an application policy, separate from lossless envelope decoding.

## Ordinary entity-control schema evidence

The seven clientbound entity-control layouts were verified in all fourteen
hash-pinned schemas. The [wire audit](docs/entity-control-wire-audit.md) documents
source links, unchanged body shapes, raw scalar-domain choices and a limited
protocol-776 public implementation cross-check. An original standalone Python
encoder supplies 224 synthetic fixture bodies; these are schema-derived fixtures,
not new release-serializer outputs or live-server captures. No game code or
binaries were copied. Damage registry and source-entity references remain
unresolved wire values.

## HUD and player-feedback schema-backed envelopes

The [HUD wire audit](docs/hud-wire-audit.md) covers ten clientbound play packet
families over all fourteen pinned protocols. The original read-only
`tools/verify_hud_schemas.py` checks SHA-256, complete body equality, name-to-type
dispatch and all 140 fixture IDs. It preserves protocol 775's hand mapper in the
schema comparison, while the wire API explicitly retains opaque hand VarInts.
Original hand-authored fixture bodies and malformed/resource-budget tests use
no library encoder to construct expected wire data. This increment makes no
new upstream serializer, receiving-client acceptance or live-server claim.

## World/player-control schema and source audit

The [world-control audit](docs/world-control-wire-audit.md) records all eleven
new clientbound names, 142 present packet/family combinations and their exact
IDs against the existing fourteen hash-pinned schemas. The original verifier
checks structural equality and known absences without modifying schema hashes.
Version-pinned MCProtocolLib and independent PacketEvents implementations
corroborate VarInt target anchors for 763–764, where the pinned schema has a
stale string field. Source inspection also corroborates nullable compound NBT
and the 773 rotation-relative-flag boundary; exact commits and caveats are
linked in the audit. No external implementation code is copied.

The 198 original Python-encoded fixture rows are synthetic, independent of the
Rust encoder: 184 valid bodies plus 14 unknown-anchor cases. No new Minecraft
release-API execution or live-server validation is claimed for this increment.

## Advancement wire codecs

Advancement update/progress/removal and tab packets are audited against all
fourteen SHA-256-pinned schema inputs by `tools/verify_advancement_schemas.py`.
Independent clarification sources are MCProtocolLib
`39fa9e822670d6cadd826e4760835b46a17d092a` and Minestom
`9c20510def19ba151153c9a20aff8417753a0367`, linked precisely in
`docs/advancements-wire-audit.md`. These establish frame ordinals, flags,
progress timestamp units and template icon ordering, not a vanilla release
oracle. Original fixtures are independently assembled Python bytes; no upstream
implementation code, game binary or packet capture is copied.

## Server and chat metadata envelopes

The [server/chat metadata audit](docs/server-metadata-wire-audit.md) records six
new bounded packet families and their exact version/state/direction boundaries.
`tools/verify_server_metadata_schemas.py` hash-checks all fourteen schemas and
independently constructs 310 original synthetic wire fixtures. Pinned Azalea,
MCProtocolLib and Minestom source reads corroborate packed signatures, suggestion
actions and report-detail limits omitted from schema types. This is source-level
corroboration, not an executed release oracle or live-server result. Only protocol
facts and original Rust/Python test code were added; no upstream implementation
code, game binaries or packet captures were copied.

## Protocol-766 component-slot correction

The [slot-count audit](docs/slot-count-wire-audit.md) records official 1.20.6
bundle, inner archive and mappings hashes, mapped stream bindings and reproducible
static inspection commands. This primary evidence corrects the stale pinned
schema's `i8`: component-slot counts are VarInts already at protocol 766.
Original count-128/300 fixtures distinguish the encodings; earlier small-count
live checks did not. This increment executes no game serializer or server.

## Recipe-book control schema and source evidence

The [recipe-control audit](docs/recipe-control-wire-audit.md) checks 79 complete
outer layouts across fourteen hash-pinned families, of which 70 have implemented
typed bodies. Nine modern ghost-response envelopes are audited only to establish
the excluded RecipeDisplay boundary. The independent original Python encoder
produces 306 synthetic fixture rows, including 19 unsupported-enum cases.
Pinned MCProtocolLib and PacketEvents source inspection corroborates wire widths
and modern signed VarInts but exposes a legacy byte-signedness disagreement.
The API retains raw legacy bytes; it does not label that disagreement a proven
vanilla schema bug. Immutable URLs and inspected-source hashes are recorded in
[the source manifest](docs/validation/recipe-control-source-audit.json).
No external implementation code is copied and no new game/runtime oracle or
live-server interoperability claim is made.
