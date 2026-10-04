import hashlib
import json
import tempfile
import unittest
from pathlib import Path
import check_waypoint_fixtures as check
class WaypointAuditTests(unittest.TestCase):
 def schema(self,p):
  ids={}
  for line in (check.ROOT/'tests/fixtures/waypoint.tsv').read_text().splitlines():
   if not line.startswith('#'):
    protocol,ident,_,_=line.split('\t')
    if int(protocol)==p:ids[hex(int(ident))]='tracked_waypoint'
  types={'packet':['container',[{'name':'name','type':['mapper',{'type':'varint','mappings':ids}]},{'name':'params','type':['switch',{'compareTo':'name','fields':{'tracked_waypoint':'packet_tracked_waypoint'} if p>=771 else {}}]}]]}
  if p>=771:types['packet_tracked_waypoint']=check.layout()
  return {'types':check.aliases(),'play':{'toClient':{'types':types}}}
 def test_all_families_reproduce_fixtures_without_downloads(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);rows=[]
   for p in range(763,777):
    raw=json.dumps(self.schema(p)).encode();(root/f'{p}.json').write_bytes(raw);rows.append({'protocol':p,'schema':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
   self.assertEqual(check.generate(root,rows),(check.ROOT/'tests/fixtures/waypoint.tsv').read_text())
   rows[0]['sha256']='0'*64
   with self.assertRaisesRegex(ValueError,'hash'):check.generate(root,rows)
 def test_nested_field_and_primitive_alias_drift_fails(self):
  for p in range(771,777):
   for alias in check.aliases():
    s=self.schema(p);s['types'][alias]='wrong'
    with self.assertRaisesRegex(ValueError,'alias'):check.audit(s,p)
   s=self.schema(p);s['play']['toClient']['types']['packet_tracked_waypoint'][1][1]['type'][1][2]['type'][1][1]['type']=['option','i32']
   with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,p)
 def test_packet_id_name_and_direction_checks(self):
  s=self.schema(776);s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields'].clear()
  with self.assertRaisesRegex(ValueError,'mapping'):check.audit(s,776)
  s=self.schema(776);s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0x100']='tracked_waypoint'
  with self.assertRaisesRegex(ValueError,'duplicate'):check.audit(s,776)
  s=self.schema(776);s['play']['toServer']=s['play'].pop('toClient')
  with self.assertRaises(KeyError):check.audit(s,776)
 def test_old_version_absence_and_family_set_checks(self):
  s=self.schema(770);s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0x00']='tracked_waypoint'
  with self.assertRaisesRegex(ValueError,'unexpected'):check.audit(s,770)
  s=self.schema(770);s['play']['toClient']['types']['packet_tracked_waypoint']=check.layout()
  with self.assertRaisesRegex(ValueError,'layout'):check.audit(s,770)
  with tempfile.TemporaryDirectory() as tmp:
   with self.assertRaisesRegex(ValueError,'fourteen'):check.generate(Path(tmp),[])
if __name__=='__main__':unittest.main()
