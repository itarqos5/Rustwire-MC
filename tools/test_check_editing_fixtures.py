"""Self-contained negative regressions; no ignored schemas or network required."""
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import check_editing_fixtures as check

class EditingAuditTests(unittest.TestCase):
    def schema(self, p):
        ids = {}
        for line in (check.ROOT / 'tests/fixtures/editing.tsv').read_text().splitlines():
            if line.startswith('#'): continue
            protocol, name, packet_id, _, _ = line.split('\t')
            if int(protocol) == p: ids[name] = int(packet_id)
        packet = ['container', [{'name': 'name', 'type': ['mapper', {'type': 'varint', 'mappings': {hex(i): n for n, i in ids.items()}}]},
                  {'name': 'params', 'type': ['switch', {'compareTo': 'name', 'fields': {n: 'packet_' + n for n in ids}}]}]]
        return {'types': check.aliases(p), 'play': {'toServer': {'types': {'packet': packet, **{'packet_' + n: shape for n, shape in check.layouts(p).items()}}}}}
    def test_all_synthetic_families_reproduce_committed_fixtures_without_network(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); records = []
            for p in range(763, 777):
                raw = json.dumps(self.schema(p)).encode(); (root / f'{p}.json').write_bytes(raw)
                records.append({'protocol': p, 'schema': str(p), 'sha256': hashlib.sha256(raw).hexdigest()})
            with patch('socket.create_connection', side_effect=AssertionError('network forbidden')):
                self.assertEqual(check.generate(root, records), (check.ROOT / 'tests/fixtures/editing.tsv').read_text())
            records[0]['sha256'] = '0' * 64
            with self.assertRaisesRegex(ValueError, 'hash'): check.generate(root, records)
    def test_order_type_and_optional_prefix_drift(self):
        for p in range(763, 777):
            for name in check.NAMES:
                s = self.schema(p); fields = s['play']['toServer']['types']['packet_' + name][1]
                fields[0]['type'] = 'f64'
                with self.assertRaisesRegex(ValueError, 'layout'): check.audit(s, p)
    def test_primitive_position_and_untrusted_alias_drift(self):
        for p in range(763, 777):
            for name in check.aliases(p):
                s = self.schema(p); s['types'][name] = 'wrong'
                with self.assertRaisesRegex(ValueError, 'alias'): check.audit(s, p)
    def test_wrong_direction_and_missing_dispatch_fail(self):
        s = self.schema(776); s['play']['toClient'] = s['play'].pop('toServer')
        with self.assertRaises(KeyError): check.audit(s, 776)
        s = self.schema(776); del s['play']['toServer']['types']['packet'][1][1]['type'][1]['fields']['edit_book']
        with self.assertRaisesRegex(ValueError, 'mapping'): check.audit(s, 776)
    def test_duplicate_packet_name_and_numeric_id_fail(self):
        s = self.schema(776); m = s['play']['toServer']['types']['packet'][1][0]['type'][1]['mappings']; m['0x100'] = 'edit_book'
        with self.assertRaisesRegex(ValueError, 'duplicate'): check.audit(s, 776)
        s = self.schema(776); m = s['play']['toServer']['types']['packet'][1][0]['type'][1]['mappings']; key = next(iter(m)); m['0x0' + key[2:]] = 'other'
        with self.assertRaisesRegex(ValueError, 'duplicate'): check.audit(s, 776)
    def test_wrong_family_set_and_framing_boundary_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError, 'fourteen'): check.generate(Path(tmp), [])
        for p in [769, 770]:
            s = self.schema(p); s['play']['toServer']['types']['packet_set_creative_slot'] = check.layouts(770 if p == 769 else 769)['set_creative_slot']
            with self.assertRaisesRegex(ValueError, 'layout'): check.audit(s, p)
if __name__ == '__main__': unittest.main()
