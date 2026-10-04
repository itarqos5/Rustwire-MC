#!/usr/bin/env python3
"""Check hash-pinned bidirectional vehicle schemas and original Python fixtures.

No Rust encoder, game execution, downloaded source or network access is used.
--write regenerates synthetic fixtures; these are not captured/server-API bytes.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DIRECTIONS = (("serverbound", "toServer"), ("clientbound", "toClient"))


def layout(protocol, direction):
    fields = [("x", "f64"), ("y", "f64"), ("z", "f64"), ("yaw", "f32"), ("pitch", "f32")]
    if direction == "serverbound" and protocol >= 769:
        fields.append(("onGround", "bool"))
    return ["container", [{"name": name, "type": kind} for name, kind in fields]]


def fixtures(protocol, direction):
    # Explicit bit patterns avoid platform NaN canonicalization. These are
    # original examples, not obtained by running Minecraft or the Rust encoder.
    bodies = [
        ("finite", struct.pack(">dddff", -1.25, 2.5, -3.75, 450.0, -180.0)),
        ("extrema", struct.pack(">QQQII", 0x7fefffffffffffff, 0x0010000000000000, 1, 0x7f7fffff, 1)),
        ("bits", struct.pack(">QQQII", 0x7ff8000000001234, 0x8000000000000000, 0x7ff0000000000000, 0xff800000, 0x7fc01234)),
    ]
    if direction == "serverbound" and protocol >= 769:
        return [(case + suffix, body + flag) for case, body in bodies
                for suffix, flag in [("-air", b"\x00"), ("-ground", b"\x01")]]
    return bodies


def generate(schema_dir):
    records = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [row["protocol"] for row in records] != list(range(763, 777)):
        raise ValueError("Expected exactly fourteen ordered protocol families")
    lines = ["# Original Python-encoded fixtures; not release-API or live-server captures.",
             "# protocol\tdirection\tpacket_id\tcase\thex"]
    layouts = 0
    for row in records:
        protocol = row["protocol"]
        raw = (schema_dir / f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError(f"Schema hash mismatch: {row['schema']}")
        schema = json.loads(raw)
        for primitive in ("f32", "f64", "bool"):
            if schema["types"].get(primitive) != "native":
                raise ValueError(f"Unexpected scalar alias: {protocol} {primitive}")
        for direction, key in DIRECTIONS:
            types = schema["play"][key]["types"]
            mapping = types["packet"][1][0]["type"][1]["mappings"]
            ids = [int(packet_id, 16) for packet_id, name in mapping.items() if name == "vehicle_move"]
            if (types.get("packet_vehicle_move") != layout(protocol, direction)
                    or len(ids) != 1 or ids[0] < 0):
                raise ValueError(f"Unexpected vehicle layout/presence: {protocol} {direction}")
            switch = types["packet"][1][1]["type"]
            if (switch[0] != "switch" or switch[1].get("compareTo") != "name"
                    or switch[1]["fields"].get("vehicle_move") != "packet_vehicle_move"):
                raise ValueError(f"Unexpected vehicle dispatch: {protocol} {direction}")
            layouts += 1
            for case, body in fixtures(protocol, direction):
                lines.append(f"{protocol}\t{direction}\t{ids[0]}\t{case}\t{body.hex()}")
    return "\n".join(lines) + "\n", layouts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=ROOT / "research/protocols")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    text, layouts = generate(args.schema_dir)
    target = ROOT / "tests/fixtures/vehicle-movement.tsv"
    if args.write:
        target.write_text(text)
    elif not target.exists() or target.read_text() != text:
        raise SystemExit("Vehicle-movement fixtures are stale; review and run with --write")
    print(f"Verified {layouts} directional layouts and {len(text.splitlines()) - 2} synthetic fixtures across 14 hash-pinned families")


if __name__ == "__main__":
    main()
