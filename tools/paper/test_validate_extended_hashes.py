"""Ensure a connected socket, missing case, or unrelated correction cannot pass."""
import unittest
import validate_extended_hashes as h


class ExtendedHashAssessment(unittest.TestCase):
    def setUp(self):
        self.case = {'case': 'fixture', 'component': 'custom_name', 'expected_hash': 123, 'bad': False}
        self.good = ['HASH_VALUE case=fixture component=custom_name value=123',
                     'HASH_RESULT case=fixture bad=false corrections=0 target_reconciled=false passed=true']
        self.server = ['[12:34:56 INFO]: [Server] RW_HASH_SERVER:fixture:ok']

    def test_positive_requires_hash_result_and_server_state(self):
        self.assertTrue(h.assess_case(self.case, self.good, self.server)['passed'])
        for lines, server in [([], self.server), (self.good, []), (self.good[:1], self.server),
                              ([self.good[1]], self.server), (self.good * 2, self.server)]:
            self.assertFalse(h.assess_case(self.case, lines, server)['passed'])

    def test_wrong_hash_and_unsolicited_correction_fail(self):
        wrong = [self.good[0].replace('=123', '=124'), self.good[1]]
        corrected = [self.good[0], self.good[1].replace('corrections=0', 'corrections=1')]
        self.assertFalse(h.assess_case(self.case, wrong, self.server)['passed'])
        self.assertFalse(h.assess_case(self.case, corrected, self.server)['passed'])

    def test_negative_requires_exact_target_reconciliation(self):
        bad = {**self.case, 'bad': True}
        result = 'HASH_RESULT case=fixture bad=true corrections=2 target_reconciled=true passed=true'
        self.assertTrue(h.assess_case(bad, [self.good[0], result], self.server)['passed'])
        for invalid in [result.replace('corrections=2', 'corrections=0'),
                        result.replace('target_reconciled=true', 'target_reconciled=false'),
                        result.replace('passed=true', 'passed=false')]:
            self.assertFalse(h.assess_case(bad, [self.good[0], invalid], self.server)['passed'])

    def test_command_echo_does_not_prove_inventory_state(self):
        echo = ['console: say RW_HASH_SERVER:fixture:ok']
        self.assertFalse(h.assess_case(self.case, self.good, echo)['passed'])

    def test_version_scoping_and_recovery(self):
        for version in h.VERSIONS:
            cases = h.selected_cases(version)
            self.assertEqual(sum(case['bad'] for case in cases), 1)
            self.assertEqual(cases[-1]['case'], 'recovery_styled')
            self.assertFalse(cases[-1]['bad'])
            self.assertEqual(len({case['case'] for case in cases}), len(cases))
            if h.base.PROTOCOLS[version] < 774:
                self.assertFalse(any(case['component'] in {'attack_range', 'use_effects', 'swing_animation'} for case in cases))


if __name__ == '__main__':
    unittest.main()
