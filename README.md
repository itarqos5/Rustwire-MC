# Rustwire

Cargo package: `rustwire-mc`. A lean Minecraft Java protocol library under active initial development.

## Landed and tested

- Checked VarInt/VarLong, Java-length UTF-8 strings, packed positions
- Partial-frame decoding with strict resource bounds
- Optional pure-Rust zlib compression and decompressed-size validation
- Explicit packet ID/name catalogs for 14 protocol families, covering 23 releases from 1.20 through 26.2

- Semantic chunk sections, block/biome palettes, typed heightmaps, lights and block entities
- Bounded Java NBT with modified UTF-8, registry decoding and dimension metadata

- TCP transport, status/ping, login/configuration transitions and common control packets
- Optional RSA/AES-CFB8 encryption with redacted secrets
- Loopback mock-server login tests across all 14 protocol families

`cargo test --all-features`: 55 passing tests. `cargo test --no-default-features`: 42 passing tests; zero runtime dependencies. Rust 1.88+.

A packet catalog is not full typed packet support. The optional `auth` feature implements Microsoft device/refresh flow, Xbox Live, XSTS, Minecraft profile lookup and Mojang session join. Live account-backed authentication has not been tested. Supply your own properly registered and service-authorized application ID; no borrowed launcher ID or token persistence is included. Many play packet payloads remain raw; inventory, entity metadata and signed chat are not typed yet. No crates.io publication has occurred.

See [PROVENANCE.md](PROVENANCE.md) for immutable source references and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Try it

`cargo run --example status -- 127.0.0.1 25565 1.21.1`

`cargo run --features crypto --example offline_chunks -- 127.0.0.1 25565 1.21.1 24 20`

Offline mode is only for authorized test servers. The last two arguments are section count and minimum observation seconds. Use the server dimension metadata for custom dimensions. Real isolated Paper tests passed on 1.21.1 build 133 and 26.2 build 129: login, configuration, registries, teleport acknowledgement, chunk decoding and keepalives over 20-second sessions. Paper 1.20.1 build 196 also passed after fixing early settings transmission and exact singleton-section zero padding. Other version families currently have mock/fixture coverage.

Join Game is now typed separately from Login Success. Wait for `Event::Joined` before sending legacy play settings. The example uses the server dimension registry to determine section counts when no override is given.

`cargo run --features auth --example online_login -- YOUR_CLIENT_ID HOST 25565 1.21.1`

Tokens redact Debug and zeroize owned storage; HTTP JSON internals may retain transient copies. The full feature build uses TLS dependencies; default and no-default builds remain lean.
