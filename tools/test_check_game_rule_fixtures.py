import hashlib
import json
import tempfile
import unittest
from pathlib import Path
import check_game_rule_fixtures as check
class GameRuleAuditTests(unittest.TestCase):
 def schema(self,p):
  branches={}
  for direction,names in check.NAMES.items():
   ids={}
   for line in (check.ROOT/'tests/fixtures/game-rules.tsv').read_text().splitlines():
    if not line.startswith('#'):
     protocol,way,name,ident,_,_=line.split('\t')
     if int(protocol)==p and way==direction:ids[hex(int(ident))]=name
   types={'packet':['container',[{'name':'name','type':['mapper',{'type':'varint','mappings':ids}]},{'name':'params','type':['switch',{'compareTo':'name','fields':{n:'packet_'+n for n in ids.values()}}]}]]}
   if p>=775:types.update({'packet_'+n:check.layout(n,p) for n in names})
   branches[direction]={'types':types}
  return {'types':{'string':['pstring',{'countType':'varint'}],'varint':'native','GameRule':check.rule_alias()},'play':branches}
 def test_reproduction_without_downloaded_schemas(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);records=[]
   for p in range(763,777):
    raw=json.dumps(self.schema(p)).encode();(root/f'{p}.json').write_bytes(raw);records.append({'protocol':p,'schema':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
   self.assertEqual(check.generate(root,records),(check.ROOT/'tests/fixtures/game-rules.tsv').read_text())
   records[0]['sha256']='0'*64
   with self.assertRaisesRegex(ValueError,'hash'):check.generate(root,records)
 def test_field_order_empty_shape_and_alias_drift(self):
  for p in [775,776]:
   s=self.schema(p);s['play']['toClient']['types']['packet_low_disk_space_warning']=check.container([('extra','bool')])
   with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,p)
   s=self.schema(p);s['types']['string']='wrong'
   with self.assertRaisesRegex(ValueError,'alias'):check.audit(s,p)
  s=self.schema(775);s['types']['GameRule']=check.container([('value','string'),('name','string')])
  with self.assertRaisesRegex(ValueError,'alias'):check.audit(s,775)
 def test_mapping_presence_and_duplicates(self):
  s=self.schema(776);s['play']['toServer']['types']['packet'][1][1]['type'][1]['fields'].clear()
  with self.assertRaisesRegex(ValueError,'mapping'):check.audit(s,776)
  s=self.schema(776);s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0x100']='game_rule_values'
  with self.assertRaisesRegex(ValueError,'duplicate'):check.audit(s,776)
  s=self.schema(774);s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0x00']='game_rule_values'
  with self.assertRaisesRegex(ValueError,'unexpected'):check.audit(s,774)
 def test_old_bodies_and_wrong_family_list_fail(self):
  s=self.schema(774);s['play']['toClient']['types']['packet_game_rule_values']=check.layout('game_rule_values',775)
  with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,774)
  with tempfile.TemporaryDirectory() as tmp:
   with self.assertRaisesRegex(ValueError,'fourteen'):check.generate(Path(tmp),[])
if __name__=='__main__':unittest.main()
