import copy
import struct
import unittest
from check_debug_value_fixtures import NAMES, cases, expected, fixtures, inspect, payload, pos, raw_types, string, vi


def schema(protocol):
    bodies = expected(protocol)
    ts = {'packet': ['container', [
        {'name': 'name', 'type': ['mapper', {'mappings': {hex(i+26): n for i,n in enumerate(bodies)}}]},
        {'name': 'params', 'type': ['switch', {'compareTo': 'name', 'fields': {n: 'packet_'+n for n in bodies}}]}]]}
    ts.update({'packet_'+n: v for n,v in bodies.items()})
    return {'types': raw_types() if protocol >= 773 else {}, 'play': {'toClient': {'types': ts}}}


class DebugValueFixtures(unittest.TestCase):
    def test_family_inventory_and_fixture_count(self):
        rows = [(p, inspect(schema(p), p)) for p in range(763, 777)]
        self.assertEqual(sum(len(ids) for _,ids in rows), 16)
        lines = fixtures(rows).splitlines()[1:]
        self.assertEqual(len(lines), 588)
        self.assertEqual(len(set(tuple(line.split('\t')[:3]) for line in lines)), 588)
        self.assertEqual({int(line.split('\t')[2].split('_')[1]) for line in lines}, set(range(1,16)))

    def test_presence_direction_id_and_dispatch_guards(self):
        with self.assertRaises(ValueError): inspect(schema(773), 772)
        for change in ('state', 'direction', 'missing', 'duplicate', 'id', 'dispatch'):
            s=schema(773); ts=s['play']['toClient']['types']; ids=ts['packet'][1][0]['type'][1]['mappings']
            if change == 'state': s['configuration']=s.pop('play')
            if change == 'direction': s['play']['toServer']=s['play'].pop('toClient')
            if change == 'missing': ids.pop('0x1a')
            if change == 'duplicate': ids['0x20']='debug_event'
            if change == 'id': ids['0x20']=ids.pop('0x1a')
            if change == 'dispatch': ts['packet'][1][1]['type'][1]['fields']['debug_event']='other'
            with self.subTest(change=change), self.assertRaises(ValueError): inspect(s,773)

    def test_exact_raw_schema_defects_are_guarded(self):
        s=schema(773); s['types']['PathDebugData'][1].reverse()
        with self.assertRaises(ValueError): inspect(s,773)
        s=schema(773); e=s['types']['DebugSubscriptionEvent'][1][1]['type'][1]['fields']
        e['GoalSelectors']=['array', {'countType':'varint','type':e['GoalSelectors']}]
        with self.assertRaises(ValueError): inspect(s,773)
        s=schema(775); s['types']['Node'][1][4]['type'][1]['mappings']['26']='big_mobs_close_to_danger'
        with self.assertRaises(ValueError): inspect(s,775)
        s=schema(776); s['types']['DebugSubscriptionDataType'][1]['mappings'].pop('0')
        with self.assertRaises(ValueError): inspect(s,776)

    def test_each_transitive_descriptor_is_guarded(self):
        for name in raw_types():
            s=schema(776); s['types'][name]=['unexpected']
            with self.subTest(name=name), self.assertRaises(ValueError): inspect(s,776)
        s=schema(774); s['play']['toClient']['types']['packet_debug_entity_value'][1][0]['type']='i32'
        with self.assertRaises(ValueError): inspect(s,774)

    def test_original_goal_unit_and_chunk_goldens(self):
        samples={(k,label): v for k,label,v,_ in cases()}
        self.assertEqual((vi(4)+payload(4,samples[4,'rich'],773)).hex(),'0402070101610000026263')
        self.assertEqual(payload(10,None,773),b'')
        rows=fixtures([(773,dict(zip(NAMES,range(26,30))))]).splitlines()
        self.assertIn('773\tdebug_event\tevent_10_rich\t29\t0a',rows)
        self.assertIn('773\tdebug_chunk_value\tpresent_10_rich\t27\t00000003fffffffe0a01',rows)
        self.assertIn('773\tdebug_chunk_value\tremoved_10\t27\t00000003fffffffe0a00',rows)
        self.assertIn('773\tdebug_entity_value\tremoved_10\t28\tffffffff0f0a00',rows)

    def test_varint_signed_and_position_edges(self):
        self.assertEqual(vi(-1).hex(),'ffffffff0f')
        self.assertEqual(vi(-(1<<31)).hex(),'8080808008')
        self.assertEqual(vi(128),b'\x80\x01')
        self.assertEqual(pos([-1,-1,-1]),b'\xff'*8)
        for n in (-(1<<31)-1,1<<31):
            with self.assertRaises(ValueError): vi(n)
        for p in ([1<<25,0,0],[0,2048,0],[0,0,-(1<<25)-1]):
            with self.assertRaises(ValueError): pos(p)

    def test_string_utf16_limits(self):
        self.assertEqual(len(string('😀'*127+'x',255)),511)
        with self.assertRaises(ValueError): string('😀'*128,255)
        self.assertEqual(string('Farmer test').hex(),'0b4661726d65722074657374')

    def test_path_order_and_versioned_type(self):
        samples={(k,label): v for k,label,v,_ in cases()}; path=samples[5,'rich']
        b=payload(5,path,773)
        # bool + fixed i32 + packed target, then four count-prefixed 26-byte-node lists.
        offset=13
        for count in (1,2,3,4):
            self.assertEqual(b[offset],count); offset += 1+26*count
        self.assertEqual(b[offset:].hex(),'3f000000')
        self.assertEqual(b[1:5],struct.pack('>i',-123))
        self.assertEqual(path['targets'][0]['position'],path['targets'][1]['position'])
        for protocol in (773,774):
            with self.assertRaises(ValueError): payload(5,samples[5,'path_type_26'],protocol)
        for protocol in (775,776): self.assertEqual(len(payload(5,samples[5,'path_type_26'],protocol)),281)

    def test_direct_registry_ids_and_float_payloads(self):
        samples={(k,label): v for k,label,v,_ in cases()}
        self.assertEqual(payload(7,samples[7,'rich'],773).hex(),'8001feffffff0f0701')
        self.assertEqual(payload(8,samples[8,'rich'],773)[8:10].hex(),'ac02')
        self.assertEqual(payload(15,samples[15,'float_bits'],776).hex(),'800180000000000000007ff00000000000007ff8000000001234')
        self.assertEqual(payload(2,samples[2,'float_bits'],776)[25:33].hex(),'7fc0123480000000')
        v=copy.deepcopy(samples[15,'rich']);v['event']=-1
        with self.assertRaises(ValueError): payload(15,v,776)
        with self.assertRaises(ValueError): payload(0,None,776)

    def test_empty_decoder_valid_path_and_sibling_budgets(self):
        samples={(k,label): v for k,label,v,_ in cases()}
        self.assertEqual(payload(5,samples[5,'empty'],773),b'\0'*21)
        brain=samples[2,'rich']
        self.assertEqual(sum(len(brain[n]) for n in ('activities','behaviors','memories','gossips','pois','potential_pois')),9)
        structures=samples[12,'rich'];self.assertEqual(len(structures)+sum(len(s['pieces']) for s in structures),4)
        self.assertEqual(len(payload(4,[],773)),1)


if __name__ == '__main__': unittest.main()
