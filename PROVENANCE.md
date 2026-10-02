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
