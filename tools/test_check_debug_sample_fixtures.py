import unittest
from check_debug_sample_fixtures import NAMES, REGISTRY_NAMES, cases, expected, inspect, fixtures, vi


def schema(p):
    s = {'types': {}, 'play': {}}
    for direction in ('toClient', 'toServer'):
        bodies = {name: body for (d, name), body in expected(p).items() if d == direction}
        if not bodies: continue
        ts = {'packet': ['container', [
            {'name': 'name', 'type': ['mapper', {'mappings': {hex(i): n for i, n in enumerate(bodies)}}]},
            {'name': 'params', 'type': ['switch', {'compareTo': 'name', 'fields': {n:'packet_'+n for n in bodies}}]}]]}
        ts.update({'packet_'+n: b for n, b in bodies.items()}); s['play'][direction] = {'types': ts}
    if p >= 773: s['types']['DebugSubscriptionDataType'] = ['mapper', {'type':'varint', 'mappings':{str(i):n for i,n in enumerate(REGISTRY_NAMES)}}]
    return s


class DebugSampleAudit(unittest.TestCase):
    def test_all_family_and_replacement_boundaries(self):
        for p in range(763, 777): self.assertEqual(set(inspect(schema(p), p)), set(expected(p)))
        self.assertIn(('toServer','debug_sample_subscription'), expected(772))
        self.assertNotIn(('toServer','debug_sample_subscription'), expected(773))
        self.assertIn(('toServer','debug_subscription_request'), expected(773))

    def test_original_signed_array_bytes_and_no_option_marker(self):
        self.assertEqual(dict(cases('debug_sample'))['signed_edges'].hex(), '03ffffffffffffffff80000000000000007fffffffffffffff00')
        self.assertEqual(dict(cases('debug_sample_subscription'))['tick_time'], b'\x00')
        self.assertEqual(dict(cases('debug_subscription_request'))['duplicates'], b'\x03\x02\x02\x00')
        self.assertEqual(vi(128), b'\x80\x01')
        with self.assertRaises(ValueError): vi(-1)

    def test_presence_and_state_drift(self):
        with self.assertRaises(ValueError): inspect(schema(766), 765)
        with self.assertRaises(ValueError): inspect(schema(772), 773)
        s=schema(773);s['configuration']=s.pop('play')
        with self.assertRaises(ValueError): inspect(s,773)

    def test_width_and_collection_framing_drift(self):
        s=schema(766);s['play']['toClient']['types']['packet_debug_sample'][1][0]['type'][1]['type']='varlong'
        with self.assertRaises(ValueError): inspect(s,766)
        s=schema(773);s['play']['toServer']['types']['packet_debug_subscription_request'][1][0]['type'][1]['countType']='i32'
        with self.assertRaises(ValueError): inspect(s,773)

    def test_duplicate_and_redirected_ids(self):
        s=schema(766);s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0x02']='debug_sample'
        with self.assertRaises(ValueError): inspect(s,766)
        s=schema(766);s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields']['debug_sample']='other'
        with self.assertRaises(ValueError): inspect(s,766)

    def test_registry_representation_and_fixture_inventory(self):
        s=schema(773);s['types']['DebugSubscriptionDataType'][1]['mappings']['16']='unknown'
        with self.assertRaises(ValueError): inspect(s,773)
        rows=[(p,inspect(schema(p),p)) for p in range(763,777)]
        self.assertEqual(len(fixtures(rows).splitlines())-1,75)
        self.assertEqual(sum(len(ids) for _,ids in rows),22)


if __name__ == '__main__': unittest.main()
