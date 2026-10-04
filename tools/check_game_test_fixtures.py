#!/usr/bin/env python3
"""Audit game-test envelopes and independently generate original fixture bytes."""
import hashlib
import json
import struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
NAMES=('game_test_highlight_pos','test_instance_block_status','set_test_block','test_instance_block_action')


def container(*fields): return ['container',[{'name':n,'type':t} for n,t in fields]]

def expected(p):
    out={}
    if p>=770:
        out[('toClient','test_instance_block_status')]=container(('status','anonymousNbt'),('size',['option','vec3i']))
        out[('toServer','set_test_block')]=container(('position','position'),('mode','varint'),('message','string'))
        data=container(('test',['option','string']),('size','vec3i'),('rotation','varint'),('ignoreEntities','bool'),('status','varint'),('errorMessage',['option','anonymousNbt']))
        out[('toServer','test_instance_block_action')]=container(('pos','position'),('action','varint'),('data',data))
    if p>=773: out[('toClient','game_test_highlight_pos')]=container(('absolutePos','position'),('relativePos','position'))
    return out

def inspect(schema,p):
    wanted=expected(p);found={}
    for state in ['handshaking','status','login','configuration','play']:
        for direction in ['toClient','toServer']:
            ts=schema.get(state,{}).get(direction,{}).get('types',{})
            if 'packet' not in ts: continue
            fs=ts['packet'][1];mapping=fs[0]['type'][1]['mappings'];switch=fs[1]['type'][1]
            for key,name in mapping.items():
                if name not in NAMES: continue
                pair=direction,name
                if state!='play' or pair not in wanted or pair in found: raise ValueError('game-test presence')
                if switch['compareTo']!='name' or switch['fields'].get(name)!='packet_'+name: raise ValueError('game-test dispatch')
                if ts.get('packet_'+name)!=wanted[pair]: raise ValueError('game-test layout')
                packet_id=int(key,0)
                if p==776 and name=='test_instance_block_action':
                    # Existing independently audited 776 spectator omission shifts
                    # this raw-schema ID. Never undo the catalog correction.
                    if packet_id!=0x40: raise ValueError('review 776 test-instance ID correction')
                    packet_id=0x41
                found[pair]=packet_id
    if set(found)!=set(wanted): raise ValueError('missing game-test packet')
    if p>=770 and schema['types'].get('vec3i')!=container(('x','varint'),('y','varint'),('z','varint')): raise ValueError('integer vector alias')
    return found

def vi(n):
    n &= 0xffffffff;out=bytearray()
    while n>127: out.append((n&127)|128);n>>=7
    out.append(n);return bytes(out)

def string(s):
    b=s.encode('utf-8');return vi(len(b))+b

def text(s):
    # Original fixture components are ASCII NBT strings, not JSON or captures.
    b=s.encode('ascii');return b'\x08'+struct.pack('>H',len(b))+b

def pos(v):
    x,y,z=v;return struct.pack('>Q',((x&0x3ffffff)<<38)|((z&0x3ffffff)<<12)|(y&0xfff))

def vec(v): return b''.join(vi(n) for n in v)

def cases(name):
    if name=='game_test_highlight_pos':
        return [('zero',pos([0,0,0])*2),('extremes',pos([-33554432,-2048,33554431])+pos([33554431,2047,-33554432])),('mixed',pos([4,5,6])+pos([-4,-5,-6]))]
    if name=='test_instance_block_status':
        return [('no_size',text('')+b'\x00'),('small',text('Ready')+b'\x01'+vec([1,2,3])),('wide',text('Done')+b'\x01'+vec([-2147483648,0,2147483647])),('zero_size',text('0')+b'\x01'+vec([0,0,0]))]
    if name=='set_test_block':
        messages=['','hello','line\nerror','ok😀'];return [(f'mode_{mode}',pos([mode-2,64+mode,2-mode])+vi(mode)+string(messages[mode])) for mode in range(4)]
    out=[]
    for action in range(7):
        test=(b'\x01'+string('rustwire:test')) if action%2 else b'\x00'
        error=(b'\x01'+text('failure')) if action>=3 else b'\x00'
        out.append((f'action_{action}',pos([action,63,-action])+vi(action)+test+vec([action,-action,128+action])+vi(action%4)+bytes([int(action%2==0)])+vi(action%3)+error))
    out.append(('extreme_sizes',pos([-33554432,2047,33554431])+vi(6)+b'\x01'+string('')+vec([-2147483648,2147483647,-1])+vi(3)+b'\x00'+vi(2)+b'\x01'+text('')))
    return out

def fixtures(rows):
    out='# protocol\tname\tdirection\tcase\tpacket_id\tbody_hex\n'
    for p,ids in rows:
        for (direction,name),packet_id in sorted(ids.items()):
            for case,body in cases(name):out+=f'{p}\t{name}\t{direction}\t{case}\t{packet_id}\t{body.hex()}\n'
    return out

def main():
    records=json.loads((ROOT/'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records]!=list(range(763,777)): raise ValueError('family inventory')
    rows=[]
    for r in records:
        raw=(ROOT/f"research/protocols/{r['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest()!=r['sha256']:raise ValueError('schema hash')
        rows.append((r['protocol'],inspect(json.loads(raw),r['protocol'])))
    if fixtures(rows)!=(ROOT/'tests/fixtures/game-test.tsv').read_text():raise ValueError('fixture drift')
    print('Verified 25 game-test layouts, the retained 776 ID correction, and 124 independent fixtures')

if __name__=='__main__':main()
