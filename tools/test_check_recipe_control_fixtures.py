"""Regression checks for full-layout recipe-control verification."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from contextlib import contextmanager

SPEC = importlib.util.spec_from_file_location("recipe_check", Path(__file__).with_name("check_recipe_control_fixtures.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)

REPO_ROOT = CHECK.ROOT
FIXTURES = (REPO_ROOT / "tests/fixtures/recipe-control.tsv").read_text()

@contextmanager
def synthetic_schemas():
    """CI mutation corpus; the real pinned-schema audit is a separate command.

    No ignored research files or network are used. Committed independent fixture
    IDs drive all implemented packets; modern response stubs emit no fixtures.
    These constructed shapes exercise verifier logic, not upstream conformance.
    """
    ids = {}
    for line in FIXTURES.splitlines():
        if line.startswith("#"): continue
        protocol, direction, name, packet_id, _, _ = line.split("\t")
        ids[(int(protocol), direction, name)] = int(packet_id)
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "research").mkdir()
        schemas = root / "protocols"
        schemas.mkdir()
        records = []
        for protocol in range(763, 777):
            schema = {"types": {"varint": "native", "bool": "native", "u8": "native", "i8": "native", "string": ["pstring", {"countType": "varint"}]}, "play": {}}
            if protocol >= 766: schema["types"]["ContainerID"] = "u8" if protocol < 768 else "varint"
            for direction, names in CHECK.PACKETS.items():
                types, mapping, fields = {}, {}, {}
                if direction == "toClient" and protocol >= 771:
                    types["RecipeBookSetting"] = ["container", [{"name": "open", "type": "bool"}, {"name": "filtering", "type": "bool"}]]
                for name in names:
                    body = CHECK.layout(name, protocol)
                    if body is None: continue
                    types["packet_" + name] = body
                    mapping[hex(ids.get((protocol, direction, name), 250))] = name
                    fields[name] = "packet_" + name
                types["packet"] = ["container", [{"name": "name", "type": ["mapper", {"type": "varint", "mappings": mapping}]}, {"name": "params", "type": ["switch", {"compareTo": "name", "fields": fields}]}]]
                schema["play"][direction] = {"types": types}
            raw = json.dumps(schema).encode()
            (schemas / f"{protocol}.json").write_bytes(raw)
            records.append({"protocol": protocol, "schema": str(protocol), "sha256": hashlib.sha256(raw).hexdigest()})
        manifest = root / "research/schema-hashes.json"
        manifest.write_text(json.dumps(records))
        with patch.object(CHECK, "ROOT", root): yield root, schemas

class RecipeVerifierTests(unittest.TestCase):
    def mutate(self, callback, protocol=763, repin=True):
        with synthetic_schemas() as (root, schemas):
            manifest = root / "research/schema-hashes.json"
            records = json.loads(manifest.read_text())
            row = next(row for row in records if row["protocol"] == protocol)
            path = schemas / f"{protocol}.json"
            schema = json.loads(path.read_text()); callback(schema)
            raw = json.dumps(schema, indent=2).encode()
            path.write_bytes(raw)
            if repin: row["sha256"] = hashlib.sha256(raw).hexdigest()
            manifest.write_text(json.dumps(records))
            return CHECK.generate(schemas)

    def test_synthetic_corpus_matches_committed_fixtures(self):
        with synthetic_schemas() as (_, schemas): text, layouts = CHECK.generate(schemas)
        self.assertEqual(layouts, 79)
        self.assertEqual(len(text.splitlines()) - 2, 306)
        self.assertEqual(text, FIXTURES)

    def test_hash_checked(self):
        with self.assertRaisesRegex(ValueError, "Schema hash mismatch"):
            self.mutate(lambda schema: None, repin=False)

    def test_mutated_field_layout_even_when_repinned(self):
        def mutate(schema):
            schema["play"]["toServer"]["types"]["packet_craft_recipe_request"][1][0]["type"] = "varint"
        with self.assertRaisesRegex(ValueError, "Unexpected layout/presence"):
            self.mutate(mutate)

    def test_array_count_and_action_switch_are_checked(self):
        for index in [9, 10]:
            def mutate(schema):
                schema["play"]["toClient"]["types"]["packet_unlock_recipes"][1][index]["type"] = "varint"
            with self.assertRaisesRegex(ValueError, "Unexpected layout/presence"):
                self.mutate(mutate)

    def test_absence_and_duplicate_id_mappings_are_checked(self):
        def presence(schema): schema["play"]["toClient"]["types"]["packet_recipe_book_remove"] = CHECK.layout("recipe_book_remove", 768)
        def duplicate(schema): schema["play"]["toServer"]["types"]["packet"][1][0]["type"][1]["mappings"]["0xff"] = "recipe_book"
        for mutate in [presence, duplicate]:
            with self.assertRaisesRegex(ValueError, "Unexpected layout/presence"):
                self.mutate(mutate)

    def test_reachable_aliases_are_checked(self):
        for protocol, alias, location in [(766, "ContainerID", "global"), (771, "RecipeBookSetting", "client")]:
            def mutate(schema):
                types = schema["types"] if location == "global" else schema["play"]["toClient"]["types"]
                types[alias] = "varint"
            with self.assertRaisesRegex(ValueError, "Unexpected .* alias"):
                self.mutate(mutate, protocol=protocol)

    def test_primitive_and_packet_dispatch_aliases_are_checked(self):
        def primitive(schema): schema["types"]["string"] = "native"
        def dispatch(schema): schema["play"]["toServer"]["types"]["packet"][1][1]["type"][1]["fields"]["recipe_book"] = "packet_displayed_recipe"
        for mutate, error in [(primitive, "Unexpected primitive alias"), (dispatch, "Unexpected packet dispatch")]:
            with self.assertRaisesRegex(ValueError, error): self.mutate(mutate)

    def test_modern_response_checks_outer_layout_but_has_no_typed_fixture(self):
        self.assertEqual(CHECK.layout("craft_recipe_response", 768)[1][1]["type"], "RecipeDisplay")
        self.assertEqual(CHECK.fixtures("craft_recipe_response", 768), [])

    def test_sign_width_and_reference_boundary_are_deliberate(self):
        self.assertEqual(CHECK.varint(-1).hex(), "ffffffff0f")
        self.assertEqual(CHECK.varint(-2147483648).hex(), "8080808008")
        old = dict(CHECK.fixtures("craft_recipe_request", 767))
        modern = dict(CHECK.fixtures("craft_recipe_request", 768))
        self.assertEqual(old["window-byte-255"][0], 255)
        self.assertEqual(old["window-byte-128"][0], 128)
        self.assertTrue(modern["window-300"].startswith(bytes.fromhex("ac02ac02")))
        self.assertEqual(CHECK.layout("craft_recipe_request", 767)[1][1]["type"], "string")
        self.assertEqual(CHECK.layout("craft_recipe_request", 768)[1][1]["type"], "varint")

if __name__ == "__main__": unittest.main()
