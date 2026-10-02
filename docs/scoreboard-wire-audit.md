# Scoreboard wire audit

The original API harnesses invoke independently prepared Paper release APIs for
all 14 supported protocol versions, 763–776. They construct synthetic wire
payloads and inspect decoded fields; no game implementation is included. No
server, client connection, account, or world is started or changed.

The Java harnesses emit observations. The Python driver asserts the expected case
set, acceptance/rejection, enum domains, and selected decoded values. It does not
interpret a successful Java exit as proof that malformed cases were accepted.
`validation/scoreboard-wire-oracle.json` records compact results, exact prepared
server-jar SHA-256 values, pinned schema hashes and packet IDs, source hashes,
and the Java version. Full stdout/stderr stay in the chosen output directory.

## Relevant boundaries

| Protocol | Change |
| --- | --- |
| 763 | Display slot is a signed byte; text components are JSON strings |
| 764 | Display slot becomes a VarInt ID, canonical range 0–18 |
| 765 | Components become anonymous NBT; objective/score number formats, optional score display, and a separate reset-score packet appear |
| 770 | Team visibility and collision strings become VarInt IDs |
| 776 | Team parameters reorder to display, prefix, suffix, visibility, collision, optional color, flags |

Before 776 the team parameter order is display, flags, visibility, collision,
formatting, prefix, suffix. Team parameters occur for mode 0/create and 2/update;
member lists occur for mode 0/create, 3/join, and 4/leave. Mode 1 removes a team.

## Domains that differ from common assumptions

- Display slot 763 retains any signed byte. From 764, IDs outside 0–18 normalize
  to LIST/0 in the Java decoder. Slots 0, 1, 2 are list, sidebar, and below-name;
  slots 3–18 are team-color sidebars
- Team color through 775 is the ChatFormatting **ordinal**: all 0–21 deserialize,
  including decorations 16–20 and RESET/21. It is not a `-1` reset sentinel
- Color 776 is optional TeamColor, with canonical IDs 0–15. An invalid present
  color ID normalizes to BLACK/0
- Legacy visibility/collision are raw strings: unknown values are retained.
  From 770 their canonical IDs are 0–3, and invalid IDs normalize to ALWAYS/0
- Team flag bits are not restricted by the packet decoder; `0xff` survives as
  Java byte `-1`. Unknown objective action and team mode bytes also survive
  with no conditional fields
- Render type is a true ordinal enum: 0/integer and 1/hearts; invalid ordinals
  reject. Legacy score action is likewise closed: 0/change or 1/remove
- Score values use the full signed-i32 VarInt domain. Modern reset distinguishes
  absent objective from a present empty string; legacy empty objective becomes
  null, including in decoded change packets

Java fallback normalization is a documented compatibility fact, not a requirement
that a lossless raw-wire library discard the original invalid ID.

## Number formats and limits

From 765, optional number format is presence boolean, registry ID VarInt, then
its payload: 0/blank has none, 1/styled has style NBT, 2/fixed has component NBT.
Styled requires a compound root. Empty compound is accepted; null/end, byte,
string, and list roots reject. Fixed accepts a literal component-string root.
ID 3 rejects in every probed supporting release.

Ordinary scoreboard names and entries use the default UTF codec, not historical
gameplay limits of 16 or 40. The API probes accept 32,767 ASCII units and reject
32,768 for objective/team names, score holders, and team members. Serializer
inspection establishes a 32,767 Java UTF-16-unit limit with a three-times byte
ceiling. Legacy visibility/collision readers use 40 units; the probe exercises
visibility at 40/41 and inspection confirms the same collision bound. JSON
component limits belong to the separate existing component codec.

## Reproduce

Prepare the official Paper releases and their dependency libraries independently
in a validation workspace with `servers/<release>/versions/**/paper-*.jar`,
`servers/<release>/libraries/**/*.jar`, and `jdk25/bin/{java,javac}`. The validator
never downloads software, starts a server, or accepts an EULA.

From the repository root:

```sh
python3 tools/paper/validate_scoreboard_oracles.py \
  --workspace ../rustwire-server-validation \
  --output /tmp/rustwire-scoreboard-oracle
```

Optional `--java-home` selects another compatible JDK. Optional `--schema-root`
selects the pinned schema directory, defaulting to `research/protocols`. Existing
schema files must match recorded hashes; absent schemas are explicitly marked
unverified and do not prevent the independent API probes. The new compact report
is written to `<output>/scoreboard-wire-oracle.json`; raw logs, classes, and any
Java library logs remain under `<output>`.

## Exact scope and gaps

The checked report covers 762 packet probes and 560 enum/default-UTF probes.
The driver also checks specific decoded field values and expected rejections.
This is API-based evidence, not a live gameplay or Rust/Java byte-equality test.
There is no packet-encoder comparison, though ordinary UTF writes are tested.
Nested team parameters are decoded directly; a full create/update-team packet
is not exercised. Other gaps include absent optional color at 776, exhaustive
canonical rule IDs, collision length directly, Unicode edge cases, trailing or
truncated input, maximum list counts, NBT nesting, and every style field.

Paper's encoder can override collision using its global configuration. These
probes inspect stored decoded collision fields directly, avoiding that accessor
side effect. Pinned schemas and read-only serializer inspection corroborate the
layout changes; the executable probes cover the acceptance/fallback cases listed
in the compact report.

## Rust codec validation

`tests/scoreboard.rs` independently constructs wire bytes for all five packet
families across all 14 protocols, including every defined action and number
format. It separately covers exact packet IDs/state/direction, truncation and
trailing bytes, transactional body reads/writes, full signed scores, raw unknown
header actions, release-specific numeric fallback domains, all legacy color
ordinals and modern optional colors, duplicate/empty member strings, UTF-16 and
packet/collection/NBT budgets, style-root rejection, version mismatches and
single-byte mutation resilience. API-oracle gaps above are not claims that these
separate Rust checks are absent. Typed connection dispatch and live scoreboard
command validation are integration work outside this focused patch.
