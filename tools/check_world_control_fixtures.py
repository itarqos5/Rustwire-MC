#!/usr/bin/env python3
"""Hash-check all world-control layouts/IDs and independently encode fixtures.

The two early face_player string fields are explicitly audited schema defects;
wire fixtures use VarInt anchors based on independently authored version-pinned
implementations. This is neither a Minecraft release-API nor a live-server test.
No network access or Rust encoder is used. --write regenerates the fixture file.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMES = (
    "block_break_animation", "block_action", "open_sign_entity", "nbt_query_response",
    "collect", "vehicle_move", "face_player", "player_rotation", "set_projectile_power",
    "set_ticking_state", "step_tick",
)


def container(fields):
    return ["container", [{"name": name, "type": kind} for name, kind in fields]]


def optional(kind):
    return ["switch", {"compareTo": "isEntity", "fields": {"true": kind}, "default": "void"}]


def layout(name, protocol):
    if name == "block_break_animation":
        fields = [("entityId", "varint"), ("location", "position"), ("destroyStage", "i8")]
    elif name == "block_action":
        fields = [("location", "position"), ("byte1", "u8"), ("byte2", "u8"), ("blockId", "varint")]
    elif name == "open_sign_entity":
        fields = [("location", "position"), ("isFrontText", "bool")]
    elif name == "nbt_query_response":
        fields = [("transactionId", "varint"), ("nbt", "optionalNbt" if protocol == 763 else "anonOptionalNbt")]
    elif name == "collect":
        fields = [("collectedEntityId", "varint"), ("collectorEntityId", "varint"), ("pickupItemCount", "varint")]
    elif name == "vehicle_move":
        fields = [("x", "f64"), ("y", "f64"), ("z", "f64"), ("yaw", "f32"), ("pitch", "f32")]
    elif name == "face_player":
        fields = [("feet_eyes", "varint"), ("x", "f64"), ("y", "f64"), ("z", "f64"),
                  ("isEntity", "bool"), ("entityId", optional("varint")),
                  ("entity_feet_eyes", optional("string" if protocol < 765 else "varint"))]
    elif name == "player_rotation":
        if protocol < 768:
            return None
        fields = [("yaw", "f32"), ("pitch", "f32")]
        if protocol >= 773:
            fields = [("yaw", "f32"), ("relativeYaw", "bool"), ("pitch", "f32"), ("relativePitch", "bool")]
    elif name == "set_projectile_power":
        if protocol < 766:
            return None
        fields = [("id", "varint"), ("power", "vec3f64") if protocol == 766 else ("accelerationPower", "f64")]
    elif name == "set_ticking_state":
        if protocol < 765:
            return None
        fields = [("tick_rate", "f32"), ("is_frozen", "bool")]
    elif name == "step_tick":
        if protocol < 765:
            return None
        fields = [("tick_steps", "varint")]
    else:
        raise ValueError(name)
    return container(fields)


def varint(value):
    value &= 0xffffffff
    result = bytearray()
    while value > 0x7f:
        result.append((value & 0x7f) | 0x80)
        value >>= 7
    result.append(value)
    return bytes(result)


def position(x, y, z):
    return struct.pack(">Q", ((x & 0x3ffffff) << 38) | ((z & 0x3ffffff) << 12) | (y & 0xfff))


def fixtures(name, protocol):
    """Original semantic examples, independent of the library's Rust encoder."""
    pos = position(-33554432, -2048, 33554431)
    if name == "block_break_animation":
        return [("signed", varint(-1) + pos + b"\x80")]
    if name == "block_action":
        return [("raw", pos + b"\xff\x80" + varint(-2147483648))]
    if name == "open_sign_entity":
        return [("front", pos + b"\x01"), ("back", pos + b"\x00")]
    if name == "nbt_query_response":
        root = b"\x0a" + (b"\x00\x04root" if protocol == 763 else b"")
        # Compound: int 'x'=-1, byte-array 'a'=[0, -1, 127], empty-list 'l'.
        root += b"\x03\x00\x01x\xff\xff\xff\xff\x07\x00\x01a\x00\x00\x00\x03\x00\xff\x7f"
        root += b"\x09\x00\x01l\x01\x00\x00\x00\x00\x00"
        return [("compound", varint(-2147483648) + root), ("absent", varint(-1) + b"\x00")]
    if name == "collect":
        return [("signed", varint(-1) + varint(-2147483648) + varint(2147483647))]
    if name == "vehicle_move":
        # NaN payload, signed zero, +inf; -inf, a second payload NaN.
        return [("bits", bytes.fromhex("7ff800000000123480000000000000007ff0000000000000ff8000007fc01234"))]
    if name == "face_player":
        base = varint(1) + struct.pack(">ddd", -1.25, 2.5, -3.75)
        return [("entity", base + b"\x01" + varint(-1) + varint(0)),
                ("position", base + b"\x00"),
                ("raw-anchors", varint(-2147483648) + struct.pack(">ddd", -0.0, float("inf"), float("-inf")) + b"\x01" + varint(-1) + varint(2147483647))]
    if name == "player_rotation":
        return [("bits", bytes.fromhex("7fc01234") + (b"\x01" if protocol >= 773 else b"") + bytes.fromhex("80000000") + (b"\x00" if protocol >= 773 else b""))]
    if name == "set_projectile_power":
        power = bytes.fromhex("7ff8000000001234")
        if protocol == 766:
            power += bytes.fromhex("80000000000000007ff0000000000000")
        return [("bits", varint(-2147483648) + power)]
    if name == "set_ticking_state":
        return [("bits", bytes.fromhex("7fc01234") + b"\x01")]
    if name == "step_tick":
        return [("signed", varint(-2147483648))]
    raise ValueError(name)


def generate(schema_dir):
    records = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [r["protocol"] for r in records] != list(range(763, 777)):
        raise ValueError("Expected exactly fourteen protocol families")
    lines = ["# Synthetic Python-encoded fixtures; not release-API or live-server captures.",
             "# protocol\tname\tpacket_id\tcase\thex"]
    layouts = 0
    for row in records:
        protocol = row["protocol"]
        raw = (schema_dir / f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError(f"Schema hash mismatch: {row['schema']}")
        schema = json.loads(raw)
        types = schema["play"]["toClient"]["types"]
        if schema["types"].get("vec3f64") != container([("x", "f64"), ("y", "f64"), ("z", "f64")]):
            raise ValueError(f"Unexpected f64 vector in {protocol}")
        nbt_alias = "optionalNbt" if protocol == 763 else "anonOptionalNbt"
        if schema["types"].get(nbt_alias) != "native":
            raise ValueError(f"Unexpected optional NBT alias in {protocol}")
        mapping = types["packet"][1][0]["type"][1]["mappings"]
        if schema["types"]["position"] != ["bitfield", [{"name": "x", "size": 26, "signed": True}, {"name": "z", "size": 26, "signed": True}, {"name": "y", "size": 12, "signed": True}]]:
            raise ValueError(f"Unexpected packed position in {protocol}")
        for name in NAMES:
            expected = layout(name, protocol)
            ids = [int(i, 16) for i, n in mapping.items() if n == name]
            if types.get("packet_" + name) != expected or len(ids) != int(expected is not None):
                raise ValueError(f"Unexpected layout/presence: {protocol} {name}")
            if expected is None:
                continue
            layouts += 1
            for case, body in fixtures(name, protocol):
                lines.append(f"{protocol}\t{name}\t{ids[0]}\t{case}\t{body.hex()}")
    return "\n".join(lines) + "\n", layouts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=ROOT / "research/protocols")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    text, layouts = generate(args.schema_dir)
    path = ROOT / "tests/fixtures/world-control.tsv"
    if args.write:
        path.write_text(text)
    elif not path.exists() or path.read_text() != text:
        raise SystemExit("World-control fixtures are stale; review and run with --write")
    print(f"Verified {layouts} packet layouts and {len(text.splitlines()) - 2} synthetic fixtures across 14 hash-pinned families")
    print("763–764 entity-anchor schema defects are recorded explicitly; fixtures use source-backed VarInts")


if __name__ == "__main__":
    main()
