#!/usr/bin/env python3
"""Replay synthetic auxiliary-container fixtures against existing prepared Paper artifacts.

No downloads, servers, worlds, network connections, or EULA acceptance. Requires
an already prepared root containing servers/<release>/{versions,libraries} and
jdk25/bin/{java,javac}, as used by the other Paper validation tools.

Example from the repository root:
  python3 tools/paper/validate_container_auxiliary.py --root ../rustwire-server-validation \
    --schemas research/protocols --output container-auxiliary-wire-oracle.json
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
SOURCE = Path(__file__).with_name("ContainerAuxiliaryOracle.java")
def labels(protocol):
    simple = [f"{packet}_{value}" for packet, values in (
        ("property", (7, 255, 300, -1)), ("horse", (7, 255, 300, -1)),
        ("button", (7, 127, 255, 300, -1)), ("select", (0, 300, -1, -2147483648, 2147483647))) for value in values]
    return ([label + suffix for label in simple for suffix in ("", "_roundtrip", "_values")]
        + ["cooldown"] + [f"merchant_empty_{i}" for i in (7, 255, 300, -1)]
        + [f"trades_{second}_{component}" for second in ("false", "true")
           for component in (("false", "true") if protocol >= 766 else ("false",))]
        + [f"price_{bits}" for bits in ("80000000", "bf800000", "7f800000", "ff800000", "7fc01234")]
        + (["invalid_empty_result"] + [f"{kind}_{count}" for kind in ("cost_count", "cost_only") for count in (0, -1, 128)] if protocol >= 766 else []))


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_output(output, protocol):
    checks = {}
    LABELS = labels(protocol)
    if f"protocol={protocol}" not in output.splitlines():
        raise ValueError("missing or incorrect protocol marker")
    for line in output.splitlines():
        key, separator, value = line.partition("=")
        if separator and key in LABELS:
            if key in checks:
                raise ValueError(f"duplicate oracle result: {key}")
            if key.startswith("invalid_"):
                if not value.startswith("rejected:"):
                    raise ValueError(f"malformed rejection for {key}")
            elif key.endswith("_values"):
                for field in value.split(","):
                    int(field)
            else:
                bytes.fromhex(value)
                if not value:
                    raise ValueError(f"empty fixture for {key}")
            checks[key] = value
    if set(checks) != set(LABELS):
        raise ValueError(f"missing oracle checks: {sorted(set(LABELS) - set(checks))}")
    for key, value in checks.items():
        if key.endswith("_roundtrip") and value != checks[key.removesuffix("_roundtrip")]:
            raise ValueError(f"roundtrip mismatch: {key}")
        if key.endswith("_values"):
            kind, raw = key.removesuffix("_values").split("_", 1)
            n = int(raw)
            if kind in ("property", "horse") and protocol < 768:
                n &= 255
            elif kind == "button" and protocol < 766:
                n = (n + 128) % 256 - 128
            expected = {"property": [n, -32768, 32767], "horse": [n, -2, -2147483648], "button": [n, n], "select": [n]}[kind]
            if list(map(int, value.split(","))) != expected:
                raise ValueError(f"decoded values mismatch: {key}")
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
        "base_commit": "78025ad",
        "oracle": "tools/paper/ContainerAuxiliaryOracle.java",
        "oracle_source_sha256": sha256(SOURCE),
        "runner": "tools/paper/validate_container_auxiliary.py",
        "method": "Original synthetic bodies checked with installed release APIs; merchant bodies decoded and inspected only from protocol 769 because Paper outbound item sanitization needs a live server. Other fixtures decoded and re-encoded. No game implementation code copied. No server launched.",
        "reproduce": "python3 tools/paper/validate_container_auxiliary.py --root ../rustwire-server-validation --schemas research/protocols --output container-auxiliary-wire-oracle.json",
        "schemas": schema_checks,
        "scope": ["craft_progress_bar", "open_horse_window", "enchant_item", "select_trade", "set_cooldown", "trade_list"],
        "findings": [
            "Container property and mount IDs use unsigned bytes through 767 and signed VarInts from 768.",
            "Button fields use signed bytes through 765 and signed VarInts from 766.",
            "Cooldown targets change from item registry IDs to identifiers at 768; tick counts remain signed VarInts.",
            "Merchant container IDs remain signed VarInts in every family, including 766-767 despite the schema ContainerID alias.",
            "Modern merchant results require nonempty Slots; the empty sentinel is rejected. Legacy results retain the ordinary Slot layout.",
            "Modern merchant costs use item ID, signed VarInt count and exact component values, without a removal-count field.",
            "Merchant price multipliers preserve negative zero, negative values, infinities and NaN payload bits.",
            "Protocol 775+ uses an explicit empty default-component map for the stone fixture holder; no data-pack semantics are claimed.",
            "Merchant costs accept zero, negative and multi-byte counts from protocol 766; Rustwire preserves these wire domains.",
        ],
        "total_checks": sum(row["checks"] for row in rows),
        "releases": rows,
    }


def fixture_text(rows):
    lines = ["# Release-API-verified bodies; generated by validate_container_auxiliary.py.",
             "# protocol|label|hex|decoded integer fields (when applicable)"]
    for row in rows:
        fixtures = row["fixtures"]
        for label, value in fixtures.items():
            if label.endswith(("_roundtrip", "_values")) or label.startswith(("cost_only_", "invalid_")):
                continue
            lines.append(f"{row['protocol']}|{label}|{value}|{fixtures.get(label + '_values', '')}")
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--java-home", type=Path)
    parser.add_argument("--schemas", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--release", choices=RELEASES, action="append")
    parser.add_argument("--fixtures", type=Path, help="also emit Rust test fixtures")
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    root = args.root.resolve()
    java_home = (args.java_home or root / "jdk25").resolve()
    rows = []
    with tempfile.TemporaryDirectory(prefix="rustwire-container-auxiliary-oracle-") as directory:
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
                                     "ContainerAuxiliaryOracle", str(protocol)], cwd=work, capture_output=True,
                                    text=True, timeout=args.timeout)
            if result.returncode:
                raise RuntimeError(f"{release} oracle failed:\n{result.stdout}\n{result.stderr}")
            fixtures = parse_output(result.stdout, protocol)
            rows.append({"release": release, "protocol": protocol, "result": "passed",
                         "checks": len(fixtures), "prepared_jar_sha256": sha256(jars[0]),
                         "merchant_verification": "decode_and_inspect" if protocol >= 769 else "decode_and_reencode",
                         "fixtures": fixtures})
            print(f"{release}: {len(fixtures)} checks passed", flush=True)
    value = report(rows, args.schemas)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(value, indent=2) + "\n")
    if args.fixtures:
        args.fixtures.parent.mkdir(parents=True, exist_ok=True)
        args.fixtures.write_text(fixture_text(rows))
    print(f"Wrote {value['total_checks']} checked results to {args.output}")


if __name__ == "__main__":
    main()
