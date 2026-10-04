"""Offline negative controls using synthetic schemas and committed fixture facts.

The separate audit command still verifies the real hash-pinned upstream inputs.
These unit tests never require downloads or a research/protocols checkout.
"""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import verify_server_metadata_schemas as audit


class ServerMetadataAuditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = (audit.ROOT / 'tests/fixtures/server-metadata.tsv').read_text()
        cls.ids = {}
        for line in cls.fixtures.splitlines():
            if line.startswith('#'):
                continue
            protocol, state, direction, name, _, packet_id, _ = line.split('\t')
            key = (int(protocol), state, direction, name)
            if key in cls.ids and cls.ids[key] != int(packet_id):
                raise ValueError('Inconsistent committed fixture packet IDs')
            cls.ids[key] = int(packet_id)
        cls.schema = cls.synthetic_schema(770)

    @classmethod
    def synthetic_schema(cls, protocol):
        """Build only the structures consumed by the audit, without downloads.

        Packet IDs come from committed wire facts. Shape builders are inputs to
        exercise checker control flow; they are not fresh upstream evidence.
        """
        schema = {'types': {}}
        if protocol >= 766:
            schema['types']['ByteArray'] = ['buffer', {'countType': 'varint'}]
        grouped = {}
        for (state, direction, name), (type_name, layout) in audit.expected(protocol).items():
            local = schema.setdefault(state, {}).setdefault(direction, {}).setdefault('types', {})
            if type_name.startswith('packet_common_'):
                schema['types'][type_name] = copy.deepcopy(layout)
            else:
                local[type_name] = copy.deepcopy(layout)
            # Unimplemented outbound report metadata generates no fixture. Give
            # that synthetic-only mapping an arbitrary noncolliding local ID.
            packet_id = cls.ids.get((protocol, state, direction, name), 0)
            grouped.setdefault((state, direction), []).append((packet_id, name, type_name))
        for (state, direction), entries in grouped.items():
            entries.sort()
            schema[state][direction]['types']['packet'] = audit.container(
                ('name', ['mapper', {'type': 'varint', 'mappings': {
                    hex(packet_id): name for packet_id, name, _ in entries}}]),
                ('params', ['switch', {'compareTo': 'name', 'fields': {
                    name: type_name for _, name, type_name in entries}}]),
            )
        return schema

    def test_all_family_fixtures_match_without_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            schemas = root / 'research/protocols'
            schemas.mkdir(parents=True)
            records = []
            for protocol in range(763, 777):
                data = json.dumps(self.synthetic_schema(protocol)).encode()
                records.append({'protocol': protocol, 'schema': str(protocol),
                                'sha256': hashlib.sha256(data).hexdigest()})
                (schemas / f'{protocol}.json').write_bytes(data)
            (root / 'research/schema-hashes.json').write_text(json.dumps(records))
            output = audit.generate(root)
        self.assertEqual(output, self.fixtures)
        self.assertEqual(len(output.splitlines()) - 2, 310)

    def test_boundaries_are_explicit(self):
        self.assertNotIn(('play', 'toClient', 'ping_response'), audit.expected(763))
        self.assertIn(('play', 'toClient', 'ping_response'), audit.expected(764))
        self.assertNotIn(('configuration', 'toClient', 'custom_report_details'), audit.expected(766))
        self.assertIn(('configuration', 'toClient', 'custom_report_details'), audit.expected(767))
        self.assertIn(('configuration', 'toServer', 'custom_report_details'), audit.expected(770))
        self.assertNotIn(('configuration', 'toServer', 'custom_report_details'), audit.expected(771))
        def fields(protocol):
            return audit.expected(protocol)[('play', 'toClient', 'server_data')][1][1]
        self.assertEqual(fields(764)[0]['type'], 'string')
        self.assertEqual(fields(765)[0]['type'], 'anonymousNbt')
        self.assertEqual(len(fields(765)), 3)
        self.assertEqual(len(fields(766)), 2)

    def test_changed_layout_and_nested_alias_are_rejected(self):
        for mutate in [
            lambda s: s['play']['toClient']['types']['packet_server_data'][1][0].update(type='string'),
            lambda s: s['types'].update(ByteArray='string'),
            lambda s: s['types']['packet_common_custom_report_details'][1][0]['type'][1]['type'][1][1].update(type='i32'),
        ]:
            schema = copy.deepcopy(self.schema)
            mutate(schema)
            with self.assertRaisesRegex(ValueError, 'layout'):
                audit.inspect_schema(schema, 770)

    def test_wrong_name_to_type_dispatch_is_rejected(self):
        schema = copy.deepcopy(self.schema)
        schema['play']['toClient']['types']['packet'][1][1]['type'][1]['fields']['hide_message'] = 'packet_chat_suggestions'
        with self.assertRaisesRegex(ValueError, 'dispatch'):
            audit.inspect_schema(schema, 770)

    def test_wrong_presence_and_duplicate_packet_ids_are_rejected(self):
        for replacement in [{}, {'0xff': 'ping_response'}]:
            schema = copy.deepcopy(self.schema)
            mapping = schema['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']
            if replacement:
                mapping.update(replacement)
            else:
                for key in list(mapping):
                    if mapping[key] == 'ping_response':
                        del mapping[key]
            with self.assertRaisesRegex(ValueError, 'presence'):
                audit.inspect_schema(schema, 770)

    def test_wrong_direction_or_state_is_rejected(self):
        schema = copy.deepcopy(self.schema)
        schema['configuration']['toServer']['types']['packet'][1][0]['type'][1]['mappings']['0xfe'] = 'ping_response'
        with self.assertRaisesRegex(ValueError, 'presence'):
            audit.inspect_schema(schema, 770)

    def test_hash_mismatch_and_missing_family_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / 'research/protocols'
            path.mkdir(parents=True)
            records = [{'protocol': p, 'schema': str(p), 'sha256': hashlib.sha256(b'{}').hexdigest()} for p in range(763, 777)]
            (root / 'research/schema-hashes.json').write_text(json.dumps(records))
            (path / '763.json').write_bytes(b'{ }')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                audit.generate(root)
            records.pop()
            (root / 'research/schema-hashes.json').write_text(json.dumps(records))
            with self.assertRaisesRegex(ValueError, 'fourteen'):
                audit.generate(root)

    def test_independent_fixture_primitives(self):
        self.assertEqual(audit.varint(127), b'\x7f')
        self.assertEqual(audit.varint(128), b'\x80\x01')
        self.assertEqual(audit.varint(0x7fffffff), b'\xff\xff\xff\xff\x07')
        self.assertEqual(audit.string('😀'), b'\x04\xf0\x9f\x98\x80')
        for value in [-1, 0x80000000]:
            with self.assertRaises(ValueError):
                audit.varint(value)
        self.assertEqual(audit.bodies(763, 'server_data')['empty'], b'\x0c{"text":"x"}\x01\x00\x01')
        self.assertEqual(audit.bodies(766, 'server_data')['empty'], b'\x08\x00\x01x\x01\x00')


if __name__ == '__main__':
    unittest.main()
