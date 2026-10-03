#!/usr/bin/env python3
"""Verify HUD/player-feedback layouts and fixture IDs from hash-pinned schemas.

Read-only structural comparison; no downloads, release-API or live-server claim.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def container(*fields):
    return ["container", [{"name": name, "type": kind} for name, kind in fields]]


def expected(protocol):
    component = "string" if protocol < 765 else "anonymousNbt"
    hand = "varint"
    if protocol == 775:
        hand = ["mapper", {"type": "varint", "mappings": {"0": "main_hand", "1": "off_hand"}}]
    return {
        "clear_titles": container(("reset", "bool")),
        "action_bar": container(("text", component)),
        "set_title_text": container(("text", component)),
        "set_title_subtitle": container(("text", component)),
        "set_title_time": container(("fadeIn", "i32"), ("stay", "i32"), ("fadeOut", "i32")),
        "open_book": container(("hand", hand)),
        "experience": container(("experienceBar", "f32"), ("level", "varint"), ("totalExperience", "varint")),
        "enter_combat_event": container(),
        "end_combat_event": container(("duration", "varint")),
        "death_combat_event": container(("playerId", "varint"), ("message", component)),
    }


def verify(schema_dir, fixture_path):
    records = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [r["protocol"] for r in records] != list(range(763, 777)):
        raise ValueError("Expected all fourteen explicit protocol families")
    fixtures = {}
    for line in fixture_path.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        protocol, name, packet_id, body = line.split("\t")
        key = (int(protocol), name)
        if key in fixtures:
            raise ValueError(f"Duplicate fixture: {key}")
        bytes.fromhex(body)
        fixtures[key] = int(packet_id)
    checked = set()
    for row in records:
        data = (schema_dir / f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise ValueError(f"Schema hash mismatch: {row['schema']}")
        types = json.loads(data)["play"]["toClient"]["types"]
        packet = types["packet"][1]
        mappings = packet[0]["type"][1]["mappings"]
        dispatch = packet[1]["type"][1]
        if dispatch["compareTo"] != "name":
            raise ValueError("Unexpected packet discriminator")
        for name, layout in expected(row["protocol"]).items():
            if types.get("packet_" + name) != layout:
                raise ValueError(f"Unexpected {name} layout in {row['schema']}")
            if dispatch["fields"].get(name) != "packet_" + name:
                raise ValueError(f"Unexpected {name} dispatch in {row['schema']}")
            ids = [int(raw_id, 0) for raw_id, packet_name in mappings.items() if packet_name == name]
            key = (row["protocol"], name)
            if len(ids) != 1 or fixtures.get(key) != ids[0]:
                raise ValueError(f"Missing/incorrect fixture ID: {key}")
            checked.add(key)
        print(f"{row['protocol']} {row['schema']}: ten HUD/combat layouts and packet IDs match")
    if fixtures.keys() != checked:
        raise ValueError("Unexpected fixture keys")
    print("Verified 140 complete layouts and fixture IDs; schema evidence only")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=ROOT / "research/protocols")
    parser.add_argument("--fixtures", type=Path, default=ROOT / "tests/fixtures/hud.tsv")
    args = parser.parse_args()
    try:
        verify(args.schema_dir, args.fixtures)
    except (ValueError, KeyError) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
