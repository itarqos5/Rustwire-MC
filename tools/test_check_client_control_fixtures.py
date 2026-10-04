import hashlib
import json
import tempfile
import unittest
from pathlib import Path
import check_client_control_fixtures as check

class ClientControlAuditTests(unittest.TestCase):
 def schema(self,p):
  ids={}
  for line in (check.ROOT/'tests/fixtures/client-control.tsv').read_text().splitlines():
   if not line.startswith('#'):
    protocol,name,ident,_,_=line.split('\t')
    if int(protocol)==p:ids[hex(int(ident))]=name
  if p==776:
   ids={k:n for k,n in ids.items() if int(k,16)<0x3e}
   ids.update({hex(k):n for k,n in {0x3e:'arm_animation',0x3f:'spectator_action',0x40:'test_instance_block_action',0x41:'block_place',0x42:'use_item',0x43:'custom_click_action'}.items()})
  bodies={'packet_'+n:check.expected(n,p) for n in check.NAMES if check.expected(n,p) is not None}
  packet=['container',[{'name':'name','type':['mapper',{'type':'varint','mappings':ids}]},{'name':'params','type':['switch',{'compareTo':'name','fields':{n:'packet_'+n for n in ids.values()}}]}]]
  return {'types':check.aliases(p),'play':{'toServer':{'types':{'packet':packet,**bodies}}}}
 def test_fixture_reproduction_without_downloaded_schemas(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);records=[]
   for p in range(763,777):
    raw=json.dumps(self.schema(p)).encode();(root/f'{p}.json').write_bytes(raw);records.append({'protocol':p,'schema':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
   output,count=check.generate(root,records);self.assertEqual(count,107);self.assertEqual(output,(check.ROOT/'tests/fixtures/client-control.tsv').read_text())
   records[0]['sha256']='0'*64
   with self.assertRaisesRegex(ValueError,'hash'):check.generate(root,records)
 def test_primitive_and_semantic_alias_drift(self):
  for p in range(763,777):
   for alias in check.aliases(p):
    s=self.schema(p);s['types'][alias]='wrong'
    with self.assertRaisesRegex(ValueError,'alias'):check.audit(s,p)
 def test_wrong_width_order_and_optional_framing(self):
  for p in range(763,777):
   s=self.schema(p);s['play']['toServer']['types']['packet_set_difficulty']=check.expected('set_difficulty',771 if p<771 else 770)
   with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,p)
  s=self.schema(776);s['play']['toServer']['types']['packet_spectator_action']=check.container([('entityId',['option','varint'])])
  with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,776)
 def test_missing_dispatch_and_duplicate_ids_fail(self):
  s=self.schema(776);s['play']['toServer']['types']['packet'][1][1]['type'][1]['fields'].pop('steer_boat')
  with self.assertRaisesRegex(ValueError,'mapping'):check.audit(s,776)
  for duplicate in ['name','id']:
   s=self.schema(776);m=s['play']['toServer']['types']['packet'][1][0]['type'][1]['mappings'];key=next(iter(m))
   m['0x100' if duplicate=='name' else '0x0'+key[2:]]='steer_boat' if duplicate=='name' else 'other'
   with self.assertRaisesRegex(ValueError,'duplicate'):check.audit(s,776)
  s=self.schema(776);s['play']['toServer']['types']['packet'][1][0]['type'][1]['mappings']['0x3e']='unexpected_tail_packet'
  with self.assertRaisesRegex(ValueError,'spectator catalog discrepancy'):check.audit(s,776)
 def test_all_absent_boundaries_are_checked(self):
  for p in range(763,777):
   for name in check.NAMES:
    if check.expected(name,p) is None:
     s=self.schema(p);s['play']['toServer']['types']['packet_'+name]=check.container([])
     with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,p)
 def test_wrong_direction_and_family_set_fail(self):
  s=self.schema(776);s['play']['toClient']=s['play'].pop('toServer')
  with self.assertRaises(KeyError):check.audit(s,776)
  with tempfile.TemporaryDirectory() as temp:
   with self.assertRaisesRegex(ValueError,'fourteen'):check.generate(Path(temp),[])
if __name__=='__main__':unittest.main()
