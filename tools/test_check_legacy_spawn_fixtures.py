"""Offline regressions: construct minimal schemas instead of downloading inputs."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "legacy_spawn_fixtures", Path(__file__).with_name("check_legacy_spawn_fixtures.py")
)
F = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(F)


class LegacySpawnFixturesTests(unittest.TestCase):
    def build_fake(self, change=None, protocol=763, bad_hash=False, missing_protocol=False):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "research").mkdir()
            schemas = root / "schemas"
            schemas.mkdir()
            rows = []
            for current in range(763, 777):
                names = [n for n in F.LAYOUTS if F.present(current, n)]
                types = {"packet_" + n: F.LAYOUTS[n] for n in names}
                types["packet"] = ["container", [
                    {"name": "name", "type": ["mapper", {"mappings": {
                        hex(2 if n == F.ORB else 3): n for n in names}}]},
                    {"name": "params", "type": ["switch", {"compareTo": "name", "fields": {
                        n: "packet_" + n for n in names}}]},
                ]]
                if current == protocol and change:
                    change(types)
                raw = json.dumps({"play": {"toClient": {"types": types}}}).encode()
                (schemas / f"{current}.json").write_bytes(raw)
                rows.append({"protocol": current, "schema": str(current),
                             "sha256": hashlib.sha256(raw).hexdigest()})
            if bad_hash:
                rows[-1]["sha256"] = "0" * 64
            if missing_protocol:
                rows.pop()
            (root / "research/schema-hashes.json").write_text(json.dumps(rows))
            with patch.object(F, "ROOT", root):
                return F.build(schemas)

    def test_complete_exact_presence_matrix(self):
        lines = [s for s in self.build_fake().splitlines() if not s.startswith("#")]
        self.assertEqual(len(lines), 62)
        self.assertEqual(sum("\tnamed_entity_spawn\t" in s for s in lines), 6)
        self.assertEqual({int(s.split("\t")[0]) for s in lines}, set(range(763, 770)))

    def test_independent_varint_extrema(self):
        for value, expected in [(0, "00"), (127, "7f"), (128, "8001"),
                                (300, "ac02"), (-1, "ffffffff0f"),
                                (-2**31, "8080808008"), (2**31-1, "ffffffff07")]:
            self.assertEqual(F.varint(value).hex(), expected)
        for value in (-2**31-1, 2**31):
            with self.assertRaises(ValueError):
                F.varint(value)

    def test_bad_hash(self):
        with self.assertRaisesRegex(ValueError, "Schema hash mismatch for 776"):
            self.build_fake(bad_hash=True)

    def test_missing_protocol(self):
        with self.assertRaisesRegex(ValueError, "exact fourteen"):
            self.build_fake(missing_protocol=True)

    def test_layout_drift_even_with_updated_hash(self):
        def change(types):
            types["packet_" + F.ORB] = F.container([("entityId", "i32")])
        with self.assertRaisesRegex(ValueError, "Layout drift for 763"):
            self.build_fake(change)

    def test_missing_present_type(self):
        with self.assertRaisesRegex(ValueError, "Layout drift for 763"):
            self.build_fake(lambda t: t.pop("packet_" + F.PLAYER))

    def test_id_drift(self):
        def change(t):
            m = t["packet"][1][0]["type"][1]["mappings"]
            m["0x04"] = m.pop("0x2")
        with self.assertRaisesRegex(ValueError, "Packet ID drift for 763"):
            self.build_fake(change)

    def test_duplicate_id_name(self):
        def change(t):
            t["packet"][1][0]["type"][1]["mappings"]["0x04"] = F.ORB
        with self.assertRaisesRegex(ValueError, "Packet ID drift for 763"):
            self.build_fake(change)

    def test_missing_id(self):
        def change(t):
            t["packet"][1][0]["type"][1]["mappings"].pop("0x3")
        with self.assertRaisesRegex(ValueError, "Packet ID drift for 763"):
            self.build_fake(change)

    def test_switch_drift(self):
        def change(t):
            t["packet"][1][1]["type"][1]["fields"][F.PLAYER] = "packet_spawn_entity"
        with self.assertRaisesRegex(ValueError, "Packet switch drift for 763"):
            self.build_fake(change)

    def test_switch_selector_drift(self):
        def change(t):
            t["packet"][1][1]["type"][1]["compareTo"] = "other"
        with self.assertRaisesRegex(ValueError, "Packet switch selector drift for 763"):
            self.build_fake(change)

    def test_lingering_type_after_removal(self):
        for name, protocol in [(F.ORB, 770), (F.PLAYER, 764)]:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "Unexpected legacy packet presence"):
                self.build_fake(lambda t: t.update({"packet_" + name: F.LAYOUTS[name]}), protocol)

    def test_lingering_id_after_removal(self):
        def change(t):
            t["packet"][1][0]["type"][1]["mappings"]["0x02"] = F.ORB
        with self.assertRaisesRegex(ValueError, "Unexpected legacy packet presence for 776"):
            self.build_fake(change, 776)

    def test_lingering_switch_after_removal(self):
        def change(t):
            t["packet"][1][1]["type"][1]["fields"][F.PLAYER] = "packet_" + F.PLAYER
        with self.assertRaisesRegex(ValueError, "Unexpected legacy packet presence for 776"):
            self.build_fake(change, 776)


if __name__ == "__main__":
    unittest.main()
