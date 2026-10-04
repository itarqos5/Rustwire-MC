#!/usr/bin/env python3
"""Offline acceptance/parser regression tests; no network, Java, or servers."""

import copy
import importlib.util
from pathlib import Path
import unittest
from unittest import mock
import tempfile
import hashlib
import io
from types import SimpleNamespace
import build_grim_probe as build


SPEC = importlib.util.spec_from_file_location("validate_grim", Path(__file__).with_name("validate_grim.py"))
GRIM = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GRIM)


def clean_fixture(version="1.21.1"):
    commands = [
        {"name": name, "command": command, "phase": phase, "sent_seconds": 10.0 + index, "sent_utc": "synthetic"}
        for index, (name, command, _delay, phase) in enumerate(GRIM.scenario(version))
    ]
    by_name = {command["name"]: command for command in commands}
    slot = "Slot: -106b, " if version == "1.21.1" else ""
    outputs = {
        "grim_version": "Grim Version: 2.3.73",
        "grim_profile": f"Grim » Profile for Rustwire\nVersion: {version}",
        "grim_prediction_debug": "Console output for Rustwire is now enabled",
        "initial_survival": "Rustwire has the following entity data: 0",
        "initial_position": "Rustwire has the following entity data: [0.5d, -60.0d, 0.5d]",
        "walk_position": "Rustwire has the following entity data: [0.5d, -60.0d, 4.8134259902859675d]",
        "initial_inventory": 'Rustwire has the following entity data: [{Slot: 0b, id: "minecraft:wind_charge", count: 8}]',
        "swapped_inventory": 'Rustwire has the following entity data: {' + slot + 'id: "minecraft:wind_charge", count: 8}',
        "wind_inventory": 'Rustwire has the following entity data: {' + slot + 'id: "minecraft:wind_charge", count: 6}',
        "wind_entities": "Test passed, count: 2",
        "negative_begin": "Grim » Rustwire failed BadPacketsF (x1) state=true",
    }
    server = [
        {"seconds": by_name[name]["sent_seconds"] + 0.1, "line": line, "utc": "synthetic"}
        for name, text in outputs.items() for line in text.splitlines()
    ]
    client = [
        {"seconds": 0.0, "line": f"GRIM_ACTION phase={phase}", "utc": "synthetic"}
        for phase in ["walk", "inventory", "wind", "negative"]
    ]
    record = {
        "version": version, "commands": commands, "seconds": 99.0, "server_exit": 0,
        "client_exit": 0, "ready_seen": True, "grim_version": "2.3.73",
        "no_ops_or_permission_grants": True, "check_configuration_unchanged": True,
        "listener_closed": True, "cleanup_errors": [], "readers_drained": True,
    }
    return record, server, client


class AssessmentTests(unittest.TestCase):
    def assess(self, fixture):
        record, server, client = copy.deepcopy(fixture)
        GRIM.assess(record, server, client)
        return record

    def assert_rejected(self, fixture, expected_check):
        record = self.assess(fixture)
        self.assertFalse(record["passed"])
        self.assertIn(expected_check, record["failed_checks"])
        return record

    def test_clean_result_and_separate_negative_window(self):
        record = self.assess(clean_fixture())
        self.assertTrue(record["passed"], record["failed_checks"])
        self.assertEqual(record["legitimate_flags"], [])
        self.assertEqual(len(record["negative_flags"]), 1)
        self.assertEqual(record["negative_flags"][0]["check"], "BadPacketsF")

    def test_modern_offhand_equipment_location(self):
        record = self.assess(clean_fixture("1.21.5"))
        self.assertTrue(record["passed"], record["failed_checks"])
        self.assertEqual(record["server_action_confirmation"]["offhand_nbt_location"], "equipment.offhand")
        command = next(command for command in record["commands"] if command["name"] == "swapped_inventory")
        self.assertTrue(command["command"].endswith("equipment.offhand"))

    def test_newer_console_count_punctuation(self):
        fixture = clean_fixture("26.2")
        for row in fixture[1]:
            row["line"] = row["line"].replace("Test passed, count: 2", "Test passed. Count: 2")
        record = self.assess(fixture)
        self.assertTrue(record["passed"], record["failed_checks"])
        self.assertEqual(record["server_action_confirmation"]["wind_entity_count"], 2)

    def test_legitimate_simulation_flag_rejected(self):
        fixture = clean_fixture()
        begin = next(command["sent_seconds"] for command in fixture[0]["commands"] if command["name"] == "wind_begin")
        fixture[1].append({"seconds": begin + 0.2, "utc": "synthetic", "line": "Grim » Rustwire failed Simulation (x1) offset=0.4"})
        record = self.assert_rejected(fixture, "no_flags_outside_negative_control")
        self.assertEqual(record["legitimate_flags"][0]["phase"], "valid_wind")
        self.assertEqual(len(record["negative_flags"]), 1)

    def test_startup_setup_settling_and_late_flags_rejected(self):
        for name, expected_phase in [(None, "startup"), ("grim_profile", "setup"), ("teleport", "settling"), ("done", "done")]:
            with self.subTest(phase=expected_phase):
                fixture = clean_fixture()
                timestamp = 1.0 if name is None else next(command["sent_seconds"] for command in fixture[0]["commands"] if command["name"] == name) + 0.2
                fixture[1].append({"seconds": timestamp, "utc": "synthetic", "line": "Grim » Rustwire failed Simulation (x1) offset=0.4"})
                record = self.assert_rejected(fixture, "no_flags_outside_negative_control")
                self.assertEqual(record["legitimate_flags"][0]["phase"], expected_phase)

    def test_missing_negative_flag_rejected(self):
        fixture = clean_fixture()
        fixture[1][:] = [row for row in fixture[1] if "failed BadPacketsF" not in row["line"]]
        self.assert_rejected(fixture, "negative_control_flagged")

    def test_negative_flag_outside_its_window_rejected(self):
        fixture = clean_fixture()
        flag = next(row for row in fixture[1] if "failed BadPacketsF" in row["line"])
        flag["seconds"] = 1.0
        record = self.assert_rejected(fixture, "negative_control_flagged")
        self.assertIn("no_flags_outside_negative_control", record["failed_checks"])

    def test_missing_action_marker_rejected(self):
        fixture = clean_fixture()
        fixture[2][:] = [row for row in fixture[2] if "phase=wind" not in row["line"]]
        self.assert_rejected(fixture, "all_client_actions_observed")

    def test_wrong_server_item_or_count_rejected(self):
        for replacement in ['id: "minecraft:stone", count: 6', 'id: "minecraft:wind_charge", count: 7']:
            with self.subTest(replacement=replacement):
                fixture = clean_fixture()
                for row in fixture[1]:
                    row["line"] = row["line"].replace('id: "minecraft:wind_charge", count: 6', replacement)
                self.assert_rejected(fixture, "two_wind_uses_confirmed")

    def test_changed_settings_or_op_permission_evidence_rejected(self):
        for field, check in [("check_configuration_unchanged", "check_configuration_unchanged"), ("no_ops_or_permission_grants", "no_ops_or_permission_grants")]:
            with self.subTest(field=field):
                fixture = clean_fixture()
                fixture[0][field] = False
                self.assert_rejected(fixture, check)

    def test_missing_position_and_projectile_evidence_rejected(self):
        fixture = clean_fixture()
        fixture[1][:] = [row for row in fixture[1] if "count: 2" not in row["line"] and "4.8134259902859675d" not in row["line"]]
        record = self.assert_rejected(fixture, "wind_entities_confirmed")
        self.assertIn("walking_confirmed", record["failed_checks"])

    def test_packet_events_listener_exception_rejected(self):
        fixture = clean_fixture()
        fixture[1].append({"seconds": 20.2, "utc": "synthetic", "line": "[packetevents] PacketEvents caught an unhandled exception while calling your listener."})
        self.assert_rejected(fixture, "no_runtime_errors")

    def test_unexpected_or_duplicate_negative_flags_fail(self):
        for check in ["Simulation", "BadPacketsF"]:
            fixture = clean_fixture()
            begin = next(command["sent_seconds"] for command in fixture[0]["commands"] if command["name"] == "negative_begin")
            fixture[1].append({"seconds": begin + 0.2, "utc": "synthetic", "line": f"Grim » Rustwire failed {check} (x1) test"})
            self.assert_rejected(fixture, "only_expected_negative_flag")

    def test_listener_or_cleanup_failure_rejected(self):
        for field, value, check in [("listener_closed", False, "listener_closed"), ("cleanup_errors", ["client stop failed"], "cleanup_succeeded")]:
            fixture = clean_fixture()
            fixture[0][field] = value
            self.assert_rejected(fixture, check)

    def test_receipt_binds_binary_and_sources(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "probe"
            binary.write_bytes(b"binary")
            receipt = {"method": build.METHOD, "source_base_commit": "a" * 40,
                       "source_sha256": {"library": "hash"}, "binary_sha256": hashlib.sha256(b"binary").hexdigest(),
                       "cargo_version": "cargo", "rustc_version": "rustc"}
            with mock.patch.object(build, "verify_library"), mock.patch.object(build, "verify_harness"), mock.patch.object(build, "sources", return_value={"library": "hash"}):
                self.assertTrue(build.verify_receipt(receipt, root, binary, "a" * 40))
                for key in receipt:
                    bad = receipt.copy(); bad[key] = None
                    with self.subTest(key=key), self.assertRaises(RuntimeError):
                        build.verify_receipt(bad, root, binary, "a" * 40)

    def test_wrong_player_nbt_does_not_count(self):
        fixture = clean_fixture()
        for row in fixture[1]:
            row["line"] = row["line"].replace("Rustwire has the following entity data", "OtherPlayer has the following entity data")
        result = self.assert_rejected(fixture, "walking_confirmed")
        for check in ["initial_wind_stack_confirmed", "offhand_swap_confirmed", "two_wind_uses_confirmed", "survival_confirmed"]:
            self.assertIn(check, result["failed_checks"])

    def test_command_order_content_and_phase_are_exact(self):
        for field in ["name", "command", "phase"]:
            fixture = clean_fixture(); fixture[0]["commands"][0][field] = "wrong"
            self.assert_rejected(fixture, "all_scenario_commands_sent")
        fixture = clean_fixture(); fixture[0]["commands"][:2] = reversed(fixture[0]["commands"][:2])
        self.assert_rejected(fixture, "all_scenario_commands_sent")

    def test_bukkit_and_java_runtime_failures_rejected(self):
        for line in ["Could not pass event PlayerMoveEvent to GrimAC v2.3.73", "java.lang.NullPointerException: fixture", "java.lang.NoSuchMethodError: fixture"]:
            fixture = clean_fixture()
            fixture[1].append({"seconds": 20.2, "utc": "synthetic", "line": line})
            self.assert_rejected(fixture, "no_runtime_errors")

    def test_partial_log_cannot_prove_zero_flags(self):
        fixture = clean_fixture(); fixture[0]["readers_drained"] = False
        self.assert_rejected(fixture, "complete_log_capture")
        writer = mock.MagicMock()
        writer.write.side_effect = [None, OSError("disk full")]
        path = mock.MagicMock()
        path.open.return_value.__enter__.return_value = writer
        consumed = []
        process = SimpleNamespace(stdout=io.StringIO("positive evidence\nlate Simulation flag\n"))
        state = GRIM.capture_output(process, path, consumed.append)
        self.assertFalse(state["eof"])
        self.assertIn("disk full", state["error"])
        self.assertEqual(consumed, ["positive evidence\n"])

    def test_exact_fixture_displacement_and_projectile_count(self):
        fixture = clean_fixture()
        for row in fixture[1]: row["line"] = row["line"].replace("4.8134259902859675d", "4.0d")
        self.assert_rejected(fixture, "walking_fixture_displacement")
        fixture = clean_fixture()
        for row in fixture[1]: row["line"] = row["line"].replace("count: 2", "count: 3")
        self.assert_rejected(fixture, "wind_entities_confirmed")

    def test_invoked_harness_matches_receipted_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "snapshot"; invoked = Path(directory) / "invoked"
            root.mkdir(); invoked.mkdir()
            (root / "runner.py").write_text("original")
            (invoked / "runner.py").write_text("original")
            with mock.patch.object(build, "PROJECT", invoked), mock.patch.object(build, "HARNESS", ["runner.py"]):
                build.verify_harness(root)
                (invoked / "runner.py").write_text("different")
                with self.assertRaisesRegex(RuntimeError, "invoked harness/manifest differs"):
                    build.verify_harness(root)

    def test_configuration_sources_remain_native(self):
        for version in GRIM.VERSIONS:
            with self.subTest(version=version):
                commands = [command for _name, command, _delay, _phase in GRIM.scenario(version)]
                self.assertFalse(any(command.startswith(("op ", "grim reload", "grim log", "gamemode creative")) for command in commands))
                self.assertIn('tellraw Rustwire {"text":"GRIM_NEGATIVE"}', commands)


if __name__ == "__main__":
    unittest.main()
