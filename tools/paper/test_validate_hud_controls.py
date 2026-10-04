#!/usr/bin/env python3
"""Offline acceptance controls; no Java server or network listener is started."""
import unittest
from unittest import mock
from pathlib import Path
import tempfile
import json
import build_hud_probe as builder
import validate_hud_controls as probe


def fixture(version):
    lines = ["PROBE_READY"]
    for category, row in probe.required_values(version).items():
        lines.append("VALUE category=" + category + " " + " ".join(f"{k}={v}" for k, v in row.items()))
    lines.extend("VALUE category=clear reset=" + reset for reset in ["false", "true"])
    if probe.PROTOCOLS[version] >= 765:
        lines.extend(f"VALUE category=ticking rate={rate} frozen={frozen}" for rate, frozen in [("25", "false"), ("25", "true")])
        lines.append("VALUE category=step ticks=7")
        lines.extend(f"VALUE category=ticking rate={rate} frozen={frozen}" for rate, frozen in [("25", "false"), ("20", "false")])
    lines.extend(f"VALUE category=wire_roundtrip packet={name} equal=true" for name in probe.required_packets(version))
    lines.extend(["LIFECYCLE event=death_received", "ACTION kind=respawn", "LIFECYCLE event=respawn_received",
                  "LIFECYCLE event=respawn_position_acknowledged",
                  f"PROBE_DONE protocol={probe.PROTOCOLS[version]} roundtrips={len(probe.required_packets(version))}"])
    return lines


class AcceptanceTests(unittest.TestCase):
    def test_all_protocol_families_accept_correct_values(self):
        self.assertEqual(len(probe.BUILDS), 14)
        for version in probe.BUILDS:
            self.assertTrue(probe.assess_transcript(version, fixture(version))["passed"], version)

    def test_every_required_value_is_checked(self):
        for version in probe.BUILDS:
            for category, fields in probe.required_values(version).items():
                prefix = "VALUE category=" + category + " "
                lines = fixture(version)
                self.assertFalse(probe.assess_transcript(version, [x for x in lines if not x.startswith(prefix)])["passed"])
                for key, value in fields.items():
                    with self.subTest(version=version, category=category, key=key):
                        wrong = [x.replace(f"{key}={value}", f"{key}=WRONG") if x.startswith(prefix) else x for x in lines]
                        self.assertFalse(probe.assess_transcript(version, wrong)["passed"])

    def test_reset_and_clear_are_separate(self):
        for reset in ["false", "true"]:
            lines = [x for x in fixture("26.2") if x != "VALUE category=clear reset=" + reset]
            self.assertFalse(probe.assess_transcript("26.2", lines)["passed"])

    def test_ticking_transition_values_are_separate(self):
        for rate, frozen in [("25", "false"), ("25", "true"), ("20", "false")]:
            lines = [x for x in fixture("26.2") if x != f"VALUE category=ticking rate={rate} frozen={frozen}"]
            self.assertFalse(probe.assess_transcript("26.2", lines)["passed"])

    def test_every_roundtrip_required_and_any_mismatch_fails(self):
        for version in probe.BUILDS:
            for name in probe.required_packets(version):
                line = f"VALUE category=wire_roundtrip packet={name} equal=true"
                self.assertFalse(probe.assess_transcript(version, [x for x in fixture(version) if x != line])["passed"])
            self.assertFalse(probe.assess_transcript(version, fixture(version) + ["VALUE category=wire_roundtrip packet=experience equal=false"])["passed"])

    def test_exact_lifecycle_and_protocol(self):
        for marker in ["PROBE_READY", "LIFECYCLE event=death_received", "ACTION kind=respawn", "LIFECYCLE event=respawn_received", "LIFECYCLE event=respawn_position_acknowledged", "PROBE_DONE"]:
            lines = fixture("26.2")
            matches = [x for x in lines if x.startswith(marker)]
            self.assertFalse(probe.assess_transcript("26.2", [x for x in lines if x not in matches])["passed"])
            self.assertFalse(probe.assess_transcript("26.2", lines + matches)["passed"])
        for suffix in ["protocol=763 roundtrips=20", "protocol=776 roundtrips=0", "protocol=776 roundtrips=20 extra=true"]:
            lines = [x for x in fixture("26.2") if not x.startswith("PROBE_DONE")]
            self.assertFalse(probe.assess_transcript("26.2", lines + ["PROBE_DONE " + suffix])["passed"])

    def test_errors_and_malformed_fields_fail(self):
        for extra in ["UNSUPPORTED name=experience", "RAW_SCENARIO name=experience", "DECODE_ERROR name=experience", "Error: closed", "VALUE category=title fixture=true fixture=false", "VALUE category=title fixture", "VALUE fixture=true", "VALUE category=title fixture=", "VALUE", "PROBE_DONE", "PROBE_DONEbad", "LIFECYCLE", "LIFECYCLE event=respawn_received extra=true", "ACTION", "ACTION kind=respawn extra=true", "PROBE_READY extra=true", "UNSUPPORTED", "RAW_SCENARIO", "DECODE_ERROR"]:
            lines = fixture("26.2")
            lines.insert(-1, extra)
            self.assertFalse(probe.assess_transcript("26.2", lines)["passed"], extra)

    def test_order_and_roundtrip_count_are_verified(self):
        lines = fixture("26.2")
        self.assertFalse(probe.assess_transcript("26.2", list(reversed(lines)))["passed"])
        count = len(probe.required_packets("26.2"))
        self.assertFalse(probe.assess_transcript("26.2", [x.replace(f"roundtrips={count}", "roundtrips=1") for x in lines])["passed"])
        for marker in ["VALUE category=ticking rate=25 frozen=false", "VALUE category=ticking rate=20 frozen=false"]:
            # Startup values cannot substitute for the later unfreeze/restore.
            changed = [x for x in lines if x != marker]
            changed.insert(1, marker)
            self.assertFalse(probe.assess_transcript("26.2", changed)["passed"])

    def test_scenario_values_must_follow_readiness(self):
        lines = fixture("26.2")
        title = next(x for x in lines if x.startswith("VALUE category=title "))
        lines.remove(title)
        lines.insert(0, title)
        self.assertFalse(probe.assess_transcript("26.2", lines)["passed"])
        self.assertFalse(probe.assess_transcript("26.2", fixture("26.2") + [title])["passed"])

    def test_cleanup_attempts_server_even_if_client_stop_fails(self):
        client, server = object(), object()
        with mock.patch.object(probe.base, "stop_process", side_effect=[RuntimeError("client"), None]) as stop:
            self.assertEqual(probe.stop_processes(client, server), ["RuntimeError: client"])
            self.assertEqual(stop.call_args_list, [mock.call(client, None), mock.call(server, "stop")])

    def test_build_receipt_uses_cargo_artifact_not_assumed_target_path(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "custom-target" / "probe"
            binary.parent.mkdir()
            binary.write_bytes(b"binary")
            artifact = {"reason": "compiler-artifact", "target": {"name": "hud_control_probe", "kind": ["example"]}, "executable": str(binary)}
            line = json.dumps(artifact)
            self.assertEqual(builder.artifact_executable(line), binary)
            for invalid in ["", line + "\n" + line, json.dumps({**artifact, "executable": "relative"}), json.dumps({**artifact, "target": {"name": "other", "kind": ["example"]}})]:
                with self.assertRaises(RuntimeError): builder.artifact_executable(invalid)
            binary.unlink()
            with self.assertRaises(RuntimeError): builder.artifact_executable(line)

    def test_counts_do_not_substitute_for_values(self):
        lines = ["EVENT category=" + category for category in probe.required_values("26.2")]
        self.assertFalse(probe.assess_transcript("26.2", lines)["passed"])

    def test_explicit_unexercised_limits(self):
        self.assertTrue(probe.assess_transcript("26.2", fixture("26.2"))["explicitly_unexercised"])

    def test_command_boundaries_and_local_scope(self):
        for version in probe.BUILDS:
            commands = probe.scenario_commands(version)
            names = [x[0] for x in commands]
            self.assertEqual(len(names), len(set(names)))
            self.assertEqual("tick_step" in names, probe.PROTOCOLS[version] >= 765)
            self.assertEqual("rotation" in names, probe.PROTOCOLS[version] >= 768)
            self.assertEqual(names[-1], "finish")
            for name, command, delay in commands:
                self.assertTrue("Rustwire" in command or command.startswith("tick "), name)
                self.assertFalse(any(x in command for x in ["@a", "@p", "@e", "op ", "whitelist", "plugin"]))
                self.assertGreater(delay, 0)

    def test_build_receipt_binds_source_binary_and_toolchain(self):
        import hashlib
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "probe"
            binary.write_bytes(b"binary")
            receipt = {"method": "cargo build --locked --all-features --example hud_control_probe",
                       "source_base_commit": "a" * 40, "source_sha256": {"src/lib.rs": "digest"},
                       "binary_sha256": hashlib.sha256(b"binary").hexdigest(),
                       "cargo_version": "cargo test", "rustc_version": "rustc test"}
            with mock.patch.object(probe, "source_hashes", return_value={"src/lib.rs": "digest"}):
                self.assertTrue(probe.verify_build_receipt(receipt, root, binary, "a" * 40))
                for key in receipt:
                    bad = receipt.copy()
                    bad[key] = None
                    with self.subTest(key=key), self.assertRaises(RuntimeError):
                        probe.verify_build_receipt(bad, root, binary, "a" * 40)
                binary.write_bytes(b"stale binary")
                with self.assertRaisesRegex(RuntimeError, "binary mismatch"):
                    probe.verify_build_receipt(receipt, root, binary, "a" * 40)

    def test_source_attribution_rejects_library_and_harness_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "src").mkdir()
            files = {"src/lib.rs": b"library", "Cargo.toml": b"manifest", "Cargo.lock": b"lock", "probe.rs": b"probe"}
            for name, value in files.items(): (source / name).write_bytes(value)
            def git_output(args, **kwargs):
                if args[1] == "ls-tree": return "\n".join(x for x in files if x != "probe.rs")
                return files[args[-1].partition(":")[2]]
            with mock.patch.object(probe.subprocess, "check_output", side_effect=git_output), mock.patch.object(probe, "SOURCE_PATHS", ["probe.rs"]), mock.patch.object(probe, "PROJECT", source):
                self.assertTrue(probe.verify_source_snapshot(source, "a" * 40))
                for name in ["src/lib.rs"]:
                    (source / name).write_bytes(b"changed")
                    with self.assertRaisesRegex(RuntimeError, "differs"):
                        probe.verify_source_snapshot(source, "a" * 40)
                    (source / name).write_bytes(files[name])
                with tempfile.TemporaryDirectory() as other:
                    invoked = Path(other)
                    (invoked / "probe.rs").write_bytes(b"different invoked harness")
                    with mock.patch.object(probe, "PROJECT", invoked):
                        with self.assertRaisesRegex(RuntimeError, "probe/harness differs"):
                            probe.verify_source_snapshot(source, "a" * 40)
                (source / "src/extra.rs").write_bytes(b"new")
                with self.assertRaisesRegex(RuntimeError, "file set differs"):
                    probe.verify_source_snapshot(source, "a" * 40)


if __name__ == "__main__":
    unittest.main()
