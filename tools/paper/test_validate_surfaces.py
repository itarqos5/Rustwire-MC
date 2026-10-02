#!/usr/bin/env python3
"""Offline acceptance gates; does not launch any process or listener."""
import unittest
from unittest import mock
from pathlib import Path
import tempfile
import validate_surfaces as surface


def fixture(version):
    lines = ["PROBE_READY", "ACTION kind=suggestion_request transaction=1701 text=/msg_Rustw", "PROBE_DONE protocol=776"]
    for category, expected in surface.required_values(version).items():
        row = expected.copy()
        row.update({"command_tree": {"root": "0", "nodes": "20", "arguments": "4"}, "suggestions": {"matches": "1"}, "item_data": {"id": "1"}, "world_event": {"data": "1"}, "explosion_blocks": {"count": "5", "particles": "1"}}.get(category, {}))
        lines.append("VALUE category=" + category + " " + " ".join(f"{k}={v}" for k,v in row.items()))
    lines.extend(["VALUE category=stop_sound source=Master sound=rustwire:surface_fixture", "VALUE category=stop_sound source=none sound=rustwire:surface_fixture", "VALUE category=stop_sound source=Music sound=none", "VALUE category=stop_sound source=none sound=none", "VALUE category=explosion x=4.5 y=200 z=0.5"])
    if surface.PROTOCOLS[version] >= 773:
        lines.extend(["EXPLOSION_PARTICLE kind=poof scaling=0.5 speed=1 weight=1", "EXPLOSION_PARTICLE kind=smoke scaling=1 speed=1 weight=1"])
    if surface.PROTOCOLS[version] >= 771:
        lines.append("VALUE category=sound sound=inline:rustwire:surface_ui source=Ui x=10 y=1604 z=-14 volume=0.75 pitch=1.25")
    lines.extend("VALUE category=wire_roundtrip packet=" + name + " equal=true" for name in ["declare_commands", "tab_complete", "world_particles", "explosion", "sound_effect", "stop_sound", "world_event"])
    return lines


class AcceptanceTests(unittest.TestCase):
    def test_all_fourteen_families(self):
        self.assertEqual(len(surface.BUILDS), 14)
        for version in surface.BUILDS:
            self.assertTrue(surface.assess_transcript(version, fixture(version))["passed"], version)

    def test_every_required_value_missing_or_wrong_fails(self):
        for version in surface.BUILDS:
            for category, fields in surface.required_values(version).items():
                lines = fixture(version)
                prefix = "VALUE category=" + category + " "
                self.assertFalse(surface.assess_transcript(version, [x for x in lines if not x.startswith(prefix)])["passed"])
                for key, value in fields.items():
                    with self.subTest(version=version, category=category, key=key):
                        wrong = [x.replace(f"{key}={value}", f"{key}=WRONG") if x.startswith(prefix) else x for x in lines]
                        self.assertFalse(surface.assess_transcript(version, wrong)["passed"])

    def test_weighted_particle_values_are_required(self):
        for kind in ["poof", "smoke"]:
            lines = [x.replace("weight=1", "weight=0") if x.startswith("EXPLOSION_PARTICLE kind=" + kind) else x for x in fixture("26.2")]
            self.assertFalse(surface.assess_transcript("26.2", lines)["passed"])

    def test_roundtrip_all_families_required_and_mismatch_fails(self):
        for name in ["declare_commands", "tab_complete", "world_particles", "explosion", "sound_effect", "stop_sound", "world_event"]:
            lines = [x for x in fixture("26.2") if x != "VALUE category=wire_roundtrip packet=" + name + " equal=true"]
            self.assertFalse(surface.assess_transcript("26.2", lines)["passed"])
        self.assertFalse(surface.assess_transcript("26.2", fixture("26.2") + ["VALUE category=wire_roundtrip packet=sound_effect equal=false"])["passed"])

    def test_packet_counts_never_substitute_for_values(self):
        self.assertFalse(surface.assess_transcript("26.2", ["EVENT category=" + c for c in surface.required_values("26.2")])["passed"])

    def test_all_error_kinds_fail(self):
        for error in ["UNSUPPORTED name=declare_commands", "RAW_SCENARIO name=world_particles", "DECODE_ERROR name=explosion", "Error: disconnected"]:
            self.assertFalse(surface.assess_transcript("26.2", fixture("26.2") + [error])["passed"])

    def test_completion_and_request_markers_required(self):
        for prefix in ["PROBE_READY", "ACTION", "PROBE_DONE"]:
            self.assertFalse(surface.assess_transcript("26.2", [x for x in fixture("26.2") if not x.startswith(prefix)])["passed"])

    def test_stop_flags_each_required(self):
        for i in range(4):
            lines = fixture("26.2")
            targets = [x for x in lines if x.startswith("VALUE category=stop_sound")]
            lines.remove(targets[i])
            self.assertFalse(surface.assess_transcript("26.2", lines)["passed"])

    def test_explosion_center_nan_or_wrong_fails(self):
        for value in ["nan", "inf", "0", "WRONG"]:
            lines = [x.replace("x=4.5", "x=" + value) for x in fixture("26.2")]
            self.assertFalse(surface.assess_transcript("26.2", lines)["passed"])

    def test_unexercised_entity_sound_is_explicit(self):
        result = surface.assess_transcript("26.2", fixture("26.2"))
        self.assertTrue(result["explicitly_unexercised"])
        self.assertFalse(surface.assess_transcript("26.2", fixture("26.2") + ["VALUE category=entity_sound fixture=true source=Neutral volume=1"])["explicitly_unexercised"])

    def test_commands_remain_scoped_to_disposable_fixture(self):
        for version in surface.BUILDS:
            for name, command, delay in surface.scenario_commands(version):
                self.assertIn("Rustwire", command, name)
                self.assertFalse(any(s in command for s in ["@a", "@p", "op Rustwire", "rcon", "whitelist", "plugin"]))
                if "@e[" in command:
                    self.assertIn("tag=rustwire_surface", command)
                    self.assertIn("distance=..8", command)
                    self.assertIn("limit=1", command)
                self.assertGreater(delay, 0)

    def test_snapshot_attribution_rejects_changes_and_extra_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir(); (source / "src").mkdir()
            files = {"src/lib.rs": b"library", "Cargo.toml": b"manifest", "Cargo.lock": b"lock"}
            for name, value in files.items(): (source / name).write_bytes(value)
            def git_output(args, **kwargs):
                if args[1] == "ls-tree": return "\n".join(files)
                return files[args[-1].partition(":")[2]]
            with mock.patch.object(surface.subprocess, "check_output", side_effect=git_output), mock.patch.object(surface, "SOURCE_PATHS", []):
                self.assertTrue(surface.verify_source_snapshot(source, "a" * 40))
                (source / "src/lib.rs").write_bytes(b"changed")
                with self.assertRaisesRegex(RuntimeError, "source differs"):
                    surface.verify_source_snapshot(source, "a" * 40)
                (source / "src/lib.rs").write_bytes(b"library")
                (source / "src/unfinished.rs").write_bytes(b"new")
                with self.assertRaisesRegex(RuntimeError, "file set differs"):
                    surface.verify_source_snapshot(source, "a" * 40)

    def test_layout_boundaries(self):
        self.assertEqual(surface.required_values("1.20.4")["dust_data"]["layout"], "rgb")
        self.assertEqual(surface.required_values("1.21.3")["dust_data"]["layout"], "packed")
        self.assertEqual(surface.required_values("1.21.3")["particle_flame"]["always"], "absent")
        self.assertEqual(surface.required_values("1.21.4")["particle_flame"]["always"], "false")
        self.assertEqual(surface.required_values("1.21.11")["item_data"]["layout"], "slot")
        self.assertEqual(surface.required_values("26.1.2")["item_data"]["layout"], "template")


if __name__ == "__main__":
    unittest.main()
