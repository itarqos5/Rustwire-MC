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
