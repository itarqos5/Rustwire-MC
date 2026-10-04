"""Clean-checkout unit controls using synthetic schemas and committed fixtures.

The real hash-pinned schema audit is a separate command. No downloads or game
artifacts are needed by these tests.
"""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import verify_dialog_schemas as audit


class DialogAuditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixtures = (audit.ROOT / 'tests/fixtures/dialog.tsv').read_text()
        cls.ids = {}
        for line in cls.fixtures.splitlines():
            if line.startswith('#'):
                continue
            p, state, direction, name, _, packet_id, _ = line.split('\t')
            key = (int(p), state, direction, name)
            if key in cls.ids and cls.ids[key] != int(packet_id):
                raise ValueError('Inconsistent fixture packet IDs')
            cls.ids[key] = int(packet_id)

    @classmethod
    def synthetic_schema(cls, protocol):
        schema = {'types': {}}
        if protocol >= 771:
            schema['types'].update(anonymousNbt='native', registryEntryHolder='native')
        groups = {}
        for (state, direction, name), (type_name, shape) in audit.expected(protocol).items():
            local = schema.setdefault(state, {}).setdefault(direction, {}).setdefault('types', {})
            if type_name.startswith('packet_common_'):
                schema['types'][type_name] = copy.deepcopy(shape)
            else:
                local[type_name] = copy.deepcopy(shape)
            packet_id = cls.ids[(protocol, state, direction, name)]
            if (protocol, state, direction, name) == (776, 'play', 'toServer', 'custom_click_action'):
                # Supply the stale schema ID, not the already corrected fixture ID.
                packet_id = 0x43
            groups.setdefault((state, direction), []).append((packet_id, name, type_name))
        for (state, direction), entries in groups.items():
            entries.sort()
            schema[state][direction]['types']['packet'] = audit.container(
                ('name', ['mapper', {'type': 'varint', 'mappings': {hex(i): n for i, n, _ in entries}}]),
                ('params', ['switch', {'compareTo': 'name', 'fields': {n: t for _, n, t in entries}}]),
            )
        return schema

    def test_all_fixtures_reproduce_without_downloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            schemas = root / 'research/protocols'
            schemas.mkdir(parents=True)
            rows = []
            for p in range(763, 777):
                data = json.dumps(self.synthetic_schema(p)).encode()
                (schemas / f'{p}.json').write_bytes(data)
                rows.append({'protocol': p, 'schema': str(p), 'sha256': hashlib.sha256(data).hexdigest()})
            (root / 'research/schema-hashes.json').write_text(json.dumps(rows))
            text = audit.generate(root)
        self.assertEqual(text, self.fixtures)
        self.assertEqual(len(text.splitlines()) - 2, 102)

    def test_version_state_and_direction_boundaries(self):
        for p in range(763, 771):
            self.assertEqual(audit.expected(p), {})
        for p in range(771, 777):
            self.assertEqual(len(audit.expected(p)), 6)
            self.assertIn(('configuration', 'toServer', 'custom_click_action'), audit.expected(p))
            self.assertNotIn(('play', 'toClient', 'custom_click_action'), audit.expected(p))
            self.assertEqual(len(audit.inspect_schema(self.synthetic_schema(p), p)), 6)

    def test_distinct_show_dialog_state_layouts(self):
        layouts = audit.expected(771)
        config = layouts[('configuration', 'toClient', 'show_dialog')][1]
        play = layouts[('play', 'toClient', 'show_dialog')][1]
        self.assertEqual(config, audit.container(('dialog', 'anonymousNbt')))
        self.assertNotEqual(config, play)
        schema = self.synthetic_schema(771)
        schema['configuration']['toClient']['types']['packet_show_dialog'] = copy.deepcopy(play)
        with self.assertRaisesRegex(ValueError, 'layout'):
            audit.inspect_schema(schema, 771)

    def test_schema_defect_is_explicit_but_fixtures_use_correct_framing(self):
        old = audit.expected(771)[('play', 'toServer', 'custom_click_action')][1]
        self.assertEqual(old[1][1]['type'], ['option', 'anonymousNbt'])
        self.assertEqual(audit.bodies('play', 'custom_click_action')['absent'], b'\x03x:y\x01\x00')
        self.assertEqual(audit.bodies('configuration', 'custom_click_action')['byte'], b'\x03x:y\x02\x01\x7f')
        schema = self.synthetic_schema(771)
        schema['types']['packet_common_custom_click_action'][1][1]['type'] = 'anonymousNbt'
        with self.assertRaisesRegex(ValueError, 'layout'):
            audit.inspect_schema(schema, 771)

    def test_corrected_26_2_id_remains_explicit(self):
        schema = self.synthetic_schema(776)
        self.assertEqual(audit.inspect_schema(schema, 776)[('play', 'toServer', 'custom_click_action')], 0x44)
        mapping = schema['play']['toServer']['types']['packet'][1][0]['type'][1]['mappings']
        del mapping['0x43']
        mapping['0x44'] = 'custom_click_action'
        with self.assertRaisesRegex(ValueError, 'corrected protocol-776'):
            audit.inspect_schema(schema, 776)

    def test_wrong_dispatch_and_discriminator_are_rejected(self):
        for mutate in [
            lambda s: s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields'].update(show_dialog='packet_common_clear_dialog'),
            lambda s: s['play']['toClient']['types']['packet'][1][1]['type'][1].update(compareTo='other'),
        ]:
            schema = self.synthetic_schema(771)
            mutate(schema)
            with self.assertRaises(ValueError):
                audit.inspect_schema(schema, 771)

    def test_missing_duplicate_and_wrong_direction_are_rejected(self):
        for mode in ['missing', 'duplicate', 'direction']:
            schema = self.synthetic_schema(771)
            maps = schema['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']
            if mode == 'missing':
                del maps['0x84']
            elif mode == 'duplicate':
                maps['0xff'] = 'show_dialog'
            else:
                maps['0xff'] = 'custom_click_action'
            with self.assertRaisesRegex(ValueError, 'presence'):
                audit.inspect_schema(schema, 771)

    def test_native_nbt_and_holder_alias_drift_is_rejected(self):
        for name in ['anonymousNbt', 'registryEntryHolder']:
            schema = self.synthetic_schema(771)
            schema['types'][name] = 'varint'
            with self.assertRaisesRegex(ValueError, 'native type'):
                audit.inspect_schema(schema, 771)

    def test_hash_mismatch_and_missing_family_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            schemas = root / 'research/protocols'
            schemas.mkdir(parents=True)
            rows = [{'protocol': p, 'schema': str(p), 'sha256': hashlib.sha256(b'{}').hexdigest()} for p in range(763, 777)]
            (root / 'research/schema-hashes.json').write_text(json.dumps(rows))
            (schemas / '763.json').write_bytes(b'{ }')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                audit.generate(root)
            rows.pop()
            (root / 'research/schema-hashes.json').write_text(json.dumps(rows))
            with self.assertRaisesRegex(ValueError, 'fourteen'):
                audit.generate(root)

    def test_independent_fixture_primitives_and_zero_body(self):
        self.assertEqual(audit.varint(65536), b'\x80\x80\x04')
        self.assertEqual(audit.varint(0x7fffffff), b'\xff\xff\xff\xff\x07')
        self.assertEqual(audit.bodies('configuration', 'clear_dialog'), {'empty': b''})
        self.assertEqual(audit.bodies('configuration', 'show_dialog')['empty_compound'], b'\x0a\0')
        self.assertEqual(audit.bodies('play', 'show_dialog')['empty_compound'], b'\0\x0a\0')
        for n in [-1, 0x80000000]:
            with self.assertRaises(ValueError):
                audit.varint(n)


if __name__ == '__main__':
    unittest.main()
