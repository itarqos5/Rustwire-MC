# Validation

Date: 2026-10-02. The results distinguish fixture/mock evidence from actual server interoperability.

## Automated checks

- 55 tests with all features; 42 applicable tests without default features
- Golden VarInt/position/NBT/palette fixtures, signed SHA-1 examples, an independent OpenSSL AES-CFB8 fixture and RSA response decryption checks
- Truncation, malformed-data, deterministic fuzz-style input, compression-bomb size checks, frame fragmentation, stream cipher continuity and resource-budget regressions
- Loopback mock-server login/control traffic for all 14 protocol families
- Typed Join Game fixtures and chunk roundtrips for all 14 families
- Rustfmt, clippy with warnings denied, Rustdoc and example builds
- Full-feature tests also run locally on Rust 1.88.0; CI covers that minimum version and stable Rust on Linux, Windows and macOS

The most recent GitHub Actions result must be checked for the exact commit under review. [Workflow runs](https://github.com/itarqos5/Rustwire-MC/actions) contain the external results; a prior green commit is not evidence for a later commit.

## Actual Paper servers

All used official, checksum-verified downloads, new superflat worlds, offline mode and an actual listener verified as `127.0.0.1:25565`. One server ran at a time. All shut down cleanly; no account login or public listener was used.

| Exact release/build | Protocol | Decoded chunks | Registries | Keepalive replies | Duration |
|---|---:|---:|---:|---:|---:|
| Paper 1.20.1 / 196 | 763 | 77 | 6 legacy | 1 | 20.62 s |
| Paper 1.21.1 / 133 | 767 | 77 | 11 | 1 | 20.73 s |
| Paper 26.2 / 129 | 776 | 81 | 29 | 19 | 20.25 s |

Each passed status/ping, offline login, typed overworld metadata (min Y -64, height 384, 24 sections), teleport acknowledgement, chunk decoding and keepalive handling. The two modern versions also completed configuration. Exact client hashes, outputs and artifact provenance are in:

- [Machine-readable results](docs/validation/results-summary.json)
- [Text observations](docs/validation/textual-observations.json)
- [Official artifact URLs and SHA-256 checks](docs/validation/download-provenance.json)
- [Legacy interoperability findings](docs/validation/interoperability-findings.md)

The tested client snapshot is identified by its SHA-256 in the results. Later changes limited to validation guards or documentation do not retroactively change that recorded binary. Re-run the server tests after wire-format changes.

Paper 1.20.1 testing found two issues that mock roundtrips missed: play settings must wait for Join Game, and its legacy section buffer can contain exactly one zero-padding byte per singleton block palette. Both now have synthetic regression tests. No captured packet/world data is included in the repository.

### Reproduce on Linux x86-64

Python 3, curl, tar, a C/Rust build toolchain and Java 21 are prerequisites. The script downloads the pinned official Temurin 25 runtime for 26.2. Other host platforms need appropriate official Java binaries and a portable process/listener checker; the bundled real-server harness is Linux-specific.

```sh
cargo build --examples --features crypto
python3 tools/paper/prepare.py
# Read the Minecraft EULA and linked agreements. Only if you accept them,
# manually change eula=false to eula=true in each selected test server directory.
python3 tools/paper/validate.py 1.20.1 1.21.1 26.2
python3 tools/paper/summarize.py
```

Preparation creates `.rustwire-validation/`, defaults new EULA files to **false**, never starts a server and never accepts terms. Read the [Minecraft EULA](https://www.minecraft.net/en-us/eula) and its linked Microsoft Services Agreement before proceeding. The runner refuses to run without explicit acceptance. Use `--root PATH` during preparation and `RUSTWIRE_VALIDATION_DIR=PATH` for the runner/summarizer to choose another isolated workspace.

The runner uses disposable server configuration (1 GiB heap,2 JVM processors, view/simulation distance 2) and snapshots the compiled examples before each group of runs. `RUSTWIRE_PROJECT` can select another checkout; `RUSTWIRE_MIN_SECONDS` defaults to 20. Results go into the isolated validation directory, not the checked-in evidence files. Do not run this against an existing personal server directory; preparation writes test configuration files.

On the original test host, Java could not resolve some external services directly. The preparation script fetched the exact Mojang download URL embedded by Paper into the expected cache and verified its checksum. Optional public-key/version service lookups failed but did not prevent offline testing. This is not evidence of successful online-account authentication.

## Performance observation

`cargo bench --bench codec`, optimized build on this Linux cloud host, decoded 2,000,000 three-byte VarInts in 22.590401 ms, approximately 11.30 ns/op. This is one microbenchmark observation, not an end-to-end throughput claim, comparison with another library or guarantee on other hardware. Network/chunk/auth performance still needs representative application benchmarks. Splitting generated tables into modules improves maintainability; it does not by itself make packet handling faster.

## Not verified

Live Microsoft/Xbox/Minecraft account authorization, secure-chat signing, every release alias, every gameplay packet, arbitrary plugins/modded servers, other dimensions and production-scale hostile traffic have not been exhaustively tested. The README lists currently untyped gameplay areas explicitly.
