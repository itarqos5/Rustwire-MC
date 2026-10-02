# Validation

Date: 2026-10-02. The results distinguish fixture/mock evidence from actual server interoperability.

## Automated checks

- 270 core tests with all features; 256 applicable core tests without default features
- Golden VarInt/position/NBT/palette fixtures, signed SHA-1 examples, an independent OpenSSL AES-CFB8 fixture and RSA response decryption checks
- Truncation, malformed-data, deterministic fuzz-style input, compression-bomb size checks, frame fragmentation, stream cipher continuity and resource-budget regressions
- Loopback mock-server login/control traffic for all 14 protocol families
- Typed Join Game fixtures and chunk roundtrips for all 14 families
- Rustfmt, clippy with warnings denied, Rustdoc and example builds
- Full-feature tests also run locally on Rust 1.88.0; CI covers that minimum version and stable Rust on Linux, Windows and macOS

The most recent GitHub Actions result must be checked for the exact commit under review. [Workflow runs](https://github.com/itarqos5/Rustwire-MC/actions) contain the external results; a prior green commit is not evidence for a later commit.

## Actual Paper servers

All 14 release-protocol families passed status/ping, offline login, applicable configuration, dimension metadata, teleport acknowledgement, real chunk decoding and keepalive handling. Each used one pinned representative release, a checksum-verified official Paper artifact, an isolated superflat world and a verified 127.0.0.1 listener. All servers shut down cleanly.

| Exact release/build | Protocol | Decoded chunks | Registries | Keepalive replies | Client duration |
|---|---:|---:|---:|---:|---:|
| 1.20.1 / 196 | 763 | 77 | 6 | 1 | 20.76 s |
| 1.20.2 / 318 | 764 | 77 | 6 | 1 | 20.59 s |
| 1.20.4 / 499 | 765 | 77 | 6 | 1 | 20.68 s |
| 1.20.6 / 151 | 766 | 77 | 8 | 1 | 20.79 s |
| 1.21.1 / 133 | 767 | 77 | 11 | 1 | 20.80 s |
| 1.21.3 / 83 | 768 | 77 | 12 | 1 | 20.57 s |
| 1.21.4 / 232 | 769 | 81 | 12 | 1 | 20.77 s |
| 1.21.5 / 114 (ALPHA) | 770 | 81 | 20 | 1 | 20.78 s |
| 1.21.6 / 48 | 771 | 81 | 21 | 20 | 20.67 s |
| 1.21.8 / 60 | 772 | 81 | 21 | 19 | 20.39 s |
| 1.21.10 / 130 | 773 | 81 | 21 | 19 | 20.04 s |
| 1.21.11 / 132 | 774 | 81 | 23 | 19 | 20.02 s |
| 26.1.2 / 74 | 775 | 81 | 28 | 18 | 20.05 s |
| 26.2 / 129 | 776 | 81 | 29 | 19 | 20.17 s |

The 1.21.5 server artifact is labeled **ALPHA** by Paper; Minecraft 1.21.5 itself is a release protocol. The matrix does not independently test every patch alias that shares a protocol.

- [Full matrix results and client hashes](docs/validation/matrix-results.json)
- [All 14 pinned Paper/Mojang artifact URLs and SHA-256 checks](docs/validation/matrix-download-provenance.json)
- [Original three-version connectivity evidence](docs/validation/results-summary.json)

### Deeper gameplay probes

Separate 45-second scenarios passed on Paper 1.20.1 build 196, 1.21.1 build 133 and 26.2 build 129. They exercised real typed inventory updates (plain items, damage and custom NBT/components), exact system-chat marker, entity spawn/movement/removal, metadata, block updates, health/damage, death and explicit respawn, plus chunks and keepalives after respawn. Server-side data queries confirmed the client moved +0.125 X and selected hotbar slot 1. Commands operated only on the test player and tagged probe entities in disposable worlds.

[Gameplay results](docs/validation/gameplay-results.json) retain missing-category checks, console failures, binary hashes and observed unsupported payloads. Both modern runs encountered one intentionally unsupported complex metadata payload after respawn and preserved it as raw data. Successful common metadata observations are **not** a claim of complete metadata coverage. The initial probe wrongly searched NBT Debug output for plain text; the modern runs were repeated after fixing the probe to inspect actual NBT strings.

### Interoperability fixes

- Paper 1.20.1 play settings must wait for Join Game
- Paper 1.20.1 can append one unused zero byte per singleton block palette
- Paper 1.21.5 build 114 counts removed long-array length prefixes in its section-buffer allocation, leaving a precise zero tail
- Legacy respawn keep-data is a two-bit byte, not the boolean declared by an upstream schema
- Legacy optional global-position metadata contains both a dimension identifier and a packed position
- Set-slot cursor/player-inventory container IDs remain signed bytes through protocol 767 (1.21.1)

The relevant real server serializers were inspected to verify protocol facts. Independent synthetic fixtures cover the corrections; no proprietary implementation source, class files or captured world packets are committed. Both padding allowances are strictly version-scoped and size-checked; nonzero or unexplained trailing data remains an error.

[Original findings](docs/validation/interoperability-findings.md) and [protocol provenance](PROVENANCE.md) provide context. Recorded binary hashes identify the exact tested snapshots; later guard/documentation changes do not retroactively change those hashes.

### Reproduce on Linux x86-64

Python 3, curl, tar, a C/Rust build toolchain and Java 21 are prerequisites. The script downloads the pinned official Temurin 25 runtime for 26.2. Other host platforms need appropriate official Java binaries and a portable process/listener checker; the bundled real-server harness is Linux-specific.

```sh
cargo build --examples --features crypto
python3 tools/paper/prepare.py --manifest docs/validation/matrix-download-provenance.json
# Read the Minecraft EULA and linked agreements. Only if you accept them,
# manually change eula=false to eula=true in each selected test server directory.
python3 tools/paper/validate.py  # all releases in the prepared manifest
python3 tools/paper/summarize.py
```

Preparation creates `.rustwire-validation/`, defaults new EULA files to **false**, never starts a server and never accepts terms. Read the [Minecraft EULA](https://www.minecraft.net/en-us/eula) and its linked Microsoft Services Agreement before proceeding. The runner refuses to run without explicit acceptance. Use `--root PATH` during preparation and `RUSTWIRE_VALIDATION_DIR=PATH` for the runner/summarizer to choose another isolated workspace.

For the destructive gameplay scenario, prepare an external validation directory with the same scripts, explicitly accept the terms, and run:

```sh
cargo build --features crypto --example gameplay_probe
python3 tools/paper/validate_gameplay.py --mode bounded-gameplay --validation-dir /absolute/path/to/test-workspace --versions 1.20.1 1.21.1 26.2
```

The gameplay harness creates fresh worlds and requires observed typed events plus server-side action confirmations. It never accepts the EULA automatically. It must not point at a personal or production world.

The runner uses disposable server configuration (1 GiB heap,2 JVM processors, view/simulation distance 2) and snapshots the compiled examples before each group of runs. `RUSTWIRE_PROJECT` can select another checkout; `RUSTWIRE_MIN_SECONDS` defaults to 20. Results go into the isolated validation directory, not the checked-in evidence files. Do not run this against an existing personal server directory; preparation writes test configuration files.

On the original test host, Java could not resolve some external services directly. The preparation script fetched the exact Mojang download URL embedded by Paper into the expected cache and verified its checksum. Optional public-key/version service lookups failed but did not prevent offline testing. This is not evidence of successful online-account authentication.

## Performance observations

Same Linux cloud host, optimized builds, two million operations per case:

| Microbenchmark | Before indexed catalog | After indexed catalog |
|---|---:|---:|
| Three-byte VarInt decoding | 5.70 ns/op | 5.62 ns/op |
| Mixed protocol 776 play/clientbound packet-ID lookup | 105.48 ns/op | 5.34 ns/op |

The lookup workload cycles IDs 0–179, including unknown IDs. `Version::packets`
uses generated state/direction offsets, and ordinary dense packet IDs index
that group directly; sparse legacy IDs fall back to binary search. The VarInt
codec was unchanged between these measurements. The earlier first-run VarInt
observation was 11.30 ns/op, illustrating host/run variability.

Run `cargo bench --bench codec` to repeat. These are microbenchmark observations,
not end-to-end throughput claims or guarantees on other hardware. Network,
chunk and authentication performance still need representative application
benchmarks. Splitting files alone improves maintainability; the indexed lookup
is the actual runtime change.

## Not verified

Live Microsoft/Xbox/Minecraft account authorization, secure-chat signing, every release alias, every gameplay packet, arbitrary plugins/modded servers, other dimensions and production-scale hostile traffic have not been exhaustively tested. The README lists currently untyped gameplay areas explicitly.

## Component hash fixtures

Eight hash tests cover independently executed official HashOps fixtures, supported
component-codec defaults, item-patch derivation, byte-width and UTF-16 preservation,
map ordering/duplicate normalization, malformed shapes and resource limits. The
primitive oracle returned identical results on pinned Paper 1.21.5 and 26.2. These
fixtures are separate from the earlier live gameplay runs: the newer extended runs below independently exercise predicted hashes end-to-end.

The particle/holder increment adds 14 tests and removes the intentional missing
particle-list decoder. The older live-run evidence above is retained unchanged;
its historical raw-fallback observations are not rewritten as newer test results.

The earlier ordinary-component increment added 19 tests. Its added-component coverage
was 50/56 at 766, 50/57 at 767, 56/67 at 768–769, 79/96 at 770–773, 80/104 at
774, 85/110 at 775 and 87/111 at 776. Removed component IDs and caller-supplied
hash representations can still carry every known component ID.

## Extended component and metadata gameplay

Fresh isolated Paper scenarios passed on **1.20.6, 1.21.1, 1.21.3, 1.21.5,
26.1.2 and 26.2**, each observed for at least 45 seconds. All six decoded the
new food, potion/effect, stew, writable/written-book and firework components,
nonempty particle metadata, and full post-respawn metadata without an
Unsupported fallback. The 26.x runs additionally verified actual template
components for use-remainder, projectiles, bundles and optional containers;
26.2 also verified sulfur-cube content.

Modern inventory prediction used a positive/negative control on 1.21.5 and both
26.x releases: changing one expected component hash triggered one server
correction; the following swap using Rustwire-derived hashes triggered zero
corrections. Server-side Inventory queries confirmed the predicted swaps. These
controls test inventory synchronization, not any anti-cheat guarantee.

[Extended results](docs/validation/components-gameplay-results.json) include
exact binaries/artifacts, required observations, command failures, server action
confirmations and the control results. The first older-version attempt used the
wrong written-book command syntax; all three affected versions were rerun after
fixing the harness to use JSON-encoded text components. No library decoder was
changed to hide that harness failure. All final commands passed and all servers
stopped with exit 0. No anti-cheat plugin was present in these scenarios.

Reproduce with `--extended-components` on the gameplay runner. It uses only fresh
disposable worlds and already accepted baseline EULA files, as described above.

The four serverbound movement forms have independent golden bytes across all
14 families, truncation/trailing-data tests, reserved flag and finite-value
validation, and byte-budget checks. They report caller-simulated state; the
codec itself does not implement gravity, collision, input or a client clock.

## Grim compatibility validation

The same final client binary passed all 20 acceptance checks on Paper 1.21.1,
1.21.5, 26.1.2 and 26.2. Stable Grim 2.3.73 was used for the two older targets;
a pinned official 2.3.74-abb95b6 alpha was used for 26.x. There were zero flags
outside each explicit negative-control window and exactly one expected
BadPacketsF flag within it. Server queries confirmed walking, the offhand swap,
two projectiles and stack reduction from eight to six. The player remained
non-OP and in survival, all checks/thresholds remained stock, and every process
exited cleanly.

[Report and limitations](docs/validation/grim-validation.md) and
[compact evidence](docs/validation/grim-results.json) preserve exact artifact,
JVM and client hashes, all acceptance criteria, diagnostic failures and earlier
passing snapshots. No earlier binary's pass is counted in the final matrix.
The invalid-sprint control establishes active packet checks; it is not a
standalone invalid-trajectory test of movement prediction. No graphical vanilla
client or account-backed authentication was exercised.

The probe has **15** additional Rust unit tests, including real TCP frame order,
a coalesced teleport/ping sequence, bounded-channel cancellation, physics
initialization and inventory predictions. They pass with default, no-default,
all features and Rust 1.88. The harness has **13** offline Python acceptance
regressions. CI now runs the probe tests on all three OS targets and the Python
checks in its Linux quality job.

The investigation also corrected the protocol-776 spectator/item-action tail
against official client and server registrations (69 serverbound entries), and
independent Java fixtures confirmed ISO-8859-1 encoding for session-hash server
IDs. Live Microsoft/Minecraft account authentication remains unverified.

## Offline chat-state API oracle

Eight additional Rust tests validate the fixed acknowledgement window and
packed-signature cache. An original Java harness executed the corresponding
official APIs on all fourteen cached release artifacts; [fixture outputs and
hashes](docs/validation/chat-state-oracle.json) record the result. This verifies
tracking, checksum and cache semantics without an account, not live secure-chat
signing or signature trust. Full core tests and strict lint/docs checks passed
on stable and the core tests passed on Rust 1.88 for this increment.

## Player and entity-state codecs

Twenty-two additional tests cover player-list actions/removal, equipment,
attributes/modifiers and status effects across all fourteen protocol families.
They include independent golden bytes, every truncated prefix, trailing bytes,
shared budgets, malformed actions/identifiers, canonical duplicate identities,
Connection dispatch/raw preservation and 10,752 bounded deterministic mutations.
Isolated full suites, strict clippy/rustdoc/format checks and Rust 1.88 passed.
Real-server evidence for these new packet APIs is recorded separately when run;
the tests do not authenticate profile keys or resolve numeric registry identities.

## Remaining known component layouts

Thirty-two new tests cover consumables, equippable/combat payloads, profiles,
registry holders, trims, instruments, banners and adventure-mode predicates.
They exercise independent golden bytes, all truncated prefixes, atomic failures,
version boundaries, recursive exact predicates and shared packet/collection/NBT
budgets. All known outer vanilla component layouts are classified as implemented:
56 at 766, 57 at 767, 67 at 768–769, 96 at 770–773, 104 at 774, 110 at 775 and
111 at 776. Unknown IDs still fail closed. Registry-aware partial predicate
payloads genuinely use NBT on the wire; their game-specific semantics are not
validated or replaced with opaque byte blobs.

Full no-default/all-feature suites, strict all-target clippy, generator checks
and an initial Rust 1.88 library check passed before publication. Existing
persistent component-hash coverage is unchanged. Fresh live-server evidence for
this increment is separate from the earlier six-version component checkpoint.

## Command-tree and suggestion codecs

Fifteen command-codec tests cover 761 parser/version golden combinations, all
truncated prefixes and trailing bytes, invalid flags/references, aggregate
budgets, tooltip format changes, bounded mutations and a 20,000-node iterative
graph. Valid mixed child/redirect cycles are preserved; invalid dependency cycles
within either relation are rejected. Two additional tests cover semantic dispatch
and malformed-known-packet errors. Focused tests pass on stable and Rust 1.88
with and without features; the isolated full suites, strict clippy and rustdoc
checks pass. This increment has fixture/oracle evidence, not a live command
execution claim.

## World effects and sounds

Fifteen new codec tests cover particles, all explosion-layout boundaries,
positional/entity sounds, stop-sound filters and world events across all fourteen
protocols. Independent golden bodies, every-prefix truncation, malformed
discriminants, aggregate byte/collection/item/NBT/depth limits, weight overflow
and bounded mutations pass. Two additional dispatch regressions keep malformed
known payloads as errors. Full isolated stable suites, strict clippy/rustdoc and
focused Rust 1.88 checks passed. These are wire-codec tests; they do not simulate
explosion physics or establish new real-server particle/explosion coverage.

## Signed-chat wire envelopes and signing input

Seven additional tests cover signed-message/command/session envelope bytes and
packet IDs across all fourteen families, argument and acknowledgement limits,
UTF-16/byte budgets, exact packet-budget boundaries and provider-error propagation.
An original Java harness captured official canonical signing input from all
fourteen prepared release APIs: empty content, UTF-8 text with a supplementary
character, negative milliseconds and a previous signature, and a positive
sub-second timestamp. [Hashes and oracle results](docs/validation/chat-signing-oracle.json)
record the API executions; the Rust fixtures match them exactly.

Full isolated stable and Rust 1.88 all-feature suites, no-default tests, strict
clippy/rustdoc and formatting pass. No private key, certificate service, account,
cryptographic provider or live signed-chat exchange was exercised. The callback
API delegates cryptography to an application-owned provider rather than adding
private RSA operations to Rustwire's existing dependency.
