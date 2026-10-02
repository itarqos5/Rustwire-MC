"""Mutation checks for the independent live-evidence acceptance gate."""
from pathlib import Path
import hashlib
import json
import tempfile
import unittest

import summarize_extended_hashes as audit
import validate_extended_hashes as h


def valid_record():
    username = 'RWH770fixture'
    cases = h.selected_cases('1.21.5')
    client, server, commands = [], [f'[12:00:00 INFO]: [Server] RW_HASH_SURVIVAL:{username}:ok'], []
    for case in cases:
        name = case['case']
        client.append(f'HASH_VALUE case={name} component={case["component"]} value={case["expected_hash"]}')
        if case['bad']:
            client.append(f'HASH_MUTATION component={case["component"]} original={case["expected_hash"]} submitted={case["expected_hash"] ^ 1} xor=1')
        if case['bad']:
            client.append(f'HASH_CORRECTION case={name} slot=45 target_match=true')
        client.append(f'HASH_RESULT case={name} bad={str(case["bad"]).lower()} corrections={int(case["bad"])} target_reconciled={str(case["bad"]).lower()} passed=true')
        server.append(f'[12:00:00 INFO]: [Server] RW_HASH_SERVER:{name}:ok')
        commands.append({'command': h.server_state_command(username, name)})
    client.append(f'HASH_DONE cases={len(cases)}')
    record = {'version': '1.21.5', 'username': username, 'commands': commands,
              'client_output': client.copy(), 'server_state_markers': server.copy(),
              'client_exit': 0, 'server_exit': 0, 'remaining_port_25565_listeners': [],
              'ops_empty': True, 'survival_confirmed': True, 'console_failures': []}
    return record, server, client


class AcceptanceMutations(unittest.TestCase):
    def test_complete_positive_control(self):
        self.assertTrue(audit.audit_record(*valid_record())['passed'])

    def test_wrong_case_and_missing_correction(self):
        for old, new in [('case=negative_styled bad=true', 'case=other_case bad=true'),
                         ('bad=true corrections=1', 'bad=true corrections=0'),
                         ('bad=true corrections=1 target_reconciled=true', 'bad=true corrections=1 target_reconciled=false')]:
            record, server, client = valid_record()
            client = [line.replace(old, new) for line in client]
            record['client_output'] = client.copy()
            self.assertFalse(audit.audit_record(record, server, client)['passed'])

    def test_claimed_correction_requires_raw_matching_slot_observation(self):
        for mutation in ['remove', 'wrong_case', 'wrong_slot', 'wrong_item']:
            record, server, client = valid_record()
            changed = []
            for line in client:
                if line.startswith('HASH_CORRECTION '):
                    if mutation == 'remove':
                        continue
                    if mutation == 'wrong_case':
                        line = line.replace('case=negative_styled', 'case=other_case')
                    if mutation == 'wrong_slot':
                        line = line.replace('slot=45', 'slot=36')
                    if mutation == 'wrong_item':
                        line = line.replace('target_match=true', 'target_match=false')
                changed.append(line)
            record['client_output'] = changed.copy()
            self.assertFalse(audit.audit_record(record, server, changed)['passed'])

    def test_forged_record_marker_is_not_raw_evidence(self):
        record, server, client = valid_record()
        del server[1]
        self.assertFalse(audit.audit_record(record, server, client)['passed'])

    def test_wrong_case_or_command_echo_state_marker(self):
        for prefix in ['console: say ', '[12:00:00 INFO]: [Server] WRONG_']:
            record, server, client = valid_record()
            server[1] = prefix + 'RW_HASH_SERVER:name_plain:ok'
            record['server_state_markers'] = [line for line in server if '[Server] RW_HASH_' in line]
            self.assertFalse(audit.audit_record(record, server, client)['passed'])

    def test_duplicate_result_and_marker_fail(self):
        record, server, client = valid_record()
        result = next(line for line in client if line.startswith('HASH_RESULT '))
        client.insert(2, result)
        record['client_output'] = client.copy()
        self.assertFalse(audit.audit_record(record, server, client)['passed'])
        record, server, client = valid_record()
        server.append(server[1])
        record['server_state_markers'] = server.copy()
        self.assertFalse(audit.audit_record(record, server, client)['passed'])

    def test_failed_or_missing_cleanup_and_permissions(self):
        for key, value in [('server_exit', 1), ('client_exit', 1), ('server_exit', None),
                           ('remaining_port_25565_listeners', ['0100007F:63DD']),
                           ('remaining_port_25565_listeners', None), ('ops_empty', False)]:
            record, server, client = valid_record()
            record[key] = value
            self.assertFalse(audit.audit_record(record, server, client)['passed'])

    def test_missing_server_predicate_cannot_be_replaced_by_say(self):
        record, server, client = valid_record()
        record['commands'][0]['command'] = 'say RW_HASH_SERVER:name_plain:ok'
        self.assertFalse(audit.audit_record(record, server, client)['passed'])

    def test_merge_rejects_changed_source_or_binary_manifest(self):
        for changed_field, changed_value in [('source_sha256', {'src/lib.rs': '0' * 64}),
                                              ('client_binary_sha256', '0' * 64)]:
            with tempfile.TemporaryDirectory() as directory:
                paths = []
                for index in range(2):
                    run = Path(directory) / str(index)
                    run.mkdir()
                    (run / 'extended_hash_probe').write_bytes(b'frozen fixture binary')
                    source = {'source_sha256': {}, 'client_binary_sha256': hashlib.sha256(b'frozen fixture binary').hexdigest(),
                              'created_utc': '2026-10-02T00:00:00+00:00', 'source_base_commit': '0' * 40, 'records': []}
                    if index == 1:
                        source[changed_field] = changed_value
                    path = run / 'hash-results.json'
                    path.write_text(json.dumps(source))
                    paths.append(path)
                with self.assertRaisesRegex(ValueError, 'different execution-source manifests or client binaries'):
                    audit.merge_runs(paths, Path(directory), [])

    def test_merge_rehashes_archived_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            run = Path(directory)
            (run / 'extended_hash_probe').write_bytes(b'changed binary')
            source = {'source_sha256': {}, 'client_binary_sha256': hashlib.sha256(b'original binary').hexdigest(),
                      'created_utc': '2026-10-02T00:00:00+00:00', 'source_base_commit': '0' * 40, 'records': []}
            path = run / 'hash-results.json'
            path.write_text(json.dumps(source))
            with self.assertRaisesRegex(ValueError, 'Archived client binary hash'):
                audit.merge_runs([path], run, [])

    def test_unchanged_eula_is_required(self):
        with tempfile.TemporaryDirectory() as directory:
            baseline, world = Path(directory) / 'baseline.txt', Path(directory) / 'world.txt'
            baseline.write_bytes(b'# Already approved\neula=true\n')
            world.write_bytes(baseline.read_bytes())
            self.assertTrue(audit.audit_eula(baseline, world)['passed'])
            world.write_bytes(b'eula=true\n')
            self.assertFalse(audit.audit_eula(baseline, world)['passed'])
            baseline.write_bytes(b'eula=false\n')
            world.write_bytes(baseline.read_bytes())
            self.assertFalse(audit.audit_eula(baseline, world)['passed'])


if __name__ == '__main__':
    unittest.main()
