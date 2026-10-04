#!/usr/bin/env python3
"""Check hash-pinned recipe-control schemas and independently encoded fixtures.

No Rust encoder, game implementation, network or captured traffic is used.
Legacy signedness disagreements are preserved as raw byte identities in the
synthetic fixtures, using the source-level audit documented separately.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKETS = {
    "toServer": ("craft_recipe_request", "recipe_book", "displayed_recipe"),
    "toClient": ("craft_recipe_response", "unlock_recipes", "recipe_book_settings", "recipe_book_remove"),
}

def container(fields):
    return ["container", [{"name": n, "type": t} for n, t in fields]]

def array(kind):
    return ["array", {"countType": "varint", "type": kind}]

def layout(name, protocol):
    if name == "craft_recipe_request":
        return container([("windowId", "i8" if protocol < 767 else "ContainerID"),
                          ("recipe", "string") if protocol < 768 else ("recipeId", "varint"), ("makeAll", "bool")])
    if name == "displayed_recipe":
        return container([("recipeId", "string" if protocol < 768 else "varint")])
    if name == "recipe_book":
        return container([("bookId", "varint"), ("bookOpen", "bool"), ("filterActive", "bool")])
    if name == "craft_recipe_response":
        return container([("windowId", "i8" if protocol < 766 else "ContainerID"),
                          ("recipe", "string") if protocol < 768 else ("recipeDisplay", "RecipeDisplay")])
    if name == "unlock_recipes":
        if protocol >= 768: return None
        pairs = ["craftingBookOpen", "filteringCraftable", "smeltingBookOpen", "filteringSmeltable",
                 "blastFurnaceOpen", "filteringBlastFurnace", "smokerBookOpen", "filteringSmoker"]
        return container([("action", "varint")] + [(n, "bool") for n in pairs] + [
            ("recipes1", array("string")), ("recipes2", ["switch", {"compareTo": "action", "fields": {"0": array("string")}, "default": "void"}])])
    if name == "recipe_book_remove":
        return None if protocol < 768 else container([("recipeIds", array("varint"))])
    if name == "recipe_book_settings":
        if protocol < 768: return None
        if protocol >= 771:
            return container([(n, "RecipeBookSetting") for n in ("crafting", "furnace", "blast", "smoker")])
        return container([(n, "bool") for n in ("craftingGuiOpen", "craftingFilteringCraftable", "smeltingGuiOpen", "smeltingFilteringCraftable", "blastGuiOpen", "blastFilteringCraftable", "smokerGuiOpen", "smokerFilteringCraftable")])
    raise ValueError(name)

def varint(n):
    n &= 0xffffffff
    output = bytearray()
    while n > 127:
        output.append((n & 127) | 128)
        n >>= 7
    output.append(n)
    return bytes(output)

def string(text):
    encoded = text.encode("utf-8")
    return varint(len(encoded)) + encoded

def keys(values):
    return varint(len(values)) + b"".join(string(v) for v in values)

SETTINGS = bytes((1, 0, 0, 1, 1, 1, 0, 0))

def fixtures(name, protocol):
    if name == "recipe_book":
        return [(f"book-{id}", varint(id) + bytes((id & 1, not (id & 1)))) for id in range(4)] + [("unknown-book", varint(-1) + b"\x01\x00")]
    if name == "displayed_recipe":
        return [("registry-key", string("rustwire:test/recipe"))] if protocol < 768 else [(f"display-{id}", varint(id)) for id in (-2147483648, -1, 0, 127, 128, 255, 300, 2147483647)]
    if name in ("craft_recipe_request", "craft_recipe_response"):
        if name == "craft_recipe_response" and protocol >= 768: return []
        ids = (0, 127, 128, 255) if protocol < 768 else (-2147483648, -1, 0, 127, 128, 255, 300, 2147483647)
        result = []
        for id in ids:
            window = bytes((id & 255,)) if protocol < 768 else varint(id)
            recipe = string("rustwire:test/recipe") if protocol < 768 else varint(-1 if id < 0 else 300)
            tail = bytes((id & 1,)) if name == "craft_recipe_request" else b""
            case = f"window-byte-{id}" if protocol < 768 else f"window-{id}"
            result.append((case, window + recipe + tail))
        return result
    if name == "unlock_recipes":
        return [("init", b"\x00" + SETTINGS + keys(["rustwire:first", "rustwire:second", "rustwire:first"]) + keys(["rustwire:second"])),
                ("add", b"\x01" + SETTINGS + keys(["rustwire:first"])),
                ("remove", b"\x02" + SETTINGS + keys([])),
                ("unknown-action", varint(-1) + SETTINGS + keys(["rustwire:first"]))]
    if name == "recipe_book_settings": return [("mixed", SETTINGS)]
    if name == "recipe_book_remove":
        ids = [-2147483648, -1, 0, 127, 128, 255, 300, 2147483647, 300]
        return [("signed-ids", varint(len(ids)) + b"".join(varint(n) for n in ids)), ("empty", b"\x00")]
    raise ValueError(name)

def generate(schema_dir):
    records = json.loads((ROOT / "research/schema-hashes.json").read_text())
    if [r["protocol"] for r in records] != list(range(763, 777)):
        raise ValueError("Expected exactly fourteen protocol families")
    lines = ["# Original Python-encoded synthetic fixtures; not release-API or live-server captures.",
             "# protocol\tdirection\tname\tpacket_id\tcase\thex"]
    layouts = 0
    for row in records:
        protocol = row["protocol"]
        raw = (schema_dir / f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError(f"Schema hash mismatch: {row['schema']}")
        schema = json.loads(raw)
        primitives = {"varint": "native", "bool": "native", "u8": "native", "i8": "native", "string": ["pstring", {"countType": "varint"}]}
        for alias, expected in primitives.items():
            if schema["types"].get(alias) != expected:
                raise ValueError(f"Unexpected primitive alias: {protocol} {alias}")
        if protocol >= 766 and schema["types"].get("ContainerID") != ("u8" if protocol < 768 else "varint"):
            raise ValueError(f"Unexpected ContainerID alias: {protocol}")
        for direction, names in PACKETS.items():
            types = schema["play"][direction]["types"]
            mapping = types["packet"][1][0]["type"][1]["mappings"]
            packet_switch = types["packet"][1][1]["type"]
            if direction == "toClient" and protocol >= 771 and types.get("RecipeBookSetting") != container([("open", "bool"), ("filtering", "bool")]):
                raise ValueError(f"Unexpected RecipeBookSetting alias: {protocol}")
            for name in names:
                expected = layout(name, protocol)
                ids = [int(i, 16) for i, n in mapping.items() if n == name]
                if types.get("packet_" + name) != expected or len(ids) != int(expected is not None):
                    raise ValueError(f"Unexpected layout/presence: {protocol} {direction} {name}")
                if expected is None: continue
                if packet_switch[0] != "switch" or packet_switch[1]["compareTo"] != "name" or packet_switch[1]["fields"].get(name) != "packet_" + name:
                    raise ValueError(f"Unexpected packet dispatch: {protocol} {direction} {name}")
                layouts += 1
                for case, body in fixtures(name, protocol):
                    lines.append(f"{protocol}\t{direction}\t{name}\t{ids[0]}\t{case}\t{body.hex()}")
    return "\n".join(lines) + "\n", layouts

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=ROOT / "research/protocols")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    text, layouts = generate(args.schema_dir)
    path = ROOT / "tests/fixtures/recipe-control.tsv"
    if args.write: path.write_text(text)
    elif not path.exists() or path.read_text() != text:
        raise SystemExit("Recipe-control fixtures stale; review and run with --write")
    print(f"Verified {layouts} outer layouts and {len(text.splitlines()) - 2} synthetic fixtures across 14 hash-pinned families")
    print("Modern RecipeDisplay internals excluded; legacy raw-byte interpretation explicitly audited")

if __name__ == "__main__": main()
