import unittest
from check_custom_payload_fixtures import inspect, body, fixtures


def schema(p):
    result = {}
    for s in ('configuration', 'play'):
        if s == 'configuration' and p == 763: continue
        result[s] = {}
        for d in ('toClient', 'toServer'):
            result[s][d] = {'types': {
                'packet': ['container', [
                    {'name': 'name', 'type': ['mapper', {'mappings': {'0x01': 'custom_payload'}}]},
                    {'name': 'params', 'type': ['switch', {'compareTo': 'name', 'fields': {'custom_payload': 'packet_custom_payload'}}]}]],
                'packet_custom_payload': ['container', [{'name': 'channel', 'type': 'string'}, {'name': 'data', 'type': 'restBuffer'}]]}}
    return result


class CustomPayloadAudit(unittest.TestCase):
    def test_all_family_presence(self):
        for p in range(763, 777): self.assertEqual(len(inspect(schema(p), p)), 2 if p == 763 else 4)

    def test_independent_unprefixed_tail(self):
        self.assertEqual(body('a:b', b'\xff\x00'), bytes.fromhex('03613a62ff00'))
        self.assertEqual(body('x' * 128, b'')[:2], b'\x80\x01')
        self.assertEqual(len(fixtures([(763, {('play', 'toClient'): 1})]).splitlines()), 7)

    def test_changed_payload_framing(self):
        for name in ('string', 'buffer', 'varint'):
            s = schema(776); s['play']['toClient']['types']['packet_custom_payload'][1][1]['type'] = name
            with self.assertRaises(ValueError): inspect(s, 776)

    def test_wrong_state_or_missing_direction(self):
        with self.assertRaises(ValueError): inspect(schema(764), 763)
        s = schema(776); del s['play']['toClient']
        with self.assertRaises(ValueError): inspect(s, 776)
        s = schema(776); s['login'] = s.pop('configuration')
        with self.assertRaises(ValueError): inspect(s, 776)

    def test_duplicate_or_redirected_dispatch(self):
        s = schema(776); fields = s['play']['toClient']['types']['packet'][1]
        fields[0]['type'][1]['mappings']['0x02'] = 'custom_payload'
        with self.assertRaises(ValueError): inspect(s, 776)
        s = schema(776); s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields']['custom_payload'] = 'other'
        with self.assertRaises(ValueError): inspect(s, 776)


if __name__ == '__main__': unittest.main()
