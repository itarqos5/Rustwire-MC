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
