"""Regression checks for the offline overlay oracle report parser."""
import json
from pathlib import Path
import unittest
import validate_overlays as oracle

REPORT = json.loads((Path(__file__).resolve().parents[2] /
                     "docs/validation/overlay-wire-oracle.json").read_text())


def output(row):
    return f"protocol={row['protocol']}\n" + "\n".join(
        f"{key}={value}" for key, value in row["fixtures"].items())


class OverlayVerifier(unittest.TestCase):
    def test_saved_reports_have_exact_check_set(self):
        self.assertEqual(len(REPORT["releases"]), 14)
        self.assertEqual(REPORT["total_checks"], 896)
        for row in REPORT["releases"]:
            self.assertEqual(oracle.parse_output(output(row), row["protocol"]), row["fixtures"])
            self.assertEqual(row["checks"], 64)

    def test_wrong_protocol_rejects(self):
        row = REPORT["releases"][0]
        with self.assertRaises(ValueError):
            oracle.parse_output(output(row), 776)

    def test_missing_or_duplicate_check_rejects(self):
        row = REPORT["releases"][0]
        text = output(row)
        first = "action_0=" + row["fixtures"]["action_0"]
        for mutated in (text.replace(first + "\n", ""), text + "\n" + first):
            with self.assertRaises(ValueError):
                oracle.parse_output(mutated, row["protocol"])

    def test_malformed_payload_or_rejection_rejects(self):
        row = REPORT["releases"][0]
        for key, value in (("action_0", "zz"), ("action_0", ""),
                           ("invalid_action_-1", "accepted")):
            text = output(row).replace(key + "=" + row["fixtures"][key], key + "=" + value)
            with self.assertRaises(ValueError):
                oracle.parse_output(text, row["protocol"])


if __name__ == "__main__":
    unittest.main()
