import unittest
from check_game_test_fixtures import container,expected,inspect,fixtures,cases,vi,pos


def schema(p):
    out={'types':{'vec3i':container(('x','varint'),('y','varint'),('z','varint'))},'play':{}}
    for direction in ['toClient','toServer']:
        bodies={n:b for (d,n),b in expected(p).items() if d==direction}
        if not bodies:continue
        ids={hex(0x40 if p==776 and n=='test_instance_block_action' else i):n for i,n in enumerate(bodies)}
        ts={'packet':container(('name',['mapper',{'mappings':ids}]),('params',['switch',{'compareTo':'name','fields':{n:'packet_'+n for n in bodies}}]))}
        ts.update({'packet_'+n:b for n,b in bodies.items()});out['play'][direction]={'types':ts}
    return out


class GameTestAudit(unittest.TestCase):
    def test_all_families_and_corrected_id(self):
        rows=[(p,inspect(schema(p),p)) for p in range(763,777)]
        self.assertEqual(sum(len(ids) for _,ids in rows),25)
        self.assertEqual(len(fixtures(rows).splitlines())-1,124)
        self.assertEqual(rows[-1][1][('toServer','test_instance_block_action')],0x41)

    def test_missing_or_wrong_state_and_introduction(self):
        with self.assertRaises(ValueError):inspect(schema(770),769)
        with self.assertRaises(ValueError):inspect(schema(773),772)
        s=schema(773);s['configuration']=s.pop('play')
        with self.assertRaises(ValueError):inspect(s,773)
        s=schema(770);del s['play']['toClient']
        with self.assertRaises(ValueError):inspect(s,770)

    def test_vector_width_and_optional_framing(self):
        s=schema(776);s['types']['vec3i'][1][0]['type']='i32'
        with self.assertRaises(ValueError):inspect(s,776)
        s=schema(770);s['play']['toClient']['types']['packet_test_instance_block_status'][1][1]['type']='vec3i'
        with self.assertRaises(ValueError):inspect(s,770)

    def test_nbt_or_field_order_drift(self):
        s=schema(770);s['play']['toClient']['types']['packet_test_instance_block_status'][1][0]['type']='string'
        with self.assertRaises(ValueError):inspect(s,770)
        s=schema(770);s['play']['toServer']['types']['packet_test_instance_block_action'][1][2]['type'][1].reverse()
        with self.assertRaises(ValueError):inspect(s,770)

    def test_duplicate_redirected_and_uncorrected_ids(self):
        s=schema(770);s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0x03']='test_instance_block_status'
        with self.assertRaises(ValueError):inspect(s,770)
        s=schema(770);s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields']['test_instance_block_status']='other'
        with self.assertRaises(ValueError):inspect(s,770)
        s=schema(776);m=s['play']['toServer']['types']['packet'][1][0]['type'][1]['mappings'];m['0x41']=m.pop('0x40')
        with self.assertRaises(ValueError):inspect(s,776)

    def test_original_scalar_bytes(self):
        self.assertEqual(vi(-1),b'\xff\xff\xff\xff\x0f')
        self.assertEqual(pos([-1,-1,-1]),b'\xff'*8)
        self.assertEqual(dict(cases('test_instance_block_status'))['small'].hex(),'080005526561647901010203')
        self.assertEqual(dict(cases('game_test_highlight_pos'))['zero'],bytes(16))


if __name__=='__main__':unittest.main()
