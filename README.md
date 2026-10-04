# Rustwire

A lean, bounded Minecraft Java networking library in Rust. Cargo package: **`rustwire-mc`**.

Rustwire targets the 14 release-protocol families spanning **1.20 through 26.2**. It implements connection/control, semantic world data, common entity/inventory/chat packets and interaction codecs, with raw packet access for remaining gameplay layouts. **This is an initial 0.1 library, not complete typed coverage of every Minecraft packet.**

## Quick start

Rust 1.88 or newer. No Java, code generator, native compression library, or game assets are needed to build the library. Java is needed only for optional real-server validation.

```toml
[dependencies]
rustwire-mc = { git = "https://github.com/itarqos5/Rustwire-MC", branch = "main" }
```

For reproducible applications, replace `branch` with a reviewed `rev`. This project is **not published on crates.io**. The separate crate named `rustwire` is unrelated.

```rust
use rustwire_mc::{connection::Connection, Limits, Version};
use std::time::Duration;

fn main() -> Result<(), rustwire_mc::Error> {
    let mut server = Connection::connect(
        ("127.0.0.1", 25565), Version::V1_21,
        Duration::from_secs(10), Limits::default(),
    )?;
    let status = server.status("localhost", 25565, 42)?;
    println!("{}", status.json);
    Ok(())
}
```

```sh
cargo run --example status -- 127.0.0.1 25565 1.21.1
cargo run --features crypto --example offline_chunks -- 127.0.0.1 25565 26.2 24 20
```

`offline_chunks` is for an offline-mode server you own or are authorized to test. It handles login/configuration, registries, teleport acknowledgement, chunk batches and keepalives, then exits after at least four decoded chunks and the optional minimum observation time. Section count is read from the dimension registry when omitted. Never expose an offline-mode test server publicly.

## Small default build

| Feature | Contents | Build trade-off |
|---|---|---|
| No default features | Standard-library transport, binary codecs, catalogs, NBT, registries, chunks | **Zero runtime dependencies**; compressed/online-mode connections require features |
| Default: `compression` | Pure-Rust zlib via `flate2`/`miniz_oxide` | No system zlib or C compiler for this feature |
| `crypto` | AES-128-CFB8, RSA encryption response, signed SHA-1 server hash, offline UUID | Pure-Rust crypto; does not perform HTTP authentication |
| `auth` | Microsoft device/refresh flow, Xbox Live, XSTS, Minecraft profile/session join | Includes `crypto`; TLS stack adds dependencies and may require a platform C toolchain for `ring` |

```sh
cargo test --no-default-features
cargo test
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --all-features --no-deps
cargo bench --bench codec
cargo bench --no-default-features --bench protocol_workloads
```

## What is implemented

- Checked big-endian primitives, VarInt/VarLong, UTF-8/UTF-16 string limits, UUID bytes and packed block positions
- Transactional partial-frame decoding; packet/frame/collection/NBT limits; strict zlib decompressed sizes and trailing-stream rejection
- Generic `Read + Write` transport plus timeout-aware TCP; separate continuous encrypt/decrypt states; failed I/O poisons the connection rather than retrying a partly consumed stream
- Handshake, status/ping, login start/success, compression negotiation, RSA challenge/response and configuration transitions
- Client settings, keepalive/ping replies, known-pack selection, cookie/plugin responses, custom payloads, all four player movement forms and teleport acknowledgement
- Typed Join Game and dimension metadata, including legacy in-login registries and 26.2's online-mode field
- Full NBT tag tree with Java modified UTF-8, including lossless unpaired UTF-16 surrogates
- Dynamic registry entries, omitted known-pack data, dimension height/min-Y lookup
- Bidirectional registry/tag packets with state/version checks, last-key-wins tag lookups, optional numeric-ID resolution and aggregate resource budgets
- Chunk coordinates, sections, block-state and biome palettes, non-spanning packed storage, heightmaps, block entities and light data
- Exact release-specific packet ID/name catalogs, split into `src/catalog/p763.rs` through `p776.rs`; unknown versions fail closed

### Typed gameplay

`Connection::next_typed_event()` adds semantic dispatch while retaining complete raw packets when a nested layout is intentionally unsupported. Malformed known layouts remain errors. `next_event()` keeps the lower-level control/raw interface.

`next_typed_event_with_context(DecodeContext::for_dimension(&dimension))` also
decodes full chunks, streamed light and biome replacements with the active
dimension's section count. Refresh that context after Join Game, Respawn and
registry changes. Without it, those packets remain raw rather than guessing a
world height. Standalone block-entity and view/simulation controls need no context.

- Entity spawn, relative movement/look, velocity, teleports/synchronization, removal, status, health and abilities
- Passenger lists, entity attachments, head rotation, camera, animation, damage events and hurt animation, with raw entity/registry references and exact-version packet IDs
- Player-list actions/removal, equipment, attributes/modifiers and status-effect additions/removals with exact version boundaries
- Advancement definitions/removals/progress and tab notifications with shared text/icon budgets and exact item-template boundaries
- Command-tree graphs with semantic argument parsers, suggestion providers/tooltips, and suggestion request/response packets
- Scoreboard objectives/display/scores/reset/teams, boss-bar operations and player-list headers/footers, with release-specific wire boundaries
- Map-data decorations/pixel patches and statistics envelopes, with unresolved registry IDs, shared resource budgets and opt-in canvas validation
- Title/subtitle/action-bar controls, book opening, experience and combat notifications with bounded JSON/NBT components and typed play dispatch
- Game-state reasons, versioned world clocks/time, spawn/difficulty, all world-border updates, block acknowledgements and player-loaded notifications
- Block/section updates, chunk unload and respawn with dimension metadata
- Block actions/break animations, sign editors, compound NBT queries, item collection, vehicle/look-at controls and version-gated tick/rotation/projectile controls
- Streamed lighting, biome-only chunk replacements, compound block-entity updates and view/simulation-distance controls, with dimension and aggregate-allocation limits
- World particles, legacy/modern explosions, positional/entity sounds, stop-sound filters and world events with shared nested-payload budgets
- Classic inventory NBT, modern component patches, content/set-slot/open/close/selected-slot and cursor/player-slot packets
- Container properties, horse/mount screens, button/select-trade requests, item/group cooldowns and bounded merchant offers with release-specific exact component costs
- Typed food/effects, potions, books, attributes, lodestones, fireworks, bees, tools, consumables, equipment/combat components, profiles, trims, instruments, banners and adventure-mode predicates, including release-specific holders and nested matchers
- Full-stack container clicks through 1.21.4; hashed click representations from 1.21.5, with structured CRC32C derivation for an explicit component subset
- `HashedItemStack::from_slot` derives verified hashes for ordinary scalar, damage, custom-data NBT, block-state, custom-model-data and tooltip components, plus literal/styled names and lore, food, books, fireworks, lodestones, cooldowns and selected combat/consumption components
- Persistent hash support is an explicit subset: numeric registry references, translated/click/hover text and unknown persistent codecs fail rather than producing guessed checksums
- All known entity-metadata outer serializers, including semantic particles, painting/wolf holders and resolvable profiles, with shared slot/NBT budgets
- Server MOTD/icon data, bounded custom report metadata, chat suggestions/deletion and play ping envelopes with no automatic reporting or trust-policy changes
- System/disguised/signed-player-chat envelope decoding; signatures are retained but not authenticated
- Signed-message, signed-command and public chat-session envelopes, plus canonical signing input and a caller-owned signing-provider hook
- Bounded last-seen tracking, version-aware acknowledgement checksums and transactional packed-signature cache resolution, after explicit application display/trust decisions
- Typed hand swings, digging, use-block/use-item, entity interaction/actions, respawn/statistics commands, player input/abilities and tick-end packets
- Tags, resource-pack offers/status replies, cookies, transfers, server links and code-of-conduct payloads, with no implicit consent or URL navigation

All known vanilla added-item outer payload layouts are implemented: 56/56 in 1.20.5 through 111/111 in 26.2, together with all known outer metadata serializers. Registry resolution, game-specific NBT predicate semantics and persistent component-hash derivation retain explicit limits. Secure-chat certificate trust, the cryptographic provider and session/index policy remain application responsibilities. [Exact per-family supported and unsupported names](docs/typed-coverage.json) are reproducible with `python3 tools/report_coverage.py`.

Numeric block-state and biome IDs remain numeric. Dynamic registry names can be looked up with `RegistryStore`; a bundled static block-state-name dataset is not included.

### Authentication

```sh
cargo run --features auth --example online_login -- YOUR_MICROSOFT_CLIENT_ID SERVER_HOST 25565 1.21.1
```

Use **your own properly registered and Minecraft-service-authorized public-client application ID**. A generic Entra registration may not be enough for Minecraft Services. Rustwire does not borrow another launcher's app ID, accept passwords, store refresh tokens, or log tokens. Token holders redact `Debug` and zeroize their owned strings on drop; this is best-effort memory hygiene, not a guarantee that no transient HTTP/JSON copy ever existed.

Twenty-three loopback HTTP tests verify request/response contracts, error limits and redaction with synthetic credentials. Account-backed interoperability still needs an eligible account and authorized application.

You may instead use an application-owned authentication layer and pass its Minecraft token/profile to the session-join helper. Join the session server for the server's exact hash before completing encryption when `should_authenticate` is true. Old Mojang username/password authentication is not implemented.

**Live account-backed Microsoft authentication has not been tested.** Auth parsing/redaction and crypto tests are offline; server interoperability below uses isolated offline-mode servers.

## Version coverage

Every row has catalog checks, loopback login/control tests and synthetic chunk roundtrips. All 14 families also passed real Paper status/login/configuration (when applicable)/chunk/keepalive tests. “Paper tested” names the exact representative release, not every patch alias. Initial gameplay probes ran on 1.20.1, 1.21.1 and 26.2; the later six-version component and four-version Grim scenarios are described below.

| Protocol | Releases | Paper tested |
|---|---|---|
| 763 | 1.20, 1.20.1 | 1.20.1 build 196 |
| 764 | 1.20.2 | 1.20.2 build 318 |
| 765 | 1.20.3, 1.20.4 | 1.20.4 build 499 |
| 766 | 1.20.5, 1.20.6 | 1.20.6 build 151 |
| 767 | 1.21, 1.21.1 | 1.21.1 build 133 |
| 768 | 1.21.2, 1.21.3 | 1.21.3 build 83 |
| 769 | 1.21.4 | 1.21.4 build 232 |
| 770 | 1.21.5 | 1.21.5 build 114 (ALPHA) |
| 771 | 1.21.6 | 1.21.6 build 48 |
| 772 | 1.21.7, 1.21.8 | 1.21.8 build 60 |
| 773 | 1.21.9, 1.21.10 | 1.21.10 build 130 |
| 774 | 1.21.11 | 1.21.11 build 132 |
| 775 | 26.1, 26.1.1, 26.1.2 | 26.1.2 build 74 |
| 776 | 26.2 | 26.2 build 129 |

Important wire boundaries are explicit: configuration/anonymous NBT from 764, per-registry entry lists from 766, changed position synchronization from 768, typed heightmaps/implicit palette storage lengths from 770, chunk fluid counts from 775, and 26.2 login-session UUID/online-mode metadata at 776. Observed Paper 1.20.1 and 1.21.5 palette-buffer zero padding is accepted only in the exact version-specific patterns; unexplained trailing data remains an error.

## Deliberate limits and next work

Rustwire is a protocol building block, not a full game client, bot, proxy or server.

- Known component wire layouts are implemented; registry identities, NBT-backed predicate semantics and many persistent component-hash forms remain application responsibilities. Recipes and remaining gameplay packets still need codecs; the command tree describes syntax but does not execute commands or replace Brigadier parsing
- Secure-chat certificates, private-key signing/verification providers and session/index policy remain application responsibilities. Canonical signing-input and acknowledgement/cache helpers do not establish trust. Unsigned sending requires an explicit allowed-by-server policy
- Resource-pack consent/downloads, code-of-conduct acceptance, transfers and custom login plugins are application decisions. Examples stop clearly on conduct/resource-pack challenges rather than accepting them
- No automatic SRV lookup, proxy connector, async-runtime adapter, reconnect policy, Mojang chat-certificate acquisition, mod-loader handshake, world simulation or rendering
- NBT is bounded and owned; large registry/packet processing can still allocate materially. Adjust `Limits` for your application
- TCP connect/read/write have timeouts; operating-system DNS resolution can block outside the connect timeout
- Bedrock, snapshots, pre-releases and versions outside the explicit matrix are unsupported

See [PROVENANCE.md](PROVENANCE.md), [VALIDATION.md](VALIDATION.md), [SECURITY.md](SECURITY.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT for the Rust implementation. Generated metadata attribution is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Not affiliated with or endorsed by Mojang or Microsoft. No game binaries, source decompilations, worlds or assets are distributed.

### Extended interoperability checkpoint

Six real Paper versions (1.20.6, 1.21.1, 1.21.3, 1.21.5, 26.1.2, 26.2) passed
additional component and particle-metadata scenarios. Derived click hashes passed
positive/negative server-correction controls on the three hashed-click versions.
See [validation](VALIDATION.md#extended-component-and-metadata-gameplay) for
precise coverage and reproducible evidence. These are bounded protocol tests;
there is no promise that arbitrary client behavior will satisfy an anti-cheat.

### Grim compatibility checkpoint

A bounded headless client passed walking, player-inventory swapping and two
upward wind-charge uses on **1.21.1, 1.21.5, 26.1.2 and 26.2**. Every run used
the same binary and an ordinary non-OP survival player with default checks.
There were zero flags outside a deliberately invalid duplicate-sprint control,
which produced exactly one expected BadPacketsF flag per run. The 26.x tests
use a pinned official **alpha** Grim build; older tests use stable 2.3.73.

These tests found and fixed a real 26.2 packet-ID catalog error, plus test-client
physics and response-ordering mistakes. Full evidence, limitations and
reproduction are in [the Grim report](docs/validation/grim-validation.md).
Walking is limited to a known flat stone floor. Wind-charge use verifies
projectile creation, not explosion knockback or wind-charge jumping. This is
an interoperability checkpoint, not a complete graphical client or a guarantee
for arbitrary client behavior.

### Expanded fourteen-family gameplay checkpoint

All fourteen pinned representative releases passed one frozen client built from
library snapshot `11674342`, with **259 version-specific decoded-value checks**.
The scenario covers a second player's roster entry/removal, damaged equipment,
attribute modifiers, effect add/remove and selected modern component values.
There were no unsupported decodes, raw fallbacks in the explicitly gated packet
set, or console failures. Every process exited cleanly.

[Expanded evidence](docs/validation/expanded-gameplay-results.json) preserves
the real equippable-slot correction and earlier test-harness timing failures.
Each later increment has separately scoped evidence. The command/world-effect
checkpoint below has its own live matrix; signing input remains API-oracle tested.

### Command and world-effect checkpoint

All fourteen representatives also passed a separate frozen-source scenario for
command trees/suggestions, particles, explosions, positional sounds, stop-sound
and world events: **381 value-check groups and 216 byte-exact real-packet
roundtrips**, with no decode, raw-gate, value or console failures.
[Evidence and scope](docs/validation/surface-results.json) identify snapshot
`19444536` and the common client binary. Entity-attached sounds remain fixture-only;
ordinary pig sounds used positional packets. These checks do not establish
rendering, physics or complete coverage of every command parser/tooltip value.

### Streamed-world verification

Fresh real-server checks verified full chunks, block entities, biome replacements
and view controls on all 14 families. Standalone light updates were verified on
13; Paper 1.20.4 emitted none in the bounded fixture. The report keeps that gap
explicit. It records 277/280 check groups and 3,421 received-payload roundtrips;
[exact evidence and limits](VALIDATION.md#live-streamed-world-updates).

### Expanded inventory-hash verification

All seven modern families passed 149 survival-inventory clicks: 142 correct
predictions without corrections and seven deliberate wrong-hash controls with
exact authoritative reconciliation. Server inventory state and official-oracle
hashes were checked independently. [Scenarios, reproducibility and limits](docs/validation/extended-item-hash-report.md).

### Measured chunk-codec optimization

Packed palette validation scans storage words without repeated per-block division
and skips impossible out-of-range checks for direct/full-domain palettes. On the
recorded Linux host, a synthetic 24-section chunk decode changed from 124.75µs to
51.69µs; this is an allocation-inclusive microbenchmark, not network throughput.
[Workloads, controls, source hashes and all runs](VALIDATION.md#packed-palette-validation-benchmark).

### Recipe-book control envelopes

Bounded serverbound craft requests, book-setting changes and seen-recipe notices
cover all fourteen families. Clientbound legacy unlock/ghost responses and
modern settings/removal packets have typed dispatch. Registry keys and modern
display IDs are explicitly versioned; legacy window bytes are retained losslessly.
Recipe declarations, modern displays/additions and crafting execution remain
outside this slice. [Exact boundaries and schema/source evidence](docs/recipe-control-wire-audit.md).

### Legacy recipe declarations (763–767)

`packet::recipe_declarations::LegacyDeclareRecipes` adds the clientbound legacy
`declare_recipes` body for protocols 763–767, including all 23 audited vanilla
serializer kinds, ingredient alternatives, outputs and shared bounded item/NBT/
component payloads. Its separate `DecodedPacket::LegacyRecipes` branch is
Play-only. Unknown unframed serializers/components preserve the whole raw packet;
known malformed bodies remain errors. Modern declarations (768+) remain raw and
unimplemented. This extends the earlier recipe-control scope; it does not
execute crafting or maintain a recipe registry.

The [legacy wire audit](docs/legacy-recipe-wire-audit.md) documents two checked
schema discrepancies: obsolete banner-add-pattern serializer IDs and the 766
slot count. Component-slot counts are VarInts from **766**, as corroborated by
release-era ViaVersion and PacketEvents; the shared inventory codec is corrected
accordingly. Independent count-128/count-300 fixtures distinguish this from the
previous signed-byte assumption.

### Modern recipe displays (768–776)

`packet::recipe_display` adds bounded SlotDisplay/RecipeDisplay payloads,
`RecipeBookAdd`, and modern `CraftRecipeResponse`, with a separate modern typed
dispatch family preserving legacy control APIs. It supports recursive composites,
versioned smithing patterns, shifted groups, holder-set requirements, and the
775–776 item-template boundary. The pinned 776 Slot leaf is explicitly corrected
from independent and static official-release evidence. Unknown unframed kinds or
components preserve the full raw packet. Modern recipe declarations remain a
separate unsupported slice. See [the wire audit](docs/recipe-display-wire-audit.md).
