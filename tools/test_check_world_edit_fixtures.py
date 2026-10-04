import hashlib
import json
import tempfile
import unittest
from pathlib import Path
import check_world_edit_fixtures as check
class WorldEditAuditTests(unittest.TestCase):
 def schema(self,p):
  ids={}
  for line in (check.ROOT/'tests/fixtures/world-edit.tsv').read_text().splitlines():
   if not line.startswith('#'):
    protocol,name,ident,_,_=line.split('\t')
    if int(protocol)==p:ids[hex(int(ident))]=name
  packet=['container',[{'name':'name','type':['mapper',{'type':'varint','mappings':ids}]},{'name':'params','type':['switch',{'compareTo':'name','fields':{n:'packet_'+n for n in ids.values()}}]}]]
  return {'types':check.aliases(),'play':{'toServer':{'types':{'packet':packet,**{'packet_'+n:check.layout(n,p) for n in check.NAMES}}}}}
 def test_all_families_reproduce_fixtures_without_downloads(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);records=[]
   for p in range(763,777):
    raw=json.dumps(self.schema(p)).encode();(root/f'{p}.json').write_bytes(raw);records.append({'protocol':p,'schema':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
   self.assertEqual(check.generate(root,records),(check.ROOT/'tests/fixtures/world-edit.tsv').read_text())
   records[0]['sha256']='0'*64
   with self.assertRaisesRegex(ValueError,'hash'):check.generate(root,records)
 def test_actual_seed_flag_and_priority_schema_shapes_are_checked(self):
  for p in range(763,777):
   s=self.schema(p);fields=s['play']['toServer']['types']['packet_update_structure_block'][1]
   next(f for f in fields if f['name']=='seed')['type']='varlong'
   with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,p)
   s=self.schema(p);s['play']['toServer']['types']['packet_update_jigsaw_block']=check.layout('update_jigsaw_block',765 if p<765 else 764)
   with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,p)
  s=self.schema(770);s['play']['toServer']['types']['packet_update_structure_block'][1][-1]['type']='u8'
  with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,770)
 def test_aliases_and_mapping_drift_fail(self):
  for n in check.aliases():
   s=self.schema(776);s['types'][n]='wrong'
   with self.assertRaisesRegex(ValueError,'alias'):check.audit(s,776)
  s=self.schema(776);s['play']['toServer']['types']['packet'][1][1]['type'][1]['fields'].clear()
  with self.assertRaisesRegex(ValueError,'mapping'):check.audit(s,776)
  s=self.schema(776);s['play']['toServer']['types']['packet'][1][0]['type'][1]['mappings']['0x100']='generate_structure'
  with self.assertRaisesRegex(ValueError,'duplicate'):check.audit(s,776)
 def test_wrong_direction_and_family_set_fail(self):
  s=self.schema(776);s['play']['toClient']=s['play'].pop('toServer')
  with self.assertRaises(KeyError):check.audit(s,776)
  with tempfile.TemporaryDirectory() as tmp:
   with self.assertRaisesRegex(ValueError,'fourteen'):check.generate(Path(tmp),[])
if __name__=='__main__':unittest.main()
