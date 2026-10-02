# Validation

Date: 2026-10-02. The results distinguish fixture/mock evidence from actual server interoperability.

## Automated checks

- 340 core tests with all features; 301 applicable core tests without default features
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

## Expanded fourteen-family gameplay matrix

A final replay passed on all fourteen pinned representative releases using one
frozen binary, SHA-256
`bf233954114d58ddd247cfb4dba27932bb49fcefbf9b55c04ceef84496d30505`.
Its library source is exactly `11674342d6aaa07bf2cb1ed5b2c504d07babce9f`,
plus the recorded probe source files. All 259 version-specific decoded-value
checks passed. Each client ran for 55.053–55.935 seconds. All servers, primary
clients and roster peers exited zero; no listeners or test processes remained.

Checks include matching peer UUID add/remove, nonempty damaged-helmet equipment,
attribute modifier identity/operation/value, effect amplifier/flags/removal,
and version-appropriate predicate, trim, banner, profile, inline instrument,
jukebox, consumption/equipment and template component values. Server-side
queries confirm movement and selected slot. The raw-packet gate covers eleven
explicit scenario packet names; other intentionally untyped packets are not
misrepresented as semantic coverage. No Grim plugin is installed in this matrix.

[Compact results](docs/validation/expanded-gameplay-results.json) include source
and artifact hashes, every required value check, example decoded values, command
counts, exits, the earlier diagnostic runs and the movement smoke test. The first
run exposed a genuine equippable component bug: EquipmentSlot's stream IDs differ
from its Java ordinals. The library fix has a 70-combination independent fixture
and leaves entity-equipment ordinals unchanged. Initial command syntax/default
item assumptions were corrected in the harness, without weakening value checks.
A later 1.20.2 run caught an initial-teleport race in the test client; post-readiness
signaling and bounded coordinate polling fixed it, followed by a passing targeted
smoke test and this full same-binary replay. The earlier failure is retained.

The harness now has eighteen offline acceptance regressions, and CI discovers
both gameplay and Grim suites: **31 Python tests** total. The later command/world-effect live run is recorded separately below; signing
input remains API-oracle tested. None is retroactively attributed to this frozen
live binary.

Reproduce with the already accepted, isolated baseline and
`validate_gameplay.py --mode bounded-gameplay --expanded-state --extended-components --min-seconds 55` (all fourteen
versions by default). Use `--source-commit` when running an archived snapshot;
the harness records actual source hashes in addition to that explicit label.

## Live command and world-effect matrix

All fourteen pinned releases passed a separate frozen-source run based on
`1944453662b44e2b749d2e12f26e54477fba5c27`, using client SHA-256
`23064bf2b7f1aa6a804c0e823dee08fdf560e65d165b001e268aad2bee9e18c0`.
It checked 381 version-specific value groups and 216 byte-exact decode/re-encode
observations across seven packet surfaces: command trees, command suggestions,
world particles, explosions, positional sound, stop-sound and world events.
There were zero decode, raw-gate, value or console failures, and all processes
exited zero with no listeners remaining. The source snapshot was checked against
the exact commit, rather than inferred from a working-tree label.

[Results](docs/validation/surface-results.json) retain hashes, required values,
roundtrip counts, boundary attempts and explicit limits. Six corrected boundary
smokes passed before the final matrix. Initial failures were harness expectations,
not codec failures: force-particle requests have always-show false, packed colors
include opaque alpha, legacy item particles use count one, default TNT uses
Destroy, and the ordinary targets parser stops advertising AskServer at 769.
The original observations are preserved. No production codec changed for the run.

EntitySound remains independently fixture-checked, without a natural live trigger
in this scenario: pig damage/death emitted positional sound packets. The matrix
also does not cover every command parser or tooltip value, arbitrary particle
parameters, rendering, game physics, account authentication or Grim behavior.
No OP privileges or plugins were used.

Thirteen new offline acceptance tests bring CI's Python total to **44**. The
existing all-three-platform, minimum-Rust and quality jobs remain active.
Run `validate_surfaces.py --help` for its bounded reproduction options; prepare
and explicitly accept the same isolated baseline as the earlier scenarios.

The separate RSA response hardening keeps temporary secrets in a zeroizing guard
through failure paths and rejects PKCS#1-v1.5-oversized challenges before random
secret generation. A new 1024/2048/4096-bit capacity-boundary regression passes
alongside the existing independent encryption/decryption fixtures. This does not
change the documented upstream private-RSA advisory assessment.

## Full-chunk compound-root validation

A two-test regression increment checks every non-compound root kind against all
applicable protocol families. Heightmaps through protocol 769 require a non-null
compound; embedded chunk block-entity data permits either a compound or TAG_End
in every supported family. Standalone block-entity updates have a different
nullability boundary and are not used to infer this rule. Both encoding and
decoding now reject scalar/list/array roots. [Inspected serializer facts and
artifact hashes](docs/validation/chunk-nbt-roots.json) record the independent
all-family check. The exact isolated snapshot passed 273 all-feature and 258
no-default-feature tests, formatting, and strict all-target/all-feature Clippy.

## Streamed chunk updates and contextual dispatch

Fourteen packet tests and six typed-dispatch tests cover all 14 protocol families,
exact boundaries, truncation, malformed roots/masks, aggregate budgets, all
light-section boundary layers, and biome palette transitions. Dimension-dependent
full-chunk/light/biome packets remain raw when no DecodeContext is supplied.
View/simulation controls and standalone block entities decode without that context.
The isolated increment passed 293 full-feature tests on stable and Rust 1.88.0,
278 no-default-feature tests, formatting and strict all-target/all-feature Clippy.

[Independent all-family serializer facts](docs/validation/chunk-update-protocol-facts.json)
record standalone block-entity nullability and the exact Paper 1.21.5 biome-buffer
zero-tail compatibility rule. Encoding emits canonical compact palettes. These
fixture/source checks are separate from the earlier initial-chunk server matrix;
fresh streamed-update live results are recorded below. No world cache, numeric
registry-name database or gameplay light simulation is implied.

## Extended persistent component hashes

Eight additional tests verify 17 new component names with version-specific
subsets: custom_name, item_name, lore, food, use_cooldown, weapon, use_effects,
attack_range, swing_animation, firework_explosion, fireworks, lodestone_tracker,
writable_book_content, written_book_content, break_sound, consumable and
death_protection. Inline sounds and registry-independent consumption effects are
supported; numeric registry references, translated text, click/hover events and
unknown style forms remain explicitly unsupported. Literal text is normalized
semantically, including default omissions, siblings and Java UTF-16 units.

The original [Java oracle](tools/paper/ComponentHashExtendedOracle.java) exercises
122 cases against cached official APIs from all seven modern release families.
[Structured results](docs/validation/component-hash-extended-oracle.json) include
artifact hashes, introduction boundaries, defaults, signed byte-width/float
edge cases, filtered books and intentional unsupported diagnostics. This is
independent API evidence, not a live click-acceptance result. Fresh positive and
negative loopback inventory controls for this expanded subset are recorded below.

The coherent snapshot passed 301 all-feature tests on stable and Rust 1.88.0,
286 no-default-feature tests, formatting and strict all-target/all-feature Clippy.
No runtime dependency was added. HashOps CRC32C remains a synchronization
checksum, not a cryptographic integrity or authentication mechanism.

## Live streamed-world updates

The same frozen production source `fe7db1fb60ff3a8ceec634c4d3b03e74cdc4bc4b`
was exercised on all 14 pinned release families. Full chunks, standalone block
entities, biome replacements, view center, view distance and simulation distance
passed on every family. Standalone UpdateLight passed on 13: Paper 1.20.4 emitted
no such packet in the bounded scenario, so its light value/context/roundtrip
checks remain explicitly unverified. This is an observation gap, not evidence
of a decoder failure or a claim that all streamed surfaces passed everywhere.

There were 277/280 required check groups, 3,421 received-payload roundtrips
(2,968 byte-exact and 453 restricted version-specific canonicalizations), and
zero decode/console errors. Exact assertions include registry-derived dimension
bounds, packed light-mask/nibble semantics, 1,536 desert biome entries followed
by exactly 16 plains replacements, embedded/standalone red/glowing/waxed signs,
and view controls. Deliberate luminous transitions were not value-observed;
non-overworld dimensions and null NBT sentinels remain fixture-only here.

The [compact report](docs/validation/chunk-update-results.json) preserves each
distinct observation plus per-version multiplicities, all source/artifact/log
hashes, original run IDs and transparent cleanup-only reassessments for generated
Paper remap caches. The full report hash is retained; no game binaries, worlds
or proprietary implementation text are distributed. All clients/servers exited
zero, listeners closed, OP lists remained empty and approved EULA copies were
unchanged. The evidence gate distinguishes generated caches from installed plugins.

Reproduce with `tools/paper/validate_chunk_updates.py --client PATH --source-commit
COMMIT --source-root FROZEN_SOURCE`, optionally selecting `--versions`. The
source-root must match the attributed library commit and include the current
probe/harness; existing approved baseline worlds and pinned artifacts are required.
Use `summarize_chunk_updates.py --help` for merge/compaction commands. The added
14 acceptance and six compaction tests bring this checkpoint to 64 Python
checks. All 16 example unit tests passed in no-default/all-feature modes and on
Rust 1.88.0; CI now runs every example test and every `test_*.py` harness test.

## Live expanded persistent-hash controls

All seven modern families (770–776) passed 149 actual-server inventory clicks
using frozen production source `fe7db1fb60ff3a8ceec634c4d3b03e74cdc4bc4b`.
The 142 correct predictions produced zero corrections; seven deliberately
wrong styled-name hashes each produced exactly one authoritative offhand update
matching the expected item. Seven following recovery swaps succeeded. These
recoveries are included in the 142 correct predictions, not additional cases.
Every received component hash matched the independent official API oracle, and
every action passed a server-side inventory predicate.

The [detailed report](docs/validation/extended-item-hash-report.md) and
[149-case evidence](docs/validation/extended-item-hash-results.json) retain
commands, per-case values, source/binary/log hashes and the two retained run IDs.
An independent audit re-reads raw logs rather than trusting stored success
booleans. Sixteen Python tests reject missing, fabricated, mismatched or
duplicated success/correction/cleanup evidence. With the streamed-world tests,
this checkpoint passes 80 Python harness tests and 20 example tests (stable
no-default/all-feature and Rust 1.88.0). Core tests remain 301/286.

The fresh isolated servers used survival mode, no operators and unchanged
approved EULA copies; all processes exited zero and listeners closed. No
production decoder/hash correction was needed. Registry-dependent holders,
translated/click/hover text, other inventories/click modes, online accounts
and third-party anti-cheat behavior are outside this particular scenario.

## Bidirectional registry and tag codecs

The existing tag decoder now has encode/packet helpers, explicit state checks,
identifier validation and conservative preallocation checks. The dedicated tags
module preserves the original common-module public paths. Wire order, duplicate
keys and repeated membership IDs remain intact; lookup helpers implement the
release readers' last-key-wins semantics and default namespace aliases. Optional
resolution uses only a supplied Registry, leaving unknown IDs unresolved.

Modern RegistryData now has state-checked clientbound packet helpers and a
packet-wide NBT-node budget shared by its optional entry payloads. Arbitrary
non-End modern NBT roots remain legal, including scalar Int roots; they are not
incorrectly constrained to compounds. Known-pack omissions remain unresolved.

Eight regression tests plus the [original API oracle](tools/paper/RegistryTagsOracle.java)
verify all 14 families. [Facts and cached artifact hashes](docs/validation/registry-tags-oracle.json)
cover tag state availability, duplicate keys, identifier boundaries and registry
wire representations. This increment passed 309 all-feature and 294 no-default
tests on stable and Rust1.88.0, strict stable Clippy and Rustdoc. It is fixture/API
conformance evidence; the earlier real-server results identify their own exact
source snapshots. No static block/item registry database was added.

## Packed-palette validation benchmark

Palette validation now scans packed words directly instead of calling a
division/remainder-based getter for every entry. Direct storage already has a
validated bit width and exact length; a complete indirect index domain likewise
cannot contain an out-of-range index. Those redundant index scans are skipped.
Incomplete palettes still validate every meaningful index, including final
partial words, while ignoring only the same unused padding bits as before.

Four independent regression tests compare the optimized decision with the public
per-entry accessor over deterministic mutations, word edges, partial final words,
full palette domains, maximum direct IDs and invalid palette IDs. Integrated
checks passed 313 full-feature tests on stable and Rust1.88.0, 298 no-default
tests, strict Clippy and an MSRV compile of the new benchmark.

The new dependency-free `protocol_workloads` benchmark constructs synthetic
protocol776 fixtures, checks roundtrips before timing, and includes allocation
and destruction in each codec measurement. Three alternating baseline/optimized
executions each took five timed samples after eight warm-up calls. Baseline
production source was `fe7db1fb60ff3a8ceec634c4d3b03e74cdc4bc4b`; the isolated
comparison changed only palette index validation. Median of the three per-run
medians, microseconds per operation:

| Workload | Baseline | Word-scan validation |
|---|---:|---:|
| Mixed-palette 24-section chunk decode, 105,033-byte body | 124.75 | 51.69 |
| Same chunk encode | 137.96 | 66.59 |
| Nine-chunk biome update decode, 2,683 bytes | 35.14 | 19.55 |
| Light decode control, 55,394 bytes | 2.27 | 2.34 |
| Styled-name hash control | 0.773 | 0.813 |

[All six runs, source/binary hashes and host/compiler metadata](docs/validation/palette-performance.json)
are retained, including the unchanged controls and host variability. The local
server lock was held and its loopback listener absent; the entire cloud machine
was not claimed to be otherwise idle. These are synthetic in-memory codec
observations, not a claim about real-world chunk distributions, framed/compressed
network throughput, authentication or other machines. Reproduce the current
workloads with `cargo bench --locked --no-default-features --bench protocol_workloads`.

## Legacy registry-root correction

A follow-up oracle invoked the actual 763 Join Game and 764–765 configuration
registry constructors for all 13 NBT root kinds. Only a non-null compound was
accepted. Rustwire previously allowed other non-End roots at those boundaries;
the generic legacy RegistryData read/encode paths and the separate 763 Join Game
reader now reject them. Modern 766+ optional registry-entry NBT remains generic,
including scalar/list/array roots.

[Original constructor oracle](tools/paper/LegacyRegistryRootOracle.java),
[39-case facts and artifact hashes](docs/validation/legacy-registry-roots.json),
and three independent Rust regressions document the boundary. Integrated checks
passed 316 all-feature tests on stable/Rust1.88.0 and 301 no-default tests, plus
strict Clippy and formatting. This is a malformed-input correction, with no
change to valid packet representations.

## Offline authentication HTTP exchanges

Twenty-three new tests perform actual HTTP exchanges with in-process loopback
servers and exclusively synthetic tokens. They cover device-code requests, form
encoding, pending/slow-down/denial/expiry, persistent polling delay, cancellation,
refresh rotation, the Xbox/XSTS/Minecraft request chain, profile parsing and
session-join responses. Other cases cover status errors, redirects, timeouts,
truncation, malformed/chunked/oversized bodies and credential-redacted errors.

This exposed five request-contract/validation gaps, now corrected: the documented
Xbox/XSTS contract header, UUID hyphen placement, malformed supplied refresh
tokens, empty matching Xbox user hashes, and an overlong positive server hash.
Valid absent/rotated refresh tokens, matching nonempty identities and up-to-40-hex-
digit signed server hashes remain accepted.

The loopback override is private and compiled only for unit tests. Production
URLs remain fixed HTTPS endpoints with redirect refusal, 30-second deadlines,
TLS verification and bounded bodies. No dependency or public endpoint override
was added. The integrated snapshot passed 339 all-feature tests on stable and
Rust1.88.0, 301 no-default tests, 20 example tests, strict Clippy/Rustdoc and
formatting. The 80 Python harness tests also pass. These checks do not authenticate
an eligible account or prove application/service approval; live Microsoft/Xbox/
Minecraft account interoperability remains unverified.

## Authentication mock socket portability

The first authentication-suite CI run failed on macOS because accepted sockets
inherited the mock listener's nonblocking mode. The test server now explicitly
restores blocking mode before applying read/write timeouts. A new regression
forces nonblocking mode on every platform, verifies an initial WouldBlock, then
requires a delayed HTTP request to be read successfully after configuration.
No production authentication behavior, timeout or rejection gate was weakened.

[The original failed run](https://github.com/itarqos5/Rustwire-MC/actions/runs/37073546148)
is retained. This portability increment passed 340 all-feature tests locally on
stable/Rust1.88.0 and strict Clippy. The exact replacement commit's multi-platform
CI must still be checked before treating that external failure as resolved.
