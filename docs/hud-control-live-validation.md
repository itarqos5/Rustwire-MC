# Bounded HUD and world-control live validation

The optional `hud_control_probe` and `validate_hud_controls.py` pair exercise
real incoming packets on disposable, loopback-only, offline Paper servers. This
is receive-side protocol validation, not a rendered client or a physics model.

## Recorded result

The 2026-10-04 [report](validation/hud-control-live-results.json) passed on pinned
Paper 1.20.1 build 196, 1.21.1 build 133 and 26.2 build 129. One binary produced
13, 22 and 25 byte-identical incoming-payload roundtrips (60 total), with all
required values, ordered lifecycle/tick transitions and cleanup gates passing.
The report's library baseline is `243e390dc0605aaf287766ab6a6e43e57a6932fb`;
probe/harness additions are separately hash-pinned. See [VALIDATION](../VALIDATION.md#fresh-hudcontrol-live-replay-2026-10-04)
for local regression counts and exact limitations.

## Scenario and gates

The console fixture sets a title, subtitle and action bar; custom title timing;
clear and reset; level 12 with 3 experience points; a fixed facing position; and
a death/respawn cycle. Protocol 765+ also changes the tick rate, freezes, steps
seven ticks, unfreezes and restores 20 TPS. Protocol 768+ adds absolute rotation.

Acceptance requires exact decoded values, including the experience bar's f32
bits, and byte-identical received-body re-encoding for every required packet
family. Baseline/startup observations cannot substitute for scenario values.
Tick transitions must occur in order. Death receipt, respawn request, received
respawn packet, subsequent acknowledged position and finish must occur in order.
The terminal roundtrip count must match the actual transcript rows. Missing,
wrong, duplicate, reordered or malformed required lifecycle markers fail closed.
The client additionally refuses to finish before post-respawn readiness.

The harness requires verified Paper artifact hashes, prior explicit EULA
acceptance, an exclusively loopback listener, successful command execution,
complete fixture submission, zero client/server exit status and a closed
listener after shutdown. Client and server cleanup are attempted independently.
All worlds are newly created outside the repository. No plugin is installed,
player is granted OP, account is logged in, or anti-cheat configuration changed.

## Reproduce

Python 3, the supported Rust toolchain and Java are development-only requirements.
For the pinned three-version set, first run the official-artifact preparer:

```sh
python3 tools/paper/prepare.py --root ../rustwire-server-validation
```

Read and explicitly accept the applicable Minecraft terms yourself before
changing each prepared `eula.txt` from `false` to `true`. The scripts never accept
terms for you. The prepared Java 25 is used for 26.x, system Java for older Paper.

From the exact checkout to test, build a source/binary receipt before the run:

```sh
SOURCE_COMMIT=$(git rev-parse HEAD)
python3 tools/paper/build_hud_probe.py \
  --source-root . --source-commit "$SOURCE_COMMIT" \
  --target-dir target --output ../rustwire-hud-build-receipt.json
python3 tools/paper/validate_hud_controls.py \
  --versions 1.20.1 1.21.1 26.2 \
  --validation-dir ../rustwire-server-validation \
  --source-root . --source-commit "$SOURCE_COMMIT" \
  --client target/debug/examples/hud_control_probe \
  --build-receipt ../rustwire-hud-build-receipt.json
```

If Cargo uses a custom target, use the actual executable path printed by the
builder for `--client`. The builder selects Cargo's reported compiler artifact,
not an assumed host-target path. It records the exact command, toolchain,
source hashes and binary hash, and rejects source changes during compilation.
The runner independently verifies the receipt. This is build provenance within
the local test environment, not a cryptographic attestation against a malicious
build toolchain or forged receipt.

The library/Cargo snapshot is checked against `--source-commit`. The separately
hash-pinned probe, runner, builder and acceptance tests may be new relative to
that library baseline; the report explicitly distinguishes those identities.
Freeze those files until a run finishes. The same copied binary is used for all
selected versions. The latest report path is written below the validation root
at `hud-controls/latest-run.txt`. Keep game binaries, worlds and runtime caches
out of the repository.

Offline negative controls are available without Java or network access:

```sh
python3 -m unittest discover -s tools/paper -p test_validate_hud_controls.py -v
```

## Explicit boundaries

Only deliberately value-gated packet families count as live coverage. Extra
observed packet roundtrips do not establish their full semantics. Book opening,
enter/end-combat notifications, entity-target facing, relative rotation,
projectile/vehicle behavior and other world-control layouts are not covered by
this scenario. It does not establish rendering, title timing in a real UI,
physics correctness, all 14 protocol families, live Microsoft authentication,
or a new Grim compatibility result.
