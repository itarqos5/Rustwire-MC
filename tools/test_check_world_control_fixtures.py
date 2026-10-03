"""Regression checks for the schema/independent-fixture verifier."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("world_control_check", Path(__file__).with_name("check_world_control_fixtures.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class WorldControlVerifierTests(unittest.TestCase):
    def mutate(self, mutate_schema, *, repin=True):
        records = json.loads((CHECK.ROOT / "research/schema-hashes.json").read_text())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "research").mkdir()
            schemas = root / "protocols"
            schemas.mkdir()
            for row in records:
                path = CHECK.ROOT / "research/protocols" / f"{row['schema']}.json"
                if row["protocol"] != 763:
                    (schemas / path.name).symlink_to(path)
                    continue
                schema = json.loads(path.read_text())
                mutate_schema(schema)
                data = json.dumps(schema).encode()
                (schemas / path.name).write_bytes(data)
                if repin:
                    row["sha256"] = hashlib.sha256(data).hexdigest()
            (root / "research/schema-hashes.json").write_text(json.dumps(records))
            with patch.object(CHECK, "ROOT", root):
                return CHECK.generate(schemas)

    def test_checked_in_fixtures_are_reproducible(self):
        text, layouts = CHECK.generate(CHECK.ROOT / "research/protocols")
        self.assertEqual(layouts, 142)
        self.assertEqual(len(text.splitlines()) - 2, 198)
        self.assertEqual(text, (CHECK.ROOT / "tests/fixtures/world-control.tsv").read_text())

    def test_unsigned_mask_and_position_bit_order(self):
        self.assertEqual(CHECK.varint(-1), bytes.fromhex("ffffffff0f"))
        self.assertEqual(CHECK.varint(-2147483648), bytes.fromhex("8080808008"))
        self.assertEqual(CHECK.position(-33554432, -2048, 33554431), bytes.fromhex("8000001ffffff800"))

    def test_hash_mismatch_fails_before_layout_check(self):
        with self.assertRaisesRegex(ValueError, "Schema hash mismatch"):
            self.mutate(lambda schema: None, repin=False)

    def test_changed_layout_is_rejected_even_if_repinned(self):
        def mutate(schema):
            schema["play"]["toClient"]["types"]["packet_collect"][1][2]["type"] = "u8"
        with self.assertRaisesRegex(ValueError, "Unexpected layout/presence"):
            self.mutate(mutate)

    def test_duplicate_id_mapping_is_rejected(self):
        def mutate(schema):
            schema["play"]["toClient"]["types"]["packet"][1][0]["type"][1]["mappings"]["0xff"] = "collect"
        with self.assertRaisesRegex(ValueError, "Unexpected layout/presence"):
            self.mutate(mutate)

    def test_absent_packet_cannot_gain_layout(self):
        def mutate(schema):
            schema["play"]["toClient"]["types"]["packet_step_tick"] = CHECK.layout("step_tick", 765)
        with self.assertRaisesRegex(ValueError, "Unexpected layout/presence"):
            self.mutate(mutate)

    def test_packed_position_drift_is_rejected(self):
        def mutate(schema):
            schema["types"]["position"][1][0]["size"] = 25
        with self.assertRaisesRegex(ValueError, "Unexpected packed position"):
            self.mutate(mutate)

    def test_vector_and_nbt_alias_drift_are_rejected(self):
        for name, error in [("vec3f64", "Unexpected f64 vector"), ("optionalNbt", "Unexpected optional NBT alias")]:
            def mutate(schema):
                schema["types"][name] = "u8"
            with self.assertRaisesRegex(ValueError, error):
                self.mutate(mutate)

    def test_historical_schema_defect_is_explicit(self):
        old = CHECK.layout("face_player", 763)[1][-1]["type"]
        new = CHECK.layout("face_player", 765)[1][-1]["type"]
        self.assertEqual(old[1]["fields"]["true"], "string")
        self.assertEqual(new[1]["fields"]["true"], "varint")
        self.assertEqual(CHECK.fixtures("face_player", 763), CHECK.fixtures("face_player", 765))

    def test_rotation_flags_start_at_773_and_are_interleaved(self):
        self.assertEqual([field["name"] for field in CHECK.layout("player_rotation", 772)[1]], ["yaw", "pitch"])
        self.assertEqual([field["name"] for field in CHECK.layout("player_rotation", 773)[1]], ["yaw", "relativeYaw", "pitch", "relativePitch"])


if __name__ == "__main__":
    unittest.main()
