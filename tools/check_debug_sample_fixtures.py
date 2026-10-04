#!/usr/bin/env python3
"""Check debug-sample/subscription outer envelopes and independent wire fixtures."""
import hashlib
import json
import struct
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
NAMES = ('debug_sample', 'debug_sample_subscription', 'debug_subscription_request')
REGISTRY_NAMES = ['DedicatedServerTickTime', 'Bees', 'Brains', 'Breezes', 'GoalSelectors', 'EntityPaths',
                  'EntityBlockIntersections', 'BeeHives', 'Pois', 'RedstoneWireOrientations', 'VillageSections',
                  'Raids', 'Structures', 'GameEventListeners', 'NeighborUpdates', 'GameEvents']


def container(*fields): return ['container', [{'name': n, 'type': t} for n, t in fields]]


def expected(p):
    result = {}
    if p >= 766:
        result[('toClient', 'debug_sample')] = container(('sample', ['array', {'countType': 'varint', 'type': 'i64'}]), ('type', 'varint'))
    if 766 <= p <= 772:
        result[('toServer', 'debug_sample_subscription')] = container(('type', 'varint'))
    if p >= 773:
        result[('toServer', 'debug_subscription_request')] = container(('subscriptions', ['array', {'countType': 'varint', 'type': 'DebugSubscriptionDataType'}]))
    return result


def inspect(schema, p):
    wanted = expected(p); found = {}
    for state in ['handshaking', 'status', 'login', 'configuration', 'play']:
        for direction in ['toClient', 'toServer']:
            ts = schema.get(state, {}).get(direction, {}).get('types', {})
            if 'packet' not in ts: continue
            fields = ts['packet'][1]; mapping = fields[0]['type'][1]['mappings']; switch = fields[1]['type'][1]
            for raw_id, name in mapping.items():
                if name not in NAMES: continue
                key = direction, name
                if state != 'play' or key not in wanted or key in found: raise ValueError('debug packet presence')
                if switch['compareTo'] != 'name' or switch['fields'].get(name) != 'packet_' + name: raise ValueError('debug packet dispatch')
                if ts.get('packet_' + name) != wanted[key]: raise ValueError('debug packet layout')
                found[key] = int(raw_id, 0)
    if set(found) != set(wanted): raise ValueError('missing debug packet')
    if p >= 773 and schema['types'].get('DebugSubscriptionDataType') != ['mapper', {
        'type': 'varint', 'mappings': {str(i): n for i, n in enumerate(REGISTRY_NAMES)}}]:
        raise ValueError('debug registry wire representation')
    return found


def vi(n):
    if not 0 <= n <= 0x7fffffff: raise ValueError('fixture VarInt domain')
    out = bytearray()
    while n > 127: out.append((n & 127) | 128); n >>= 7
    out.append(n)
    return bytes(out)


def cases(name):
    if name == 'debug_sample':
        values = [('empty', []), ('zero', [0]), ('signed_edges', [-1, -(1 << 63), (1 << 63)-1]), ('wide_count', list(range(128)))]
        return [(n, vi(len(v)) + b''.join(struct.pack('>q', x) for x in v) + b'\x00') for n, v in values]
    if name == 'debug_sample_subscription': return [('tick_time', b'\x00')]
    values = [('empty', []), ('tick_time', [0]), ('all_builtin', list(range(16))), ('duplicates', [2, 2, 0]),
              ('unresolved_references', [127, 128, 16383, 16384, 0x7fffffff]), ('maximum_count', [0] * 32)]
    return [(n, vi(len(v)) + b''.join(vi(x) for x in v)) for n, v in values]


def fixtures(rows):
    result = '# protocol\tname\tdirection\tcase\tpacket_id\tbody_hex\n'
    for p, ids in rows:
        for (direction, name), packet_id in sorted(ids.items()):
            for case, body in cases(name): result += f'{p}\t{name}\t{direction}\t{case}\t{packet_id}\t{body.hex()}\n'
    return result


def main():
    records = json.loads((ROOT/'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records] != list(range(763, 777)): raise ValueError('family inventory')
    rows = []
    for r in records:
        raw = (ROOT/f"research/protocols/{r['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != r['sha256']: raise ValueError('schema hash')
        rows.append((r['protocol'], inspect(json.loads(raw), r['protocol'])))
    result = fixtures(rows)
    if result != (ROOT/'tests/fixtures/debug-samples.tsv').read_text(): raise ValueError('fixture drift')
    print('Verified 22 debug sample/subscription layouts and 75 original fixtures across 14 families')


if __name__ == '__main__': main()
