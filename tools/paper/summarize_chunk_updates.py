#!/usr/bin/env python3
"""Compact frozen chunk-update evidence without rerunning any server.

The compact schema retains each distinct observation in a shared category
catalog. Per-record multiplicities preserve the exact observation multiset;
chronological transcripts remain in the hashed local unabridged report/logs.

Normal use:
  python3 tools/paper/summarize_chunk_updates.py \
    --full-report ../rustwire-server-validation/chunk-updates/combined-results.json \
    --evidence-root ../rustwire-server-validation/chunk-updates \
    --output docs/validation/chunk-update-results.json

To reproduce the preceding cleanup-only merge, additionally supply
--merge-reports FIRST_REPORT SECOND_REPORT and choose a new --full-report
output directory. The byte-identical original merger is published alongside
this tool; its globals are set explicitly rather than relying on its original
execution directory. Existing unabridged reports are never overwritten.
"""
import argparse
import collections
import copy
import hashlib
import json
import os
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[2]
SCHEMA = "rustwire.paper.chunk-updates.compact.v1"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def inflate_observations(record, catalog):
    result = {}
    for category, counts in record["value_checks"]["observation_multiplicities"].items():
        values = []
        for index, count in counts.items():
            require(type(count) is int and count > 0, "invalid observation multiplicity")
            values.extend(copy.deepcopy(catalog[category][int(index)]) for _ in range(count))
        result[category] = values
    return result


def packet_counts(observations):
    result = {}
    for row in observations.get("wire_roundtrip", []):
        counts = result.setdefault(row["packet"], {
            "total": 0, "byte_exact": 0, "canonicalized": 0,
            "other_or_missing_equal": 0, "semantic_mismatches": 0,
        })
        counts["total"] += 1
        if row.get("equal") == "true":
            counts["byte_exact"] += 1
        elif row.get("equal") == "false":
            counts["canonicalized"] += 1
        else:
            counts["other_or_missing_equal"] += 1
        counts["semantic_mismatches"] += row.get("semantic") != "true"
    return dict(sorted(result.items()))


def compact_report(full):
    require("schema" not in full, "expected an unabridged report, not compact input")
    result = copy.deepcopy(full)
    distinct = collections.defaultdict(dict)
    for record in full["records"]:
        for category, rows in record["value_checks"]["observations"].items():
            for row in rows:
                distinct[category][canonical(row)] = row
    catalog = {
        category: [copy.deepcopy(rows[key]) for key in sorted(rows)]
        for category, rows in sorted(distinct.items())
    }
    indexes = {
        category: {canonical(row): str(index) for index, row in enumerate(rows)}
        for category, rows in catalog.items()
    }
    totals = {}
    for original, record in zip(full["records"], result["records"]):
        observations = record["value_checks"].pop("observations")
        record["value_checks"]["observation_multiplicities"] = {
            category: dict(sorted(
                collections.Counter(indexes[category][canonical(row)] for row in rows).items(),
                key=lambda item: int(item[0]),
            ))
            for category, rows in observations.items()
        }
        lines = record.pop("client_output")
        value_lines = sum(line.startswith("VALUE ") for line in lines)
        require(value_lines == sum(map(len, observations.values())),
                "VALUE transcript/observation count mismatch: " + record["version"])
        record["client_value_line_count"] = value_lines
        record["client_non_value_lines"] = [
            {"line": line, "count": count}
            for line, count in sorted(collections.Counter(
                line for line in lines if not line.startswith("VALUE ")
            ).items())
        ]
        record["packet_roundtrip_counts"] = packet_counts(observations)
        restored = inflate_observations(record, catalog)
        require(set(restored) == set(observations), "observation categories changed")
        for category in observations:
            require(collections.Counter(map(canonical, restored[category])) ==
                    collections.Counter(map(canonical, observations[category])),
                    "observation values/multiplicities changed")
        for name, counts in record["packet_roundtrip_counts"].items():
            total = totals.setdefault(name, {key: 0 for key in counts})
            for key, value in counts.items():
                total[key] += value
        # All failures, cleanup amendments, hashes, commands and per-surface
        # status remain byte-for-byte equivalent JSON values.
        for key in original.keys() - {"client_output", "value_checks"}:
            require(record[key] == original[key], "record metadata changed: " + key)
        for key in original["value_checks"].keys() - {"observations"}:
            require(record["value_checks"][key] == original["value_checks"][key],
                    "value-gate metadata changed: " + key)
    if "summary" in full:
        summary = full["summary"]
        require({name: counts["total"] for name, counts in totals.items()} ==
                summary["roundtrip_packet_counts"], "packet totals changed")
        require(sum(row["byte_exact"] for row in totals.values()) ==
                summary["byte_exact_roundtrips"], "exact total changed")
        require(sum(row["canonicalized"] for row in totals.values()) ==
                summary["canonicalized_semantic_roundtrips"], "canonical total changed")
    result["schema"] = SCHEMA
    result["compact_layout"] = {
        "observation_catalog": "Category-scoped zero-based lists of complete distinct decoded observation objects",
        "observation_multiplicities": "Per-record category maps: catalog index string -> positive occurrence count",
        "client_non_value_lines": "All distinct non-VALUE transcript lines with exact occurrence counts, including lifecycle/stage/error lines",
        "ordering": "Observation and transcript sequence order is retained in the hashed local unabridged report and logs; the compact artifact preserves exact multisets",
        "preserved": "All assertions, failures, expected values, cleanup amendments, source run IDs, packet counts and existing evidence hashes remain present",
    }
    result["observation_catalog"] = catalog
    result["packet_roundtrip_counts"] = dict(sorted(totals.items()))
    return result


def verify_local_evidence(full, root):
    for run in full["runs"]:
        path = root / "runs" / run["run_id"] / "chunk-update-results.json"
        require(sha256(path) == run["original_report_sha256"], "run report hash mismatch")
    for record in full["records"]:
        world = root / "runs" / record["source_run_id"] / record["version"]
        require(sha256(world / "chunk-update-result.json") == record["original_record_sha256"],
                "original version record hash mismatch")
        for filename, digest in record["log_sha256"].items():
            require(sha256(world / filename) == digest, "log hash mismatch: " + filename)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--full-report", required=True, type=Path)
    parser.add_argument("--evidence-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--merge-reports", nargs=2, type=Path, metavar=("FIRST", "SECOND"))
    args = parser.parse_args()
    full_path = args.full_report.resolve()
    root = args.evidence_root.resolve()
    output = args.output.resolve()
    require(output != full_path, "compact output must not overwrite unabridged input")
    if args.merge_reports:
        require(not full_path.exists() and not (full_path.parent / "combined-results.json").exists(),
                "choose a new full-report output directory for merge reproduction")
        import merge_chunk_updates as merger
        full_path.parent.mkdir(parents=True, exist_ok=True)
        merger.ROOT = full_path.parent
        merger.FIRST = args.merge_reports[0].resolve()
        merger.DEST = full_path
        merger.merge(args.merge_reports[1].resolve())
    full_bytes = full_path.read_bytes()
    full = json.loads(full_bytes)
    original_merger = Path(__file__).with_name("merge_chunk_updates.py")
    require(sha256(original_merger) == full["merge_script_sha256"],
            "published original merger does not match unabridged provenance")
    verify_local_evidence(full, root)
    digest = hashlib.sha256(full_bytes).hexdigest()
    archive = root / "full-reports" / (digest + ".json")
    archive.parent.mkdir(parents=True, exist_ok=True)
    if archive.exists():
        require(archive.read_bytes() == full_bytes, "immutable archive collision")
    else:
        with archive.open("xb") as stream:
            stream.write(full_bytes)
    compact = compact_report(full)
    compact["unabridged_report"] = {
        "sha256": digest, "bytes": len(full_bytes),
        "local_path_relative_to_project": os.path.relpath(archive, PROJECT),
        "original_logs_verified": True,
    }
    compact["published_original_merge_script"] = {
        "path": "tools/paper/merge_chunk_updates.py", "sha256": sha256(original_merger),
    }
    compact["compaction_script"] = {
        "path": "tools/paper/summarize_chunk_updates.py", "sha256": sha256(Path(__file__)),
    }
    output.write_text(json.dumps(compact, indent=2, allow_nan=False) + "\n")
    print(json.dumps({"output": str(output), "sha256": sha256(output),
                      "bytes": output.stat().st_size, "unabridged_sha256": digest,
                      "unabridged_bytes": len(full_bytes)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
