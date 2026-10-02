"""Regression checks proving the API-oracle verifier rejects missing/changed evidence."""
import copy
import json
from pathlib import Path
import unittest
import validate_world_state as validator


class ValidatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        report = Path(__file__).resolve().parents[2]/'docs/validation/world-state-wire-oracle.json'
        cls.results = json.loads(report.read_text())['results']

    def test_every_recorded_case_passes(self):
        self.assertEqual([r['protocol'] for r in self.results], list(range(763, 777)))
        for r in self.results:
            validator.validate(r['protocol'], r['cases'])

    def test_missing_case_fails(self):
        rows = copy.deepcopy(self.results[-1]['cases'])
        del rows['time_negative_id']
        with self.assertRaises(AssertionError):
            validator.validate(776, rows)

    def test_unexpected_case_fails(self):
        rows = copy.deepcopy(self.results[-1]['cases'])
        rows['surprise'] = rows['block_ack']
        with self.assertRaises(AssertionError):
            validator.validate(776, rows)

    def test_wrong_scalar_fails(self):
        rows = copy.deepcopy(self.results[-1]['cases'])
        rows['border_size']['output_hex'] = '0000000000000000'
        with self.assertRaises(AssertionError):
            validator.validate(776, rows)

    def test_wrong_difficulty_fallback_fails(self):
        rows = copy.deepcopy(self.results[-1]['cases'])
        rows['difficulty_-1']['output_hex'] = '0001'
        with self.assertRaises(AssertionError):
            validator.validate(776, rows)

    def test_changed_duplicate_map_result_fails(self):
        rows = copy.deepcopy(self.results[-1]['cases'])
        rows['time_duplicate']['output_hex'] = '0000000000000001010003000000003f800000'
        with self.assertRaises(AssertionError):
            validator.validate(776, rows)

    def test_new_negative_registry_acceptance_fails(self):
        rows = copy.deepcopy(self.results[-1]['cases'])
        row = rows['time_negative_id']
        del row['error']
        row['output_hex'] = row['input_hex']
        with self.assertRaises(AssertionError):
            validator.validate(776, rows)

    def test_parser_rejects_repeated_or_unconsumed_rows(self):
        for text in ['x\t00\t00\tremaining=1', 'x\t00\t00\tremaining=0\nx\t00\t00\tremaining=0']:
            with self.assertRaises(AssertionError):
                validator.parse(text)


if __name__ == '__main__':
    unittest.main()
