# Contributing

Keep changes focused and independently testable. Do not add game binaries or decompiled Mojang implementation code.

## Structure

- `codec.rs`, `frame.rs`: wire primitives and framing
- `version.rs`, `catalog/`: explicit version metadata and generated ID tables
- `packet.rs`, `packet/`: typed connection/control and world metadata
- `connection.rs`: synchronous generic transport/state handling
- `nbt.rs`, `registry.rs`, `chunk.rs`: bounded semantic world codecs
- `crypto.rs`, `auth.rs`: optional online-mode support
- `tests/`: malformed-input, golden fixture, crypto and loopback tests
- `examples/`: minimal status, offline chunk and online login clients

Keep the library one crate unless a concrete dependency or ownership boundary warrants a workspace. File organization alone does not improve runtime performance.

## Before a pull request

```sh
cargo fmt --all -- --check
cargo test --locked --no-default-features
cargo test --locked
cargo test --locked --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --all-features --no-deps
```

Test on the stated minimum Rust version too. Add independent wire fixtures, truncation/malformed cases and version-boundary tests. Self roundtrips alone do not establish interoperability.

## Protocol metadata

```sh
python3 tools/fetch_schemas.py
python3 tools/generate_catalog.py
```

Python 3 and rustfmt are development-only requirements. The fetcher pins immutable commits and checks SHA-256. Update hashes and PROVENANCE.md deliberately when adding a release. Never silently fall back to a neighboring protocol. A generated packet name is not a typed implementation; update the coverage matrix accurately.

## Real servers

Use only servers you own or are authorized to test. Read and explicitly accept applicable terms yourself, bind offline-mode servers to 127.0.0.1, use a new temporary world and shut them down afterward. Do not commit server binaries, downloaded game assets, worlds or packet captures from private servers. No user account is required for the isolated offline tests documented here.
