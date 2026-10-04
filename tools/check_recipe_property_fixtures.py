#!/usr/bin/env python3
"""Audit modern declaration schemas and emit original synthetic wire fixtures.

Actual schema checks are separate from the clean-copy synthetic unit tests.
No Rust/upstream/game serializer, runtime, or network is used by this script.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
SPEC=importlib.util.spec_from_file_location('display_fixture_tools',Path(__file__).with_name('check_recipe_display_fixtures.py'))
DISPLAY=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(DISPLAY)
container=DISPLAY.container
array=DISPLAY.array
varint=DISPLAY.varint
string=DISPLAY.string

def layout():
    return container([('recipes',array(container([('name','string'),('items',array('varint'))]))),('stoneCutterRecipes',array(container([('input','IDSet'),('slotDisplay','SlotDisplay')])) )])
def property_set(name,ids):return string(name)+varint(len(ids))+b''.join(varint(x) for x in ids)
def fixture_rows(p):
    yield 'empty',b'\x00\x00'
    yield 'empty-property-keys',b'\x02'+property_set('',[])+property_set(':',[])+b'\x00'
    sets=[('rustwire:smithing_base',[0,127,128,300,2147483647,300]),('rustwire:empty',[]),('rustwire:smithing_base',[1])]
    props=varint(len(sets))+b''.join(property_set(name,ids) for name,ids in sets)
    yield 'properties-only',props+b'\x00'
    # Shared original display fixtures make all known variants observable inside
    # modern declarations, without invoking the Rust encoder.
    for case,display in DISPLAY.slot_cases(p):
        for input_name,ingredient in [('ids',b'\x04\x01\xac\x02\x01'),('tag',b'\x00'+string('rustwire:stone')),('empty',b'\x01')]:
            yield case+'-'+input_name,props+b'\x01'+ingredient+display
    # Multiple nested text roots exercise whole-packet rather than per-entry caps.
    displays=dict(DISPLAY.slot_cases(p))
    yield 'multiple-stonecutter',props+b'\x03\x01'+displays['stack-name']+b'\x01'+displays['trim-inline']+b'\x01'+displays['composite']

def generate(schema_dir):
    records=json.loads((ROOT/'research/schema-hashes.json').read_text())
    modern=[row for row in records if 768<=row['protocol']<=776]
    if [row['protocol'] for row in modern]!=list(range(768,777)):raise ValueError('Expected all nine modern protocol families')
    lines=['# Original synthetic fixtures; no release-API or captured-traffic claim.','# protocol\tpacket_id\tcase\thex']
    for row in modern:
        p=row['protocol'];raw=(schema_dir/f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise ValueError('Schema hash mismatch')
        schema=json.loads(raw);types=schema['play']['toClient']['types'];packet=types['packet'];mapping=packet[1][0]['type'];dispatch=packet[1][1]['type']
        if types.get('packet_declare_recipes')!=layout():raise ValueError(f'Declaration layout mismatch: {p}')
        if mapping[0]!='mapper' or mapping[1]['type']!='varint':raise ValueError('Packet mapping mismatch')
        ids=[int(id,16) for id,name in mapping[1]['mappings'].items() if name=='declare_recipes']
        if len(ids)!=1:raise ValueError('Packet presence/ID mismatch')
        if dispatch[0]!='switch' or dispatch[1]['compareTo']!='name' or dispatch[1]['fields'].get('declare_recipes')!='packet_declare_recipes':raise ValueError('Packet dispatch mismatch')
        server=schema['play']['toServer']['types']
        if server.get('packet_declare_recipes') is not None or 'declare_recipes' in server['packet'][1][0]['type'][1]['mappings'].values():raise ValueError('Unexpected serverbound declaration')
        for key,value in DISPLAY.aliases(p).items():
            if schema['types'].get(key)!=value:raise ValueError(f'Alias mismatch: {key}')
        if types.get('SlotDisplay')!=DISPLAY.slot_layout(p):raise ValueError('Nested display mismatch')
        names=schema['types']['SlotComponentType'][1]['mappings'];component=schema['types']['SlotComponent']
        if names.get(str(6 if p>=774 else 5))!='custom_name' or component[1][0]['type']!='SlotComponentType' or component[1][1]['type'][1]['fields'].get('custom_name')!='anonymousNbt':raise ValueError('Fixture component alias mismatch')
        for case,body in fixture_rows(p):lines.append(f'{p}\t{ids[0]}\t{case}\t{body.hex()}')
    return '\n'.join(lines)+'\n'
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--schema-dir',type=Path,default=ROOT/'research/protocols');parser.add_argument('--write',action='store_true');args=parser.parse_args()
    text=generate(args.schema_dir);file=ROOT/'tests/fixtures/recipe-properties.tsv'
    if args.write:file.write_text(text)
    elif not file.exists() or file.read_text()!=text:raise SystemExit('Recipe property fixtures stale')
    print(f'Verified 9 modern declarations and nested aliases, {len(text.splitlines())-2} original synthetic fixtures')
if __name__=='__main__':main()
