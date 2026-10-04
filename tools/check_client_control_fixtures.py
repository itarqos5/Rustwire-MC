#!/usr/bin/env python3
"""Audit player-control request layouts and original independent wire fixtures."""
import argparse
import hashlib
import json
import struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
NAMES=('steer_boat','spectate','spectate_entity','spectator_action','pick_item','pick_item_from_block','pick_item_from_entity','select_bundle_item','set_slot_state','set_difficulty','lock_difficulty','change_gamemode')
def container(fields):return ['container',[{'name':n,'type':t} for n,t in fields]]
def layout(name,p):
 if name=='steer_boat':return container([('leftPaddle','bool'),('rightPaddle','bool')])
 if name=='spectate':return container([('target','UUID')]) if p<776 else None
 if name=='spectate_entity':return container([('entityId','varint')]) if p==775 else None
 if name=='spectator_action':return container([('entityId','optvarint')]) if p==776 else None
 if name=='pick_item':return container([('slot','varint')]) if p<769 else None
 if name=='pick_item_from_block':return container([('position','position'),('includeData','bool')]) if p>=769 else None
 if name=='pick_item_from_entity':return container([('entityId','varint'),('includeData','bool')]) if p>=769 else None
 if name=='select_bundle_item':return container([('slotId','varint'),('selectedItemIndex','varint')]) if p>=768 else None
 if name=='set_slot_state':return container([('slot_id','varint'),('window_id','varint' if p<768 else 'ContainerID'),('state','bool')]) if p>=765 else None
 if name=='set_difficulty':return container([('newDifficulty',['mapper',{'type':'u8' if p<771 else 'varint','mappings':dict(enumerate(['peaceful','easy','normal','hard']))}])])
 if name=='lock_difficulty':return container([('locked','bool')])
 if name=='change_gamemode':return container([('mode',['mapper',{'type':'varint','mappings':dict(enumerate(['survival','creative','adventure','spectator']))}])]) if p>=771 else None
 raise ValueError(name)
def expected(name,p):
 # JSON schemas stringify mapper keys; keep construction convenient and explicit.
 return json.loads(json.dumps(layout(name,p)))
def aliases(p):
 out={'bool':'native','u8':'native','varint':'native','UUID':'native','position':['bitfield',[{'name':n,'size':s,'signed':True} for n,s in [('x',26),('z',26),('y',12)]]]}
 if p>=768:out['ContainerID']='varint'
 if p==776:out['optvarint']='varint'
 return out
def audit(schema,p):
 for n,e in aliases(p).items():
  if schema['types'].get(n)!=e:raise ValueError('client-control alias drift: '+n)
 types=schema['play']['toServer']['types'];packet=types['packet'][1];mapping=packet[0]['type'][1]['mappings'];fields=packet[1]['type'][1]['fields']
 if len(mapping)!=len(set(mapping.values())) or len(mapping)!=len(set(int(k,16) for k in mapping)):raise ValueError('duplicate packet mapping')
 ids={}
 for n in NAMES:
  e=expected(n,p)
  if types.get('packet_'+n)!=e:raise ValueError('client-control layout drift: '+n)
  found=[int(k,16) for k,v in mapping.items() if v==n]
  if e is None:
   if found or n in fields:raise ValueError('unexpected absent packet mapping')
  else:
   if len(found)!=1 or fields.get(n)!='packet_'+n:raise ValueError('missing packet mapping')
   ids[n]=found[0]
 if p==776:
  # Preserve the existing independently verified catalog correction. The pinned
  # input omits UUID spectate and shifts spectator_action; do not trust raw IDs.
  tail={0x3e:'arm_animation',0x3f:'spectator_action',0x40:'test_instance_block_action',0x41:'block_place',0x42:'use_item',0x43:'custom_click_action'}
  if {int(k,16):n for k,n in mapping.items() if int(k,16)>=0x3e}!=tail:raise ValueError('review known 776 spectator catalog discrepancy')
  ids['spectator_action']=0x3e
  ids['spectate']=0x40
 return ids

def vi(n):
 n&=0xffffffff;out=bytearray()
 while n>127:out.append((n&127)|128);n>>=7
 out.append(n);return bytes(out)
def pos(x,y,z):return struct.pack('>Q',((x&0x3ffffff)<<38)|((z&0x3ffffff)<<12)|(y&4095))
def fixtures(name,p):
 if expected(name,p) is None and name!='spectate':return []
 if name=='steer_boat':return [(str(n),bytes([n&1,(n>>1)&1])) for n in range(4)]
 if name=='spectate':return [('zero',bytes(16)),('bytes',bytes(range(16)))]
 if name in ('spectate_entity','pick_item','change_gamemode'):return [(str(n),vi(n)) for n in [-2147483648,-1,0,300,2147483647]]
 if name=='spectator_action':return [('absent',b'\0')]+[(str(n),vi(n+1)) for n in [-2147483648,-2,0,300,2147483647]]
 if name=='pick_item_from_block':return [('origin',pos(0,0,0)+b'\0'),('bounds',pos(-33554432,-2048,33554431)+b'\1')]
 if name=='pick_item_from_entity':return [('signed',vi(-1)+b'\0'),('maximum',vi(2147483647)+b'\1')]
 if name=='select_bundle_item':return [('clear',vi(-2147483648)+vi(-1)),('zero',b'\0\0'),('maximum',vi(2147483647)*2)]
 if name=='set_slot_state':return [('off',b'\0\0\0'),('bounds',vi(-2147483648)+vi(2147483647)+b'\1')]
 if name=='set_difficulty':return [(str(n),bytes([n]) if p<771 else vi(n)) for n in ([0,1,2,3,128,255] if p<771 else [-2147483648,-1,0,1,2,3,300,2147483647])]
 if name=='lock_difficulty':return [('off',b'\0'),('on',b'\1')]
 raise ValueError(name)
def generate(directory,records=None):
 if records is None:records=json.loads((ROOT/'research/schema-hashes.json').read_text())
 if [r['protocol'] for r in records]!=list(range(763,777)):raise ValueError('expected fourteen families')
 out=['# Original Python-encoded player-control fixtures, not captures or executed game serializers.','# protocol\tname\tpacket_id\tcase\thex'];count=0
 for row in records:
  data=(directory/(row['schema']+'.json')).read_bytes()
  if hashlib.sha256(data).hexdigest()!=row['sha256']:raise ValueError('schema hash mismatch')
  ids=audit(json.loads(data),row['protocol']);count+=len(ids)
  for n,ident in ids.items():
   for case,body in fixtures(n,row['protocol']):out.append(f"{row['protocol']}\t{n}\t{ident}\t{case}\t{body.hex()}")
 return '\n'.join(out)+'\n',count
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
 output,count=generate(ROOT/'research/protocols');dest=ROOT/'tests/fixtures/client-control.tsv'
 if args.write:dest.write_text(output)
 elif dest.read_text()!=output:raise SystemExit('client-control fixtures differ')
 print(f'Checked {count} supported wire layouts including the guarded 776 spectator correction, and {len(output.splitlines())-2} independent fixtures')
if __name__=='__main__':main()
