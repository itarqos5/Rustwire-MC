#!/usr/bin/env python3
"""Reproduce opaque custom-payload bodies from hash-pinned envelope schemas."""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
CASES = [('empty', 'rustwire:test', b''), ('binary', 'rustwire:opaque', bytes([0, 255, 128, 10])),
         ('implicit_namespace', 'brand', bytes([0, 128])), ('empty_namespace', ':x', bytes([1, 2])),
         ('empty_path', 'minecraft:', b''), ('long_prefix', 'x' * 128, bytes(range(128)))]


def varint(n):
    out = bytearray()
    while n > 127:
        out.append((n & 127) | 128); n >>= 7
    out.append(n)
    return bytes(out)


def body(channel, payload):
    raw = channel.encode('utf-8')
    return varint(len(raw)) + raw + payload


def inspect(schema, protocol):
    expected = {(s, d) for s in ('configuration', 'play') for d in ('toClient', 'toServer')
                if s != 'configuration' or protocol >= 764}
    found = {}
    for state in ('handshaking', 'status', 'login', 'configuration', 'play'):
        for direction in ('toClient', 'toServer'):
            types = schema.get(state, {}).get(direction, {}).get('types', {})
            if 'packet' not in types: continue
            fields = types['packet'][1]
            mapping = fields[0]['type'][1]['mappings']; switch = fields[1]['type'][1]
            for key, name in mapping.items():
                if name != 'custom_payload': continue
                pair = (state, direction)
                if pair not in expected or pair in found: raise ValueError('custom payload presence')
                if switch['compareTo'] != 'name' or switch['fields'].get(name) != 'packet_custom_payload':
                    raise ValueError('custom payload dispatch')
                if types.get('packet_custom_payload') != ['container', [
                    {'name': 'channel', 'type': 'string'}, {'name': 'data', 'type': 'restBuffer'}]]:
                    raise ValueError('custom payload shape')
                found[pair] = int(key, 0)
    if set(found) != expected: raise ValueError('missing custom payload')
    return found


def fixtures(rows):
    out = '# protocol\tstate\tdirection\tcase\tpacket_id\tbody_hex\n'
    for p, ids in rows:
        for (s, d), packet_id in sorted(ids.items()):
            for name, channel, payload in CASES:
                out += f'{p}\t{s}\t{d}\t{name}\t{packet_id}\t{body(channel, payload).hex()}\n'
    return out


def main():
    rows = []
    for row in json.loads((ROOT/'research/schema-hashes.json').read_text()):
        data = (ROOT/f"research/protocols/{row['schema']}.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != row['sha256']: raise ValueError('schema hash mismatch')
        rows.append((row['protocol'], inspect(json.loads(data), row['protocol'])))
    result = fixtures(rows)
    if (ROOT/'tests/fixtures/custom-payload.tsv').read_text() != result: raise ValueError('fixture drift')
    print('Verified 54 exact state/direction layouts and 324 independently encoded opaque payload fixtures')


if __name__ == '__main__': main()
