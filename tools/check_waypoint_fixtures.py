#!/usr/bin/env python3
"""Audit exact waypoint schemas and independent original wire fixtures."""
import argparse
import hashlib
import json
import struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def container(fields):return ['container',[{'name':n,'type':t} for n,t in fields]]
def mapper(names):return ['mapper',{'type':'varint','mappings':{str(i):n for i,n in enumerate(names)}}]
def layout():
 identity=['switch',{'compareTo':'hasUUID','fields':{'true':container([('uuid','UUID')]),'false':container([('id','string')])}}]
 icon=container([('style','string'),('color',['option',container([(n,'u8') for n in ['red','green','blue']])])])
 data=['switch',{'compareTo':'type','fields':{'vec3i':'vec3i','chunk':container([('chunkX','varint'),('chunkZ','varint')]),'azimuth':'f32'}}]
 waypoint=['container',[{'name':'hasUUID','type':'bool'},{'anon':True,'type':identity},{'name':'icon','type':icon},{'name':'type','type':mapper(['empty','vec3i','chunk','azimuth'])},{'name':'data','type':data}]]
 return container([('operation',mapper(['track','untrack','update'])),('waypoint',waypoint)])
def aliases():return {'UUID':'native','varint':'native','bool':'native','u8':'native','f32':'native','string':['pstring',{'countType':'varint'}],'vec3i':container([(n,'varint') for n in ['x','y','z']])}
def audit(schema,p):
 types=schema['play']['toClient']['types'];expected=layout() if p>=771 else None
 if types.get('packet_tracked_waypoint')!=expected:raise ValueError('waypoint schema layout drift')
 packet=types['packet'][1];mapping=packet[0]['type'][1]['mappings'];fields=packet[1]['type'][1]['fields']
 if len(mapping)!=len(set(mapping.values())) or len(mapping)!=len(set(int(k,16) for k in mapping)):raise ValueError('duplicate packet mapping')
 found=[int(k,16) for k,n in mapping.items() if n=='tracked_waypoint']
 if p<771:
  if found or 'tracked_waypoint' in fields:raise ValueError('unexpected old waypoint mapping')
  return None
 if len(found)!=1 or fields.get('tracked_waypoint')!='packet_tracked_waypoint':raise ValueError('missing waypoint mapping')
 for n,e in aliases().items():
  if schema['types'].get(n)!=e:raise ValueError('waypoint alias drift')
 return found[0]
def vi(n):
 n&=0xffffffff;out=bytearray()
 while n>127:out.append((n&127)|128);n>>=7
 out.append(n);return bytes(out)
def string(s):
 b=s.encode('utf-8');return vi(len(b))+b
def fixtures():
 locations=[b'\x00',b'\x01'+vi(-2147483648)+vi(0)+vi(2147483647),b'\x02'+vi(-1)+vi(300),b'\x03'+struct.pack('>I',0x80000000)]
 for op in range(3):
  for identity in range(2):
   for color in range(2):
    prefix=vi(op)+(b'\1'+bytes(range(16)) if identity else b'\0'+string('A Name 😀'))+string('minecraft:default')+(b'\1\x00\x80\xff' if color else b'\0')
    for kind,body in enumerate(locations):yield f'{op}-{identity}-{color}-{kind}',prefix+body
 for bits in [0x7fc00001,0x7f800000,0xff800000]:yield f'azimuth-{bits}',b'\2\0'+string('')+string(':')+b'\0\3'+struct.pack('>I',bits)
def generate(directory,records=None):
 if records is None:records=json.loads((ROOT/'research/schema-hashes.json').read_text())
 if [r['protocol'] for r in records]!=list(range(763,777)):raise ValueError('expected fourteen families')
 out=['# Original Python waypoint fixtures; not captures or executed release serializers.','# protocol\tpacket_id\tcase\thex']
 for row in records:
  raw=(directory/(row['schema']+'.json')).read_bytes()
  if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise ValueError('schema hash mismatch')
  ident=audit(json.loads(raw),row['protocol'])
  if ident is not None:
   for case,body in fixtures():out.append(f"{row['protocol']}\t{ident}\t{case}\t{body.hex()}")
 return '\n'.join(out)+'\n'
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
 output=generate(ROOT/'research/protocols');path=ROOT/'tests/fixtures/waypoint.tsv'
 if args.write:path.write_text(output)
 elif path.read_text()!=output:raise SystemExit('waypoint fixtures differ')
 print(f'Verified six present/eight absent waypoint bodies and {len(output.splitlines())-2} original fixtures')
if __name__=='__main__':main()
