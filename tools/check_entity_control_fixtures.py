#!/usr/bin/env python3
"""Verify seven entity-control schema layouts and independent Python wire fixtures.

Fixtures are synthetic schema-derived evidence, not release-API serializer
outputs or live captures. Uses only the standard library and never runs Rust.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]


def container(fields):
    return ["container", [{"name": name, "type": kind} for name, kind in fields]]


LAYOUTS = {
    "set_passengers": container([
        ("entityId", "varint"),
        ("passengers", ["array", {"countType": "varint", "type": "varint"}]),
    ]),
    "attach_entity": container([("entityId", "i32"), ("vehicleId", "i32")]),
    "entity_head_rotation": container([("entityId", "varint"), ("headYaw", "i8")]),
    "camera": container([("cameraId", "varint")]),
    "animation": container([("entityId", "varint"), ("animation", "u8")]),
    "damage_event": container([
        ("entityId", "varint"), ("sourceTypeId", "varint"),
        ("sourceCauseId", "varint"), ("sourceDirectId", "varint"),
        ("sourcePosition", ["option", "vec3f64"]),
    ]),
    "hurt_animation": container([("entityId", "varint"), ("yaw", "f32")]),
}


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


def varints(*values):
    return b"".join(map(varint, values))


def samples():
    """Original hand-written encoders; fixture cases include raw scalar domains."""
    return [
        ("set_passengers", "empty", varints(300, 0)),
        ("set_passengers", "ordered_signed", varints(-1, 6, 127, 128, -1, -2**31, 2**31-1, 128)),
        ("attach_entity", "zero_holder", struct.pack(">ii", 300, 0)),
        ("attach_entity", "signed_extrema", struct.pack(">ii", -2**31, -1)),
        ("entity_head_rotation", "byte_high", varint(300) + bytes([255])),
        ("entity_head_rotation", "signed_entity", varint(-1) + bytes([128])),
        ("camera", "ordinary", varint(300)),
        ("camera", "signed_min", varint(-2**31)),
        ("animation", "ordinary", varint(300) + bytes([4])),
        ("animation", "unknown_byte", varint(-1) + bytes([255])),
        ("damage_event", "absent", varints(300, 2, 0, 0) + bytes([0])),
        ("damage_event", "position", varints(300, 2, 129, 256) + bytes([1]) + struct.pack(">ddd", -1.25, 64.5, 30000000.0)),
        ("damage_event", "raw_bits", varints(-2**31, -1, -2**31, 2**31-1) + bytes([1]) + struct.pack(">QQQ", 0x8000000000000000, 0x7ff8000000000123, 0xfff0000000000000)),
        ("hurt_animation", "ordinary", varint(300) + struct.pack(">f", -90.5)),
        ("hurt_animation", "negative_zero", varint(-1) + struct.pack(">I", 0x80000000)),
        ("hurt_animation", "nan_payload", varint(-2**31) + struct.pack(">I", 0x7fc00123)),
    ]


def build(schemas):
    rows = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [row["protocol"] for row in rows] != list(range(763, 777)):
        raise ValueError("Expected the exact fourteen supported protocols")
    output = [
        "# Synthetic schema-derived fixtures from a standalone Python encoder; not release-API outputs or live captures.",
        "# protocol\tname\tpacket_id\tcase\thex",
    ]
    for row in rows:
        raw = (schemas / f'{row["schema"]}.json').read_bytes()
        if hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError(f'Schema hash mismatch for {row["protocol"]}')
        data = json.loads(raw)
        if data["types"]["vec3f64"] != container([(axis, "f64") for axis in "xyz"]):
            raise ValueError(f'vec3f64 layout drift for {row["protocol"]}')
        types = data["play"]["toClient"]["types"]
        for name, expected in LAYOUTS.items():
            if types.get("packet_" + name) != expected:
                raise ValueError(f'Layout drift for {row["protocol"]} {name}')
        mappings = types["packet"][1][0]["type"][1]["mappings"]
        ids = {name: int(number, 0) for number, name in mappings.items()}
        for name, case, payload in samples():
            output.append(f'{row["protocol"]}\t{name}\t{ids[name]}\t{case}\t{payload.hex()}')
    return "\n".join(output) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schemas", type=Path, default=ROOT / "research/protocols")
    parser.add_argument("--write", action="store_true", help="Explicitly regenerate the fixture file")
    args = parser.parse_args()
    output = build(args.schemas)
    target = ROOT / "tests/fixtures/entity-control.tsv"
    if args.write:
        target.write_text(output)
    elif target.read_text() != output:
        raise SystemExit("Entity-control fixtures drifted; inspect before --write")
    print("Verified 98 packet layouts and IDs across 14 hash-pinned schemas; 224 synthetic fixtures match")


if __name__ == "__main__":
    main()
