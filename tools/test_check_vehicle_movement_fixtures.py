"""Offline verifier regressions; the unit suite needs no downloaded schemas."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("vehicle_check", Path(__file__).with_name("check_vehicle_movement_fixtures.py"))
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class VehicleMovementVerifierTests(unittest.TestCase):
    def synthetic_inputs(self, mutate=None, *, protocol=769, repin=True, mutate_records=None):
        expected = (CHECK.ROOT / "tests/fixtures/vehicle-movement.tsv").read_text()
        ids = {}
        for line in expected.splitlines():
            if not line.startswith("#"):
                version, direction, packet_id, _, _ = line.split("\t")
                ids[(int(version), direction)] = int(packet_id)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "research").mkdir()
            schemas = root / "schemas"
            schemas.mkdir()
            records = []
            for version in range(763, 777):
                schema = {"types": {kind: "native" for kind in ("f32", "f64", "bool")}, "play": {}}
                for direction, key in CHECK.DIRECTIONS:
                    schema["play"][key] = {"types": {
                        "packet_vehicle_move": CHECK.layout(version, direction),
                        "packet": ["container", [
                            {"name": "name", "type": ["mapper", {"mappings": {hex(ids[(version, direction)]): "vehicle_move"}}]},
                            {"name": "params", "type": ["switch", {"compareTo": "name", "fields": {"vehicle_move": "packet_vehicle_move"}}]},
                        ]],
                    }}
                original = json.dumps(schema).encode()
                if version == protocol and mutate:
                    mutate(schema)
                data = json.dumps(schema).encode()
                if version == protocol and not repin:
                    data += b"\n"
                records.append({"protocol": version, "schema": str(version),
                                "sha256": hashlib.sha256(data if repin else original).hexdigest()})
                (schemas / f"{version}.json").write_bytes(data)
            if mutate_records:
                mutate_records(records)
            (root / "research/schema-hashes.json").write_text(json.dumps(records))
            with patch.object(CHECK, "ROOT", root):
                return CHECK.generate(schemas)

    def test_reproduces_committed_bytes_without_downloads(self):
        text, layouts = self.synthetic_inputs()
        self.assertEqual(layouts, 28)
        self.assertEqual(len(text.splitlines()) - 2, 108)
        self.assertEqual(text, (CHECK.ROOT / "tests/fixtures/vehicle-movement.tsv").read_text())

    def test_hash_mismatch_precedes_schema_validation(self):
        with self.assertRaisesRegex(ValueError, "Schema hash mismatch"):
            self.synthetic_inputs(lambda s: s.clear(), repin=False)

    def test_exact_ordered_families_required(self):
        for change in [lambda rows: rows.pop(), lambda rows: rows.reverse(), lambda rows: rows.append(rows[0])]:
            with self.assertRaisesRegex(ValueError, "fourteen ordered"):
                self.synthetic_inputs(mutate_records=change)

    def test_ground_boundary_is_directional_and_exact(self):
        self.assertEqual(len(CHECK.layout(768, "serverbound")[1]), 5)
        self.assertEqual(CHECK.layout(769, "serverbound")[1][-1], {"name": "onGround", "type": "bool"})
        for version in range(763, 777):
            self.assertEqual(len(CHECK.layout(version, "clientbound")[1]), 5)

    def test_missing_or_extra_ground_field_is_rejected_when_repinned(self):
        for version, key, replacement in [(768, "toServer", CHECK.layout(769, "serverbound")),
                                           (769, "toServer", CHECK.layout(768, "serverbound")),
                                           (776, "toClient", CHECK.layout(776, "serverbound"))]:
            def mutate(schema):
                schema["play"][key]["types"]["packet_vehicle_move"] = replacement
            with self.assertRaisesRegex(ValueError, "layout/presence"):
                self.synthetic_inputs(mutate, protocol=version)

    def test_width_and_order_drift_rejected(self):
        for index, replacement in [(0, {"name": "x", "type": "f32"}), (3, {"name": "pitch", "type": "f32"}),
                                    (5, {"name": "onGround", "type": "u8"})]:
            def mutate(schema):
                schema["play"]["toServer"]["types"]["packet_vehicle_move"][1][index] = replacement
            with self.assertRaisesRegex(ValueError, "layout/presence"):
                self.synthetic_inputs(mutate)

    def test_scalar_alias_drift_rejected(self):
        for primitive in ("f32", "f64", "bool"):
            with self.assertRaisesRegex(ValueError, "scalar alias"):
                self.synthetic_inputs(lambda schema: schema["types"].update({primitive: "u8"}))

    def test_missing_or_duplicate_direction_mapping_rejected(self):
        for key in ("toServer", "toClient"):
            for duplicate in [False, True]:
                def mutate(schema):
                    mapping = schema["play"][key]["types"]["packet"][1][0]["type"][1]["mappings"]
                    if duplicate:
                        mapping["0xff"] = "vehicle_move"
                    else:
                        mapping.clear()
                with self.assertRaisesRegex(ValueError, "layout/presence"):
                    self.synthetic_inputs(mutate)

    def test_dispatch_link_drift_rejected(self):
        for key in ("toServer", "toClient"):
            def mutate(schema):
                schema["play"][key]["types"]["packet"][1][1]["type"][1]["fields"]["vehicle_move"] = "void"
            with self.assertRaisesRegex(ValueError, "vehicle dispatch"):
                self.synthetic_inputs(mutate)

    def test_synthetic_fields_have_independent_lengths_and_bits(self):
        old = dict(CHECK.fixtures(768, "serverbound"))
        new = dict(CHECK.fixtures(769, "serverbound"))
        self.assertEqual(old["finite"].hex(), "bff40000000000004004000000000000c00e00000000000043e10000c3340000")
        self.assertEqual(old["bits"].hex(), "7ff800000000123480000000000000007ff0000000000000ff8000007fc01234")
        for case, body in old.items():
            self.assertEqual(len(body), 32)
            self.assertEqual(new[case + "-air"], body + b"\x00")
            self.assertEqual(new[case + "-ground"], body + b"\x01")
        self.assertEqual(CHECK.fixtures(776, "clientbound"), CHECK.fixtures(768, "serverbound"))


if __name__ == "__main__":
    unittest.main()
