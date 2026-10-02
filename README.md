# Rustwire

A lean, bounded Minecraft Java networking library in Rust. Cargo package: **`rustwire-mc`**.

Rustwire targets the 14 release-protocol families spanning **1.20 through 26.2**. It implements connection/control packets and semantic chunk/registry codecs, with raw packet access for the remaining gameplay protocol. **This is an initial 0.1 library, not complete typed coverage of every Minecraft packet.**

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
```

## What is implemented

- Checked big-endian primitives, VarInt/VarLong, UTF-8/UTF-16 string limits, UUID bytes and packed block positions
- Transactional partial-frame decoding; packet/frame/collection/NBT limits; strict zlib decompressed sizes and trailing-stream rejection
- Generic `Read + Write` transport plus timeout-aware TCP; separate continuous encrypt/decrypt states; failed I/O poisons the connection rather than retrying a partly consumed stream
- Handshake, status/ping, login start/success, compression negotiation, RSA challenge/response and configuration transitions
- Client settings, keepalive/ping replies, known-pack selection, cookie/plugin responses, custom payloads, player position and teleport acknowledgement
- Typed Join Game and dimension metadata, including legacy in-login registries and 26.2's online-mode field
- Full NBT tag tree with Java modified UTF-8, including lossless unpaired UTF-16 surrogates
- Dynamic registry entries, omitted known-pack data, dimension height/min-Y lookup
- Chunk coordinates, sections, block-state and biome palettes, non-spanning packed storage, heightmaps, block entities and light data
- Exact release-specific packet ID/name catalogs, split into `src/catalog/p763.rs` through `p776.rs`; unknown versions fail closed

Numeric block-state and biome IDs remain numeric. Dynamic registry names can be looked up with `RegistryStore`; a bundled static block-state-name dataset is not included.

### Authentication

```sh
cargo run --features auth --example online_login -- YOUR_MICROSOFT_CLIENT_ID SERVER_HOST 25565 1.21.1
```

Use **your own properly registered and Minecraft-service-authorized public-client application ID**. A generic Entra registration may not be enough for Minecraft Services. Rustwire does not borrow another launcher's app ID, accept passwords, store refresh tokens, or log tokens. Token holders redact `Debug` and zeroize their owned strings on drop; this is best-effort memory hygiene, not a guarantee that no transient HTTP/JSON copy ever existed.

You may instead use an application-owned authentication layer and pass its Minecraft token/profile to the session-join helper. Join the session server for the server's exact hash before completing encryption when `should_authenticate` is true. Old Mojang username/password authentication is not implemented.

**Live account-backed Microsoft authentication has not been tested.** Auth parsing/redaction and crypto tests are offline; server interoperability below uses isolated offline-mode servers.

## Version coverage

Every row has catalog checks, loopback login/control tests and synthetic chunk roundtrips. “Paper tested” identifies exact releases exercised against real official Paper builds, not every patch alias.

| Protocol | Releases | Paper tested |
|---|---|---|
| 763 | 1.20, 1.20.1 | 1.20.1 build 196 |
| 764 | 1.20.2 | — |
| 765 | 1.20.3, 1.20.4 | — |
| 766 | 1.20.5, 1.20.6 | — |
| 767 | 1.21, 1.21.1 | 1.21.1 build 133 |
| 768 | 1.21.2, 1.21.3 | — |
| 769 | 1.21.4 | — |
| 770 | 1.21.5 | — |
| 771 | 1.21.6 | — |
| 772 | 1.21.7, 1.21.8 | — |
| 773 | 1.21.9, 1.21.10 | — |
| 774 | 1.21.11 | — |
| 775 | 26.1, 26.1.1, 26.1.2 | — |
| 776 | 26.2 | 26.2 build 129 |

Important wire boundaries are explicit: configuration/anonymous NBT from 764, per-registry entry lists from 766, changed position synchronization from 768, typed heightmaps/implicit palette storage lengths from 770, chunk fluid counts from 775, and 26.2 login-session UUID/online-mode metadata at 776. Paper 1.20.1's observed singleton-section zero padding is accepted narrowly; unexplained trailing data remains an error.

## Deliberate limits and next work

Rustwire is a protocol building block, not a full game client, bot, proxy or server.

- Inventory/item components, entity metadata, recipes, commands and signed chat do not yet have comprehensive typed codecs; receive/send these through `RawPacket` with the version catalog
- Resource-pack consent/downloads, code-of-conduct acceptance, transfers and custom login plugins are application decisions. Examples stop clearly on conduct/resource-pack challenges rather than accepting them
- No automatic SRV lookup, proxy connector, async-runtime adapter, reconnect policy, Mojang secure-chat signing session, mod-loader handshake, world simulation or rendering
- NBT is bounded and owned; large registry/packet processing can still allocate materially. Adjust `Limits` for your application
- TCP connect/read/write have timeouts; operating-system DNS resolution can block outside the connect timeout
- Bedrock, snapshots, pre-releases and versions outside the explicit matrix are unsupported

See [PROVENANCE.md](PROVENANCE.md), [VALIDATION.md](VALIDATION.md), [SECURITY.md](SECURITY.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT for the Rust implementation. Generated metadata attribution is in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Not affiliated with or endorsed by Mojang or Microsoft. No game binaries, source decompilations, worlds or assets are distributed.
