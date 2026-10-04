#!/usr/bin/env python3
"""Check game-rule/warning schemas and independent Python wire fixtures."""
import argparse
import hashlib
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
NAMES={'toClient':('game_rule_values','low_disk_space_warning'),'toServer':('set_game_rule',)}
def container(fields):return ['container',[{'name':n,'type':t} for n,t in fields]]
def rule_alias():return container([('name','string'),('value','string')])
def layout(name,p):
 if p<775:return None
 if name=='low_disk_space_warning':return container([]) if p==775 else 'void'
 if p==775:return container([('values' if name=='game_rule_values' else 'entries',['array',{'countType':'varint','type':'GameRule'}])])
 return container([('rules',['array',{'countType':'varint','type':container([('gameRule','string'),('value','string')])}])])
def audit(schema,p):
 ids={}
 for direction,names in NAMES.items():
  types=schema['play'][direction]['types'];packet=types['packet'][1];mapping=packet[0]['type'][1]['mappings'];fields=packet[1]['type'][1]['fields']
  if len(mapping)!=len(set(mapping.values())) or len(mapping)!=len(set(int(k,16) for k in mapping)):raise ValueError('duplicate packet mapping')
  for name in names:
   if types.get('packet_'+name)!=layout(name,p):raise ValueError('game-rule layout drift')
   found=[int(k,16) for k,v in mapping.items() if v==name]
   if p<775:
    if found or name in fields:raise ValueError('unexpected old packet mapping')
   else:
    if len(found)!=1 or fields.get(name)!='packet_'+name:raise ValueError('missing packet mapping')
    ids[(direction,name)]=found[0]
 if p>=775:
  if schema['types'].get('string')!=['pstring',{'countType':'varint'}] or schema['types'].get('varint')!='native':raise ValueError('primitive alias drift')
  if p==775 and schema['types'].get('GameRule')!=rule_alias():raise ValueError('GameRule alias drift')
 return ids
def vi(n):
 n&=0xffffffff;out=bytearray()
 while n>127:out.append((n&127)|128);n>>=7
 out.append(n);return bytes(out)
def string(s):
 b=s.encode('utf-8');return vi(len(b))+b
def fixtures(name):
 if name=='low_disk_space_warning':return [('empty',b'')]
 pairs=[('rustwire:rule','true'),('rule','not a parsed value 😀'),('rustwire:rule','false'),('',''),(':','\0')]
 body=vi(len(pairs))+b''.join(string(k)+string(v) for k,v in pairs)
 return [('empty',b'\0'),('ordered-duplicates',body),('multi-byte',vi(1)+string('a'*128)+string('😀'*64))]
def generate(directory,records=None):
 if records is None:records=json.loads((ROOT/'research/schema-hashes.json').read_text())
 if [r['protocol'] for r in records]!=list(range(763,777)):raise ValueError('expected fourteen families')
 out=['# Original Python game-rule/warning fixtures; not captures or executed release serializers.','# protocol\tdirection\tname\tpacket_id\tcase\thex']
 for row in records:
  raw=(directory/(row['schema']+'.json')).read_bytes()
  if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise ValueError('schema hash mismatch')
  for (direction,name),ident in audit(json.loads(raw),row['protocol']).items():
   for case,body in fixtures(name):out.append(f"{row['protocol']}\t{direction}\t{name}\t{ident}\t{case}\t{body.hex()}")
 return '\n'.join(out)+'\n'
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
 output=generate(ROOT/'research/protocols');path=ROOT/'tests/fixtures/game-rules.tsv'
 if args.write:path.write_text(output)
 elif path.read_text()!=output:raise SystemExit('game-rule fixtures differ')
 print(f'Checked six present/36 absent layouts and {len(output.splitlines())-2} original fixtures')
if __name__=='__main__':main()
