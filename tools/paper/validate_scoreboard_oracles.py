#!/usr/bin/env python3
"""Probe cached release APIs without starting a server or downloading anything.

The Java programs are observational harnesses. This driver asserts their expected
case sets, acceptance/rejection, enum domains, and selected decoded field values.
Raw stdout/stderr and compiled harnesses stay under --output, outside the project.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

PROJECT = Path(__file__).resolve().parents[2]
VERSIONS = (
    "1.20.1", "1.20.2", "1.20.4", "1.20.6", "1.21.1", "1.21.3", "1.21.4",
    "1.21.5", "1.21.6", "1.21.8", "1.21.10", "1.21.11", "26.1.2", "26.2",
)
COLORS = ("BLACK DARK_BLUE DARK_GREEN DARK_AQUA DARK_RED DARK_PURPLE GOLD GRAY "
          "DARK_GRAY BLUE GREEN AQUA RED LIGHT_PURPLE YELLOW WHITE").split()
FORMATTING = COLORS + "OBFUSCATED BOLD STRIKETHROUGH UNDERLINE ITALIC RESET".split()
DISPLAY = ["LIST", "SIDEBAR", "BELOW_NAME"] + ["TEAM_" + c for c in COLORS]
VISIBILITY = ["ALWAYS", "NEVER", "HIDE_FOR_OTHER_TEAMS", "HIDE_FOR_OWN_TEAM"]
COLLISION = ["ALWAYS", "NEVER", "PUSH_OTHER_TEAMS", "PUSH_OWN_TEAM"]
IDS = [-2147483648, -1, 0, 3, 15, 18, 19, 21, 22, 100, 2147483647]
SCHEMA_EVIDENCE = {'763': {'version': '1.20',
         'sha256': '3b4fb2620f5d08927f94509ae028b475f7ef3bf2e7eb987af499d3ad632765ad',
         'packet_ids': {'scoreboard_display_objective': 81,
                        'scoreboard_objective': 88,
                        'teams': 90,
                        'scoreboard_score': 91}},
 '764': {'version': '1.20.2',
         'sha256': 'e038fd10ee94a8395062034e6ad8aff6c989e7206b74906c3a04daebc7c004fc',
         'packet_ids': {'scoreboard_display_objective': 83,
                        'scoreboard_objective': 90,
                        'teams': 92,
                        'scoreboard_score': 93}},
 '765': {'version': '1.20.3',
         'sha256': 'ea8e6d9712cd0d8122c7830a6bd23f91251b4cd5c4526486a0ac4c7f7784ed52',
         'packet_ids': {'reset_score': 66,
                        'scoreboard_display_objective': 85,
                        'scoreboard_objective': 92,
                        'teams': 94,
                        'scoreboard_score': 95}},
 '766': {'version': '1.20.5',
         'sha256': '4204f6f1d4f5aa7ab0936fe0cb78fac3d249a06e534e4903d1937614667be241',
         'packet_ids': {'reset_score': 68,
                        'scoreboard_display_objective': 87,
                        'scoreboard_objective': 94,
                        'teams': 96,
                        'scoreboard_score': 97}},
 '767': {'version': '1.21.1',
         'sha256': '1f5f23d1d1425a12d1b46d78dbc2b79038ea456cc1be11c942a79bebd8d3f43a',
         'packet_ids': {'reset_score': 68,
                        'scoreboard_display_objective': 87,
                        'scoreboard_objective': 94,
                        'teams': 96,
                        'scoreboard_score': 97}},
 '768': {'version': '1.21.3',
         'sha256': '94b9c562df3be925c12615041281d9f492c69f3fee10038787fdeb312a65c575',
         'packet_ids': {'reset_score': 73,
                        'scoreboard_display_objective': 92,
                        'scoreboard_objective': 100,
                        'teams': 103,
                        'scoreboard_score': 104}},
 '769': {'version': '1.21.4',
         'sha256': '43936c5220a43b638b73ffd8165bccb3db7fe8dd0d54049072eb21fa5b9614ea',
         'packet_ids': {'reset_score': 73,
                        'scoreboard_display_objective': 92,
                        'scoreboard_objective': 100,
                        'teams': 103,
                        'scoreboard_score': 104}},
 '770': {'version': '1.21.5',
         'sha256': '96434702de4e203d84baf092aa500c7ba4ca2af95edfc6f339a982b61cbb0777',
         'packet_ids': {'reset_score': 72,
                        'scoreboard_display_objective': 91,
                        'scoreboard_objective': 99,
                        'teams': 102,
                        'scoreboard_score': 103}},
 '771': {'version': '1.21.6',
         'sha256': 'a827d83d762738020f16aca81aad48ea3bb8690181a96425ed15590453d35fd7',
         'packet_ids': {'reset_score': 72,
                        'scoreboard_display_objective': 91,
                        'scoreboard_objective': 99,
                        'teams': 102,
                        'scoreboard_score': 103}},
 '772': {'version': '1.21.8',
         'sha256': 'a827d83d762738020f16aca81aad48ea3bb8690181a96425ed15590453d35fd7',
         'packet_ids': {'reset_score': 72,
                        'scoreboard_display_objective': 91,
                        'scoreboard_objective': 99,
                        'teams': 102,
                        'scoreboard_score': 103}},
 '773': {'version': '1.21.9',
         'sha256': 'f8ab7f079607267e5cbdd166d943fcf1ea77b87fa494cbc39737665a8d90aeb6',
         'packet_ids': {'reset_score': 77,
                        'scoreboard_display_objective': 96,
                        'scoreboard_objective': 104,
                        'teams': 107,
                        'scoreboard_score': 108}},
 '774': {'version': '1.21.11',
         'sha256': '34d280120eef9090c66abc502d43d5305ffa2f592c1d2b0eb7a96b71048b81f7',
         'packet_ids': {'reset_score': 77,
                        'scoreboard_display_objective': 96,
                        'scoreboard_objective': 104,
                        'teams': 107,
                        'scoreboard_score': 108}},
 '775': {'version': '26.1',
         'sha256': 'a5fda872e2d425d1f35a8ab7e042148f4944ac1a7f0c3c5380b64b06d383bc0d',
         'packet_ids': {'reset_score': 79,
                        'scoreboard_display_objective': 98,
                        'scoreboard_objective': 106,
                        'teams': 109,
                        'scoreboard_score': 110}},
 '776': {'version': '26.2',
         'sha256': 'fef8c96e9a25905f692b8e1129ab1d331922f4ce41909a56537a4331fff30430',
         'packet_ids': {'reset_score': 79,
                        'scoreboard_display_objective': 98,
                        'scoreboard_objective': 106,
                        'teams': 109,
                        'scoreboard_score': 110}}}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def domain_lines(stdout):
    prefixes = ("formatting=", "display_slot=", "display_slot_by_id[", "visibility=",
                "visibility_by_id[", "collision=", "collision_by_id[", "color=",
                "color_by_id[", "render_type=", "utf_read_", "utf_write_")
    rows = {}
    for line in stdout.splitlines():
        if line.startswith(prefixes):
            key, value = line.split("=", 1)
            require(key not in rows, "duplicate domain case: " + key)
            rows[key] = value
    return rows


def verify_domain(stdout, protocol):
    rows = domain_lines(stdout)
    checks = 0

    def check(condition, message):
        nonlocal checks
        require(condition, f"protocol {protocol}: {message}")
        checks += 1

    def enum_values(label, expected):
        value = "[" + ", ".join(f"{i}:{name}" for i, name in enumerate(expected)) + "]"
        check(rows.get(label) == value, label + " enum domain changed")

    def absent(label):
        check(rows.get(label, "").startswith("ClassNotFoundException:"),
              label + " should not exist in this release")

    def by_id(label, values):
        for number in IDS:
            expected = values[number] if 0 <= number < len(values) else values[0]
            check(rows.get(f"{label}_by_id[{number}]") == expected,
                  f"{label} fallback changed for {number}")

    enum_values("formatting", FORMATTING)
    enum_values("visibility", VISIBILITY)
    enum_values("collision", COLLISION)
    enum_values("render_type", ["INTEGER", "HEARTS"])
    if protocol == 763:
        absent("display_slot")
    else:
        enum_values("display_slot", DISPLAY)
        by_id("display_slot", DISPLAY)
    if protocol >= 770:
        by_id("visibility", VISIBILITY)
        by_id("collision", COLLISION)
    if protocol == 776:
        enum_values("color", COLORS)
        by_id("color", COLORS)
    else:
        absent("color")
    for length in (16, 17, 40, 41, 32767):
        check(rows.get(f"utf_read_{length}") == str(length), f"UTF read {length}")
        check(rows.get(f"utf_write_{length}") == "ok", f"UTF write {length}")
    check(rows.get("utf_read_32768", "").startswith("DecoderException:"), "UTF read bound")
    check(rows.get("utf_write_32768", "").startswith("EncoderException:"), "UTF write bound")
    expected = 18 + (11 if protocol >= 764 else 0) + (22 if protocol >= 770 else 0)
    expected += 11 if protocol == 776 else 0
    check(len(rows) == expected, f"domain case count {len(rows)} != {expected}")
    return {"probes": len(rows), "assertions": checks}


def packet_cases(protocol):
    cases = set()
    rejected = set()

    def add(prefix, values, invalid=()):
        cases.update(f"{prefix}_{value}" for value in values)
        rejected.update(f"{prefix}_{value}" for value in invalid)

    add("display_slot", [-128, -1, 0, 18, 19, 127, 128])
    add("objective_action", [-128, -1, 1, 3, 127])
    add("team_mode", [-128, -1, 1, 5, 127])
    add("team_params_color", [-1, 0, 15, 16, 20, 21, 22], [-1, 22] if protocol < 776 else [])
    if protocol < 770:
        add("visibility_length", [40, 41], [41])
    for prefix in ("objective_name_len", "team_name_len", "score_owner_len", "team_player_len"):
        add(prefix, [41, 32767, 32768], [32768])
    if protocol < 765:
        add("score_action", [-1, 0, 1, 2], [-1, 2])
    else:
        add("score_value", [-2147483648, -1, 0, 2147483647])
        cases.update(["reset_no_objective", "reset_empty_objective"])
        add("objective_format", [0, 1, 2, 3], [3])
        add("style_root", [0, 1, 8, 9, 10], [0, 1, 8, 9])
    add("render", [-1, 0, 1, 2], [-1, 2])
    return cases, rejected


def verify_packets(stdout, protocol):
    expected, rejected = packet_cases(protocol)
    rows = {}
    for line in stdout.splitlines():
        name, separator, value = line.partition(" ")
        if name in expected:
            require(separator and name not in rows, "duplicate or malformed packet case " + name)
            rows[name] = value
    checks = 0

    def check(condition, message):
        nonlocal checks
        require(condition, f"protocol {protocol}: {message}")
        checks += 1

    check(rows.keys() == expected, "packet case set mismatch: " + str(expected - rows.keys()))
    for name, value in rows.items():
        check(value.startswith("ERROR ") == (name in rejected), name + ": " + value)
        if name in rejected:
            check(not any(marker in value for marker in
                          ("NoSuch", "Not bootstrapped", "ExceptionInInitializer", "no buffer")),
                  "harness failure in " + name)
    for value in [-128, -1, 0, 18, 19, 127, 128]:
        expected_value = str(-128 if value == 128 else value) if protocol == 763 else (
            DISPLAY[value] if 0 <= value < len(DISPLAY) else "LIST")
        check("=" + expected_value + "," in rows[f"display_slot_{value}"], "display slot value")
    for prefix, values in [("objective_action", [-128, -1, 1, 3, 127]),
                           ("team_mode", [-128, -1, 1, 5, 127])]:
        for value in values:
            check(re.search(r"=" + str(value) + r"[,\]]", rows[f"{prefix}_{value}"]) is not None,
                  f"{prefix} did not retain {value}")
    for color in [-1, 0, 15, 16, 20, 21, 22]:
        name = f"team_params_color_{color}"
        if name in rejected:
            continue
        value = rows[name]
        expected_color = (COLORS[color] if 0 <= color < 16 else "BLACK") if protocol == 776 else FORMATTING[color]
        check("=" + expected_color + "," in value, name + " color mapping")
        check("=-1]" in value, name + " raw 0xff flags")
        if protocol < 770:
            check('"unknownVisibility"' in value and '"unknownCollision"' in value,
                  name + " unknown legacy strings were not retained")
        else:
            check(value.count("=ALWAYS,") == 2, name + " visibility/collision fallback")
    check("=INTEGER," in rows["render_0"], "render INTEGER")
    check("=HEARTS," in rows["render_1"], "render HEARTS")
    if protocol < 765:
        check("=null," in rows["score_action_0"], "legacy empty objective -> null")
        check("=-2147483648," in rows["score_action_0"], "legacy signed score")
    else:
        for value in [-2147483648, -1, 0, 2147483647]:
            check("=" + str(value) + "," in rows[f"score_value_{value}"], "signed score " + str(value))
        check("=null]" in rows["reset_no_objective"], "reset absent objective")
        check('=""]' in rows["reset_empty_objective"], "reset present-empty objective")
        for number, name in enumerate(["BlankFormat", "StyledFormat", "FixedFormat"]):
            check(name in rows[f"objective_format_{number}"], "number format dispatch " + name)
        check("StyledFormat" in rows["style_root_10"], "compound style root")
    return {"probes": len(rows), "accepted": len(rows) - len(rejected),
            "expected_rejections": len(rejected), "assertions": checks}


def schema_record(protocol, schema_root):
    record = dict(SCHEMA_EVIDENCE[str(protocol)])
    schema = schema_root / (record["version"] + ".json")
    record["locally_verified"] = schema.is_file()
    if schema.is_file():
        require(sha256(schema) == record["sha256"], "schema hash changed: " + str(schema))
    return record


def profile(protocol):
    return ("byte_slot_json" if protocol == 763 else "enum_slot_json" if protocol == 764 else
            "nbt_string_rules" if protocol < 770 else "nbt_numeric_rules" if protocol < 776 else
            "nbt_optional_color_reordered")


BOUNDARIES = {
    "764": "Display slot changes signed byte -> VarInt enum ID; invalid IDs normalize to LIST0.",
    "765": "Components JSON -> anonymous NBT; number formats, score display, distinct reset packet added.",
    "770": "Team visibility/collision UTF strings -> VarInt IDs; invalid IDs normalize to ALWAYS0.",
    "776": "Team parameters reorder to display,prefix,suffix,visibility,collision,optional color,flags.",
}
OBSERVED_DOMAINS = {
    "display_slot": "763 full signed byte; 764+ canonical IDs0..18, invalid IDs ->0.",
    "render_type": "0 INTEGER,1 HEARTS; invalid ordinals reject.",
    "raw_headers_flags": "Unknown objective/team action bytes and all flag bits retained.",
    "team_color": "763..775 ChatFormatting0..21, including RESET21; 776 optional TeamColor0..15, invalid IDs ->0.",
    "legacy_rules": "Unknown visibility/collision strings retained; explicit legacy visibility length40 accepted,41 rejected.",
    "number_format": "0 blank,1 compound-root style,2 component;3 rejects. Style null/byte/string/list roots reject.",
    "strings": "32767 ASCII units accepted,32768 rejected for ordinary UTF and four scoreboard name/entry fields.",
    "score_reset": "Signed i32 score values accepted; modern reset null and present-empty objective remain distinct.",
}
GAPS = [
    "API decoder/domain probes only: no server startup, account, world, live client exchange, rendering, or Rust codec comparison.",
    "No packet encoder byte-equality oracle; ordinary UTF writer limits are separately exercised.",
    "Not exhaustive malformed/truncated/oversized payload, Unicode, NBT nesting, style-field, or player-list tests.",
    "No full create/update team packet fixture: nested Parameters is decoded directly; outer unknown/removal/join modes are exercised.",
    "No packet test for absent 776 optional color, all canonical rule IDs, or collision string length; serializer inspection supplements these facts.",
    "Paper's encoder collision-configuration patch is outside these stored-field decoder probes.",
    "Java API normalization differs from lossless raw-wire preservation; the summary documents canonical/fallback behavior, not a Rust decoder mandate.",
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, required=True,
                        help="Prepared validation workspace containing servers/ and jdk25/")
    parser.add_argument("--output", type=Path, required=True,
                        help="Output directory for classes, raw logs, and compact JSON")
    parser.add_argument("--java-home", type=Path, help="Override workspace/jdk25")
    parser.add_argument("--schema-root", type=Path, default=PROJECT / "research/protocols",
                        help="Optional local pinned schemas; existing files must match recorded hashes")
    args = parser.parse_args()
    workspace, output = args.workspace.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    classes, raw = output / "classes", output / "raw"
    classes.mkdir(exist_ok=True)
    raw.mkdir(exist_ok=True)
    java_home = (args.java_home or workspace / "jdk25").resolve()
    java, javac = java_home / "bin/java", java_home / "bin/javac"
    sources = [Path(__file__).with_name(name) for name in
               ("ScoreboardDomainOracle.java", "ScoreboardPacketOracle.java")]
    require(java.is_file() and javac.is_file(), "Java/java compiler missing under " + str(java_home))
    subprocess.run([str(javac), "-d", str(classes), *map(str, sources)], check=True, cwd=output)
    report = {
        "format_version": 1, "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "method": "Original Java API probes plus Python acceptance/domain assertions; cached Paper releases only.",
        "java_version": subprocess.check_output([str(java), "--version"], text=True).splitlines()[0],
        "sources_sha256": {"tools/paper/" + p.name: sha256(p) for p in sources + [Path(__file__)]},
        "scope": "Protocols763..776; observational Java output is checked by this driver. No live gameplay claim.",
        "boundaries": BOUNDARIES, "observed_domains": OBSERVED_DOMAINS, "gaps": GAPS, "results": [],
    }
    for protocol, version in enumerate(VERSIONS, 763):
        base = workspace / "servers" / version
        candidates = sorted((base / "versions").rglob("paper-*.jar"))
        require(len(candidates) == 1, f"Expected one prepared Paper jar for {version}: {candidates}")
        jar = candidates[0]
        libraries = sorted((base / "libraries").rglob("*.jar"))
        require(bool(libraries), "Missing prepared dependency libraries for " + version)
        classpath = os.pathsep.join(map(str, [classes, jar, *libraries]))
        run_dir = output / "runs" / version
        run_dir.mkdir(parents=True, exist_ok=True)
        observations = {}
        for label, main_class, extra in (
            ("domain", "ScoreboardDomainOracle", []),
            ("packet", "ScoreboardPacketOracle", [str(protocol)]),
        ):
            result = subprocess.run([str(java), "-cp", classpath, main_class, *extra],
                                    cwd=run_dir, capture_output=True, text=True, timeout=120)
            (raw / f"{version}-{label}.stdout.txt").write_text(result.stdout)
            (raw / f"{version}-{label}.stderr.txt").write_text(result.stderr)
            require(result.returncode == 0, f"{version} {label} oracle exited {result.returncode}; see raw logs")
            observations[label] = result.stdout
        domain = verify_domain(observations["domain"], protocol)
        packet = verify_packets(observations["packet"], protocol)
        report["results"].append({
            "protocol": protocol, "release": version, "profile": profile(protocol),
            "jar": {"workspace_relative_path": jar.relative_to(workspace).as_posix(), "sha256": sha256(jar),
                    "dependency_jar_count": len(libraries)},
            "schema": schema_record(protocol, args.schema_root),
            "domain": domain, "packet": packet, "passed": True,
        })
        print(f"{version} protocol{protocol}: {domain['probes']} domain probes, {packet['probes']} packet probes passed", flush=True)
    report["totals"] = {
        "releases": len(report["results"]),
        "domain_probes": sum(r["domain"]["probes"] for r in report["results"]),
        "packet_probes": sum(r["packet"]["probes"] for r in report["results"]),
        "assertions": sum(r["domain"]["assertions"] + r["packet"]["assertions"] for r in report["results"]),
        "passed": True,
    }
    path = output / "scoreboard-wire-oracle.json"
    path.write_text(json.dumps(report, indent=2) + "\n")
    print("Saved " + str(path), flush=True)


if __name__ == "__main__":
    try:
        main()
    except (AssertionError, subprocess.CalledProcessError, subprocess.TimeoutExpired, OSError) as error:
        print("Scoreboard oracle validation failed: " + str(error), file=sys.stderr)
        sys.exit(1)
