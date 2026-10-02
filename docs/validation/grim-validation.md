# Isolated Grim compatibility scenario

This is a bounded **headless Rustwire client** test against ordinary native
Paper servers running official Grim. It is not a graphical vanilla-client
comparison, a general anti-cheat bypass test, or proof of complete Minecraft
physics compatibility. Results must name the exact client binary, Paper build,
Grim artifact, scenario and observed flags.

Movement is limited to grounded walking and deceleration on a known stone
floor. The wind-charge scenario covers legitimate item use and projectile
creation; upward throws do not test explosion knockback or wind-charge jumping.
The probe rejects unexpected active-phase teleports and nonzero player
knockback rather than pretending to simulate unsupported physics.

## Pinned official artifacts

[Grim's official repository](https://github.com/GrimAnticheat/Grim) links
[Modrinth](https://modrinth.com/plugin/grimac) as its recommended distribution.
The [download manifest](grim-download-provenance.json) records official CDN
URLs, publisher SHA-512/SHA-1, local SHA-256, supported targets, license and
embedded build/source metadata.

| Native Paper/client target | Paper build/channel | Grim | Grim channel |
|---|---|---|---|
| 1.21.1 | 133, stable | 2.3.73 | release |
| 1.21.5 | 114, alpha | 2.3.73 | release |
| 26.1.2 | 74, stable | 2.3.74-abb95b6 | alpha |
| 26.2 | 129, stable | 2.3.74-abb95b6 | alpha |

As verified on 2026-10-02, latest listed stable Grim 2.3.73 supports versions
through 1.21.11. The two 26.x runs therefore require the explicitly labeled,
pinned official alpha. No additional plugin, ViaVersion, proxy or permission
manager is installed. PacketEvents is shaded inside these official Grim artifacts.

Grim is distributed under GPL-3.0-or-later according to its Modrinth metadata;
its official source includes the GPLv3 license text. No Grim implementation is
copied into Rustwire. Checksums and embedded commits establish the documented
artifact/source relationship; no independent reproducible-build or signature
attestation is claimed.

## Recorded results

All four targets passed all 20 acceptance checks on 2026-10-02 using the same
release client binary, SHA-256
`77d87d2a49fbdd09bfc98bf8d8b3ef38523bb45e4907f47edc5130aa771c93be`.
Each run observed:

* Zero flags during startup, setup, settling, walking, inventory, wind use and
  the final legitimate tail
* Exactly one expected `BadPacketsF` flag in the separate negative-control window
* Server-confirmed movement of +4.3134259902859675 Z, an eight-item offhand swap,
  two wind-charge projectiles and a remaining offhand stack of six
* An ordinary survival player, empty OP/permission state, and unchanged default
  check, setback and punishment settings
* Clean server and client exits, followed by no remaining port-25565 listener

[Compact evidence](grim-results.json) records exact Paper/Grim/client/JVM hashes,
version output, every acceptance check, action observations, flag timestamps,
raw-log hashes and run IDs. Earlier failed attempts and intermediate passing
client snapshots remain explicitly separate. The final matrix does not borrow
passes from earlier binaries.

The older servers used Debian OpenJDK 21.0.12.1+1; both 26.x servers used Temurin
25.0.4.1+1 LTS. Optional JVM lookups for Mojang public keys and Paper/Grim updates
failed on this host. Those failures remain in the raw logs; online authentication
and external metadata-service availability are not claimed by these local tests.

## Safety and configuration

The runner creates a new disposable superflat world for each version and run.
It copies only already accepted EULA text and links existing immutable boot
caches from the isolated Paper baseline. It never copies player/world data,
accepts legal terms, operates production worlds, grants OP, or grants exemptions.

The server must bind exclusively to 127.0.0.1. Offline mode is used only on this
loopback listener; RCON, query and JMX are disabled. The actual socket is checked
from Linux `/proc/net/tcp*` before the client connects. All test runners share
the baseline workspace's exclusive `one-server.lock`.

The test player is `Rustwire`, in survival with an empty OP list and no permission
grants. Flight permission remains false. The only Grim configuration change is
`verbose.print-to-console: true`; every check, prediction threshold, setback,
packet cancellation rule and punishment threshold remains stock. The runner
checks the config/punishment hashes again after shutdown.

This logging change is necessary: default Simulation alerts start at 100
violations, so absence of normal alerts would not prove absence of flags.
Official `PunishmentManager` sends verbose output before those thresholds.
Console-only `grim profile Rustwire` establishes tracking and
`grim consoledebug Rustwire` records predicted/actual movement and offset.
The runner does not use `grim log`, which uploads diagnostics, or `grim reload`,
which resets violations. It does not grant any bypass permission.

## Reproduce

First prepare the checksum-pinned baseline using the existing Paper preparation
instructions and explicitly accept the Minecraft EULA and linked agreements.
Both 26.x servers require the pinned Java 25 runtime; earlier targets use Java
21. The runner expects Python 3, curl, a built client and Linux socket inspection.

```sh
cargo build --release --example grim_probe

# Optional: artifact/configuration preparation only; no server is launched.
python3 tools/paper/validate_grim.py --mode bounded-grim --prepare-only \
  --validation-dir /absolute/path/to/rustwire-server-validation \
  --grim-dir /absolute/path/to/rustwire-grim-validation

# Actual bounded runs, after reviewing the disposable workspace paths.
python3 tools/paper/validate_grim.py --mode bounded-grim \
  --validation-dir /absolute/path/to/rustwire-server-validation \
  --grim-dir /absolute/path/to/rustwire-grim-validation \
  --client target/release/examples/grim_probe \
  --versions 1.21.1 1.21.5 26.2 26.1.2
```

`--client PATH` selects another compatible headless executable. The runner
snapshots the executable before launch and records its SHA-256. It downloads
only absent pinned official Grim artifacts and verifies all publisher hashes;
an existing mismatched artifact is rejected. No mutable “latest” download is
used. Runtime artifacts and raw logs remain outside the repository.

The client contract is `grim_probe 127.0.0.1 25565 VERSION`. It emits
`GRIM_PROBE_READY` after acknowledging its initial teleport. It consumes the
following system-chat markers and reports `GRIM_ACTION phase=...` after sending
the corresponding action packets:

1. `GRIM_VALID_BEGIN`: ordinary walking followed by natural deceleration;
   `phase=walk`. The runner allows five seconds and queries server position.
2. `GRIM_INVENTORY`: a valid player-inventory offhand swap; `phase=inventory`.
   The server must observe all eight wind charges in offhand slot -106.
   On 1.21.5 and newer, this is queried through `equipment.offhand` and normalized
   to that slot identity in the result; the old `Inventory` list no longer stores
   player offhand equipment. This is documented in the official
   [1.21.5 release notes](https://feedback.minecraft.net/hc/en-us/articles/35298208390797-Minecraft-Java-Edition-1-21-5-Spring-to-Life).
   Opening the player inventory is a client-local screen transition in these
   releases, not a claimed serverbound inventory-open packet.
3. `GRIM_WIND`: two upward offhand throws, sequence numbers 0 then 1 and at least
   600 ms apart; `phase=wind`. The runner checks actual wind-charge entities and
   the server's remaining stack of six.
4. `GRIM_VALID_END`: stop the legitimate scenario and allow a two-second tail
5. `GRIM_NEGATIVE`: separately labeled bounded duplicate sprint-state control;
   `phase=negative`. It must trigger stock BadPacketsF, after which state is reset.
6. `GRIM_DONE`: cleanly finish and exit successfully

Before these phases the console prepares a 33-by-33 stone floor at y=-61,
clears seven blocks of headroom, teleports the player to (0.5, -60, 0.5), waits
two seconds, and places eight wind charges in hotbar slot zero. The player never
receives console privileges. All actions are confined to this disposable world.

## Acceptance and evidence

`grim-results.json` and per-version `grim-result.json` contain:

* Exact server/client commands and artifact hashes
* Loaded Grim version, tracked player/profile, survival and console-debug evidence
* Every console command with send timestamp and following server output
* Monotonic and UTC server/client line timelines, plus original raw log files
* Every parsed verbose flag and its setup, legitimate, tail or negative-control phase
* Server-confirmed displacement, item swap, consumed charges and spawned projectiles
* Individual acceptance booleans and explicit failed checks

A passing result requires all requested actions, clean process exits, unchanged
check configuration, no flags outside the explicitly labeled negative-control
window (including startup, setup, settling and any late flags), and the expected BadPacketsF
negative flag. The packet-state negative control proves packet checks are active;
it does **not** independently prove movement Simulation detects an invalid
trajectory. Report that distinction rather than calling this an invalid-movement
control or a full movement-check negative test.

Setup flags fail acceptance even when the later action window is clean.
Decode/session, tick timing/order, client physics, and plugin/runtime
failures should be diagnosed separately. Correct client behavior or codecs;
never make a test pass by weakening checks, thresholds or exemptions. A later
code change requires a new run with a new recorded client hash.

### Diagnostic findings retained

Six failed diagnostic runs are retained in the compact evidence:

* Probe physics: ground acceleration was initially applied before floor-contact
  settling after a teleport. Normal gravity/collision ticks fixed the mismatch
* Probe action ordering: a redundant selected-slot update followed a movement
  heartbeat. Actual vanilla action ordering and changed-slot semantics fixed it
* Probe response ordering: a later automatic Pong overtook an earlier teleport
  response. A completion barrier now preserves ACK, position/rotation, then Pong;
  a coalesced TCP regression and official-client inspection support this fix
* Library catalog: the upstream protocol-776 schema misordered spectator action
  after swing and omitted the separate UUID-target spectator teleport packet,
  shifting the serverbound tail. Official 26.2
  registration establishes `use_item_on=0x42` and `use_item=0x43`; the catalog was
  corrected rather than changing the anti-cheat
* Harness observation: 1.21.5 offhand NBT moved to `equipment.offhand`
* Harness observation: newer console entity-count output changed punctuation and
  capitalization; both observed formats are now covered

The diagnostic categories matter: these were three limited test-client issues,
one genuine library catalog defect, and two observation-parser issues. No Grim
check, threshold, exemption or punishment was weakened to obtain a pass.

Thirteen offline Python tests exercise acceptance and parser failures, including
unexpected flags in setup or after the negative window, missing negative-control
evidence, changed settings, OP/permission state, mismatched gameplay evidence,
and PacketEvents listener exceptions:

```sh
python3 -m unittest discover -s tools/paper -p test_validate_grim.py -v
```

## Official references

* [Stable release](https://github.com/GrimAnticheat/Grim/releases/tag/v2.3.73)
* [Stable artifact](https://modrinth.com/plugin/grimac/version/1FIGlM6Q)
* [Pinned alpha artifact](https://modrinth.com/plugin/grimac/version/YJEwvStg)
* [Console commands](https://github.com/GrimAnticheat/Grim/wiki/Commands)
* [Permissions and exemptions](https://github.com/GrimAnticheat/Grim/wiki/Permissions)
