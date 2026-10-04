"""Constructed-schema regression tests; no ignored schemas or network required."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from contextlib import contextmanager
SPEC=importlib.util.spec_from_file_location('properties_check',Path(__file__).with_name('check_recipe_property_fixtures.py'))
CHECK=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(CHECK)
FIXTURES=(CHECK.ROOT/'tests/fixtures/recipe-properties.tsv').read_text()
@contextmanager
def synthetic_schemas():
    ids={int(row.split('\t')[0]):int(row.split('\t')[1]) for row in FIXTURES.splitlines() if not row.startswith('#')}
    with tempfile.TemporaryDirectory() as directory:
        root=Path(directory);schemas=root/'schemas';schemas.mkdir();(root/'research').mkdir();records=[]
        for p in range(768,777):
            packet=lambda mapping,fields:CHECK.container([('name',['mapper',{'type':'varint','mappings':mapping}]),('params',['switch',{'compareTo':'name','fields':fields}])])
            types=CHECK.DISPLAY.aliases(p)
            types['SlotComponentType']=['mapper',{'type':'varint','mappings':{str(6 if p>=774 else 5):'custom_name'}}]
            types['SlotComponent']=CHECK.container([('type','SlotComponentType'),('data',['switch',{'compareTo':'type','fields':{'custom_name':'anonymousNbt'}}])])
            schema={'types':types,'play':{'toClient':{'types':{'SlotDisplay':CHECK.DISPLAY.slot_layout(p),'packet_declare_recipes':CHECK.layout(),'packet':packet({hex(ids[p]):'declare_recipes'},{'declare_recipes':'packet_declare_recipes'})}},'toServer':{'types':{'packet':packet({},{})}}}}
            raw=json.dumps(schema).encode();(schemas/f'{p}.json').write_bytes(raw);records.append({'protocol':p,'schema':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
        (root/'research/schema-hashes.json').write_text(json.dumps(records))
        with patch.object(CHECK,'ROOT',root):yield root,schemas
class PropertyVerifierTests(unittest.TestCase):
    def mutate(self,callback,p=776,repin=True):
        with synthetic_schemas() as (root,schemas):
            path=schemas/f'{p}.json';schema=json.loads(path.read_text());callback(schema);raw=json.dumps(schema,indent=2).encode();path.write_bytes(raw)
            manifest=root/'research/schema-hashes.json';records=json.loads(manifest.read_text())
            if repin:next(row for row in records if row['protocol']==p)['sha256']=hashlib.sha256(raw).hexdigest()
            manifest.write_text(json.dumps(records));return CHECK.generate(schemas)
    def test_synthetic_fixtures_match_committed(self):
        with synthetic_schemas() as (_,schemas):self.assertEqual(CHECK.generate(schemas),FIXTURES)
    def test_hash_checks(self):
        with self.assertRaisesRegex(ValueError,'hash mismatch'):self.mutate(lambda s:None,repin=False)
    def test_outer_field_order_counts_and_element_types(self):
        def order(s):s['play']['toClient']['types']['packet_declare_recipes'][1].reverse()
        def count(s):s['play']['toClient']['types']['packet_declare_recipes'][1][0]['type'][1]['countType']='i32'
        def element(s):s['play']['toClient']['types']['packet_declare_recipes'][1][1]['type'][1]['type'][1][0]['type']='varint'
        for mutation in [order,count,element]:
            with self.assertRaisesRegex(ValueError,'layout mismatch'):self.mutate(mutation)
    def test_presence_direction_and_dispatch(self):
        def missing(s):del s['play']['toClient']['types']['packet_declare_recipes']
        def duplicate(s):s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0xff']='declare_recipes'
        def server(s):s['play']['toServer']['types']['packet_declare_recipes']=CHECK.layout()
        def dispatch(s):s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields']['declare_recipes']='packet_recipe_book_add'
        for mutation,error in [(missing,'layout'),(duplicate,'presence'),(server,'serverbound'),(dispatch,'dispatch')]:
            with self.assertRaisesRegex(ValueError,error):self.mutate(mutation)
    def test_nested_aliases_and_explicit_776_schema_discrepancy(self):
        def alias(s):s['types']['IDSet']='varint'
        def display(s):s['play']['toClient']['types']['SlotDisplay'][1][1]['type'][1]['fields']['item_stack']='ItemStackTemplate'
        for mutation,error in [(alias,'Alias'),(display,'Nested display')]:
            with self.assertRaisesRegex(ValueError,error):self.mutate(mutation)
    def test_primitive_and_fixture_component_reference(self):
        def primitive(s):s['types']['varint']='i32'
        def component(s):s['types']['SlotComponentType'][1]['mappings']['6']='other'
        for mutation,error in [(primitive,'Alias'),(component,'component alias')]:
            with self.assertRaisesRegex(ValueError,error):self.mutate(mutation)
if __name__=='__main__':unittest.main()
