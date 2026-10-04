"""Self-contained checker regressions; no downloaded schemas or network needed."""
from contextlib import contextmanager
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("legacy_recipe_check", Path(__file__).with_name("check_legacy_recipe_fixtures.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)
FIXTURES = (CHECK.ROOT / "tests/fixtures/legacy-recipes.tsv").read_text()

@contextmanager
def synthetic_schemas():
    ids = {int(line.split("\t")[0]): int(line.split("\t")[3]) for line in FIXTURES.splitlines() if line and not line.startswith("#")}
    with tempfile.TemporaryDirectory() as path:
        root = Path(path); (root / "research").mkdir(); schemas = root / "schemas"; schemas.mkdir(); records = []
        for p in range(763, 768):
            types = CHECK.aliases(p)
            if p >= 766:
                types["SlotComponentType"] = ["mapper", {"type": "varint", "mappings": {"0": "custom_data", "3": "damage", "16": "repair_cost"}}]
                types["SlotComponent"] = CHECK.container([("type", "SlotComponentType"), ("data", CHECK.switch("type", {"custom_data": "anonymousNbt", "damage": "varint"}))])
                types["anonymousNbt"] = "native"
            schema = {"types": types, "play": {"toServer": {"types": {}}, "toClient": {"types": {
                "packet_declare_recipes": CHECK.layout(p),
                "packet": CHECK.container([("name", ["mapper", {"type": "varint", "mappings": {hex(ids[p]): "declare_recipes"}}]), ("params", CHECK.switch("name", {"declare_recipes": "packet_declare_recipes"}))])}}}}
            raw = json.dumps(schema).encode(); (schemas / f"{p}.json").write_bytes(raw)
            records.append({"protocol": p, "schema": str(p), "sha256": hashlib.sha256(raw).hexdigest()})
        (root / "research/schema-hashes.json").write_text(json.dumps(records))
        with patch.object(CHECK, "ROOT", root): yield root, schemas

class LegacyRecipeVerifierTests(unittest.TestCase):
    def mutate(self, callback, protocol=763, repin=True):
        with synthetic_schemas() as (root, schemas):
            path = schemas / f"{protocol}.json"; schema = json.loads(path.read_text()); callback(schema)
            raw = json.dumps(schema, indent=2).encode(); path.write_bytes(raw)
            manifest = root / "research/schema-hashes.json"; rows = json.loads(manifest.read_text())
            if repin: next(r for r in rows if r["protocol"] == protocol)["sha256"] = hashlib.sha256(raw).hexdigest()
            manifest.write_text(json.dumps(rows)); return CHECK.generate(schemas)
    def test_no_schema_cache_is_required(self):
        with synthetic_schemas() as (_, schemas): self.assertEqual(CHECK.generate(schemas), FIXTURES)
    def test_hash(self):
        with self.assertRaisesRegex(ValueError, "hash mismatch"): self.mutate(lambda _: None, repin=False)
    def test_full_declaration_layout(self):
        def change(s): s["play"]["toClient"]["types"]["packet_declare_recipes"][1][0]["type"][1]["type"][1][-1]["type"][1]["fields"]["minecraft:crafting_shaped"][1][-1]["type"] = "varint"
        with self.assertRaisesRegex(ValueError, "declaration layout"): self.mutate(change)
    def test_registry_anomaly_is_checked_and_not_used(self):
        self.assertEqual(CHECK.NAMES[11], "crafting_special_shielddecoration")
        self.assertEqual(CHECK.SCHEMA_NAMES[11], "crafting_special_banneraddpattern")
        def change(s): s["play"]["toClient"]["types"]["packet_declare_recipes"][1][0]["type"][1]["type"][1][1]["type"][1]["mappings"]["11"] = "minecraft:crafting_special_shielddecoration"
        with self.assertRaisesRegex(ValueError, "declaration layout"): self.mutate(change, 766)
    def test_every_direct_alias(self):
        for p in range(763, 768):
            for alias in CHECK.aliases(p):
                with self.assertRaisesRegex(ValueError, "Unexpected alias"): self.mutate(lambda s: s["types"].__setitem__(alias, "wrong"), p)
    def test_component_aliases_used_by_fixtures(self):
        def registry(s): s["types"]["SlotComponentType"][1]["mappings"]["3"] = "wrong"
        def body(s): s["types"]["SlotComponent"][1][1]["type"][1]["fields"]["damage"] = "i8"
        def dispatch(s): s["types"]["SlotComponent"][1][1]["type"][1]["compareTo"] = "wrong"
        for change in [registry, body, dispatch]:
            with self.assertRaisesRegex(ValueError, "Unexpected .*component"): self.mutate(change, 766)
    def test_direction_unique_id_and_dispatch(self):
        def direction(s): s["play"]["toServer"]["types"]["packet_declare_recipes"] = "void"
        def duplicate(s): s["play"]["toClient"]["types"]["packet"][1][0]["type"][1]["mappings"]["0xff"] = "declare_recipes"
        def dispatch(s): s["play"]["toClient"]["types"]["packet"][1][1]["type"][1]["fields"]["declare_recipes"] = "wrong"
        for change in [direction, duplicate, dispatch]:
            with self.assertRaisesRegex(ValueError, "Unexpected declaration"): self.mutate(change)
    def test_boundary_shapes_and_original_slot_count(self):
        self.assertTrue(CHECK.slot(766, 300, 300).startswith(bytes.fromhex("ac02ac02")))
        self.assertTrue(CHECK.slot(767, 300, 300).startswith(bytes.fromhex("ac02ac02")))
        self.assertNotEqual(CHECK.entry(764, 0), CHECK.entry(765, 0))
        self.assertEqual(len(CHECK.NAMES), 23)

if __name__ == "__main__": unittest.main()
