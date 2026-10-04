#!/usr/bin/env python3
"""Audit pinned editing/query schemas and original Python-encoded fixtures."""
import argparse
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMES = ('update_sign', 'edit_book', 'name_item', 'set_beacon_effect', 'query_block_nbt', 'query_entity_nbt', 'set_creative_slot')
def container(fields): return ['container', [{'name': n, 'type': t} for n, t in fields]]
def array(t): return ['array', {'countType': 'varint', 'type': t}]
def layouts(p):
    return {
        'update_sign': container([('location', 'position'), ('isFrontText', 'bool')] + [(f'text{i}', 'string') for i in range(1, 5)]),
        'edit_book': container([('hand', 'varint'), ('pages', array('string')), ('title', ['option', 'string'])]),
        'name_item': container([('name', 'string')]),
        'set_beacon_effect': container([('primary_effect', ['option', 'varint']), ('secondary_effect', ['option', 'varint'])]),
        'query_block_nbt': container([('transactionId', 'varint'), ('location', 'position')]),
        'query_entity_nbt': container([('transactionId', 'varint'), ('entityId', 'varint')]),
        'set_creative_slot': container([('slot', 'i16'), ('item', 'slot' if p < 766 else 'Slot' if p < 770 else 'UntrustedSlot')]),
    }
def aliases(p):
    out = {'varint': 'native', 'i16': 'native', 'bool': 'native', 'string': ['pstring', {'countType': 'varint'}],
           'position': ['bitfield', [{'name': n, 'size': size, 'signed': True} for n, size in [('x', 26), ('z', 26), ('y', 12)]]]}
    if p >= 770:
        out['ByteArray'] = ['buffer', {'countType': 'varint'}]
        out['UntrustedSlotComponent'] = container([('type', 'SlotComponentType'), ('data', 'ByteArray')])
        patch = container([('itemId', 'varint'), ('addedComponentCount', 'varint'), ('removedComponentCount', 'varint'),
             ('components', ['array', {'count': 'addedComponentCount', 'type': 'UntrustedSlotComponent'}]),
             ('removeComponents', ['array', {'count': 'removedComponentCount', 'type': container([('type', 'SlotComponentType')])}])])
        out['UntrustedSlot'] = ['container', [{'name': 'itemCount', 'type': 'varint'}, {'anon': True, 'type': ['switch', {'compareTo': 'itemCount', 'fields': {'0': 'void', 'false': 'void'}, 'default': patch}]}]]
    return out

def audit(schema, p):
    types = schema['types']
    for name, expected in aliases(p).items():
        if types.get(name) != expected: raise ValueError(f'{p}: alias {name}')
    play = schema['play']['toServer']['types']
    packet = play['packet'][1]
    mapping = packet[0]['type'][1]['mappings']
    ids = [int(key, 16) for key in mapping]
    if len(ids) != len(set(ids)): raise ValueError('duplicate numeric packet ID')
    names = list(mapping.values())
    if len(names) != len(set(names)): raise ValueError('duplicate packet name')
    dispatch = packet[1]['type'][1]['fields']
    result = {}
    for name, expected in layouts(p).items():
        if play.get('packet_' + name) != expected: raise ValueError(f'{p}: layout {name}')
        if names.count(name) != 1 or dispatch.get(name) != 'packet_' + name: raise ValueError(f'{p}: packet mapping {name}')
        result[name] = next(int(key, 16) for key, value in mapping.items() if value == name)
    return result

def vi(value):
    value &= 0xffffffff
    out = bytearray()
    while value > 127: out.append((value & 127) | 128); value >>= 7
    out.append(value)
    return bytes(out)
def string(s):
    b = s.encode('utf-8'); return vi(len(b)) + b
def position(x, y, z): return struct.pack('>Q', ((x & 0x3ffffff) << 38) | ((z & 0x3ffffff) << 12) | (y & 0xfff))
def fixtures(p):
    yield 'update_sign', 'empty', position(0, 0, 0) + b'\x01' + b'\x00' * 4
    yield 'update_sign', 'unicode', position(-33554432, -2048, 33554431) + b'\x00' + b''.join(map(string, ['hello', '😀', 'nul\0', '']))
    yield 'edit_book', 'empty', vi(-1) + b'\x00\x00'
    yield 'edit_book', 'signed', vi(40) + vi(3) + b''.join(map(string, ['a', '😀', ''])) + b'\x01' + string('Title')
    yield 'name_item', 'empty', b'\x00'
    yield 'name_item', 'unicode', string('Renamed 😀')
    yield 'set_beacon_effect', 'absent', b'\x00\x00'
    yield 'set_beacon_effect', 'present-negative', b'\x01' + vi(0) + b'\x01' + vi(-1)
    yield 'set_beacon_effect', 'bounds', b'\x01' + vi(-2147483648) + b'\x01' + vi(2147483647)
    yield 'query_block_nbt', 'zero', b'\x00' + position(0, 0, 0)
    yield 'query_block_nbt', 'bounds', vi(-1) + position(-33554432, 2047, 33554431)
    yield 'query_entity_nbt', 'zero', b'\x00\x00'
    yield 'query_entity_nbt', 'bounds', vi(-2147483648) + vi(2147483647)
    yield 'set_creative_slot', 'empty', b'\xff\xff\x00'
    slot = (b'\x01' + vi(5) + b'\x40\x00') if p < 766 else (vi(300) + vi(5) + b'\x00\x00')
    yield 'set_creative_slot', 'plain', b'\x80\x00' + slot
    if p >= 770:
        # Unknown IDs remain safe because each added payload has a byte length.
        slot = vi(300) + vi(5) + vi(2) + vi(2) + vi(2147483647) + vi(3) + b'\x00\xff\x01' + vi(3) + b'\x00' + vi(4) + vi(4)
        yield 'set_creative_slot', 'opaque-framed', b'\x00\x05' + slot

def generate(schema_dir, records=None):
    if records is None: records = json.loads((ROOT / 'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records] != list(range(763, 777)): raise ValueError('expected fourteen families')
    lines = ['# Original Python fixtures, not captures or executed game serializers.', '# protocol\tname\tid\tcase\thex']
    for row in records:
        data = (schema_dir / (row['schema'] + '.json')).read_bytes()
        if hashlib.sha256(data).hexdigest() != row['sha256']: raise ValueError('schema hash mismatch')
        ids = audit(json.loads(data), row['protocol'])
        for name, case, body in fixtures(row['protocol']): lines.append(f"{row['protocol']}\t{name}\t{ids[name]}\t{case}\t{body.hex()}")
    return '\n'.join(lines) + '\n'
def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('--write', action='store_true'); args = parser.parse_args()
    output = generate(ROOT / 'research/protocols'); dest = ROOT / 'tests/fixtures/editing.tsv'
    if args.write: dest.write_text(output)
    elif dest.read_text() != output: raise SystemExit('editing fixtures differ; use --write after review')
    print(f'Verified 98 outer layouts and {len(output.splitlines()) - 2} independent fixtures; framed creative components remain opaque')
if __name__ == '__main__': main()
