"""Regressions for the standalone schema/fixture verifier (standard library only)."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "entity_control_fixtures", Path(__file__).with_name("check_entity_control_fixtures.py")
)
FIXTURES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURES)


class EntityControlFixturesTests(unittest.TestCase):
    def test_independent_varint_extrema(self):
        for number, expected in [
            (0, "00"), (127, "7f"), (128, "8001"), (300, "ac02"),
            (2**31-1, "ffffffff07"), (-1, "ffffffff0f"),
            (-2**31, "8080808008"),
        ]:
            self.assertEqual(FIXTURES.varint(number).hex(), expected)
        for number in (-2**31-1, 2**31):
            with self.assertRaises(ValueError):
                FIXTURES.varint(number)

    def build_fake(self, change=None, bad_hash=False, missing_protocol=False):
        """Exercise validation without requiring downloaded research inputs."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "research").mkdir()
            schemas = root / "schemas"
            schemas.mkdir()
            rows = []
            for protocol in range(763, 777):
                data = {
                    "types": {"vec3f64": FIXTURES.container([(axis, "f64") for axis in "xyz"])},
                    "play": {"toClient": {"types": {
                        "packet_" + name: layout
                        for name, layout in FIXTURES.LAYOUTS.items()
                    }}},
                }
                data["play"]["toClient"]["types"]["packet"] = [
                    "container", [{"type": ["mapper", {"mappings": {
                        hex(index): name for index, name in enumerate(FIXTURES.LAYOUTS)
                    }}]}],
                ]
                if change is not None and protocol == 776:
                    change(data)
                raw = json.dumps(data).encode()
                (schemas / f"{protocol}.json").write_bytes(raw)
                rows.append({"protocol": protocol, "schema": str(protocol),
                             "sha256": hashlib.sha256(raw).hexdigest()})
            if bad_hash:
                rows[-1]["sha256"] = "0" * 64
            if missing_protocol:
                rows.pop()
            (root / "research/schema-hashes.json").write_text(json.dumps(rows))
            with patch.object(FIXTURES, "ROOT", root):
                return FIXTURES.build(schemas)

    def test_complete_matrix_has_224_bodies(self):
        result = self.build_fake()
        self.assertEqual(len([line for line in result.splitlines() if not line.startswith("#")]), 224)

    def test_bad_hash_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Schema hash mismatch for 776"):
            self.build_fake(bad_hash=True)

    def test_missing_protocol_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "exact fourteen"):
            self.build_fake(missing_protocol=True)

    def test_layout_drift_is_rejected_even_with_updated_hash(self):
        def change(data):
            data["play"]["toClient"]["types"]["packet_animation"] = FIXTURES.container([
                ("entityId", "varint"), ("animation", "varint")
            ])
        with self.assertRaisesRegex(ValueError, "Layout drift for 776 animation"):
            self.build_fake(change=change)

    def test_nested_vector_drift_is_rejected(self):
        def change(data):
            data["types"]["vec3f64"] = FIXTURES.container([(axis, "f32") for axis in "xyz"])
        with self.assertRaisesRegex(ValueError, "vec3f64 layout drift for 776"):
            self.build_fake(change=change)


if __name__ == "__main__":
    unittest.main()
