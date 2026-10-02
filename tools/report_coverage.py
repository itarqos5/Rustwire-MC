#!/usr/bin/env python3
"""Report exact implemented item-component and metadata serializer boundaries.

Read-only hash-verified schema inspection. --check verifies the checked-in JSON.
This measures implemented outer layouts, not live account/gameplay correctness;
nested Slot components may have a narrower coverage boundary.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT/'tools'/f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

item = load('generate_item_components')
metadata = load('generate_entity_metadata')
records = []
for row in json.loads((ROOT/'research/schema-hashes.json').read_text()):
    protocol = row['protocol']
    raw = (ROOT/'research/protocols'/f"{row['schema']}.json").read_bytes()
    if hashlib.sha256(raw).hexdigest() != row['sha256']:
        raise SystemExit(f'Schema hash mismatch for {protocol}')
    types = json.loads(raw)['types']
    record = {'protocol': protocol, 'releases': row['releases']}
    if protocol >= 766:
        component = types['SlotComponent']
        names = types['SlotComponentType'][1]['mappings']
        fields = component[1][1]['type'][1]['fields']
        layouts = {name: item.classify(name, fields[name], protocol) for name in names.values()}
        record['item_components'] = {'supported': [n for n, k in layouts.items() if k != 'Unsupported'],
                                     'unsupported': [n for n, k in layouts.items() if k == 'Unsupported']}
    else:
        record['item_components'] = None
    entry = types.get('entityMetadataEntry', types['entityMetadata'][1]['type'])
    if isinstance(entry, str):
        entry = types[entry]
    names = entry[1][1]['type'][1]['mappings']
    fields = (entry[1][2]['type'][1]['fields'] if 'entityMetadataEntry' in types
              else types['entityMetadataItem'][1]['fields'])
    supported, unsupported = [], []
    for name in names.values():
        if name in metadata.BASE or name in ('component', 'optional_component') or fields[name] == 'varint':
            supported.append(name)
        else:
            unsupported.append(name)
    record['entity_metadata'] = {'supported': supported, 'unsupported': unsupported}
    records.append(record)
report = {'description': 'Implemented payload-layout boundaries; nested item and NBT budgets still apply',
          'records': records}
output = json.dumps(report, indent=2) + '\n'
path = ROOT/'docs/typed-coverage.json'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
if args.check:
    if not path.exists() or path.read_text() != output:
        raise SystemExit('Coverage report is stale')
    print('Typed coverage report matches all hash-pinned inputs')
else:
    path.write_text(output)
    print(f'Wrote {path.relative_to(ROOT)}')
