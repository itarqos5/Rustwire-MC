#!/usr/bin/env python3
"""Offline acceptance regressions: no Java, networking, accounts, or worlds."""
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("gameplay", Path(__file__).with_name("validate_gameplay.py"))
GAMEPLAY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GAMEPLAY)


def fixture(version):
    lines = []
    for category, expected in GAMEPLAY.required_values(version).items():
        fields = expected.copy()
        if category == "entity_equipment":
            fields.update(entity="4", item_id="900")
        if category == "entity_effect_add":
            fields.update(duration="1200", effect_id="1")
        lines.extend(["EVENT category=" + category, "VALUE category=" + category + " " + " ".join(f"{k}={v}" for k, v in fields.items())])
    return lines


class AcceptanceTests(unittest.TestCase):
    def assess(self, lines, version="26.2", peer_lines=("PEER_READY", "PEER_DONE"), peer_exit=0):
        return GAMEPLAY.assess_protocol_transcript(version, lines, True, peer_lines, peer_exit)

    def test_all_fourteen_pinned_families(self):
        self.assertEqual(len(GAMEPLAY.BUILDS), 14)
        self.assertEqual(set(GAMEPLAY.PROTOCOLS.values()), set(range(763, 777)))
        for version in GAMEPLAY.BUILDS:
            with self.subTest(version=version):
                self.assertTrue(self.assess(fixture(version), version)["passed"])

    def test_events_without_actual_values_never_pass(self):
        lines = [line for line in fixture("26.2") if line.startswith("EVENT")]
        result = self.assess(lines)
        self.assertFalse(result["passed"])
        self.assertEqual(set(result["value_checks"]["missing_or_wrong"]), set(GAMEPLAY.required_values("26.2")))

    def test_every_missing_value_fails(self):
        for category in GAMEPLAY.required_values("26.2"):
            with self.subTest(category=category):
                lines = [line for line in fixture("26.2") if not line.startswith("VALUE category=" + category + " ")]
                result = self.assess(lines)
                self.assertFalse(result["passed"])
                self.assertIn(category, result["value_checks"]["missing_or_wrong"])

    def test_wrong_values_fail_even_with_matching_event_counts(self):
        for category, values in GAMEPLAY.required_values("26.2").items():
            for key in values:
                with self.subTest(category=category, key=key):
                    lines = fixture("26.2")
                    lines = [line.replace(f"{key}={values[key]}", f"{key}=WRONG") if line.startswith("VALUE category=" + category + " ") else line for line in lines]
                    self.assertFalse(self.assess(lines)["passed"])

    def test_absent_and_out_of_range_durations_fail(self):
        for duration in ["", "-1", "0", "1100", "1201", "nan"]:
            with self.subTest(duration=duration):
                lines = [line.replace("duration=1200", "duration=" + duration) for line in fixture("26.2")]
                self.assertFalse(self.assess(lines)["passed"])

    def test_missing_numeric_entity_and_effect_ids_fail(self):
        for field in ["entity=4", "item_id=900", "effect_id=1"]:
            with self.subTest(field=field):
                self.assertFalse(self.assess([line.replace(field, "") for line in fixture("26.2")])["passed"])

    def test_unsupported_and_raw_scenario_never_pass(self):
        for line in ["UNSUPPORTED name=Some(\"set_slot\") reason=nested", "RAW_SCENARIO name=player_info bytes=[]"]:
            with self.subTest(line=line):
                self.assertFalse(self.assess(fixture("26.2") + [line])["passed"])
                self.assertFalse(GAMEPLAY.assess_protocol_transcript("26.2", [line])["passed"])

    def test_peer_readiness_completion_and_exit_required(self):
        for peer, code in [((), 0), (("PEER_READY",), 0), (("PEER_DONE",), 0), (("PEER_READY", "PEER_DONE"), 1), (("PEER_READY", "PEER_DONE"), None)]:
            with self.subTest(peer=peer, code=code):
                self.assertFalse(self.assess(fixture("26.2"), peer_lines=peer, peer_exit=code)["passed"])

    def test_peer_unsupported_or_raw_cannot_hide_behind_exit_zero(self):
        for line in ["UNSUPPORTED name=player_info", "RAW_SCENARIO name=entity_equipment", "Error: invalid field"]:
            self.assertFalse(self.assess(fixture("26.2"), peer_lines=("PEER_READY", "PEER_DONE", line))["passed"])

    def test_component_layout_boundaries(self):
        self.assertNotIn("item_block_predicate", GAMEPLAY.required_values("1.20.4"))
        self.assertIn("item_block_predicate", GAMEPLAY.required_values("1.20.6"))
        self.assertNotIn("item_consumable", GAMEPLAY.required_values("1.21.1"))
        self.assertIn("item_consumable", GAMEPLAY.required_values("1.21.3"))
        self.assertNotIn("item_component_matchers", GAMEPLAY.required_values("1.21.4"))
        self.assertIn("item_component_matchers", GAMEPLAY.required_values("1.21.5"))

    def test_commands_only_target_test_player_or_tagged_entities(self):
        for version in GAMEPLAY.BUILDS:
            for name, command, delay in GAMEPLAY.scenario_commands(version, True, True):
                with self.subTest(version=version, name=name):
                    self.assertIn("Rustwire", command)
                    self.assertFalse(any(value in command for value in ["@a", "@p", "op Rustwire", "rcon", "whitelist"]))
                    if "@e[" in command:
                        self.assertIn("tag=rustwire_", command)
                        self.assertIn("distance=..8", command)
                        self.assertIn("limit=1", command)
                    self.assertGreater(delay, 0)

    def test_legacy_item_and_attribute_syntax_stays_legacy(self):
        for version in ("1.20.1", "1.20.2", "1.20.4"):
            commands = dict((name, command) for name, command, _ in GAMEPLAY.scenario_commands(version, True, True))
            self.assertIn("diamond_helmet{Damage:7}", commands["entity_equipment"])
            self.assertIn('00000000-0000-0000-0000-000000000001 "rustwire_probe" 0.125 add', commands["attribute_modifier_add"])
            self.assertFalse(any(name.startswith("component_") for name in commands))

    def test_766_retains_uuid_but_renames_operation(self):
        commands = dict((name, command) for name, command, _ in GAMEPLAY.scenario_commands("1.20.6", True, True))
        self.assertIn('00000000-0000-0000-0000-000000000001 "rustwire_probe" 0.125 add_value', commands["attribute_modifier_add"])

    def test_inline_instrument_crosses_duration_boundary(self):
        for version in GAMEPLAY.BUILDS:
            if GAMEPLAY.PROTOCOLS[version] < 766:
                continue
            commands = dict((name, command) for name, command, _ in GAMEPLAY.scenario_commands(version, True, True))
            self.assertIn('minecraft:stone[minecraft:instrument={', commands["component_instrument"])
            self.assertIn('sound_id:"rustwire:probe",range:12.0', commands["component_instrument"])
            if GAMEPLAY.PROTOCOLS[version] < 768:
                self.assertIn('use_duration:140}', commands["component_instrument"])
                self.assertNotIn('description:', commands["component_instrument"])
            else:
                self.assertIn('use_duration:3.5,description:', commands["component_instrument"])

    def test_movement_marker_precedes_coordinate_verification(self):
        for version in GAMEPLAY.BUILDS:
            commands = GAMEPLAY.scenario_commands(version, True, True)
            self.assertEqual(commands[0][0], "client_move_after_ready")
            self.assertIn("RUSTWIRE_MOVE_AFTER_READY", commands[0][1])
            self.assertEqual(commands[1][0], "verify_client_position")

    def test_server_position_requires_the_exact_requested_delta(self):
        server = [{"line": "Rustwire logged in with entity id 1 at ([gameplay-world]0.5, -60.0, 1.5)"}]
        for position, expected in [("0.5d, -60.0d, 1.5d", False), ("0.625d, -60.0d, 1.5d", True), ("0.75d, -60.0d, 1.5d", False)]:
            commands = [{"name": "verify_client_position", "server_output_in_window": ["Rustwire has the following entity data: [" + position + "]"]}]
            self.assertEqual(GAMEPLAY.client_action_confirmation(server, commands)["movement_confirmed"], expected)

    def test_later_coordinate_poll_replaces_stale_position(self):
        server = [{"line": "Rustwire logged in with entity id 1 at ([gameplay-world]0.5, -60.0, 1.5)"}]
        commands = [{"name": "verify_client_position", "server_output_in_window": ["Rustwire has the following entity data: [0.5d, -60.0d, 1.5d]", "Rustwire has the following entity data: [0.625d, -60.0d, 1.5d]"]}]
        self.assertTrue(GAMEPLAY.client_action_confirmation(server, commands)["movement_confirmed"])

    def test_console_parse_failure_is_detected(self):
        for message in ["Unknown or incomplete command", "Incorrect argument", "Expected value at position 2", "No entity was found", "Nothing changed", "error<--[HERE]"]:
            self.assertRegex(message, GAMEPLAY.COMMAND_FAILURE)


if __name__ == "__main__":
    unittest.main()
