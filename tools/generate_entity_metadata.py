#!/usr/bin/env python3
"""Reproduce metadata serializer IDs/support tables from hash-pinned schemas.

Run tools/fetch_schemas.py first if research/protocols is absent. Cargo does not
need these schemas or this generator. --check verifies without changing files.
Only the marked registry region is generated; handwritten parsers are preserved.

The optional_global_pos correction (dimension identifier plus packed position)
was verified against the 1.20.1 server serializer. Particles, registry holders,
and resolvable profiles remain explicitly Unsupported rather than guessed.
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
}


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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    if not shutil.which('rustfmt'):
        raise SystemExit('rustfmt is required')
    target = ROOT / 'src/packet/entity_metadata.rs'
    original = target.read_text()
    prefix, rest = original.split(START, 1)
    _, suffix = rest.split(END, 1)
    result = prefix + generate() + suffix
    with tempfile.TemporaryDirectory(prefix='rustwire-metadata-') as temp:
        path = pathlib.Path(temp) / 'metadata.rs'
        path.write_text(result)
        subprocess.run(['rustfmt', '--edition', '2021', str(path)], check=True)
        result = path.read_text()
    if args.check:
        if result != original:
            raise SystemExit('Metadata registry tables are not reproducible; run generator')
        print('Verified all 14 hash-pinned metadata registries')
    else:
        target.write_text(result)
        print('Generated all 14 metadata registries')


if __name__ == '__main__':
    main()
