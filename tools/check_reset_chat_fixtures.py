#!/usr/bin/env python3
"""Check the empty configuration reset-chat envelope, presence and fixture IDs."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def inspect(schema, protocol):
    found = []
    for state in ('handshaking', 'status', 'login', 'configuration', 'play'):
        for direction in ('toClient', 'toServer'):
            types = schema.get(state, {}).get(direction, {}).get('types', {})
            if 'packet' not in types:
                continue
            fields = types['packet'][1]
            mapping = fields[0]['type'][1]['mappings']
            switch = fields[1]['type'][1]
            for raw_id, name in mapping.items():
                if name != 'reset_chat':
                    continue
                if protocol < 766 or (state, direction) != ('configuration', 'toClient'):
                    raise ValueError('unexpected reset-chat presence')
                if switch['compareTo'] != 'name' or switch['fields'].get(name) != 'packet_reset_chat':
                    raise ValueError('unexpected reset-chat dispatch')
                if types.get('packet_reset_chat') != ['container', []]:
                    raise ValueError('nonempty reset-chat body')
                found.append(int(raw_id, 0))
    if found != ([6] if protocol >= 766 else []):
        raise ValueError('reset-chat presence or ID mismatch')
    return found


def fixtures(rows):
    result = '# protocol\tpacket_id\tbody_hex\n'
    for protocol, ids in rows:
        for packet_id in ids:
            result += f'{protocol}\t{packet_id}\t\n'
    return result


def main():
    rows = []
    for entry in json.loads((ROOT / 'research/schema-hashes.json').read_text()):
        data = (ROOT / f"research/protocols/{entry['schema']}.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != entry['sha256']:
            raise ValueError('schema hash mismatch')
        rows.append((entry['protocol'], inspect(json.loads(data), entry['protocol'])))
    if (ROOT / 'tests/fixtures/reset-chat.tsv').read_text() != fixtures(rows):
        raise ValueError('fixture mismatch')
    print('Verified eleven present/three absent empty reset-chat layouts and fixture IDs')


if __name__ == '__main__':
    main()
