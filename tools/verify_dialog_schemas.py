#!/usr/bin/env python3
"""Audit exact dialog schemas and independently encoded corrected wire fixtures.

Custom-click fixture framing intentionally corrects the schema's stale option
shape using release-specific source facts in docs/validation/dialog-wire-facts.json.
No downloads, game execution or upstream serializer execution occurs here.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMES = ('clear_dialog', 'show_dialog', 'custom_click_action')


def container(*fields):
    return ['container', [{'name': n, 'type': t} for n, t in fields]]


def expected(protocol):
    if protocol < 771:
        return {}
    result = {}
    for state in ['configuration', 'play']:
        result[(state, 'toClient', 'clear_dialog')] = ('packet_common_clear_dialog', container())
        dialog = 'anonymousNbt' if state == 'configuration' else ['registryEntryHolder', {
            'baseName': 'dialog', 'otherwise': {'name': 'data', 'type': 'anonymousNbt'}}]
        result[(state, 'toClient', 'show_dialog')] = ('packet_show_dialog', container(('dialog', dialog)))
        # Deliberately match the historical schema defect so schema drift is
        # visible. The implemented framing is independently recorded below.
        result[(state, 'toServer', 'custom_click_action')] = ('packet_common_custom_click_action',
            container(('id', 'string'), ('nbt', ['option', 'anonymousNbt'])))
    return result


def inspect_schema(schema, protocol):
    expected_packets = expected(protocol)
    found = {}
    for state in ['handshaking', 'status', 'login', 'configuration', 'play']:
        for direction in ['toClient', 'toServer']:
            local = schema.get(state, {}).get(direction, {}).get('types', {})
            if 'packet' not in local:
                continue
            fields = local['packet'][1]
            mappings = fields[0]['type'][1]['mappings']
            dispatch = fields[1]['type'][1]
            if dispatch['compareTo'] != 'name':
                raise ValueError('Unexpected packet discriminator')
            for raw_id, name in mappings.items():
                if name not in NAMES:
                    continue
                key = (state, direction, name)
                if key not in expected_packets or key in found:
                    raise ValueError(f'Unexpected packet presence: {protocol} {key}')
                type_name, layout = expected_packets[key]
                if dispatch['fields'].get(name) != type_name:
                    raise ValueError(f'Unexpected packet dispatch: {protocol} {key}')
                if local.get(type_name, schema['types'].get(type_name)) != layout:
                    raise ValueError(f'Unexpected packet layout: {protocol} {key}')
                packet_id = int(raw_id, 0)
                if (protocol, state, direction, name) == (776, 'play', 'toServer', 'custom_click_action'):
                    # Reuse the existing independently verified 26.2 catalog
                    # correction: the schema omitted UUID spectate before this.
                    if packet_id != 0x43:
                        raise ValueError('Review corrected protocol-776 custom-click ID')
                    packet_id = 0x44
                found[key] = packet_id
    if found.keys() != expected_packets.keys():
        raise ValueError(f'Missing packet presence: {protocol}')
    if protocol >= 771:
        for name in ['anonymousNbt', 'registryEntryHolder']:
            if schema['types'].get(name) != 'native':
                raise ValueError(f'Unexpected native type: {protocol} {name}')
    return found


def varint(n):
    if not 0 <= n <= 0x7fffffff:
        raise ValueError('Fixture VarInt outside positive i32 domain')
    result = bytearray()
    while n > 127:
        result.append((n & 127) | 128)
        n >>= 7
    result.append(n)
    return bytes(result)


def bodies(state, name):
    empty = b'\x0a\0'
    opaque = b'\x0a\x08\0\x04type\0\x01x\0'
    if name == 'clear_dialog':
        return {'empty': b''}
    if name == 'show_dialog':
        if state == 'configuration':
            return {'empty_compound': empty, 'opaque': opaque}
        return {'empty_compound': b'\0' + empty, 'opaque': b'\0' + opaque,
                'first_reference': b'\1', 'reference_127': b'\x80\1',
                'maximum_reference': b'\xff\xff\xff\xff\7'}
    # Identifier followed by a byte-length-framed anonymous NBT payload. Absence
    # is exactly the one-byte End root, not a boolean and not a zero-byte buffer.
    return {case: b'\x03x:y' + varint(len(payload)) + payload for case, payload in {
        'absent': b'\0', 'empty_compound': empty, 'byte': b'\x01\x7f',
        'string': b'\x08\0\x02hi'}.items()}


def generate(root=ROOT):
    records = json.loads((root / 'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records] != list(range(763, 777)):
        raise ValueError('Expected all fourteen explicit protocol families')
    lines = ['# Original synthetic dialog wire fixtures; custom-click corrects a pinned schema defect.',
             '# protocol\tstate\tdirection\tname\tcase\tpacket_id\tbody_hex']
    for row in records:
        raw = (root / 'research/protocols' / (row['schema'] + '.json')).read_bytes()
        if hashlib.sha256(raw).hexdigest() != row['sha256']:
            raise ValueError('Schema hash mismatch: ' + row['schema'])
        packets = inspect_schema(json.loads(raw), row['protocol'])
        for (state, direction, name), packet_id in packets.items():
            for case, body in bodies(state, name).items():
                lines.append('\t'.join(map(str, [row['protocol'], state, direction, name, case, packet_id, body.hex()])))
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    output = generate()
    target = ROOT / 'tests/fixtures/dialog.tsv'
    if args.write:
        target.write_text(output)
    elif target.read_text() != output:
        raise SystemExit('Dialog fixtures differ from independent encoding/schema IDs')
    print(f'All fourteen schemas checked; 36 supported state/direction layouts and {len(output.splitlines()) - 2} corrected fixtures')


if __name__ == '__main__':
    main()
