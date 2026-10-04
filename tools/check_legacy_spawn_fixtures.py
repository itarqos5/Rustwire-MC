#!/usr/bin/env python3
"""Verify exact legacy-spawn presence/layouts and original synthetic wire fixtures.

Uses Python's standard library, never Rustwire or a release serializer. Real
schema checks require the separately hash-pinned research inputs; unit tests do
not need downloaded schemas or network access.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
ORB = "spawn_entity_experience_orb"
PLAYER = "named_entity_spawn"


def container(fields):
    return ["container", [{"name": name, "type": kind} for name, kind in fields]]


LAYOUTS = {
    ORB: container([
        ("entityId", "varint"), ("x", "f64"), ("y", "f64"),
        ("z", "f64"), ("count", "i16"),
    ]),
    PLAYER: container([
        ("entityId", "varint"), ("playerUUID", "UUID"), ("x", "f64"),
        ("y", "f64"), ("z", "f64"), ("yaw", "i8"), ("pitch", "i8"),
    ]),
}


def present(protocol, name):
    return protocol <= (769 if name == ORB else 763)


def varint(value):
    if not -(2**31) <= value < 2**31:
        raise ValueError("VarInt outside signed i32 range")
    value &= 0xffffffff
    output = bytearray()
    while value > 127:
        output.append((value & 127) | 128)
        value >>= 7
    output.append(value)
    return bytes(output)


def samples():
    """Independent hand-written wire encoders, with no library under test."""
    ordinary = struct.pack(">ddd", -1.25, 64.5, 30000000.0)
    zero = bytes(24)
    raw = struct.pack(">QQQ", 0x8000000000000000, 0x7ff8000000000123, 0xfff0000000000000)
    other = struct.pack(">QQQ", 0x7ff0000000000000, 0xfff8000000000456, 1)
    orbs = [
        ("ordinary", 300, ordinary, 7), ("zero", 0, zero, 0),
        ("signed_min", -2**31, other, -32768),
        ("signed_max", 2**31-1, ordinary, 32767),
        ("negative_id", -1, ordinary, -1), ("raw_bits", -1, raw, -7),
        ("id_127", 127, ordinary, 128), ("id_128", 128, ordinary, 127),
    ]
    players = [
        ("ordinary", 300, bytes(range(16)), ordinary, 255, 128),
        ("zero", 0, bytes(16), zero, 0, 0),
        ("signed_min", -2**31, bytes(range(15, -1, -1)), other, 127, 0),
        ("signed_max", 2**31-1, bytes([255])*16, ordinary, 128, 255),
        ("raw_bits", -1, bytes(range(16)), raw, 1, 254),
        ("raw_bits_negative", 128, bytes(range(15, -1, -1)), other, 254, 1),
    ]
    return ([(ORB, case, varint(entity) + xyz + struct.pack(">h", value))
             for case, entity, xyz, value in orbs]
            + [(PLAYER, case, varint(entity) + uuid + xyz + bytes([yaw, pitch]))
               for case, entity, uuid, xyz, yaw, pitch in players])


def build(schemas):
    rows = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [row["protocol"] for row in rows] != list(range(763, 777)):
        raise ValueError("Expected the exact fourteen supported protocols")
    output = [
        "# Original Python-encoded synthetic fixtures; not official serializer output or live captures.",
        "# protocol\tname\tpacket_id\tcase\thex",
    ]
    for row in rows:
        protocol = row["protocol"]
        raw = (schemas / f'{row["schema"]}.json').read_bytes()
        if hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError(f"Schema hash mismatch for {protocol}")
        types = json.loads(raw)["play"]["toClient"]["types"]
        packet = types["packet"]
        mappings = packet[1][0]["type"][1]["mappings"]
        switch = packet[1][1]["type"][1]
        if switch["compareTo"] != "name":
            raise ValueError(f"Packet switch selector drift for {protocol}")
        fields = switch["fields"]
        for name, layout in LAYOUTS.items():
            ids = [int(number, 0) for number, value in mappings.items() if value == name]
            if not present(protocol, name):
                if "packet_" + name in types or name in fields or ids:
                    raise ValueError(f"Unexpected legacy packet presence for {protocol} {name}")
                continue
            if types.get("packet_" + name) != layout:
                raise ValueError(f"Layout drift for {protocol} {name}")
            if ids != [2 if name == ORB else 3]:
                raise ValueError(f"Packet ID drift for {protocol} {name}")
            if fields.get(name) != "packet_" + name:
                raise ValueError(f"Packet switch drift for {protocol} {name}")
        for name, case, payload in samples():
            if present(protocol, name):
                output.append(f'{protocol}\t{name}\t{2 if name == ORB else 3}\t{case}\t{payload.hex()}')
    return "\n".join(output) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schemas", type=Path, default=ROOT / "research/protocols")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    output = build(args.schemas)
    target = ROOT / "tests/fixtures/legacy-spawn.tsv"
    if args.write:
        target.write_text(output)
    elif target.read_text() != output:
        raise SystemExit("Legacy-spawn fixtures drifted; inspect before --write")
    print("Verified 8 present and 20 absent packet layouts/IDs/switches across 14 hash-pinned schemas; 62 synthetic fixtures match")


if __name__ == "__main__":
    main()
