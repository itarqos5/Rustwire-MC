#!/usr/bin/env python3
"""Audit five pinned legacy recipe schemas, then check original wire fixtures.

The schema's obsolete banner-add-pattern mapping is explicitly recorded, not
silently used as the 766/767 registry. Fixture IDs use the independently audited
23-kind release registry. Source inspection is not executed release-API evidence.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMES = ["crafting_shaped", "crafting_shapeless", "crafting_special_armordye",
         "crafting_special_bookcloning", "crafting_special_mapcloning", "crafting_special_mapextending",
         "crafting_special_firework_rocket", "crafting_special_firework_star", "crafting_special_firework_star_fade",
         "crafting_special_tippedarrow", "crafting_special_bannerduplicate", "crafting_special_shielddecoration",
         "crafting_special_shulkerboxcoloring", "crafting_special_suspiciousstew", "crafting_special_repairitem",
         "smelting", "blasting", "smoking", "campfire_cooking", "stonecutting", "smithing_transform",
         "smithing_trim", "crafting_decorated_pot"]
SCHEMA_NAMES = NAMES[:11] + ["crafting_special_banneraddpattern"] + NAMES[11:]

def container(fields): return ["container", [{"name": n, "type": t} for n, t in fields]]
def array(kind, count=None): return ["array", {"countType": "varint", "type": kind} if count is None else {"count": count, "type": kind}]
def switch(compare, fields, default=None):
    value = {"compareTo": compare, "fields": fields}
    if default is not None: value["default"] = default
    return ["switch", value]

def aliases(protocol):
    slot = "slot" if protocol <= 765 else "Slot"
    result = {"varint": "native", "i8": "native", "bool": "native", "f32": "native", "string": ["pstring", {"countType": "varint"}],
              "ingredient": array(slot), "minecraft_simple_recipe_format": container([("category", "varint")]),
              "minecraft_smelting_format": container([("group", "string"), ("category", "varint"), ("ingredient", "ingredient"), ("result", slot), ("experience", "f32"), ("cookTime", "varint")])}
    if protocol <= 765:
        inner = container([("itemId", "varint"), ("itemCount", "i8"), ("nbtData", "optionalNbt" if protocol == 763 else "anonOptionalNbt")])
        result[slot] = ["container", [{"name": "present", "type": "bool"}, {"anon": True, "type": switch("present", {"false": "void", "true": inner})}]]
        result["optionalNbt" if protocol == 763 else "anonOptionalNbt"] = "native"
    else:
        inner = container([("itemId", "varint"), ("addedComponentCount", "varint"), ("removedComponentCount", "varint"),
                           ("components", array("SlotComponent", "addedComponentCount")),
                           ("removeComponents", array(container([("type", "SlotComponentType")]), "removedComponentCount"))])
        result[slot] = ["container", [{"name": "itemCount", "type": "i8" if protocol == 766 else "varint"}, {"anon": True, "type": switch("itemCount", {"0": "void"}, inner)}]]
    return result

def layout(protocol):
    slot = "slot" if protocol <= 765 else "Slot"
    group = [("group", "string"), ("category", "varint")]
    width, height = ("width", "height") if protocol <= 765 else ("gridWidth", "gridHeight")
    dims = [(width, "varint"), (height, "varint")]
    kinds = {"minecraft:" + name: "minecraft_simple_recipe_format" for name in SCHEMA_NAMES}
    kinds.update({
        "minecraft:crafting_shapeless": container(group + [("ingredients", array("ingredient")), ("result", slot)]),
        "minecraft:crafting_shaped": container((dims + group if protocol < 765 else group + dims) + [("ingredients", array(array("ingredient", height), width)), ("result", slot), ("showNotification", "bool")]),
        "minecraft:stonecutting": container([("group", "string"), ("ingredient", "ingredient"), ("result", slot)]),
        "minecraft:smithing_transform": container([("template", "ingredient"), ("base", "ingredient"), ("addition", "ingredient"), ("result", slot)]),
        "minecraft:smithing_trim": container([("template", "ingredient"), ("base", "ingredient"), ("addition", "ingredient")]),
    })
    for name in NAMES[15:19]: kinds["minecraft:" + name] = "minecraft_smelting_format"
    header = [("type", "string"), ("recipeId", "string")] if protocol < 766 else [
        ("name", "string"), ("type", ["mapper", {"type": "varint", "mappings": {str(i): "minecraft:" + name for i, name in enumerate(SCHEMA_NAMES)}}])]
    return container([("recipes", array(container(header + [("data", switch("type", kinds))])))])

def varint(n):
    n &= 0xffffffff
    result = bytearray()
    while n > 127: result.append((n & 127) | 128); n >>= 7
    result.append(n)
    return bytes(result)
def string(value):
    value = value.encode()
    return varint(len(value)) + value

def slot(protocol, item=5, count=1):
    # Original nonempty compound NBT {v: byte(7)}. Named root only in 763.
    nbt = b"\x0a" + (b"\x00\x01r" if protocol == 763 else b"") + b"\x01\x00\x01v\x07\x00"
    if protocol <= 765: return b"\x01" + varint(item) + bytes((count,)) + nbt
    # custom_data compound + damage=5, remove repair_cost. Those IDs are
    # independently checked here against the full hash-pinned SlotComponentType.
    return varint(count) + varint(item) + b"\x02\x01\x00" + nbt + b"\x03\x05\x10"
def ingredient(protocol, variant=0):
    if variant == 0: return b"\x00"
    if variant == 1: return b"\x01" + slot(protocol)
    return b"\x03" + slot(protocol, 9, 2) + b"\x00" + slot(protocol, 9, 2)
def entry(protocol, kind, payload=None, key="rustwire:recipe", spelling=None):
    name = spelling if spelling is not None else "minecraft:" + NAMES[kind]
    header = string(name) + string(key) if protocol < 766 else string(key) + varint(kind)
    if payload is None:
        result = slot(protocol, 300, 127 if protocol <= 765 else 300)
        group = string("group/雪")
        if kind == 0:
            prefix = b"\x02\x01" + group + b"\x03" if protocol < 765 else group + b"\x03\x02\x01"
            payload = prefix + ingredient(protocol, 0) + ingredient(protocol, 2) + result + b"\x01"
        elif kind == 1: payload = group + b"\x02\x02" + ingredient(protocol, 1) + ingredient(protocol, 2) + result
        elif kind in range(15, 19): payload = group + varint((kind - 15) % 3) + ingredient(protocol, 2) + result + struct.pack(">f", 0.625) + varint(200)
        elif kind == 19: payload = group + ingredient(protocol, 2) + result
        elif kind in (20, 21): payload = ingredient(protocol, 1) + ingredient(protocol, 2) + ingredient(protocol, 0) + (result if kind == 20 else b"")
        else: payload = varint(kind % 4)
    return header + payload

def fixtures(protocol):
    result = [("kind-" + str(kind), b"\x01" + entry(protocol, kind)) for kind in range(len(NAMES))]
    result += [("empty", b"\x00"), ("all-kinds", varint(len(NAMES)) + b"".join(entry(protocol, k, key=f"rustwire:recipe/{k}") for k in range(len(NAMES))))]
    if protocol < 766:
        result += [("unqualified", b"\x01" + entry(protocol, 2, spelling=NAMES[2])), ("empty-namespace", b"\x01" + entry(protocol, 2, spelling=":" + NAMES[2]))]
        unknown = string("example:custom") + string("rustwire:custom")
        obsolete = string("minecraft:crafting_special_banneraddpattern") + string("rustwire:obsolete")
        result += [("unsupported-obsolete", b"\x01" + obsolete + b"\x03")]
    else: unknown = string("rustwire:custom") + varint(999)
    result += [("unsupported-custom", b"\x01" + unknown + b"\xff\x00\xfe"), ("unsupported-after-known", b"\x02" + entry(protocol, 2) + unknown + b"\x00\xff")]
    return result

def generate(schema_dir):
    rows = [r for r in json.loads((ROOT / "research/schema-hashes.json").read_text()) if 763 <= r["protocol"] <= 767]
    if [r["protocol"] for r in rows] != list(range(763, 768)): raise ValueError("Expected five legacy families")
    lines = ["# Original independent Python wire fixtures; not live captures or executed release APIs.", "# protocol\tdirection\tname\tpacket_id\tcase\thex"]
    for row in rows:
        protocol = row["protocol"]
        raw = (schema_dir / (row["schema"] + ".json")).read_bytes()
        if hashlib.sha256(raw).hexdigest() != row["sha256"]: raise ValueError("Schema hash mismatch")
        schema = json.loads(raw)
        for name, expected in aliases(protocol).items():
            if schema["types"].get(name) != expected: raise ValueError(f"Unexpected alias: {protocol} {name}")
        if protocol >= 766:
            names = schema["types"]["SlotComponentType"]
            if names[0] != "mapper" or names[1]["type"] != "varint" or any(names[1]["mappings"].get(str(i)) != name for i, name in [(0, "custom_data"), (3, "damage"), (16, "repair_cost")]): raise ValueError("Unexpected fixture component registry")
            comp = schema["types"]["SlotComponent"]
            if comp[0] != "container" or comp[1][0] != {"name": "type", "type": "SlotComponentType"} or comp[1][1]["type"][0] != "switch" or comp[1][1]["type"][1]["compareTo"] != "type": raise ValueError("Unexpected component dispatch")
            fields = comp[1][1]["type"][1]["fields"]
            if fields.get("custom_data") != "anonymousNbt" or fields.get("damage") != "varint" or schema["types"].get("anonymousNbt") != "native": raise ValueError("Unexpected fixture component body")
        types = schema["play"]["toClient"]["types"]
        if types.get("packet_declare_recipes") != layout(protocol): raise ValueError("Unexpected declaration layout")
        if "packet_declare_recipes" in schema["play"]["toServer"]["types"]: raise ValueError("Unexpected declaration direction")
        dispatch = types["packet"]
        ids = [int(i, 16) for i, name in dispatch[1][0]["type"][1]["mappings"].items() if name == "declare_recipes"]
        if dispatch[0] != "container" or dispatch[1][0]["name"] != "name" or dispatch[1][0]["type"][0] != "mapper" or dispatch[1][0]["type"][1]["type"] != "varint": raise ValueError("Unexpected declaration mapper")
        server_mapping = schema["play"]["toServer"]["types"].get("packet")
        if server_mapping and "declare_recipes" in server_mapping[1][0]["type"][1]["mappings"].values(): raise ValueError("Unexpected declaration direction")
        if len(ids) != 1: raise ValueError("Unexpected declaration ID count")
        if dispatch[1][1]["type"][0] != "switch" or dispatch[1][1]["type"][1]["compareTo"] != "name" or dispatch[1][1]["type"][1]["fields"].get("declare_recipes") != "packet_declare_recipes": raise ValueError("Unexpected declaration dispatch")
        for case, body in fixtures(protocol): lines.append(f"{protocol}\ttoClient\tdeclare_recipes\t{ids[0]}\t{case}\t{body.hex()}")
    return "\n".join(lines) + "\n"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir", type=Path, default=ROOT / "research/protocols")
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    text = generate(args.schema_dir)
    path = ROOT / "tests/fixtures/legacy-recipes.tsv"
    if args.write: path.write_text(text)
    elif not path.exists() or path.read_text() != text: raise SystemExit("Legacy recipe fixtures stale")
    print(f"Verified five complete declaration layouts, direct ingredient/slot aliases and {len(text.splitlines()) - 2} independent synthetic fixtures")
    print("Schema banneraddpattern anomaly is explicit; fixtures use audited 23-kind IDs. Modern declarations excluded.")
if __name__ == "__main__": main()
