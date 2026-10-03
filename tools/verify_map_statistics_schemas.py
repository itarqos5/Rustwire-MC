#!/usr/bin/env python3
"""Verify map/statistics envelopes against all hash-pinned protocol schemas.

This is a schema drift check, not an independent Minecraft release-API oracle.
It downloads nothing and never updates the recorded hashes.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def field(name, wire_type):
    return {"name": name, "type": wire_type}


def container(fields):
    return ["container", [field(name, kind) for name, kind in fields]]


def expected_map(protocol):
    icon = container([
        ("type", "varint"), ("x", "i8"), ("z", "i8"), ("direction", "u8"),
        ("displayName", ["option", "string" if protocol < 765 else "anonymousNbt"]),
    ])
    fields = [
        ("itemDamage", "varint"), ("scale", "i8"), ("locked", "bool"),
        ("icons", ["option", ["array", {"countType": "varint", "type": icon}]]),
        ("columns", "u8"),
    ]
    for name, kind in [
        ("rows", "u8"), ("x", "u8"), ("y", "u8"),
        ("data", ["buffer", {"countType": "varint"}]),
    ]:
        fields.append((name, ["switch", {"compareTo": "columns", "fields": {"0": "void"}, "default": kind}]))
    return container(fields)


def expected_statistics():
    entry = container([("categoryId", "varint"), ("statisticId", "varint"), ("value", "varint")])
    return container([("entries", ["array", {"countType": "varint", "type": entry}])])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=ROOT / "research/protocols")
    args = parser.parse_args()
    records = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [r["protocol"] for r in records] != list(range(763, 777)):
        raise SystemExit("Expected all fourteen explicit protocol families")
    for row in records:
        data = (args.schema_dir / f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise SystemExit(f"Schema hash mismatch: {row['schema']}")
        types = json.loads(data)["play"]["toClient"]["types"]
        for name, expected in [("packet_map", expected_map(row["protocol"])), ("packet_statistics", expected_statistics())]:
            if types.get(name) != expected:
                raise SystemExit(f"Unexpected {name} layout in {row['schema']}")
        print(f"{row['protocol']} {row['schema']}: map/statistics envelopes match")
    print("Verified 28 packet layouts from 14 hash-pinned schemas; no release-oracle claim")


if __name__ == "__main__":
    main()
