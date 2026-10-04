"""Synthetic-only verifier regression tests; no ignored files or network."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from contextlib import contextmanager
SPEC=importlib.util.spec_from_file_location('display_check',Path(__file__).with_name('check_recipe_display_fixtures.py'))
CHECK=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(CHECK)
FIXTURES=(CHECK.ROOT/'tests/fixtures/recipe-display.tsv').read_text()
@contextmanager
def synthetic_schemas():
    ids={}
    for line in FIXTURES.splitlines():
        if line.startswith('#'):continue
        p,name,id,_,_=line.split('\t')
        if int(id)>=0:ids[(int(p),name)]=int(id)
    with tempfile.TemporaryDirectory() as temp:
        root=Path(temp);schemas=root/'schemas';schemas.mkdir();(root/'research').mkdir();records=[]
        for p in range(763,777):
            types={};mapping={};dispatch={}
            if p>=768:types.update({'SlotDisplay':CHECK.slot_layout(p),'RecipeDisplay':CHECK.recipe_layout()})
            for name,body in CHECK.packets(p).items():
                if body is None:continue
                types['packet_'+name]=body;mapping[hex(ids.get((p,name),250))]=name;dispatch[name]='packet_'+name
            types['packet']=CHECK.container([('name',['mapper',{'type':'varint','mappings':mapping}]),('params',['switch',{'compareTo':'name','fields':dispatch}])])
            schema={'types':CHECK.aliases(p),'play':{'toClient':{'types':types}}}
            if p>=768:
                schema['types']['SlotComponentType']=['mapper',{'type':'varint','mappings':{str(6 if p>=774 else 5):'custom_name'}}]
                schema['types']['SlotComponent']=CHECK.container([('type','SlotComponentType'),('data',['switch',{'compareTo':'type','fields':{'custom_name':'anonymousNbt'}}])])
            raw=json.dumps(schema).encode();(schemas/f'{p}.json').write_bytes(raw)
            records.append({'protocol':p,'schema':str(p),'sha256':hashlib.sha256(raw).hexdigest()})
        (root/'research/schema-hashes.json').write_text(json.dumps(records))
        with patch.object(CHECK,'ROOT',root):yield root,schemas
class VerifierTests(unittest.TestCase):
    def mutate(self,callback,p=776,repin=True):
        with synthetic_schemas() as (root,schemas):
            path=schemas/f'{p}.json';schema=json.loads(path.read_text());callback(schema);raw=json.dumps(schema,indent=2).encode();path.write_bytes(raw)
            manifest=root/'research/schema-hashes.json';records=json.loads(manifest.read_text())
            if repin:next(row for row in records if row['protocol']==p)['sha256']=hashlib.sha256(raw).hexdigest()
            manifest.write_text(json.dumps(records));return CHECK.generate(schemas)
    def test_synthetic_corpus_matches(self):
        with synthetic_schemas() as (_,schemas):text,count=CHECK.generate(schemas)
        self.assertEqual(text,FIXTURES);self.assertEqual(count,23)
    def test_pins_are_enforced(self):
        with self.assertRaisesRegex(ValueError,'hash mismatch'):self.mutate(lambda s:None,repin=False)
    def test_union_kind_and_recursive_fields_are_checked(self):
        def kind(s):s['play']['toClient']['types']['SlotDisplay'][1][0]['type'][1]['mappings']['5']='something_else'
        def leaf(s):s['play']['toClient']['types']['SlotDisplay'][1][1]['type'][1]['fields']['item_stack']='ItemStackTemplate'
        def recipe(s):s['play']['toClient']['types']['RecipeDisplay'][1][1]['type'][1]['fields']['crafting_shaped'][1][2]['type']='varint'
        for change in [kind,leaf,recipe]:
            with self.assertRaisesRegex(ValueError,'Display mismatch'):self.mutate(change)
    def test_absent_packets_and_duplicate_mappings(self):
        def absent(s):s['play']['toClient']['types']['packet_recipe_book_add']=CHECK.packets(768)['recipe_book_add']
        with self.assertRaisesRegex(ValueError,'Packet mismatch'):self.mutate(absent,p=767)
        def duplicate(s):s['play']['toClient']['types']['packet'][1][0]['type'][1]['mappings']['0xff']='recipe_book_add'
        with self.assertRaisesRegex(ValueError,'Packet mismatch'):self.mutate(duplicate)
    def test_aliases_and_dispatch(self):
        def alias(s):s['types']['ContainerID']='u8'
        def dispatch(s):s['play']['toClient']['types']['packet'][1][1]['type'][1]['fields']['recipe_book_add']='packet_craft_recipe_response'
        for change,error in [(alias,'Alias mismatch'),(dispatch,'dispatch mismatch')]:
            with self.assertRaisesRegex(ValueError,error):self.mutate(change)
    def test_template_correction_is_explicit_not_schema_rewrite(self):
        self.assertEqual(CHECK.slot_layout(775)[1][1]['type'][1]['fields']['item_stack'],'ItemStackTemplate')
        self.assertEqual(CHECK.slot_layout(776)[1][1]['type'][1]['fields']['item_stack'],'Slot')
        for p in [775,776]:self.assertEqual(dict(CHECK.slot_cases(p))['stack-zero'],bytes.fromhex('05ac02000000'))
if __name__=='__main__':unittest.main()
