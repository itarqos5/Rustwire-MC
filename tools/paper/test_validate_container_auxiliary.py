"""Fail-closed offline auxiliary-container oracle report checks."""
import copy
import json
from pathlib import Path
import unittest
import validate_container_auxiliary as oracle

ROOT = Path(__file__).resolve().parents[2]
REPORT = json.loads((ROOT / "docs/validation/container-auxiliary-wire-oracle.json").read_text())


def output(row):
    return f"protocol={row['protocol']}\n" + "\n".join(
        f"{key}={value}" for key, value in row["fixtures"].items())


class ContainerAuxiliaryVerifier(unittest.TestCase):
    def test_saved_report_exact_check_set_and_provenance(self):
        self.assertEqual([r["protocol"] for r in REPORT["releases"]], list(range(763, 777)))
        self.assertEqual(REPORT["total_checks"], 1023)
        self.assertEqual(REPORT["oracle_source_sha256"], oracle.sha256(oracle.SOURCE))
        self.assertEqual(len(REPORT["schemas"]), 14)
        for row in REPORT["releases"]:
            self.assertEqual(oracle.parse_output(output(row), row["protocol"]), row["fixtures"])
            self.assertEqual(row["checks"], 66 if row["protocol"] < 766 else 75)
            self.assertEqual(row["merchant_verification"], "decode_and_inspect" if row["protocol"] >= 769 else "decode_and_reencode")
            self.assertEqual(len(row["prepared_jar_sha256"]), 64)

    def test_rust_fixtures_match_the_verified_report(self):
        self.assertEqual((ROOT / "tests/fixtures/container-auxiliary.txt").read_text(), oracle.fixture_text(REPORT["releases"]))

    def test_wrong_protocol_missing_duplicate_and_invalid_results_reject(self):
        row = REPORT["releases"][0]
        text = output(row)
        first = "property_7=" + row["fixtures"]["property_7"]
        for changed in (text.replace(first + "\n", ""), text + "\n" + first,
                        text.replace(first, "property_7=zz"), text.replace(first, "property_7=")):
            with self.assertRaises(ValueError):
                oracle.parse_output(changed, row["protocol"])
        with self.assertRaises(ValueError):
            oracle.parse_output(text, 776)

    def test_roundtrip_and_semantic_mismatches_reject(self):
        for row in REPORT["releases"]:
            for key, value in (("button_255_values", "255,255" if row["protocol"] < 766 else "-1,-1"),
                               ("property_7_values", "7,0,0"), ("horse_7_values", "7"),
                               ("select_300_values", "bad"), ("property_7_roundtrip", "00")):
                mutated = copy.deepcopy(row)
                mutated["fixtures"][key] = value
                with self.assertRaises(ValueError):
                    oracle.parse_output(output(mutated), row["protocol"])


if __name__ == "__main__":
    unittest.main()
