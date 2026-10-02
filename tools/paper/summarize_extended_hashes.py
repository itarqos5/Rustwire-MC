#!/usr/bin/env python3
"""Audit and merge same-source live hash runs without discarding run provenance.

Reads evidence only. Re-assesses every case from raw client/server transcripts,
checks clean shutdown, validates unchanged already-approved EULA copies, and
requires identical binary plus complete execution-source manifests across runs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

import validate_extended_hashes as h


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def audit_record(record, server_lines, client_lines):
    """Do not trust stored passed booleans or claimed state/correction summaries."""
    errors = []
    version = record['version']
    cases = h.selected_cases(version)
    expected_names = [case['case'] for case in cases]
    actual_results = [match[1] for line in client_lines if (match := h.RESULT_RE.fullmatch(line))]
    if actual_results != expected_names or sum(line.startswith('HASH_RESULT ') for line in client_lines) != len(cases):
        errors.append('wrong/missing/duplicate client result cases or order')
    if client_lines.count(f'HASH_DONE cases={len(cases)}') != 1:
        errors.append('missing/duplicate exact completion marker')
    markers = [line for line in server_lines if '[Server] RW_HASH_' in line]
    if record.get('server_state_markers') != markers:
        errors.append('recorded server state markers differ from raw server log')
    if record.get('client_output') != client_lines:
        errors.append('recorded client output differs from raw client log')
    commands = [entry['command'] for entry in record['commands']]
    for case in cases:
        proof = h.server_state_command(record['username'], case['case'])
        if commands.count(proof) != 1:
            errors.append(f'{case["case"]}: missing/duplicate exact server predicate command')
        marker = f'RW_HASH_SERVER:{case["case"]}:ok'
        if sum(bool(re.search(r'\[Server\]\s*' + re.escape(marker) + '$', line)) for line in server_lines) != 1:
            errors.append(f'{case["case"]}: missing/duplicate raw server state marker')
    correction_re = re.compile(r'^HASH_CORRECTION case=(\w+) slot=(-?\d+) target_match=(true|false)$')
    corrections = [match for line in client_lines if (match := correction_re.fullmatch(line))]
    if any(match[1] not in expected_names for match in corrections):
        errors.append('correction belongs to wrong case')
    for case in cases:
        observed = [match for match in corrections if match[1] == case['case']]
        if case['bad'] and not any(match[2] == '45' and match[3] == 'true' for match in observed):
            errors.append(f'{case["case"]}: no raw exact-target authoritative correction')
        if not case['bad'] and observed:
            errors.append(f'{case["case"]}: unexpected raw positive-control correction')
    assessments = [h.assess_case(case, client_lines, server_lines) for case in cases]
    errors += [f'{row["case"]}: hash/prediction/correction/server-state mismatch' for row in assessments if not row['passed']]
    negative = next(case for case in cases if case['bad'])
    mutation = f'HASH_MUTATION component={negative["component"]} original={negative["expected_hash"]} submitted={negative["expected_hash"] ^ 1} xor=1'
    if client_lines.count(mutation) != 1 or sum(line.startswith('HASH_MUTATION ') for line in client_lines) != 1:
        errors.append('missing/duplicate/incorrect one-bit negative mutation')
    if record.get('server_exit') != 0 or record.get('client_exit') != 0:
        errors.append('nonzero or missing process exits')
    if record.get('remaining_port_25565_listeners') != []:
        errors.append('server listener remains or cleanup observation missing')
    if record.get('ops_empty') is not True:
        errors.append('operator list not empty or evidence missing')
    survival = f'[Server] RW_HASH_SURVIVAL:{record["username"]}:ok'
    if record.get('survival_confirmed') is not True or not any(survival in line for line in server_lines):
        errors.append('survival mode not proved')
    if record.get('console_failures') != [] or any(h.base.COMMAND_FAILURE.search(line) for line in server_lines):
        errors.append('console command failure or missing failure check')
    if any(line.startswith('Error:') for line in client_lines):
        errors.append('client error in raw transcript')
    return {'passed': not errors, 'errors': errors, 'cases': assessments}


def read_lines(path):
    return [h.base.ANSI.sub('', line).strip() for line in path.read_text().splitlines()]


def audit_eula(baseline, world):
    before = baseline.read_bytes()
    copied = world.read_bytes()
    after = baseline.read_bytes()
    return {'baseline_sha256': hashlib.sha256(before).hexdigest(),
            'disposable_world_sha256': hashlib.sha256(copied).hexdigest(),
            'explicit_prior_acceptance': b'eula=true' in before.splitlines(),
            'baseline_stable_during_audit': before == after,
            'unchanged_from_approved_baseline': before == copied == after,
            'passed': b'eula=true' in before.splitlines() and before == copied == after}


def merge_runs(paths, validation_dir, required_versions):
    results = []
    source_manifest = binary_hash = None
    provenance = []
    errors = []
    for path in paths:
        source = json.loads(path.read_text())
        run_dir = path.parent
        if source_manifest is None:
            source_manifest, binary_hash = source['source_sha256'], source['client_binary_sha256']
        if source_manifest != source['source_sha256'] or binary_hash != source['client_binary_sha256']:
            raise ValueError('Refusing to merge different execution-source manifests or client binaries')
        if digest(run_dir / 'extended_hash_probe') != binary_hash:
            raise ValueError('Archived client binary hash does not match run provenance')
        run_provenance = {'run_id': run_dir.name, 'created_utc': source['created_utc'],
                          'source_base_commit': source['source_base_commit'], 'aggregate_result_sha256': digest(path),
                          'versions': [row['version'] for row in source['records']], 'records': []}
        for record in source['records']:
            version = record['version']
            world = run_dir / version
            stored = json.loads((world / 'hash-result.json').read_text())
            if stored != record:
                raise ValueError('Per-server result differs from aggregate run evidence')
            server_path, client_path = world / 'hash-server.log', world / 'hash-client.log'
            audited = audit_record(record, read_lines(server_path), read_lines(client_path))
            eula = audit_eula(validation_dir / 'servers' / version / 'eula.txt', world / 'eula.txt')
            if not eula['passed']:
                audited['errors'].append('EULA differs from already-approved baseline or lacks prior approval')
                audited['passed'] = False
            if not audited['passed']:
                errors.append({'version': version, 'errors': audited['errors']})
            entry = {**record, 'run_id': run_dir.name, 'independent_evidence_audit': audited,
                     'eula_audit': eula, 'passed': audited['passed']}
            results.append(entry)
            run_provenance['records'].append({'version': version, 'record_sha256': digest(world / 'hash-result.json'),
                                             'server_log_sha256': digest(server_path), 'client_log_sha256': digest(client_path),
                                             'eula_audit': eula})
        provenance.append(run_provenance)
    if sorted(row['version'] for row in results) != sorted(required_versions):
        errors.append({'error': 'missing/duplicate/unexpected version coverage', 'required': required_versions})
    for name, expected in (source_manifest or {}).items():
        if digest(h.PROJECT / name) != expected:
            errors.append({'error': 'Current execution source differs from archived run', 'file': name})
    records_by_version = {row['version']: row for row in results}
    results = [records_by_version[version] for version in required_versions if version in records_by_version]
    audit_paths = ['tools/paper/summarize_extended_hashes.py', 'tools/paper/test_validate_extended_hashes_acceptance.py']
    return {'format': 1, 'audited_utc': h.base.utc_now(), 'passed': not errors, 'errors': errors,
            'purpose': 'Supported persistent component hashes in ordinary survival inventory clicks',
            'scope': 'Loopback offline Paper; no OP, creative inventory, external accounts, or registry-ID guessing',
            'execution_source_sha256': source_manifest, 'client_binary_sha256': binary_hash,
            'merge_audit_source_sha256': {name: digest(h.PROJECT / name) for name in audit_paths},
            'merge_provenance': provenance, 'records': results,
            'summary': {'families': len(results), 'click_cases': sum(len(row['cases']) for row in results),
                        'positive_predictions': sum(not case['negative_control'] for row in results for case in row['cases']),
                        'negative_controls': sum(case['negative_control'] for row in results for case in row['cases']),
                        'all_eulas_unchanged': all(row['eula_audit']['passed'] for row in results)}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('runs', nargs='+', type=Path)
    parser.add_argument('--validation-dir', type=Path, default=h.PROJECT.parent / 'rustwire-server-validation')
    parser.add_argument('--versions', nargs='+', choices=h.VERSIONS, default=h.VERSIONS)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    report = merge_runs(args.runs, args.validation_dir.resolve(), args.versions)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'passed': report['passed'], 'summary': report['summary'], 'errors': report['errors']}))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
