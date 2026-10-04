import hashlib
import json
import tempfile
import unittest
from pathlib import Path
import check_minecart_fixtures as check

class MinecartAuditTests(unittest.TestCase):
    def schema(self,p):
        ids={}
        for line in (check.ROOT/'tests/fixtures/minecart.tsv').read_text().splitlines():
            if not line.startswith('#'):
                protocol,ident,_,_=line.split('\t')
                if int(protocol)==p:ids[hex(int(ident))]='move_minecart'
        types={'packet':['container',[{'name':'name','type':['mapper',{'type':'varint','mappings':ids}]},{'name':'params','type':['switch',{'compareTo':'name','fields':{'move_minecart':'packet_move_minecart'} if p>=768 else {}}]}]]}
        if p>=768:types['packet_move_minecart']=check.layout(p)
        return {'types':{'vec3f':check.container([(x,'f32') for x in ['x','y','z']]),'f32':'native','varint':'native'},'play':{'toClient':{'types':types}}}
    def test_all_families_reproduce_fixtures_without_downloads(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);records=[]
            for p in range(763,777):
                raw=json.dumps(self.schema(p)).encode();(root/f'{p}.json').write_bytes(raw);records.append({'protocol':p,'schema':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
            self.assertEqual(check.generate(root,records),(check.ROOT/'tests/fixtures/minecart.tsv').read_text())
            records[0]['sha256']='0'*64
            with self.assertRaisesRegex(ValueError,'hash'):check.generate(root,records)
    def test_actual_discrepant_schema_is_checked_without_rewriting_it(self):
        for p in range(768,777):
            s=self.schema(p);s['types']['vec3f']=check.container([(n,'f64') for n in ['x','y','z']])
            with self.assertRaisesRegex(ValueError,'alias'):check.audit(s,p)
            s=self.schema(p);s['play']['toClient']['types']['packet_move_minecart'][1][1]['type'][1]['type'][1][2]['type']='i8'
            with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,p)
    def test_mapping_and_availability_drift_fail(self):
        s=self.schema(767);s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0x01']='move_minecart'
        with self.assertRaisesRegex(ValueError,'unexpected'):check.audit(s,767)
        s=self.schema(776);s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields'].clear()
        with self.assertRaisesRegex(ValueError,'mapping'):check.audit(s,776)
        s=self.schema(776);m=s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings'];m['0x100']='move_minecart'
        with self.assertRaisesRegex(ValueError,'duplicate'):check.audit(s,776)
    def test_absent_layout_and_wrong_family_list_fail(self):
        s=self.schema(763);s['play']['toClient']['types']['packet_move_minecart']=check.layout(768)
        with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,763)
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError,'fourteen'):check.generate(Path(tmp),[])
if __name__=='__main__':unittest.main()
