#!/usr/bin/env python3
"""Check the documented minecart schema discrepancy and original wire fixtures."""
import argparse
import hashlib
import json
import struct
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
def container(fields): return ['container', [{'name': n, 'type': t} for n, t in fields]]
def layout(p):
    if p < 768: return None
    step = container([('position', 'vec3f'), ('movement' if p < 773 else 'velocity', 'vec3f'), ('yaw', 'f32'), ('pitch', 'f32'), ('weight', 'f32')])
    return container([('entityId', 'varint'), ('steps', ['array', {'countType': 'varint', 'type': step}])])
def audit(schema, p):
    t = schema['play']['toClient']['types']; expected = layout(p)
    if t.get('packet_move_minecart') != expected: raise ValueError('minecart schema layout drift')
    packet = t['packet'][1]; mapping = packet[0]['type'][1]['mappings']; dispatch = packet[1]['type'][1]['fields']
    ids = [int(i,16) for i in mapping]
    if len(ids) != len(set(ids)) or len(mapping.values()) != len(set(mapping.values())): raise ValueError('duplicate packet mapping')
    found = [int(i,16) for i,n in mapping.items() if n == 'move_minecart']
    if p < 768:
        if found or 'move_minecart' in dispatch: raise ValueError('unexpected old minecart mapping')
        return None
    if len(found) != 1 or dispatch.get('move_minecart') != 'packet_move_minecart': raise ValueError('missing minecart mapping')
    for n,e in {'vec3f':container([(axis,'f32') for axis in ['x','y','z']]),'f32':'native','varint':'native'}.items():
        if schema['types'].get(n) != e: raise ValueError('minecart alias drift')
    return found[0]
def vi(n):
    n &= 0xffffffff; out=bytearray()
    while n > 127: out.append((n & 127) | 128); n >>= 7
    out.append(n); return bytes(out)
def fixtures():
    # Six doubles, two raw angle bytes, one float: 54 bytes per step.
    bits = [0x3ff4000000000000, 0xc004000000000000, 0x433fffffffffffff, 0x8000000000000000, 0x7ff8000000000001, 0x7ff0000000000000]
    step = b''.join(struct.pack('>Q',n) for n in bits) + b'\xff\x80' + struct.pack('>I',0x7fc00001)
    finite = struct.pack('>6dBBf', 0, -30000000.5, 17.25, 0.125, -0.25, 0.5, 1, 127, -0.0)
    return [('empty',vi(-1)+b'\x00'),('bits',vi(300)+b'\x01'+step),('multiple',vi(-2147483648)+b'\x02'+finite+finite)]
def generate(schema_dir, records=None):
    if records is None: records=json.loads((ROOT/'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records] != list(range(763,777)): raise ValueError('expected fourteen families')
    out=['# Original Python wire fixtures; schema discrepancy corrected by independent/static evidence.', '# protocol\tpacket_id\tcase\thex']
    for row in records:
        data=(schema_dir/(row['schema']+'.json')).read_bytes()
        if hashlib.sha256(data).hexdigest()!=row['sha256']:raise ValueError('schema hash mismatch')
        ident=audit(json.loads(data),row['protocol'])
        if ident is not None:
            for case,body in fixtures():out.append(f"{row['protocol']}\t{ident}\t{case}\t{body.hex()}")
    return '\n'.join(out)+'\n'
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
    output=generate(ROOT/'research/protocols');path=ROOT/'tests/fixtures/minecart.tsv'
    if args.write:path.write_text(output)
    elif path.read_text()!=output:raise SystemExit('minecart fixtures differ')
    print('Verified nine present/five absent layouts and 27 fixtures; schema f32 vectors/angles explicitly NOT used as wire truth')
if __name__=='__main__':main()
