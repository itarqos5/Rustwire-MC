#!/usr/bin/env python3
"""Offline hash/layout audit; schema evidence, not a release/runtime oracle."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def container(fields):
    return ["container", [{"name": name, "type": kind} for name, kind in fields]]


def array(kind):
    return ["array", {"countType": "varint", "type": kind}]


def expected_update(protocol):
    text = "string" if protocol < 765 else "anonymousNbt"
    icon = "slot" if protocol <= 765 else "Slot" if protocol < 775 else "ItemStackTemplate"
    flags = ["bitfield", [
        {"name": "_unused" if protocol <= 765 else "unused", "size": 29, "signed": False},
        {"name": "hidden", "size": 1, "signed": False},
        {"name": "show_toast", "size": 1, "signed": False},
        {"name": "has_background_texture", "size": 1, "signed": False},
    ]]
    display = container([
        ("title", text), ("description", text), ("icon", icon),
        ("frameType", "varint"), ("flags", flags),
        ("backgroundTexture", ["switch", {"compareTo": "flags/has_background_texture", "fields": {"1": "string"}, "default": "void"}]),
        ("xCord", "f32"), ("yCord", "f32"),
    ])
    advancement = [("parentId", ["option", "string"]), ("displayData", ["option", display])]
    if protocol == 763:
        advancement.append(("criteria", array(container([("key", "string"), ("value", "void")]))))
    advancement.extend([("requirements", array(array("string"))), ("sendsTelemtryData", "bool")])
    progress = array(container([("criterionIdentifier", "string"), ("criterionProgress", ["option", "i64"])]))
    fields = [
        ("reset", "bool"),
        ("advancementMapping", array(container([("key", "string"), ("value", container(advancement))]))),
        ("identifiers", array("string")),
        ("progressMapping", array(container([("key", "string"), ("value", progress)]))),
    ]
    if protocol >= 770:
        fields.append(("showAdvancements", "bool"))
    return container(fields)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=ROOT / "research/protocols")
    args = parser.parse_args()
    records = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [row["protocol"] for row in records] != list(range(763, 777)):
        raise SystemExit("Expected all fourteen explicit protocol families")
    for row in records:
        data = (args.schema_dir / f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise SystemExit(f"Schema hash mismatch: {row['schema']}")
        play = json.loads(data)["play"]
        for direction, name, expected in [
            ("toClient", "packet_advancements", expected_update(row["protocol"])),
            ("toClient", "packet_select_advancement_tab", container([("id", ["option", "string"])])),
            ("toServer", "packet_advancement_tab", container([
                ("action", "varint"), ("tabId", ["switch", {"compareTo": "action", "fields": {"0": "string", "1": "void"}}]),
            ])),
        ]:
            if play[direction]["types"].get(name) != expected:
                raise SystemExit(f"Unexpected {direction}/{name} layout in {row['schema']}")
        print(f"{row['protocol']} {row['schema']}: advancement layouts match")
    print("Verified 42 packet layouts; schema evidence only")


if __name__ == "__main__":
    main()
