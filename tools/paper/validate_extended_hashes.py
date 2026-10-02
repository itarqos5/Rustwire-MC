#!/usr/bin/env python3
"""Live ordinary survival-inventory hashes against seven pinned, isolated Paper families.

Requires a previously approved offline/loopback baseline with eula=true. Never
changes OPs, uses creative inventory, authenticates externally, or accepts terms.
Each test uses a normal swap click, a server-NBT state predicate, and a client
correction window. One deliberately wrong synchronization hash is the negative
control, followed by a valid recovery prediction. These are not security tests.
"""
import argparse
import datetime
import fcntl
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time

import validate_gameplay as base

PROJECT = base.PROJECT
ORACLE = PROJECT / 'docs/validation/component-hash-extended-oracle.json'
VERSIONS = [v for v in base.BUILDS if base.PROTOCOLS[v] >= 770]
SELECTED = [
    'name_plain', 'name_styled', 'name_hex', 'name_shadow', 'lore_plain',
    'food_full', 'cooldown_group', 'weapon_full', 'explosion_full', 'fireworks_full',
    'lodestone_target', 'writable_pages', 'written_full', 'consumable_inline',
    'death_clear', 'death_teleport8', 'death_sound', 'sound_fixed',
    'use_effects_full', 'attack_range_full', 'swing_full',
]
RESULT_RE = re.compile(r'^HASH_RESULT case=(\w+) bad=(true|false) corrections=(\d+) target_reconciled=(true|false) passed=(true|false)$')
VALUE_RE = re.compile(r'^HASH_VALUE case=(\w+) component=(\w+) value=(-?\d+)$')


def selected_cases(version):
    """Use public-CODEC fixtures as commands and independent expected hashes."""
    oracle = json.loads(ORACLE.read_text())
    found = {case['case']: case for case in oracle['cases']}
    cases = []
    for name in SELECTED:
        case = found[name]
        result = next(row for row in case['results'] if version in row['releases'])
        if 'error' in result:
            # Modern-only components are absent, never substituted on older families.
            if case['component'] not in {'use_effects', 'attack_range', 'swing_animation'}:
                raise ValueError(f'Unexpected oracle failure for {version} {name}: {result}')
            continue
        cases.append({'case': name, 'component': case['component'], 'input': case['input'],
                      'expected_hash': result['hash'], 'bad': False})
    styled = next(case for case in cases if case['case'] == 'name_styled')
    cases.extend([{**styled, 'case': 'negative_styled', 'bad': True},
                  {**styled, 'case': 'recovery_styled', 'bad': False}])
    return cases


def component_snbt(value):
    # Quoted JSON keys/string values are valid SNBT for this bounded fixture set.
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'))


def case_commands(username, case):
    name = case['case']
    value = component_snbt(case['input'])
    return [
        f'item replace entity {username} weapon.offhand with minecraft:air',
        f'item replace entity {username} hotbar.0 with minecraft:stone[minecraft:custom_data={{rustwire_hash_case:"{name}"}},minecraft:{case["component"]}={value}] 1',
    ]


def server_state_command(username, name):
    target = '{id:"minecraft:stone",count:1,components:{"minecraft:custom_data":{rustwire_hash_case:"' + name + '"}}}'
    return f'execute if data entity {username} equipment.offhand{target} unless data entity {username} Inventory[{{Slot:0b}}] run say RW_HASH_SERVER:{name}:ok'


def assess_case(case, lines, server_lines):
    matches = [RESULT_RE.fullmatch(line) for line in lines]
    rows = [m for m in matches if m and m[1] == case['case']]
    hashes = [m for line in lines if (m := VALUE_RE.fullmatch(line)) and m[1] == case['case'] and m[2] == case['component']]
    server_marker = f'RW_HASH_SERVER:{case["case"]}:ok'
    server_ok = any(re.search(r'\[Server\]\s*' + re.escape(server_marker) + r'$', line) for line in server_lines)
    row = rows[0] if len(rows) == 1 else None
    hash_ok = len(hashes) == 1 and int(hashes[0][3]) == case['expected_hash']
    result_ok = row is not None and row[2] == str(case['bad']).lower() and row[5] == 'true'
    if row:
        result_ok &= (int(row[3]) > 0 and row[4] == 'true') if case['bad'] else int(row[3]) == 0
    return {'case': case['case'], 'component': case['component'], 'negative_control': case['bad'],
            'expected_official_hash': case['expected_hash'], 'observed_hash': int(hashes[0][3]) if len(hashes) == 1 else None,
            'hash_matches_official_oracle': hash_ok, 'server_inventory_state_confirmed': server_ok,
            'client_result': None if row is None else row[0],
            'passed': bool(hash_ok and result_ok and server_ok)}


def await_condition(predicate, processes, timeout, description):
    end = time.monotonic() + timeout
    while not predicate():
        if any(p.poll() is not None for p in processes):
            raise RuntimeError(f'Process exited before {description}')
        if time.monotonic() >= end:
            raise RuntimeError(f'Timed out waiting for {description}')
        time.sleep(0.025)


def run_version(args, run_dir, client_bin, version):
    world = base.prepare_world(args.validation_dir, run_dir, version)
    props = (world / 'server.properties').read_text().replace('gamemode=creative', 'gamemode=survival')
    (world / 'server.properties').write_text(props)
    protocol = base.PROTOCOLS[version]
    # Different usernames across releases and invocations; max 16 ASCII chars.
    username = f'RWH{protocol}{run_dir.name[-8:]}'
    jar = args.validation_dir / 'downloads' / f'paper-{version}-{base.BUILDS[version]}.jar'
    provenance = json.loads(base.MANIFEST.read_text())
    expected = next(row['download']['checksums']['sha256'] for row in provenance['paper'] if row['version'] == version)
    jar_hash = hashlib.sha256(jar.read_bytes()).hexdigest()
    if jar_hash != expected:
        raise RuntimeError('Paper artifact differs from official pinned digest')
    java = str(args.validation_dir / 'jdk25/bin/java') if version.startswith('26.') else 'java'
    server_cmd = [java, '-Xms256M', '-Xmx1024M', '-XX:ActiveProcessorCount=2', '-jar', str(jar), '--nogui']
    client_cmd = [str(client_bin), '127.0.0.1', '25565', version, username]
    record = {'version': version, 'protocol': protocol, 'username': username, 'paper_build': base.BUILDS[version],
              'server_jar_sha256': jar_hash, 'server_command': server_cmd, 'client_command': client_cmd,
              'commands': [], 'cases': [], 'passed': False, 'started_utc': base.utc_now()}
    server = client = None
    server_lines, client_lines, readers = [], [], []
    started = time.monotonic()
    cases = selected_cases(version)

    def reader(process, destination, path):
        with path.open('w') as log:
            for raw in process.stdout:
                log.write(raw)
                log.flush()
                line = base.ANSI.sub('', raw).strip()
                destination.append(line)
                if line.startswith(('HASH_RESULT', 'HASH_MUTATION', 'Error:')):
                    print(version, line, flush=True)

    def send(command):
        if server.poll() is not None:
            raise RuntimeError('Server exited before console command')
        record['commands'].append({'command': command, 'seconds': round(time.monotonic() - started, 3)})
        server.stdin.write(command + '\n')
        server.stdin.flush()

    def chat(marker):
        send(f'tellraw {username} ' + component_snbt({'text': marker}))

    print(f'=== EXTENDED HASH START {version} ===', flush=True)
    try:
        server = subprocess.Popen(server_cmd, cwd=world, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, text=True, bufsize=1)
        readers.append(threading.Thread(target=reader, args=(server, server_lines, world / 'hash-server.log'), daemon=True))
        readers[-1].start()
        await_condition(lambda: any('Done (' in line for line in server_lines), [server], 150, 'Paper startup')
        record['listeners'] = base.listeners()
        client = subprocess.Popen(client_cmd, cwd=world, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        readers.append(threading.Thread(target=reader, args=(client, client_lines, world / 'hash-client.log'), daemon=True))
        readers[-1].start()
        await_condition(lambda: f'HASH_READY username={username}' in client_lines, [server, client], 40, 'typed client readiness')
        time.sleep(0.75)
        send(f'execute if entity @a[name={username},gamemode=survival] run say RW_HASH_SURVIVAL:{username}:ok')
        await_condition(lambda: any(f'[Server] RW_HASH_SURVIVAL:{username}:ok' in s for s in server_lines), [server, client], 5, 'survival mode confirmation')
        record['survival_confirmed'] = True
        for case in cases:
            name = case['case']
            for command in case_commands(username, case):
                send(command)
                time.sleep(0.2)
            time.sleep(0.35)
            chat(f'RW_HASH:START:{name}:{"bad" if case["bad"] else "good"}')
            await_condition(lambda: any(line.startswith(f'HASH_SENT case={name} ') for line in client_lines), [server, client], 8, f'{name} click')
            # Full server ticks separate the click from the protocol marker barrier.
            time.sleep(0.75)
            send(f'data get entity {username} equipment')
            send(f'data get entity {username} Inventory')
            send(server_state_command(username, name))
            await_condition(lambda: any(re.search(r'\[Server\]\s*RW_HASH_SERVER:' + re.escape(name) + ':ok$', line) for line in server_lines), [server, client], 8, f'{name} actual server inventory state')
            time.sleep(0.15)
            chat(f'RW_HASH:END:{name}')
            await_condition(lambda: any(line.startswith(f'HASH_RESULT case={name} ') for line in client_lines), [server, client], 8, f'{name} hash result')
            assessment = assess_case(case, client_lines, server_lines)
            record['cases'].append(assessment)
            if not assessment['passed']:
                raise RuntimeError(f'Case failed: {assessment}')
        chat('RW_HASH:DONE')
        client.wait(timeout=10)
        record['passed'] = client.returncode == 0 and f'HASH_DONE cases={len(cases)}' in client_lines
    except Exception as exc:
        record['error'] = f'{type(exc).__name__}: {exc}'
        print(f'HASH FAIL {version}: {record["error"]}', flush=True)
    finally:
        base.stop_process(client)
        base.stop_process(server, 'stop')
        for thread in readers:
            thread.join(timeout=5)
        record.update({'client_exit': None if client is None else client.returncode,
                       'server_exit': None if server is None else server.returncode,
                       'client_output': client_lines, 'seconds': round(time.monotonic() - started, 3)})
        ops = world / 'ops.json'
        record['ops_empty'] = ops.exists() and json.loads(ops.read_text()) == []
        record['remaining_port_25565_listeners'] = [line.split()[1] for path in ['/proc/net/tcp', '/proc/net/tcp6']
                                                   for line in Path(path).read_text().splitlines()[1:]
                                                   if line.split()[1].endswith(':63DD') and line.split()[3] == '0A']
        record['console_failures'] = [line for line in server_lines if base.COMMAND_FAILURE.search(line)]
        record['server_inventory_snapshots'] = [line for line in server_lines if 'following entity data:' in line]
        record['server_state_markers'] = [line for line in server_lines if '[Server] RW_HASH_' in line]
        record['cases'] = [assess_case(case, client_lines, server_lines) for case in cases]
        record['passed'] &= (record['server_exit'] == 0 and record['ops_empty'] and not record['console_failures']
                             and not record['remaining_port_25565_listeners']
                             and all(case['passed'] for case in record['cases']))
        (world / 'hash-result.json').write_text(json.dumps(record, indent=2) + '\n')
    print(f'=== EXTENDED HASH STOP {version}: passed={record["passed"]}, server exit={record["server_exit"]} ===', flush=True)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--versions', nargs='+', choices=VERSIONS, default=VERSIONS)
    parser.add_argument('--validation-dir', type=Path, default=PROJECT.parent / 'rustwire-server-validation')
    parser.add_argument('--client', type=Path, required=True)
    args = parser.parse_args()
    args.validation_dir = args.validation_dir.resolve()
    args.client = args.client.resolve()
    if args.validation_dir == PROJECT or PROJECT in args.validation_dir.parents or not args.client.is_file():
        parser.error('Existing client and disposable validation directory outside the project are required')
    with (args.validation_dir / 'one-server.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%f')
        run_dir = args.validation_dir / 'extended-hashes' / 'runs' / stamp
        run_dir.mkdir(parents=True)
        binary = run_dir / 'extended_hash_probe'
        shutil.copyfile(args.client, binary)
        binary.chmod(0o755)
        source_paths = ['examples/extended_hash_probe.rs', 'tools/paper/validate_extended_hashes.py',
                        'tools/paper/validate_gameplay.py', 'tools/paper/test_validate_extended_hashes.py',
                        'docs/validation/component-hash-extended-oracle.json',
                        'tools/paper/ComponentHashExtendedOracle.java', 'Cargo.toml', 'Cargo.lock']
        source_paths += [str(path.relative_to(PROJECT)) for path in sorted((PROJECT / 'src').rglob('*.rs'))]
        results = {'source_base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=PROJECT, text=True).strip(),
                   'source_sha256': {p: hashlib.sha256((PROJECT / p).read_bytes()).hexdigest() for p in source_paths},
                   'client_binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                   'created_utc': base.utc_now(), 'purpose': 'normal survival inventory synchronization checksum validation',
                   'scope': 'Typed supported persistent component subset; no registry-ID guessing or authentication/security bypass',
                   'records': []}
        for version in args.versions:
            results['records'].append(run_version(args, run_dir, binary, version))
            (run_dir / 'hash-results.json').write_text(json.dumps(results, indent=2) + '\n')
        (args.validation_dir / 'extended-hashes' / 'latest-run.txt').write_text(str(run_dir) + '\n')
        print(f'Hash evidence: {run_dir / "hash-results.json"}', flush=True)
        return 0 if all(record['passed'] for record in results['records']) else 1


if __name__ == '__main__':
    raise SystemExit(main())
