#!/usr/bin/env python3
"""Offline compaction fidelity gates; no servers or network."""
import collections
import copy
import unittest
import summarize_chunk_updates as compact


def fixture():
    value = {"packet": "update_light", "equal": "true", "semantic": "true", "canonical_delta": "0"}
    changed = {**value, "equal": "false", "canonical_delta": "24"}
    first = {
        "version": "test-a", "passed": False,
        "source_run_id": "frozen-run", "server_jar_sha256": "artifact-hash",
        "cleanup_gate_reassessment": {"original_passed": False, "amended_passed": True},
        "additional_unverified": ["no standalone packet in an earlier run"],
        "value_checks": {
            "observations": {"wire_roundtrip": [value, value, changed], "light": [{"sky": "15"}, {"sky": "15"}]},
            "missing_or_wrong": ["one-unverified-gate"], "passed": False,
            "protocol_errors": ["DECODE_ERROR preserved"], "expected": {"light": {"sky": "15"}},
        },
        "client_output": ["PROBE_READY", "VALUE a", "VALUE b", "VALUE c", "VALUE d", "VALUE e", "STAGE x", "STAGE x", "Error: preserved"],
    }
    second = copy.deepcopy(first); second["version"] = "test-b"
    return {"source_base_commit": "frozen", "records": [first, second], "summary": {
        "roundtrip_packet_counts": {"update_light": 6},
        "byte_exact_roundtrips": 4, "canonicalized_semantic_roundtrips": 2,
    }}


class CompactTests(unittest.TestCase):
    def test_cross_record_catalog_and_exact_multiplicities(self):
        source = fixture(); result = compact.compact_report(source)
        self.assertEqual(result["schema"], compact.SCHEMA)
        self.assertEqual(len(result["observation_catalog"]["wire_roundtrip"]), 2)
        self.assertEqual(len(result["observation_catalog"]["light"]), 1)
        for original, record in zip(source["records"], result["records"]):
            restored = compact.inflate_observations(record, result["observation_catalog"])
            for category, rows in original["value_checks"]["observations"].items():
                self.assertEqual(collections.Counter(map(compact.canonical, rows)),
                                 collections.Counter(map(compact.canonical, restored[category])))
            self.assertEqual(record["client_value_line_count"], 5)
            self.assertIn({"line": "STAGE x", "count": 2}, record["client_non_value_lines"])
            self.assertIn({"line": "Error: preserved", "count": 1}, record["client_non_value_lines"])

    def test_failures_amendments_hashes_and_expected_values_survive(self):
        source = fixture(); result = compact.compact_report(source)
        for original, record in zip(source["records"], result["records"]):
            for key in ["passed", "source_run_id", "server_jar_sha256", "cleanup_gate_reassessment", "additional_unverified"]:
                self.assertEqual(original[key], record[key])
            for key in ["missing_or_wrong", "passed", "protocol_errors", "expected"]:
                self.assertEqual(original["value_checks"][key], record["value_checks"][key])

    def test_per_packet_counts_are_independent_of_deduplication(self):
        result = compact.compact_report(fixture())
        counts = result["packet_roundtrip_counts"]["update_light"]
        self.assertEqual(counts, {"total": 6, "byte_exact": 4, "canonicalized": 2,
                                  "other_or_missing_equal": 0, "semantic_mismatches": 0})

    def test_input_is_not_mutated_and_output_is_deterministic(self):
        source = fixture(); preserved = copy.deepcopy(source)
        self.assertEqual(compact.compact_report(source), compact.compact_report(source))
        self.assertEqual(source, preserved)

    def test_bad_transcript_or_summary_counts_fail(self):
        source = fixture(); source["records"][0]["client_output"].append("VALUE missing")
        with self.assertRaisesRegex(ValueError, "transcript/observation"):
            compact.compact_report(source)
        source = fixture(); source["summary"]["byte_exact_roundtrips"] = 5
        with self.assertRaisesRegex(ValueError, "exact total"):
            compact.compact_report(source)

    def test_recompacting_and_invalid_multiplicity_fail(self):
        result = compact.compact_report(fixture())
        with self.assertRaisesRegex(ValueError, "unabridged"):
            compact.compact_report(result)
        result["records"][0]["value_checks"]["observation_multiplicities"]["light"]["0"] = 0
        with self.assertRaisesRegex(ValueError, "multiplicity"):
            compact.inflate_observations(result["records"][0], result["observation_catalog"])


if __name__ == "__main__":
    unittest.main()
