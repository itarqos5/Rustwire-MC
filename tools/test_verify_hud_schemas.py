"""Offline negative controls for the schema/fixture verifier."""
import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_hud_schemas as verifier


class HudSchemaVerifierTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.schemas = self.root / "research/protocols"
        self.schemas.mkdir(parents=True)
        self.records = []
        self.lines = []
        for protocol in range(763, 777):
            layouts = verifier.expected(protocol)
            types = {"packet_" + name: layout for name, layout in layouts.items()}
            types["packet"] = verifier.container(
                ("name", ["mapper", {"type": "varint", "mappings": {hex(i): name for i, name in enumerate(layouts)}}]),
                ("params", ["switch", {"compareTo": "name", "fields": {name: "packet_" + name for name in layouts}}]),
            )
            record = {"protocol": protocol, "schema": str(protocol)}
            self.records.append(record)
            self.write_schema(record, {"play": {"toClient": {"types": types}}})
            self.lines.extend(f"{protocol}\t{name}\t{i}\t" for i, name in enumerate(layouts))
        self.fixture = self.root / "hud.tsv"
        self.fixture.write_text("\n".join(self.lines) + "\n")
        self.write_records()

    def write_schema(self, record, schema):
        raw = json.dumps(schema).encode()
        (self.schemas / f"{record['schema']}.json").write_bytes(raw)
        record["sha256"] = hashlib.sha256(raw).hexdigest()

    def write_records(self):
        (self.root / "research/schema-hashes.json").write_text(json.dumps(self.records))

    def mutate_types(self, mutation):
        record = self.records[0]
        path = self.schemas / f"{record['schema']}.json"
        schema = json.loads(path.read_bytes())
        mutation(schema["play"]["toClient"]["types"])
        self.write_schema(record, schema)
        self.write_records()

    def verify(self):
        with patch.object(verifier, "ROOT", self.root), contextlib.redirect_stdout(io.StringIO()):
            verifier.verify(self.schemas, self.fixture)

    def test_all_fourteen_complete_fixture_sets_pass(self):
        self.verify()

    def test_hash_mismatch_is_rejected_before_layout_checks(self):
        with (self.schemas / "763.json").open("ab") as file:
            file.write(b" ")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            self.verify()

    def test_same_field_name_but_wrong_wire_type_is_rejected(self):
        self.mutate_types(lambda types: types["packet_set_title_time"][1][0].update(type="varint"))
        with self.assertRaisesRegex(ValueError, "layout"):
            self.verify()

    def test_wrong_packet_dispatch_is_rejected(self):
        self.mutate_types(lambda types: types["packet"][1][1]["type"][1]["fields"].update(action_bar="packet_set_title_text"))
        with self.assertRaisesRegex(ValueError, "dispatch"):
            self.verify()

    def test_wrong_packet_id_is_rejected(self):
        self.mutate_types(lambda types: types["packet"][1][0]["type"][1]["mappings"].update({"0xff": "action_bar"}))
        with self.assertRaisesRegex(ValueError, "fixture ID"):
            self.verify()

    def test_missing_duplicate_extra_and_wrong_id_fixture_rejected(self):
        variants = [
            self.lines[1:],
            self.lines + self.lines[:1],
            self.lines + ["763\tunknown_hud\t999\t00"],
            [self.lines[0].replace("\t0\t", "\t999\t")] + self.lines[1:],
        ]
        for lines in variants:
            with self.subTest(lines=len(lines)):
                self.fixture.write_text("\n".join(lines) + "\n")
                with self.assertRaises(ValueError):
                    self.verify()

    def test_schema_representation_boundaries_are_exact(self):
        self.assertEqual(verifier.expected(764)["action_bar"][1][0]["type"], "string")
        self.assertEqual(verifier.expected(765)["action_bar"][1][0]["type"], "anonymousNbt")
        self.assertEqual(verifier.expected(774)["open_book"][1][0]["type"], "varint")
        self.assertEqual(verifier.expected(775)["open_book"][1][0]["type"], ["mapper", {"type": "varint", "mappings": {"0": "main_hand", "1": "off_hand"}}])
        self.assertEqual(verifier.expected(776)["open_book"][1][0]["type"], "varint")

    def test_missing_protocol_and_invalid_hex_fixture_rejected(self):
        records = copy.deepcopy(self.records)
        self.records.pop()
        self.write_records()
        with self.assertRaisesRegex(ValueError, "fourteen"):
            self.verify()
        self.records = records
        self.write_records()
        self.fixture.write_text("\n".join(self.lines) + "ffz\n")
        with self.assertRaises(ValueError):
            self.verify()


if __name__ == "__main__":
    unittest.main()
