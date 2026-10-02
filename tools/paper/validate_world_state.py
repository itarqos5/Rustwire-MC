#!/usr/bin/env python3
"""Call cached release APIs only; no download, server start, or game code copying.

Requires a prepared validation root containing jdk25 and servers/<release>/{versions,libraries},
and hash-pinned protocol JSONs. Fails on missing cases, changed semantics or fixture drift.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

RELEASES = ['1.20.1', '1.20.2', '1.20.4', '1.20.6', '1.21.1', '1.21.3', '1.21.4',
            '1.21.5', '1.21.6', '1.21.8', '1.21.10', '1.21.11', '26.1.2', '26.2']
GOLDENS = {
    'game_reason_7': 'game_state_change', 'difficulty_2': 'difficulty', 'time_golden': 'update_time',
    'spawn_-1024000000': 'spawn_position', 'border_initialize': 'initialize_world_border',
    'border_center': 'world_border_center', 'border_lerp': 'world_border_lerp_size',
    'border_size': 'world_border_size', 'border_warning_delay': 'world_border_warning_delay',
    'border_warning_distance': 'world_border_warning_reach', 'block_ack': 'acknowledge_player_digging',
    'player_loaded': 'player_loaded',
}


def parse(stdout):
    rows = {}
    for line in stdout.splitlines():
        parts = line.split('\t')
        if len(parts) not in (3, 4):
            continue
        label, wire, result = parts[:3]
        if label in rows:
            raise AssertionError(f'duplicate result {label}')
        row = {'input_hex': wire}
        bytes.fromhex(wire)
        if result.startswith('ERROR '):
            assert len(parts) == 3
            row['error'] = result[6:]
        else:
            bytes.fromhex(result)
            assert len(parts) == 4 and parts[3] == 'remaining=0', line
            row['output_hex'] = result
        rows[label] = row
    return rows


def read_var(data, cursor, bits):
    value = 0
    for index in range((bits + 6) // 7):
        byte = data[cursor]
        cursor += 1
        value |= (byte & 127) << (index * 7)
        if byte < 128:
            return value, cursor
    raise AssertionError('oracle emitted overlong integer')


def clock_map(wire):
    data = bytes.fromhex(wire)
    count, pos = read_var(data, 8, 32)
    clocks = {}
    for _ in range(count):
        clock, pos = read_var(data, pos, 32)
        ticks, pos = read_var(data, pos, 64)
        clocks[clock] = (ticks, data[pos:pos+8].hex())
        pos += 8
    assert pos == len(data)
    return data[:8], clocks


def validate(protocol, rows):
    labels = {f'game_reason_{i}' for i in range(16)}
    labels |= {f'game_scalar_{i}' for i in [2143289344, 2139095040, -8388608, -2147483648]}
    difficulties = [0, 1, 2, 3, 4, 127, 255]
    if protocol >= 771:
        difficulties += [-1, -2147483648, 2147483647]
    labels |= {f'difficulty_{i}' for i in difficulties}
    labels |= {'time_golden', 'spawn_-1024000000', 'spawn_2143289344', 'spawn_2139095040',
               'border_initialize', 'border_center', 'border_lerp', 'border_size',
               'border_warning_delay', 'border_warning_distance', 'block_ack'}
    if protocol >= 769:
        labels.add('player_loaded')
    if protocol >= 773:
        labels |= {'spawn_id_' + s for s in ['', ':', '..:x', 'UPPER:x', 'minecraft:']}
    if protocol >= 775:
        labels |= {'time_float', 'time_multi', 'time_duplicate', 'time_negative_id'}
    assert set(rows) == labels, (protocol, set(rows) ^ labels)
    maximum = 11 if protocol == 763 else 12 if protocol == 764 else 13
    failures = {f'game_reason_{i}' for i in range(maximum+1, 16)}
    if protocol >= 773:
        failures.add('spawn_id_UPPER:x')
    if protocol >= 775:
        failures |= {'spawn_id_..:x', 'time_negative_id'}
    assert {k for k, v in rows.items() if 'error' in v} == failures
    for label, row in rows.items():
        if label in failures:
            continue
        if label.startswith('difficulty_'):
            assert row['output_hex'] == f'{int(label[11:]) % 4:02x}01', (protocol, label)
        elif label in {'time_multi', 'time_duplicate'}:
            assert clock_map(row['input_hex']) == clock_map(row['output_hex']), (protocol, label)
        elif label in {'spawn_id_', 'spawn_id_:'}:
            assert row['output_hex'] == '0a6d696e6563726166743a' + '00' * 16
        else:
            assert row['input_hex'] == row['output_hex'], (protocol, label)


def fixture_rows(protocol, rows, schema):
    result = []
    for label, name in GOLDENS.items():
        if label not in rows:
            assert label == 'player_loaded' and protocol < 769
            continue
        row = rows[label]
        assert row['input_hex'] == row['output_hex']
        direction = 'toServer' if name == 'player_loaded' else 'toClient'
        mapping = schema['play'][direction]['types']['packet'][1][0]['type'][1]['mappings']
        ids = [int(k, 0) for k, value in mapping.items() if value == name]
        assert len(ids) == 1, (protocol, name)
        result.append(f"{protocol}\t{name}\t{ids[0]}\t{row['output_hex'] or '-'}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime-root', required=True, type=Path)
    parser.add_argument('--schemas', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    records = json.loads((root/'research/schema-hashes.json').read_text())
    assert [r['protocol'] for r in records] == list(range(763, 777))
    args.output.mkdir(parents=True, exist_ok=True)
    source = Path(__file__).with_name('WorldStateOracle.java')
    subprocess.run([str(args.runtime_root/'jdk25/bin/javac'), '-d', str(args.output), str(source)], check=True)
    results, fixtures = [], []
    for record, release in zip(records, RELEASES):
        protocol = record['protocol']
        schema_bytes = (args.schemas/(record['schema']+'.json')).read_bytes()
        assert hashlib.sha256(schema_bytes).hexdigest() == record['sha256']
        server = args.runtime_root/'servers'/release
        jars = list((server/'versions').rglob('*.jar'))
        assert len(jars) == 1, (release, jars)
        jar = jars[0]
        cp = os.pathsep.join(map(str, [args.output, jar, *sorted((server/'libraries').rglob('*.jar'))]))
        run = subprocess.run([str(args.runtime_root/'jdk25/bin/java'), '-Xmx384m', '-cp', cp,
                              'WorldStateOracle', str(protocol)], cwd=args.output,
                             text=True, capture_output=True, timeout=90)
        (args.output/(release+'.stdout')).write_text(run.stdout)
        (args.output/(release+'.stderr')).write_text(run.stderr)
        assert run.returncode == 0, (release, run.stdout[-1000:], run.stderr[-1000:])
        rows = parse(run.stdout)
        validate(protocol, rows)
        fixtures.extend(fixture_rows(protocol, rows, json.loads(schema_bytes)))
        results.append({'protocol': protocol, 'release': release, 'schema_sha256': record['sha256'],
                        'jar_sha256': hashlib.sha256(jar.read_bytes()).hexdigest(), 'cases': rows})
        print(f'{protocol} {release}: {len(rows)} API cases verified', flush=True)
    existing = (root/'tests/fixtures/world-state.tsv').read_text().splitlines()
    expected = sorted(s for s in existing if not s.startswith('#'))
    assert sorted(fixtures) == expected, 'checked-in fixture drift'
    summary = {'oracle_source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
               'fixture_count': len(fixtures), 'case_count': sum(len(r['cases']) for r in results),
               'results': results}
    (args.output/'world-state-wire-oracle.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(f"PASS: {summary['case_count']} API cases; {len(fixtures)} checked-in goldens")


if __name__ == '__main__':
    main()
