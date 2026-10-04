#!/usr/bin/env python3
"""Guard pinned debug schemas and independently encode corrected official layouts.

The raw descriptors intentionally retain their known Goal/Path/ID-0 defects.
Fixtures use audited Mojang wire order instead; see debug-values-wire-audit.md.
"""
import copy
import hashlib
import json
import struct
from pathlib import Path
from check_debug_sample_fixtures import REGISTRY_NAMES

ROOT = Path(__file__).resolve().parents[1]
NAMES = ('debug_block_value', 'debug_chunk_value', 'debug_entity_value', 'debug_event')
PATH_NAMES = ('blocked open walkable walkable_door trapdoor powder_snow danger_powder_snow '
              'fence lava water water_border rail unpassable_rail danger_fire damage_fire '
              'danger_other damage_other door_open door_wood_closed door_iron_closed breach '
              'leaves sticky_honey cocoa damage_cautious danger_trapdoor').split()


def container(*fields): return ['container', [{'name': n, 'type': t} for n, t in fields]]
def array(t): return ['array', {'countType': 'varint', 'type': t}]
def option(t): return ['option', t]
def mapper(names): return ['mapper', {'type': 'varint', 'mappings': {str(i): n for i, n in enumerate(names)}}]


def raw_types():
    goal = container(('priority', 'varint'), ('running', 'bool'), ('name', 'string'))
    payloads = {
        'DedicatedServerTickTime': 'void', 'VillageSections': 'void',
        'Bees': container(('hivePos', option('position')), ('flowerPos', option('position')),
                          ('travelTicks', 'varint'), ('blacklistedHives', array('position'))),
        'Brains': container(('name', 'string'), ('profession', 'string'), ('xp', 'i32'),
                            ('health', 'f32'), ('maxHealth', 'f32'), ('inventory', 'string'),
                            ('wantsGolem', 'bool'), ('angerLevel', 'i32'),
                            *[(n, array('string')) for n in ('activities', 'behaviors', 'memories', 'gossips')],
                            ('pois', array('position')), ('potentialPois', array('position'))),
        'Breezes': container(('attackTarget', option('varint')), ('jumpTarget', option('position'))),
        'GoalSelectors': container(('goals', array(goal))),
        'EntityPaths': container(('path', 'Path'), ('maxNodeDistance', 'f32')),
        'EntityBlockIntersections': container(('id', 'varint')),
        'BeeHives': container(('type', 'varint'), ('occupantCount', 'varint'), ('honeyLevel', 'varint'), ('sedated', 'bool')),
        'Pois': container(('pos', 'position'), ('poiType', 'varint'), ('freeTicketCount', 'varint')),
        'RedstoneWireOrientations': container(('index', 'varint')),
        'Raids': container(('positions', array('position'))),
        'Structures': container(('structures', array('DebugStructureInfo'))),
        'GameEventListeners': container(('listenerRadius', 'varint')),
        'NeighborUpdates': container(('pos', 'position')),
        'GameEvents': container(('event', 'varint'), ('pos', 'vec3f64')),
    }
    switch = lambda fields: ['switch', {'compareTo': 'type', 'fields': fields}]
    update = container(('type', 'DebugSubscriptionDataType'))
    update[1].append({'anon': True, 'type': ['switch', {'compareTo': 'type',
        'fields': {'DedicatedServerTickTime': 'void'},
        'default': container(('payload', option(switch(payloads))))}]})
    events = copy.deepcopy(payloads); events['GoalSelectors'] = goal
    return {
        'DebugSubscriptionDataType': mapper(REGISTRY_NAMES),
        'DebugSubscriptionUpdate': update,
        'DebugSubscriptionEvent': container(('type', 'DebugSubscriptionDataType'), ('value', switch(events))),
        'Path': container(('reached', 'bool'), ('nextNodeIndex', 'i32'), ('target', 'position'),
                          ('nodes', array('Node')), ('debugData', 'PathDebugData')),
        'PathDebugData': container(*[(n, array('Node')) for n in ('openSet', 'closedSet', 'targetNodes')]),
        'Node': container(('position', 'vec3i32'), ('walkedDistance', 'f32'), ('costMalus', 'f32'),
                          ('closed', 'bool'), ('type', mapper(PATH_NAMES)), ('f', 'f32')),
        'DebugStructureInfo': container(('boundingBoxMin', 'position'), ('boundingBoxMax', 'position'),
            ('pieces', array(container(('boundingBoxMin', 'position'), ('boundingBoxMax', 'position'), ('isStart', 'bool'))))),
        'position': ['bitfield', [{'name': n, 'size': size, 'signed': True} for n, size in [('x', 26), ('z', 26), ('y', 12)]]],
        'packedChunkPos': container(('z', 'i32'), ('x', 'i32')),
        'vec3i32': container(('x', 'i32'), ('y', 'i32'), ('z', 'i32')),
        'vec3f64': container(('x', 'f64'), ('y', 'f64'), ('z', 'f64')),
    }


def expected(p):
    if p < 773: return {}
    return {
        'debug_block_value': container(('blockPos', 'position'), ('update', 'DebugSubscriptionUpdate')),
        'debug_chunk_value': container(('chunkPos', 'packedChunkPos'), ('update', 'DebugSubscriptionUpdate')),
        'debug_entity_value': container(('entityId', 'varint'), ('update', 'DebugSubscriptionUpdate')),
        'debug_event': container(('event', 'DebugSubscriptionEvent')),
    }


def inspect(schema, p):
    wanted = expected(p); found = {}
    for state in ('handshaking', 'status', 'login', 'configuration', 'play'):
        for direction in ('toClient', 'toServer'):
            ts = schema.get(state, {}).get(direction, {}).get('types', {})
            if 'packet' not in ts: continue
            fields = ts['packet'][1]; ids = fields[0]['type'][1]['mappings']; dispatch = fields[1]['type'][1]
            for raw_id, name in ids.items():
                if name not in NAMES: continue
                if state != 'play' or direction != 'toClient' or name not in wanted or name in found:
                    raise ValueError('debug-value presence')
                if dispatch['compareTo'] != 'name' or dispatch['fields'].get(name) != 'packet_' + name:
                    raise ValueError('debug-value dispatch')
                if ts.get('packet_' + name) != wanted[name]: raise ValueError('debug-value envelope')
                found[name] = int(raw_id, 0)
    if set(found) != set(wanted): raise ValueError('missing debug-value packet')
    if p >= 773:
        for n, t in raw_types().items():
            if schema['types'].get(n) != t: raise ValueError('debug-value raw descriptor: ' + n)
        if found != dict(zip(NAMES, range(26, 30))): raise ValueError('debug-value packet ID')
    return found


def vi(n):
    if not -(1 << 31) <= n < (1 << 31): raise ValueError('signed VarInt domain')
    n &= 0xffffffff; out = bytearray()
    while n > 127: out.append((n & 127) | 128); n >>= 7
    out.append(n); return bytes(out)


def pos(p):
    x, y, z = p
    if not -(1 << 25) <= x < (1 << 25) or not -(1 << 25) <= z < (1 << 25) or not -2048 <= y < 2048:
        raise ValueError('packed position domain')
    return struct.pack('>Q', ((x & 0x3ffffff) << 38) | ((z & 0x3ffffff) << 12) | (y & 0xfff))


def string(s, cap=32767):
    if len(s.encode('utf-16-be')) // 2 > cap: raise ValueError('string UTF-16 cap')
    b = s.encode('utf-8'); return vi(len(b)) + b


def seq(values, encode): return vi(len(values)) + b''.join(encode(v) for v in values)
def opt(value, encode): return b'\0' if value is None else b'\1' + encode(value)
def boolean(v):
    if type(v) is not bool: raise ValueError('boolean domain')
    return bytes([v])
def bits(v, width):
    if len(v) != width // 4: raise ValueError('float bits width')
    return bytes.fromhex(v)


A = [-1, 64, 2]; B = [-2, 70, 3]


def node(i):
    return {'position': [100+i, -200-i, 300+i], 'walked': '3fc00000', 'cost': 'be800000',
            'closed': bool(i % 2), 'type': i % 26, 'f': '3e000000'}


def cases():
    rich = {
        1: {'hive': A, 'flower': None, 'travel': -7, 'blacklist': [A, B]},
        2: {'name': 'Villager', 'profession': 'Farmer test', 'xp': -1, 'health': '40600000',
            'max_health': '41a00000', 'inventory': 'slot 0: wheat', 'wants_golem': True, 'anger': -(1 << 31),
            'activities': ['a', ''], 'behaviors': ['look 👀'], 'memories': ['m1', 'm2'], 'gossips': ['g'],
            'pois': [A, A], 'potential_pois': [B]},
        3: {'attack': -3, 'jump': B},
        4: [{'priority': 7, 'running': True, 'name': 'a'}, {'priority': 0, 'running': False, 'name': 'bc'}],
        5: {'reached': False, 'next': -123, 'target': A, 'nodes': [node(1)],
            'targets': [node(2), node(3)], 'open': [node(i) for i in range(4, 7)],
            'closed': [node(i) for i in range(7, 11)], 'max_distance': '3f000000'},
        6: 2,
        7: {'block': 128, 'occupants': -2, 'honey': 7, 'sedated': True},
        8: {'position': A, 'type': 300, 'tickets': -1},
        9: 47, 10: None, 11: [A, B, A],
        12: [{'min': A, 'max': B, 'pieces': [{'min': B, 'max': A, 'start': True},
                                           {'min': A, 'max': B, 'start': False}]},
             {'min': [-(1 << 25), -2048, -(1 << 25)], 'max': [(1 << 25)-1, 2047, (1 << 25)-1], 'pieces': []}],
        13: -1, 14: A,
        15: {'event': 128, 'position': ['3ff4000000000000', 'c004000000000000', '400e000000000000']},
    }
    # Coordinate-equal Target objects still have distinct transmitted diagnostic fields.
    rich[5]['targets'][1]['position'] = rich[5]['targets'][0]['position'][:]
    empty_brain = {'name': '', 'profession': '', 'xp': 0, 'health': '00000000', 'max_health': '00000000',
                   'inventory': '', 'wants_golem': False, 'anger': 0,
                   **{n: [] for n in ('activities', 'behaviors', 'memories', 'gossips', 'pois', 'potential_pois')}}
    empty_path = {'reached': False, 'next': 0, 'target': [0, 0, 0], 'nodes': [], 'targets': [],
                  'open': [], 'closed': [], 'max_distance': '00000000'}
    empty = {1: {'hive': None, 'flower': None, 'travel': 0, 'blacklist': []}, 2: empty_brain,
             3: {'attack': None, 'jump': None}, 4: [], 5: empty_path, 11: [], 12: []}
    float_brain = copy.deepcopy(rich[2]); float_brain.update(health='7fc01234', max_health='80000000')
    float_path = copy.deepcopy(rich[5]); float_path['nodes'][0].update(walked='80000000', cost='7fc01234', f='7f800000')
    float_path['max_distance'] = 'ff800000'
    floating = {2: float_brain, 5: float_path, 15: {'event': 128,
                'position': ['8000000000000000', '7ff0000000000000', '7ff8000000001234']}}
    path26 = copy.deepcopy(rich[5]); path26['nodes'][0].update(type=26, position=[-(1 << 31), (1 << 31)-1, -1])
    result = [(kind, 'rich', value, 773) for kind, value in rich.items()]
    result += [(kind, 'empty', value, 773) for kind, value in empty.items()]
    result += [(kind, 'float_bits', value, 773) for kind, value in floating.items()]
    result.append((5, 'path_type_26', path26, 775))
    return result


def payload(kind, value, protocol):
    def registry(n):
        if n < 0: raise ValueError('negative registry ID')
        return vi(n)
    def enc_node(n):
        if not 0 <= n['type'] <= (26 if protocol >= 775 else 25): raise ValueError('path type version')
        return struct.pack('>iii', *n['position']) + bits(n['walked'], 32) + bits(n['cost'], 32) + boolean(n['closed']) + vi(n['type']) + bits(n['f'], 32)
    if kind == 1: return opt(value['hive'], pos) + opt(value['flower'], pos) + vi(value['travel']) + seq(value['blacklist'], pos)
    if kind == 2:
        return (string(value['name']) + string(value['profession']) + struct.pack('>i', value['xp']) + bits(value['health'], 32)
                + bits(value['max_health'], 32) + string(value['inventory']) + boolean(value['wants_golem']) + struct.pack('>i', value['anger'])
                + b''.join(seq(value[n], string) for n in ('activities', 'behaviors', 'memories', 'gossips'))
                + seq(value['pois'], pos) + seq(value['potential_pois'], pos))
    if kind == 3: return opt(value['attack'], vi) + opt(value['jump'], pos)
    if kind == 4: return seq(value, lambda g: vi(g['priority']) + boolean(g['running']) + string(g['name'], 255))
    if kind == 5:
        return (boolean(value['reached']) + struct.pack('>i', value['next']) + pos(value['target'])
                + b''.join(seq(value[n], enc_node) for n in ('nodes', 'targets', 'open', 'closed')) + bits(value['max_distance'], 32))
    if kind == 6:
        if not 0 <= value <= 2: raise ValueError('intersection index')
        return vi(value)
    if kind == 7: return registry(value['block']) + vi(value['occupants']) + vi(value['honey']) + boolean(value['sedated'])
    if kind == 8: return pos(value['position']) + registry(value['type']) + vi(value['tickets'])
    if kind == 9:
        if not 0 <= value <= 47: raise ValueError('orientation index')
        return vi(value)
    if kind == 10: return b''
    if kind == 11: return seq(value, pos)
    if kind == 12:
        piece = lambda x: pos(x['min']) + pos(x['max']) + boolean(x['start'])
        return seq(value, lambda x: pos(x['min']) + pos(x['max']) + seq(x['pieces'], piece))
    if kind == 13: return vi(value)
    if kind == 14: return pos(value)
    if kind == 15: return registry(value['event']) + b''.join(bits(x, 64) for x in value['position'])
    raise ValueError('not a serializable debug value kind')


def fixtures(rows):
    result = '# protocol\tname\tcase\tpacket_id\tbody_hex\n'
    prefixes = {'debug_block_value': pos([-2, 64, 3]), 'debug_chunk_value': struct.pack('>ii', 3, -2),
                'debug_entity_value': vi(-1)}
    for protocol, ids in rows:
        for name, packet_id in sorted(ids.items()):
            for kind, label, value, minimum in cases():
                if protocol < minimum: continue
                body = vi(kind) + payload(kind, value, protocol)
                if name == 'debug_event': entries = [(f'event_{kind:02}_{label}', body)]
                else:
                    entries = [(f'present_{kind:02}_{label}', prefixes[name] + vi(kind) + b'\1' + payload(kind, value, protocol))]
                    if label == 'rich': entries.append((f'removed_{kind:02}', prefixes[name] + vi(kind) + b'\0'))
                for case, body in entries: result += f'{protocol}\t{name}\t{case}\t{packet_id}\t{body.hex()}\n'
    return result


def main():
    records = json.loads((ROOT / 'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records] != list(range(763, 777)): raise ValueError('family inventory')
    rows = []
    for record in records:
        raw = (ROOT / f"research/protocols/{record['schema']}.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != record['sha256']: raise ValueError('schema hash')
        rows.append((record['protocol'], inspect(json.loads(raw), record['protocol'])))
    if fixtures(rows) != (ROOT / 'tests/fixtures/debug-values.tsv').read_text(): raise ValueError('fixture drift')
    print('Verified 16 debug-value packet layouts, 15 serializable value kinds and 588 original fixtures across 4 families')


if __name__ == '__main__': main()
