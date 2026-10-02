#!/usr/bin/env python3
"""Replay synthetic overlay fixtures against existing prepared Paper artifacts.

No downloads, servers, worlds, network connections, or EULA acceptance. Requires
an already prepared root containing servers/<release>/{versions,libraries} and
jdk25/bin/{java,javac}, as used by the other Paper validation tools.

Example from the repository root:
  python3 tools/paper/validate_overlays.py --root ../rustwire-server-validation \
    --schemas research/protocols --output overlay-wire-oracle.json
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

RELEASES = [
    "1.20.1", "1.20.2", "1.20.4", "1.20.6", "1.21.1", "1.21.3", "1.21.4",
    "1.21.5", "1.21.6", "1.21.8", "1.21.10", "1.21.11", "26.1.2", "26.2",
]
REPO = Path(__file__).resolve().parents[2]
SOURCE = Path(__file__).with_name("OverlayOracle.java")
LABELS = (
    [f"action_{action}" for action in range(6)]
    + ["header_footer"]
    + [f"style_{color}_{division}" for color in range(7) for division in range(5)]
    + [f"health_{bits}" for bits in ("80000000", "bf800000", "7f800000", "ff800000", "7fc01234")]
    + [f"flags_{flags}" for flags in (0, 1, 2, 4, 7, 8, 128, 255)]
    + [f"invalid_{field}_{value}" for field, maximum in (("action", 6), ("color", 7), ("division", 5))
       for value in (-1, maximum, 2147483647)]
)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_output(output, protocol):
    checks = {}
    if f"protocol={protocol}" not in output.splitlines():
        raise ValueError("missing or incorrect protocol marker")
    for line in output.splitlines():
        key, separator, value = line.partition("=")
        if separator and key in LABELS:
            if key in checks:
                raise ValueError(f"duplicate oracle result: {key}")
            if key.startswith("invalid_"):
                if not value.startswith("rejected:"):
                    raise ValueError(f"malformed value for {key}")
            else:
                bytes.fromhex(value)
                if not value:
                    raise ValueError(f"empty fixture for {key}")
            checks[key] = value
    if set(checks) != set(LABELS):
        raise ValueError(f"missing oracle checks: {sorted(set(LABELS) - set(checks))}")
    return checks


def report(rows, schemas=None):
    schema_checks = []
    if schemas is not None:
        for schema in json.loads((REPO / "research/schema-hashes.json").read_text()):
            actual = sha256(schemas / (schema["schema"] + ".json"))
            if actual != schema["sha256"]:
                raise ValueError(f"schema hash mismatch: {schema['schema']}")
            schema_checks.append({"protocol": schema["protocol"], "schema": schema["schema"],
                                  "sha256": actual, "verified": True})
    return {
        "base_commit": "00ac6a248cf53d59de90acdb6482470223c68d7d",
        "oracle": "tools/paper/OverlayOracle.java",
        "oracle_source_sha256": sha256(SOURCE),
        "runner": "tools/paper/validate_overlays.py",
        "method": "Original synthetic bodies decoded and re-encoded by installed release APIs. No game implementation code copied. No server launched.",
        "reproduce": "python3 tools/paper/validate_overlays.py --root ../rustwire-server-validation --schemas research/protocols --output overlay-wire-oracle.json",
        "schemas": schema_checks,
        "scope": ["boss_bar", "playerlist_header"],
        "findings": [
            "JSON components through protocol 764; anonymous NBT from 765.",
            "Boss operations 0 through 5; colors 0 through 6; divisions 0 through 4. Invalid enums rejected.",
            "Health floats outside 0..1, negative zero, infinities, and a NaN payload survive the official serializers.",
            "All flag bits are accepted; official re-encoding masks to 0x07. Rustwire deliberately preserves the original byte.",
        ],
        "total_checks": sum(row["checks"] for row in rows),
        "releases": rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--schemas", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--release", choices=RELEASES, action="append")
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    root = args.root.resolve()
    java_home = (args.java_home or root / "jdk25").resolve()
    rows = []
    with tempfile.TemporaryDirectory(prefix="rustwire-overlay-oracle-") as directory:
        work = Path(directory)
        classes = work / "classes"
        classes.mkdir()
        subprocess.run([str(java_home / "bin/javac"), "-d", str(classes), str(SOURCE)],
                       check=True, timeout=args.timeout)
        for protocol, release in enumerate(RELEASES, 763):
            if args.release and release not in args.release:
                continue
            server = root / "servers" / release
            jars = list((server / "versions").glob("*/*.jar"))
            if len(jars) != 1:
                raise ValueError(f"expected one prepared jar for {release}, found {len(jars)}")
            libraries = sorted((server / "libraries").rglob("*.jar"))
            if not libraries:
                raise ValueError(f"missing prepared libraries for {release}")
            classpath = os.pathsep.join(map(str, [classes, jars[0], *libraries]))
            result = subprocess.run([str(java_home / "bin/java"), "-Xmx512m", "-cp", classpath,
                                     "OverlayOracle", str(protocol)], cwd=work, capture_output=True,
                                    text=True, timeout=args.timeout)
            if result.returncode:
                raise RuntimeError(f"{release} oracle failed:\n{result.stdout}\n{result.stderr}")
            fixtures = parse_output(result.stdout, protocol)
            rows.append({"release": release, "protocol": protocol, "result": "passed",
                         "checks": len(fixtures), "prepared_jar_sha256": sha256(jars[0]),
                         "fixtures": fixtures})
            print(f"{release}: {len(fixtures)} checks passed", flush=True)
    value = report(rows, args.schemas)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(value, indent=2) + "\n")
    print(f"Wrote {value['total_checks']} checked results to {args.output}")


if __name__ == "__main__":
    main()
