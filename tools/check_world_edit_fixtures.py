#!/usr/bin/env python3
"""Audit world-edit schemas and independent fixtures, including the VarLong seed correction."""
import argparse
import hashlib
import json
import struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
NAMES=('generate_structure','update_command_block','update_command_block_minecart','update_jigsaw_block','update_structure_block')
def container(fields):return ['container',[{'name':n,'type':t} for n,t in fields]]
def layout(name,p):
 if name=='generate_structure':return container([('location','position'),('levels','varint'),('keepJigsaws','bool')])
 if name=='update_command_block':return container([('location','position'),('command','string'),('mode','varint'),('flags','u8')])
 if name=='update_command_block_minecart':return container([('entityId','varint'),('command','string'),('track_output','bool')])
 if name=='update_jigsaw_block':
  fields=[('location','position')]+[(n,'string') for n in ['name','target','pool','finalState','jointType']]
  if p>=765:fields += [('selection_priority','varint'),('placement_priority','varint')]
  return container(fields)
 flags='u8' if p<770 else ['mapper',{'type':'u8','mappings':{'0':'ignore_entities','1':'show_air','2':'show_bounding_box','3':'strict'}}]
 return container([('location','position'),('action','varint'),('mode','varint'),('name','string')]+[(f'{kind}_{axis}','i8') for kind in ['offset','size'] for axis in ['x','y','z']]+[('mirror','varint'),('rotation','varint'),('metadata','string'),('integrity','f32'),('seed','varint'),('flags',flags)])
def aliases():return {'varint':'native','bool':'native','u8':'native','i8':'native','f32':'native','string':['pstring',{'countType':'varint'}],'position':['bitfield',[{'name':n,'size':s,'signed':True} for n,s in [('x',26),('z',26),('y',12)]]]}
def audit(schema,p):
 for n,e in aliases().items():
  if schema['types'].get(n)!=e:raise ValueError('world-edit alias drift')
 t=schema['play']['toServer']['types'];packet=t['packet'][1];mapping=packet[0]['type'][1]['mappings'];fields=packet[1]['type'][1]['fields']
 if len(mapping)!=len(set(mapping.values())) or len(mapping)!=len(set(int(k,16) for k in mapping)):raise ValueError('duplicate packet mapping')
 ids={}
 for name in NAMES:
  if t.get('packet_'+name)!=layout(name,p):raise ValueError('world-edit layout drift: '+name)
  found=[int(k,16) for k,n in mapping.items() if n==name]
  if len(found)!=1 or fields.get(name)!='packet_'+name:raise ValueError('missing world-edit mapping')
  ids[name]=found[0]
 return ids
def vi(n,bits=32):
 n&=(1<<bits)-1;out=bytearray()
 while n>127:out.append((n&127)|128);n>>=7
 out.append(n);return bytes(out)
def string(s):
 b=s.encode('utf-8');return vi(len(b))+b
def pos(x,y,z):return struct.pack('>Q',((x&0x3ffffff)<<38)|((z&0x3ffffff)<<12)|(y&4095))
def fixtures(p):
 origin=pos(0,0,0);bounds=pos(-33554432,-2048,33554431)
 yield 'generate_structure','zero',origin+b'\0\0'
 yield 'generate_structure','signed',bounds+vi(-2147483648)+b'\1'
 for mode in range(3):yield 'update_command_block',f'mode-{mode}',bounds+string('say Rustwire 😀')+vi(mode)+bytes([0,7,255][mode:mode+1])
 yield 'update_command_block_minecart','empty',vi(-1)+string('')+b'\0'
 yield 'update_command_block_minecart','command',vi(2147483647)+string('say Rustwire')+b'\1'
 for case,values in [('ordinary',['a','b','minecraft:empty','minecraft:air','aligned']),('raw-joint',['',':','pool','unparsed [state]','Future Joint 😀'])]:
  body=origin+b''.join(string(s) for s in values)
  if p>=765:body+=vi(-2147483648)+vi(2147483647)
  yield 'update_jigsaw_block',case,body
 seeds=[0,300,1<<40,-(1<<63),(1<<63)-1];floats=[0,0x80000000,0x7fc00001,0x7f800000,0xff800000]
 for i,seed in enumerate(seeds):
  body=bounds+vi(i%4)+vi(i%4)+string('unparsed structure 😀')+bytes([128,208,127,255,0,127])+vi(i%3)+vi(i%4)+string('data 😀')+struct.pack('>I',floats[i])+vi(seed,64)+bytes([0,1,2,4,255][i:i+1])
  yield 'update_structure_block',f'seed-{seed}',body

def generate(directory,records=None):
 if records is None:records=json.loads((ROOT/'research/schema-hashes.json').read_text())
 if [r['protocol'] for r in records]!=list(range(763,777)):raise ValueError('expected fourteen families')
 out=['# Original Python world-edit fixtures; VarLong seed/flag interpretation follows independent/static evidence.','# protocol\tname\tpacket_id\tcase\thex']
 for row in records:
  raw=(directory/(row['schema']+'.json')).read_bytes()
  if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise ValueError('schema hash mismatch')
  ids=audit(json.loads(raw),row['protocol'])
  for name,case,body in fixtures(row['protocol']):out.append(f"{row['protocol']}\t{name}\t{ids[name]}\t{case}\t{body.hex()}")
 return '\n'.join(out)+'\n'
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
 output=generate(ROOT/'research/protocols');path=ROOT/'tests/fixtures/world-edit.tsv'
 if args.write:path.write_text(output)
 elif path.read_text()!=output:raise SystemExit('world-edit fixtures differ')
 print(f'Checked 70 pinned layouts and {len(output.splitlines())-2} original fixtures; schema seed/flags discrepancies explicit')
if __name__=='__main__':main()
