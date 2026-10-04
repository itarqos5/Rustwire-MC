#!/usr/bin/env python3
"""Hash-pinned display schema audit and original independent wire fixtures.

No game or Rust encoder is invoked. Unit tests use synthetic schemas; this
command alone needs the separately fetched, ignored pinned schemas.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
ROOT = Path(__file__).resolve().parents[1]

def container(fields):
    return ['container', [{'name': n, 'type': t} for n, t in fields]]
def array(kind):
    return ['array', {'countType': 'varint', 'type': kind}]
def union(fields):
    return container([('type', ['mapper', {'type': 'varint', 'mappings': {str(i): name for i, (name, _) in enumerate(fields)}}]), ('data', ['switch', {'compareTo': 'type', 'fields': dict(fields)}])])
def slot_layout(p):
    trim = container([('base', 'SlotDisplay'), ('material', 'SlotDisplay'), ('pattern', 'SlotDisplay' if p < 770 else ['registryEntryHolder', {'baseName': 'patternId', 'otherwise': {'name': 'data', 'type': 'ArmorTrimPattern'}}])])
    fields = [('empty','void'), ('any_fuel','void')]
    if p >= 775:
        fields += [('with_any_potion', container([('base' if p == 775 else 'display','SlotDisplay')])), ('only_with_component', container([('source','SlotDisplay'), ('component','SlotComponentType' if p == 775 else 'varint')]))]
    # Preserve the actual pinned 776 disagreement; implementation deliberately
    # uses ItemStackTemplate on independent source evidence recorded in the audit.
    fields += [('item','varint'), ('item_stack','ItemStackTemplate' if p == 775 else 'Slot'), ('tag','string')]
    if p >= 775: fields += [('dyed_slot_demo' if p == 775 else 'dyed',container([('dye','SlotDisplay'),('target','SlotDisplay')]))]
    return union(fields + [('smithing_trim',trim), ('with_remainder',container([('input','SlotDisplay'),('remainder','SlotDisplay')])), ('composite',array('SlotDisplay'))])
def recipe_layout():
    result = [('result','SlotDisplay'), ('craftingStation','SlotDisplay')]
    return union([
        ('crafting_shapeless',container([('ingredients',array('SlotDisplay'))]+result)),
        ('crafting_shaped',container([('width','varint'),('height','varint'),('ingredients',array('SlotDisplay'))]+result)),
        ('furnace',container([('ingredient','SlotDisplay'),('fuel','SlotDisplay')]+result+[('duration','varint'),('experience','f32')])),
        ('stonecutter',container([('ingredient','SlotDisplay')]+result)),
        ('smithing',container([('template','SlotDisplay'),('base','SlotDisplay'),('addition','SlotDisplay')]+result))])
CATEGORIES = ['crafting_building_blocks','crafting_redstone','crafting_equipment','crafting_misc','furnace_food','furnace_blocks','furnace_misc','blast_furnace_blocks','blast_furnace_misc','smoker_food','stonecutter','smithing','campfire']
def packets(p):
    if p < 768: return {'recipe_book_add':None,'craft_recipe_response':container([('windowId','i8' if p < 766 else 'ContainerID'),('recipe','string')])}
    entry = container([('recipe',container([('displayId','varint'),('display','RecipeDisplay'),('group','optvarint'),('category',['mapper',{'type':'varint','mappings':{str(i):v for i,v in enumerate(CATEGORIES)}}]),('craftingRequirements',['option',array('IDSet')])])),('flags',['bitflags',{'type':'u8','flags':['notification','highlight']}])])
    return {'recipe_book_add':container([('entries',array(entry)),('replace','bool')]), 'craft_recipe_response':container([('windowId','ContainerID'),('recipeDisplay','RecipeDisplay')])}
def aliases(p):
    out={'varint':'native','bool':'native','u8':'native','i8':'native','f32':'native','string':['pstring',{'countType':'varint'}]}
    if p >= 766: out['ContainerID']='u8' if p < 768 else 'varint'
    if p >= 768:
        out.update({'optvarint':'varint','IDSet':['registryEntryHolderSet',{'base':{'name':'name','type':'string'},'otherwise':{'name':'ids','type':'varint'}}]})
        # Inline patterns from 770 drop their item-ID field.
        out['ArmorTrimPattern']=container([('assetId','string')]+([('templateItemId','varint')] if p < 770 else [])+[('description','anonymousNbt'),('decal','bool')])
        out['anonymousNbt']='native'
        def patch_fields(added,removed):
            return [(added,'varint'),(removed,'varint'),('components',['array',{'count':added,'type':'SlotComponent'}]),('removeComponents',['array',{'count':removed,'type':container([('type','SlotComponentType')])}])]
        out['Slot']=['container',[{'name':'itemCount','type':'varint'},{'anon':True,'type':['switch',{'compareTo':'itemCount','fields':{'0':'void'},'default':container([('itemId','varint')]+patch_fields('addedComponentCount','removedComponentCount'))}]}]]
        if p>=775:
            added,removed=('templateAddedComponentCount','templateRemovedComponentCount') if p==776 else ('addedComponentCount','removedComponentCount')
            out['ItemStackTemplate']=container([('itemId','varint'),('itemCount','varint')]+patch_fields(added,removed))
    return out

def varint(n):
    n &= 0xffffffff; out=bytearray()
    while n > 127: out.append((n & 127)|128); n >>= 7
    out.append(n); return bytes(out)
def string(s):
    b=s.encode(); return varint(len(b))+b

def slot_cases(p):
    modern=p >= 775
    ids={'item':4 if modern else 2, 'stack':5 if modern else 3,'tag':6 if modern else 4,'trim':8 if modern else 5,'remainder':9 if modern else 6,'composite':10 if modern else 7}
    item=lambda n:varint(ids['item'])+varint(n)
    tag=varint(ids['tag'])+string('rustwire:ingredients')
    # Anonymous TAG_String component; independently constructed, not Rust NBT.
    stack=(varint(300)+varint(2) if modern else varint(2)+varint(300))+b'\x01\x00'+varint(6 if p >= 774 else 5)+b'\x08\x00\x02Hi'
    empty_stack=(varint(300)+b'\x00\x00\x00') if modern else b'\x00'
    pattern=item(9) if p < 770 else b'\x00'+string('rustwire:trim')+b'\x08\x00\x04Trim\x01'
    out=[('empty',b'\x00'),('fuel',b'\x01'),('item',item(300)),('stack-name',varint(ids['stack'])+stack),('stack-zero',varint(ids['stack'])+empty_stack),('tag',tag),('trim-inline',varint(ids['trim'])+item(1)+item(2)+pattern)]
    if p >= 770: out += [('trim-reference',varint(ids['trim'])+item(1)+item(2)+varint(301))]
    out += [('remainder',varint(ids['remainder'])+item(5)+b'\x00'),('composite',varint(ids['composite'])+b'\x03\x00\x01'+tag),('composite-empty',varint(ids['composite'])+b'\x00')]
    if modern: out += [('potion',b'\x02'+item(4)),('component',b'\x03'+item(4)+varint(2147483647)),('dyed',b'\x07'+item(8)+item(9))]
    return out

def recipe_cases(p):
    slots=dict(slot_cases(p)); a=slots['item']; result=slots['stack-name']; station=slots['tag']
    return [('shapeless',b'\x00\x03'+slots['remainder']+slots['trim-inline']+slots['composite']+result+station),
            ('shaped',b'\x01\x02\x01\x02'+a+slots['fuel']+result+station),
            ('furnace',b'\x02'+a+b'\x01'+result+station+varint(200)+struct.pack('>f',0.5)),
            ('stonecutter',b'\x03'+slots['composite-empty']+a+station),
            ('smithing',b'\x04'+a+a+a+slots['trim-inline']+station)]
def fixtures(p, packet_ids):
    rows=[]
    for case,body in slot_cases(p):rows.append(('SlotDisplay',-1,case,body))
    for index,(case,body) in enumerate(recipe_cases(p)):
        rows.append(('RecipeDisplay',-1,case,body))
        rows.append(('craft_recipe_response',packet_ids['craft_recipe_response'],case,varint(-1)+body))
        ingredients=b'\x01\x03\x00'+string('rustwire:tag')+b'\x04\x01\xac\x02\x01\x01'
        entry=varint(300)+body+varint(0 if index==0 else index)+varint(index)+ingredients+bytes((0xf0|index,))
        rows.append(('recipe_book_add',packet_ids['recipe_book_add'],case,b'\x01'+entry+b'\x01'))
    rows.append(('recipe_book_add',packet_ids['recipe_book_add'],'empty',b'\x00\x00'))
    minimal=b'\x03\x00\x00\x00'
    entries=b''.join(varint(i)+minimal+varint(group)+varint(cat)+b'\x00'+bytes((i,)) for i,(group,cat) in enumerate([(0,12),(0x80000000,-1),(-1,300)]))
    rows.append(('recipe_book_add',packet_ids['recipe_book_add'],'signed-group-unknown-category',b'\x03'+entries+b'\x00'))
    return rows

def generate(schema_dir):
    records=json.loads((ROOT/'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records] != list(range(763,777)):raise ValueError('Expected fourteen protocol families')
    lines=['# Original independent synthetic fixtures; not release-API output or captured traffic.', '# protocol\tname\tpacket_id\tcase\thex']; checked=0
    for row in records:
        p=row['protocol']; raw=(schema_dir/f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise ValueError('Schema hash mismatch')
        schema=json.loads(raw); types=schema['play']['toClient']['types']; ids={}
        for key,value in aliases(p).items():
            if schema['types'].get(key)!=value:raise ValueError(f'Alias mismatch: {p} {key}')
        if p>=768:
            names=schema['types']['SlotComponentType'][1]['mappings']
            component=schema['types']['SlotComponent']
            if names.get(str(6 if p>=774 else 5))!='custom_name' or component[1][0]['type']!='SlotComponentType' or component[1][1]['type'][1]['fields'].get('custom_name')!='anonymousNbt':raise ValueError('Fixture component alias mismatch')
        for key,value in {'SlotDisplay':slot_layout(p) if p>=768 else None,'RecipeDisplay':recipe_layout() if p>=768 else None}.items():
            if types.get(key)!=value:raise ValueError(f'Display mismatch: {p} {key}')
        mapping=types['packet'][1][0]['type'][1]['mappings']; switch=types['packet'][1][1]['type']
        for name,value in packets(p).items():
            found=[int(i,16) for i,n in mapping.items() if n==name]
            if types.get('packet_'+name)!=value or len(found)!=int(value is not None):raise ValueError(f'Packet mismatch: {p} {name}')
            if value is None:continue
            if switch[0]!='switch' or switch[1]['compareTo']!='name' or switch[1]['fields'].get(name)!='packet_'+name:raise ValueError('Packet dispatch mismatch')
            ids[name]=found[0]; checked+=1
        if p>=768:
            for name,id,case,body in fixtures(p,ids):lines.append(f'{p}\t{name}\t{id}\t{case}\t{body.hex()}')
    return '\n'.join(lines)+'\n',checked

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--schema-dir',type=Path,default=ROOT/'research/protocols');parser.add_argument('--write',action='store_true');args=parser.parse_args()
    text,checked=generate(args.schema_dir);path=ROOT/'tests/fixtures/recipe-display.tsv'
    if args.write:path.write_text(text)
    elif not path.exists() or path.read_text()!=text:raise SystemExit('Display fixtures stale')
    print(f'Verified {checked} outer packet layouts, 18 display unions, {len(text.splitlines())-2} independent fixtures; explicit 776 template correction')
if __name__=='__main__':main()
