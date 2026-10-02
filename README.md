# Rustwire

Cargo package: `rustwire-mc`. A lean Minecraft Java protocol library under active initial development.

## Landed and tested

- Checked VarInt/VarLong, Java-length UTF-8 strings, packed positions
- Partial-frame decoding with strict resource bounds
- Optional pure-Rust zlib compression and decompressed-size validation
- Explicit packet ID/name catalogs for 14 protocol families, covering 23 releases from 1.20 through 26.2

`cargo test`: 15 passing tests. `cargo test --no-default-features`: 13 passing tests; zero runtime dependencies.

A packet catalog is not full typed packet support. TCP/login/auth/chunks are being developed in subsequent coherent commits; they are not available in this foundation commit. No crates.io publication has occurred.

See [PROVENANCE.md](PROVENANCE.md) for immutable source references and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
