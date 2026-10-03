# Frozen connection regression: 01bf0a0

All 14 protocol families (763–776) passed on commit `01bf0a0739199949cd4aa584738aaba0e49fadc3`: 1,110 real full chunks, 125 keepalive acknowledgements, 14 position acknowledgements, 208 registry packets and 13 configuration completions. Every login lasted at least 20 seconds. All 7,526 tags / 90,843 memberships round-tripped byte-for-byte. Full chunks round-tripped semantically; known Paper padding in protocols 763 and 770 is canonicalized.

This is initial connection/full-chunk evidence. It does **not** close the earlier 1.20.4 standalone UpdateLight observation gap or exercise account/online-mode authentication.

## Check the published evidence without servers

From this folder:

```sh
sha256sum -c SHA256SUMS
python3 verify.py
python3 test_acceptance.py -v
```

`verify.py` validates the exact fixture/report hashes, all 14 families, original client exits and binary attribution, transcript-derived counters, duration, status/ping, dimension context, roundtrips, EULA/loopback/cleanup records, and the strict plugin-classifier evidence. Tests reject altered counts, omitted observations, failed clients, broader cleanup exceptions, and changed source/fixtures.

The source probe and Cargo fixtures are byte-identical to the historical run. The `.fixture` suffix keeps this validation fixture outside the main library's single-crate structure. No binaries, server jars, game assets or worlds are included. Historical paths in `historical/` are retained only as original evidence.

## Reproduce in a fresh sibling run directory

Prerequisites: Linux with `/proc/net/tcp{,6}`, Python 3.11+, Cargo/Rust, Java 21 and Java 25; cached dependencies; and the **already approved** isolated baseline layout used by `tools/paper/validate_gameplay.py`:

- `$VALIDATION/downloads/`: pinned Paper jars from the archived provenance manifest
- `$VALIDATION/servers/<version>/`: previously approved `eula.txt`, isolated `server.properties`, immutable `cache/`, `libraries/`, `versions/` caches
- `$VALIDATION/one-server.lock`: shared exclusive server lock
- `$VALIDATION/jdk25/bin/java`, or set `RUSTWIRE_JAVA_25` to a Java 25 executable

The harness never accepts an EULA, downloads assets or installs plugins. It refuses unapproved/non-loopback baselines and verifies each official Paper and cached Mojang jar against the frozen manifest. Only `stop` is sent to the server console. No gameplay, OP, account login or external game-server connection is involved.

Set paths for your checkout, this published folder, and your existing validation cache; then choose an unused run name:

```sh
REPO=$(git rev-parse --show-toplevel)
FIXTURES="$REPO/docs/validation/connection-regression-01bf0a0" # adjust if published elsewhere
VALIDATION="$(dirname "$REPO")/rustwire-server-validation"
RUN="$VALIDATION/connection-regression/reproduce-01bf0a0"
COMMIT=01bf0a0739199949cd4aa584738aaba0e49fadc3

test ! -e "$RUN" || exit 1
mkdir -p "$RUN/source" "$RUN/probe" "$RUN/bin" "$RUN/evidence"
cp -R "$FIXTURES/fixtures" "$RUN/fixtures"
cp "$FIXTURES/run_verified.py" "$FIXTURES/verify.py" "$RUN/"
git -C "$REPO" archive --format=tar "$COMMIT" > "$RUN/source.tar"
tar -xf "$RUN/source.tar" -C "$RUN/source"
cp "$RUN/fixtures/registry_tags_probe.rs" "$RUN/probe/"
cp "$RUN/fixtures/Cargo.toml.fixture" "$RUN/probe/Cargo.toml"
cp "$RUN/fixtures/Cargo.lock.fixture" "$RUN/probe/Cargo.lock"
python3 "$RUN/verify.py" --source-only "$RUN"

export CARGO_TARGET_DIR="$RUN/target"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo build --offline --locked --manifest-path "$RUN/source/Cargo.toml" \
  --features crypto --example offline_chunks --example status
cargo build --offline --locked --manifest-path "$RUN/probe/Cargo.toml"
cp "$RUN/target/debug/examples/offline_chunks" "$RUN/target/debug/examples/status" \
  "$RUN/target/debug/registry_tags_probe" "$RUN/bin/"

export RUSTWIRE_VALIDATION_DIR="$VALIDATION"
# Optional: RUSTWIRE_JAVA_21=/path/to/java21/bin/java
# Optional: RUSTWIRE_JAVA_25=/path/to/java25/bin/java
python3 "$RUN/run_verified.py" > "$RUN/evidence/run.log" 2>&1
python3 "$RUN/verify.py" --run-dir "$RUN"
```

The runner requires the exact archived source and fixture hashes before launching. A unique compact target avoids another checkout's artifacts. Compression and crypto are enabled; auth-service calls are outside this scenario. Historical toolchain details and all binary hashes are retained. New builds record their own hashes: a different build path or compiler need not reproduce the historical binary bytes.

Startup is bounded at 180 seconds, each client at 90 seconds, and each login at a minimum of 20 seconds. Each server is stopped and its listener verified closed before the next family. Existing results/worlds are never overwritten, individual failures are not retried, and failed/incomplete runs cannot receive final acceptance.

## Preserved cleanup amendment

The first run stopped after 1.20.6 solely because its initial harness counted Paper's generated `.paper-remapped/remap-classpath` jar as an installed plugin. The original failed classification is unchanged in `historical/results.json`; only the ten previously untested families were continued. No protocol scenario was rerun.

`strict-cleanup-assessment.json` records the cleanup-only amendment using the already published `tools/paper/validate_chunk_updates.py::audit_plugins` at SHA-256 `00aa559925daafaa568f98d660f8cb6e5d37578825c8cac55f78843733dd6ddf`. That gate requires the exact uppercase mapping-hash jar route, matching reversed mapping, and either a zero-plugin startup log or four exactly empty matching indexes. The 1.20.6 record uses the four-index route. Original reports and all 42 client/server transcripts retain their original hashes.

`run_verified.py` is the finalized fresh-run harness: it uses that published gate plus source preflight and strict acceptance. It was syntax/unit/preflight checked after the historical run, **not live-rerun**. Historical runner fixtures preserve the exact original execution implementations and their narrower/broader classifications for audit.
