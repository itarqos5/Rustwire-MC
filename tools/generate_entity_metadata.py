#!/usr/bin/env python3
"""Reproduce metadata serializer IDs/support tables from hash-pinned schemas.

Run tools/fetch_schemas.py first if research/protocols is absent. Cargo does not
need these schemas or this generator. --check verifies without changing files.
Only the marked registry region is generated; handwritten parsers are preserved.

The optional_global_pos correction (dimension identifier plus packed position)
was verified against the 1.20.1 server serializer. Particle and holder schema
corrections below were checked against cached release server stream codecs.
"""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
START = '// BEGIN GENERATED METADATA REGISTRIES\n'
END = '// END GENERATED METADATA REGISTRIES\n'
BASE = {
    'byte': 'Byte', 'int': 'VarInt', 'long': 'VarLong', 'float': 'Float',
    'string': 'String', 'item_stack': 'Slot', 'boolean': 'Bool',
    'rotations': 'Vec3', 'block_pos': 'BlockPosition',
    'optional_block_pos': 'OptionalBlockPosition', 'direction': 'Direction',
    'optional_uuid': 'OptionalUuid', 'block_state': 'BlockState',
    'optional_block_state': 'OptionalBlockState', 'compound_tag': 'Compound',
    'villager_data': 'VillagerData', 'optional_unsigned_int': 'OptionalUnsignedInt',
    'optional_global_pos': 'OptionalGlobalPosition', 'vector3': 'Vec3',
    'quaternion': 'Quaternion', 'humanoid_arm': 'HumanoidArm',
    'particle': 'Particle', 'particles': 'Particles',
    'resolvable_profile': 'ResolvableProfile',
}


# Registry IDs checked against 1.20.1/1.20.2/1.20.4 server ParticleTypes
# registration order. The protocol-765 schema accidentally embeds a 766 mapper.
LEGACY_PARTICLES = {
    763: (
        "ambient_entity_effect angry_villager block block_marker bubble cloud "
        "crit damage_indicator dragon_breath dripping_lava falling_lava landing_lava "
        "dripping_water falling_water dust dust_color_transition effect elder_guardian "
        "enchanted_hit enchant end_rod entity_effect explosion_emitter explosion "
        "sonic_boom falling_dust firework fishing flame cherry_leaves "
        "sculk_soul sculk_charge sculk_charge_pop soul_fire_flame soul flash "
        "happy_villager composter heart instant_effect item vibration "
        "item_slime item_snowball large_smoke lava mycelium note "
        "poof portal rain smoke sneeze spit "
        "squid_ink sweep_attack totem_of_undying underwater splash witch "
        "bubble_pop current_down bubble_column_up nautilus dolphin campfire_cosy_smoke "
        "campfire_signal_smoke dripping_honey falling_honey landing_honey falling_nectar falling_spore_blossom "
        "ash crimson_spore warped_spore spore_blossom_air dripping_obsidian_tear falling_obsidian_tear "
        "landing_obsidian_tear reverse_portal white_ash small_flame snowflake dripping_dripstone_lava "
        "falling_dripstone_lava dripping_dripstone_water falling_dripstone_water glow_squid_ink glow wax_on "
        "wax_off electric_spark scrape shriek egg_crack "
    ).split(),
    765: (
        "ambient_entity_effect angry_villager block block_marker bubble cloud "
        "crit damage_indicator dragon_breath dripping_lava falling_lava landing_lava "
        "dripping_water falling_water dust dust_color_transition effect elder_guardian "
        "enchanted_hit enchant end_rod entity_effect explosion_emitter explosion "
        "gust gust_emitter sonic_boom falling_dust firework fishing "
        "flame cherry_leaves sculk_soul sculk_charge sculk_charge_pop soul_fire_flame "
        "soul flash happy_villager composter heart instant_effect "
        "item vibration item_slime item_snowball large_smoke lava "
        "mycelium note poof portal rain smoke "
        "white_smoke sneeze spit squid_ink sweep_attack totem_of_undying "
        "underwater splash witch bubble_pop current_down bubble_column_up "
        "nautilus dolphin campfire_cosy_smoke campfire_signal_smoke dripping_honey falling_honey "
        "landing_honey falling_nectar falling_spore_blossom ash crimson_spore warped_spore "
        "spore_blossom_air dripping_obsidian_tear falling_obsidian_tear landing_obsidian_tear reverse_portal white_ash "
        "small_flame snowflake dripping_dripstone_lava falling_dripstone_lava dripping_dripstone_water falling_dripstone_water "
        "glow_squid_ink glow wax_on wax_off electric_spark scrape "
        "shriek egg_crack dust_plume gust_dust trial_spawner_detection "
    ).split(),
}
LEGACY_PARTICLES[764] = LEGACY_PARTICLES[763]

def generate():
    groups = {}
    versions = {}
    manifest = json.loads((ROOT / 'research/schema-hashes.json').read_text())
    assert [entry['protocol'] for entry in manifest] == list(range(763, 777))
    for entry in manifest:
        protocol = entry['protocol']
        path = ROOT / 'research/protocols' / (entry['schema'] + '.json')
        raw = path.read_bytes()
        if hashlib.sha256(raw).hexdigest() != entry['sha256']:
            raise SystemExit(f'Schema hash mismatch: {path}')
        types = json.loads(raw)['types']
        metadata = types.get('entityMetadataEntry', types['entityMetadata'][1]['type'])
        if isinstance(metadata, str):
            metadata = types[metadata]
        mapping = metadata[1][1]['type'][1]['mappings']
        assert list(map(int, mapping)) == list(range(len(mapping)))
        fields = (metadata[1][2]['type'][1]['fields'] if 'entityMetadataEntry' in types
                  else types['entityMetadataItem'][1]['fields'])
        entries = []
        for name in mapping.values():
            if name == 'component':
                wire = 'JsonComponent' if protocol < 765 else 'NbtComponent'
            elif name == 'optional_component':
                wire = 'OptionalJsonComponent' if protocol < 765 else 'OptionalNbtComponent'
            elif name == 'painting_variant':
                wire = 'PaintingVariant' if protocol >= 767 else 'NonnegativeVarInt'
            elif name == 'wolf_variant' and 767 <= protocol <= 769:
                wire = 'WolfVariant'
            elif name == 'wolf_variant' and protocol == 766:
                wire = 'NonnegativeVarInt'
            elif name in BASE:
                wire = BASE[name]
            elif fields[name] == 'varint':
                wire = 'NonnegativeVarInt'
            else:
                wire = 'Unsupported'
            entries.append((name, wire))
        key = tuple(entries)
        if key not in groups:
            groups[key] = len(groups)
        versions[protocol] = groups[key]
    out = [START, '''/// Index is the wire serializer ID; Unsupported entries must never be skipped.
/// Slots can additionally reject unsupported item-component payloads.
pub fn metadata_registry(version: Version) -> &'static [(&'static str, MetadataWire)] {
    match version.protocol() {
''']
    for group in range(len(groups)):
        ids = [p for p, g in versions.items() if g == group]
        protocols = (f'{ids[0]}..={ids[-1]}' if len(ids) > 2 and ids == list(range(ids[0], ids[-1] + 1))
                     else ' | '.join(map(str, ids)))
        out.append(f'        {protocols} => REGISTRY_{group},\n')
    out.append('        _ => unreachable!(),\n    }\n}\n')
    for entries, group in groups.items():
        out.append(f'const REGISTRY_{group}: &[(&str, MetadataWire)] = &[\n')
        for name, wire in entries:
            out.append(f'    ("{name}", MetadataWire::{wire}),\n')
        out.append('];\n')
    out.extend(['\n', END])
    return ''.join(out)



def particle_wire(name, protocol):
    if name in ('block', 'block_marker', 'falling_dust', 'dust_pillar', 'block_crumble'):
        return 'BlockState'
    if name == 'dust':
        return 'DustRgb' if protocol < 768 else 'DustPacked'
    if name == 'dust_color_transition':
        return ('TransitionLegacy' if protocol <= 765 else
                'TransitionRgb' if protocol <= 767 else 'TransitionPacked')
    if name in ('entity_effect', 'tinted_leaves', 'flash'):
        since = {'entity_effect': 766, 'tinted_leaves': 770, 'flash': 773}[name]
        return 'Color' if protocol >= since else 'Unit'
    if name == 'item':
        return 'ItemTemplate' if protocol >= 775 else 'Item'
    if name == 'vibration':
        return 'VibrationLegacy' if protocol <= 765 else 'Vibration'
    if name == 'sculk_charge':
        return 'SculkCharge'
    if name == 'shriek':
        return 'Shriek'
    if name == 'trail':
        return 'Trail' if protocol >= 769 else 'TrailWithoutDuration'
    if protocol >= 773:
        if name == 'dragon_breath':
            return 'Power'
        if name in ('effect', 'instant_effect'):
            return 'Spell'
    if name in ('geyser', 'geyser_plume'):
        return 'Geyser'
    if name in ('geyser_base', 'geyser_poof'):
        return 'GeyserBase'
    return 'Unit'


def generate_particles():
    groups, versions = {}, {}
    manifest = json.loads((ROOT / 'research/schema-hashes.json').read_text())
    for entry in manifest:
        raw = (ROOT / 'research/protocols' / (entry['schema'] + '.json')).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == entry['sha256']
        protocol = entry['protocol']
        if protocol <= 765:
            names = LEGACY_PARTICLES[protocol]
        else:
            particle = json.loads(raw)['types']['Particle']
            mapping = particle[1][0]['type'][1]['mappings']
            assert list(map(int, mapping)) == list(range(len(mapping)))
            names = [name.replace('trial_spawner_detected_player', 'trial_spawner_detection')
                     for name in mapping.values()]
        entries = tuple((name, particle_wire(name, protocol)) for name in names)
        if entries not in groups:
            groups[entries] = len(groups)
        versions[protocol] = groups[entries]
    out = ['// BEGIN GENERATED PARTICLE REGISTRIES\n',
           '/// Index is the release-specific particle registry ID.\n',
           "pub fn particle_registry(version: Version) -> &'static [(&'static str, ParticleWire)] {\n",
           '    match version.protocol() {\n']
    for group in range(len(groups)):
        ids = [p for p, g in versions.items() if g == group]
        protocols = (f'{ids[0]}..={ids[-1]}' if len(ids) > 2 and ids == list(range(ids[0], ids[-1] + 1))
                     else ' | '.join(map(str, ids)))
        out.append(f'        {protocols} => PARTICLES_{group},\n')
    out.extend(['        _ => unreachable!(),\n    }\n}\n'])
    for entries, group in groups.items():
        out.append(f'const PARTICLES_{group}: &[(&str, ParticleWire)] = &[\n')
        for name, wire in entries:
            out.append(f'    ("{name}", ParticleWire::{wire}),\n')
        out.append('];\n')
    out.append('// END GENERATED PARTICLE REGISTRIES\n')
    return ''.join(out)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if not shutil.which('rustfmt'):
        raise SystemExit('rustfmt is required')
    targets = [
        (ROOT / 'src/packet/entity_metadata.rs', START, END, generate()),
        (ROOT / 'src/packet/entity_metadata/particles.rs',
         '// BEGIN GENERATED PARTICLE REGISTRIES\n',
         '// END GENERATED PARTICLE REGISTRIES\n', generate_particles()),
    ]
    for target, start, end, generated in targets:
        original = target.read_text()
        prefix, rest = original.split(start, 1)
        _, suffix = rest.split(end, 1)
        result = prefix + generated + suffix
        with tempfile.TemporaryDirectory(prefix='rustwire-metadata-') as temp:
            path = pathlib.Path(temp) / 'metadata.rs'
            path.write_text(result)
            subprocess.run(['rustfmt', '--edition', '2021', '--config', 'skip_children=true', str(path)], check=True)
            result = path.read_text()
        if args.check:
            if result != original:
                raise SystemExit(f'Registry tables are not reproducible: {target}; run generator')
        else:
            target.write_text(result)
    print(('Verified' if args.check else 'Generated') + ' all 14 metadata and particle registries')



if __name__ == '__main__':
    main()
