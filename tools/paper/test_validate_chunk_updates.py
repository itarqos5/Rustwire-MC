#!/usr/bin/env python3
"""Offline fail-closed acceptance tests; no processes or listeners."""
import unittest
from unittest import mock
from pathlib import Path
import tempfile
import validate_chunk_updates as probe


def fixture(version):
    lines=["PROBE_READY", "PROBE_DONE protocol="+str(probe.PROTOCOLS[version])+" seconds=1"]
    for category, fields in probe.required_values(version).items():
        lines.append("VALUE category="+category+" "+" ".join(f"{k}={v}" for k,v in fields.items()))
    lines += ["VALUE category=biomes stage=biome_desert x=0 z=0 sections=24 desert=1536 plains=0 exact_pattern=true", "VALUE category=biomes stage=biome_mixed x=0 z=0 sections=24 desert=1520 plains=16 exact_pattern=true"]
    lines += ["VALUE category=wire_roundtrip packet="+name+" equal=true semantic=true canonical_delta=0" for name in probe.ROUNDTRIPS]
    lines += ["VALUE category=context_dispatch packet="+name+" no_context_raw=true contextual=true" for name in ["map_chunk","update_light","chunk_biomes"]]
    return lines


class AcceptanceTests(unittest.TestCase):
    def test_all_fourteen(self):
        self.assertEqual(len(probe.BUILDS),14)
        for version in probe.BUILDS: self.assertTrue(probe.assess_transcript(version,fixture(version))["passed"])

    def test_every_exact_value_required(self):
        for version in probe.BUILDS:
            for category, fields in probe.required_values(version).items():
                for key,value in fields.items():
                    lines=[x.replace(f"{key}={value}",f"{key}=WRONG") if x.startswith("VALUE category="+category+" ") else x for x in fixture(version)]
                    self.assertFalse(probe.assess_transcript(version,lines)["passed"],(version,category,key))

    def test_each_required_observation_missing_fails(self):
        for i in range(len(fixture("26.2"))):
            lines=fixture("26.2");lines.pop(i)
            self.assertFalse(probe.assess_transcript("26.2",lines)["passed"],i)

    def test_light_samples_and_biome_pattern_are_values(self):
        for old,new in [("sky=15","sky=14"),("source=0","source=1"),("neighbor=0","neighbor=1"),("desert=1520","desert=1536"),("plains=16","plains=0"),("exact_pattern=true","exact_pattern=false"),("valid_lengths=true","valid_lengths=false")]:
            self.assertFalse(probe.assess_transcript("26.2",[x.replace(old,new) for x in fixture("26.2")])["passed"])

    def test_semantic_roundtrip_always_required(self):
        self.assertFalse(probe.assess_transcript("1.21.5",fixture("1.21.5")+["VALUE category=wire_roundtrip packet=chunk_biomes equal=false semantic=false canonical_delta=24"])["passed"])

    def test_padding_only_verified_boundaries(self):
        for version in probe.BUILDS:
            extra="VALUE category=wire_roundtrip packet=chunk_biomes equal=false semantic=true canonical_delta=24"
            self.assertEqual(probe.assess_transcript(version,fixture(version)+[extra])["passed"],version=="1.21.5")
        for delta in ["0","23","25","-24"]:
            self.assertFalse(probe.assess_transcript("1.21.5",fixture("1.21.5")+["VALUE category=wire_roundtrip packet=chunk_biomes equal=false semantic=true canonical_delta="+delta])["passed"])

    def test_any_extra_mismatch_fails(self):
        for extra in ["VALUE category=wire_roundtrip packet=update_light equal=false semantic=true canonical_delta=0","VALUE category=context_dispatch packet=update_light no_context_raw=false contextual=true","DECODE_ERROR name=update_light","RAW_SCENARIO name=chunk_biomes","UNSUPPORTED name=chunk_biomes","Error: disconnected"]:
            self.assertFalse(probe.assess_transcript("26.2",fixture("26.2")+[extra])["passed"])

    def test_protocol_specific_completion(self):
        self.assertFalse(probe.assess_transcript("1.20.1",fixture("26.2"))["passed"])

    def test_commands_scoped(self):
        for version in probe.BUILDS:
            for name,command,delay in probe.scenario_commands(version):
                self.assertIn("Rustwire",command)
                self.assertFalse(any(s in command for s in ["@a","@p","op Rustwire","rcon","plugin"]))
                self.assertGreater(delay,0)

    def test_unverified_is_explicit(self):
        self.assertTrue(probe.assess_transcript("26.2",fixture("26.2"))["explicitly_unexercised"])

    def test_luminous_gap_requires_both_value_matched_transitions(self):
        lines=fixture("26.2")
        on="VALUE category=light stage=light_on x=0 z=0 source=15 neighbor=14 valid_lengths=true"
        off="VALUE category=light stage=light_off x=0 z=0 source=0 neighbor=0 valid_lengths=true"
        for extra in [[],[on],[off]]:
            result=probe.assess_transcript("26.2",lines+extra)
            self.assertFalse(result["dynamic_light_transition_observed"])
            self.assertTrue(any("glowstone" in s for s in result["explicitly_unexercised"]))
        result=probe.assess_transcript("26.2",lines+[on,off])
        self.assertTrue(result["dynamic_light_transition_observed"])
        self.assertFalse(any("glowstone" in s for s in result["explicitly_unexercised"]))

    def test_only_documented_generated_remap_cache_is_exempted(self):
        with tempfile.TemporaryDirectory() as directory:
            world=Path(directory); plugins=world/"plugins"
            name="A"*64
            jar=plugins/".paper-remapped/remap-classpath"/(name+".jar")
            mapping=plugins/".paper-remapped/mappings/reversed"/(name+".tiny")
            jar.parent.mkdir(parents=True); mapping.parent.mkdir(parents=True)
            jar.write_bytes(b"generated remap classpath"); mapping.write_bytes(b"mapping")
            lines=["[PluginInitializerManager] Initialized 0 plugins"]
            self.assertTrue(probe.audit_plugins(world,lines)["passed"])
            self.assertEqual(len(probe.audit_plugins(world,lines)["generated_remap_classpath_jars"]),1)
            self.assertFalse(probe.audit_plugins(world,[])["passed"])
            self.assertFalse(probe.audit_plugins(world,lines+["Initialized 1 plugins"])["passed"])
            mapping.unlink()
            self.assertFalse(probe.audit_plugins(world,lines)["passed"])
            mapping.write_bytes(b"mapping")
            for path in [plugins/"actual-plugin.jar",plugins/"nested/actual-plugin.jar",plugins/".paper-remapped/remap-classpath/unrecognized.jar"]:
                path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(b"plugin")
                self.assertFalse(probe.audit_plugins(world,lines)["passed"])
                path.unlink()

    def test_legacy_empty_index_evidence_is_narrow(self):
        import json
        with tempfile.TemporaryDirectory() as directory:
            world=Path(directory);base=world/"plugins/.paper-remapped";name="B"*64
            jar=base/"remap-classpath"/(name+".jar");mapping=base/"mappings/reversed"/(name+".tiny")
            jar.parent.mkdir(parents=True);mapping.parent.mkdir(parents=True)
            jar.write_bytes(b"generated");mapping.write_bytes(b"mapping")
            files=[base/p for p in ["index.json","extra-plugins/index.json","unknown-origin/index.json","libraries/index.json"]]
            empty={"hashes":{},"skippedHashes":[],"mappingsHash":name}
            for path in files:path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(empty))
            result=probe.audit_plugins(world,[])
            self.assertTrue(result["passed"])
            self.assertEqual(result["generated_remap_classpath_jars"][0]["evidence_route"],"four_empty_generated_plugin_indexes")
            for path in files:
                for changed in [{**empty,"hashes":{"plugin":"hash"}},{**empty,"skippedHashes":["plugin"]},{**empty,"mappingsHash":"C"*64},{**empty,"unknown":[]}]:
                    path.write_text(json.dumps(changed));self.assertFalse(probe.audit_plugins(world,[])["passed"])
                path.unlink();self.assertFalse(probe.audit_plugins(world,[])["passed"])
                path.write_text(json.dumps(empty))
            self.assertFalse(probe.audit_plugins(world,["Initialized 1 plugins"])["passed"])

    def test_snapshot_rejects_modified_library(self):
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory);(source/"src").mkdir()
            files={"src/lib.rs":b"library","Cargo.toml":b"manifest","Cargo.lock":b"lock"}
            for name,value in files.items():(source/name).write_bytes(value)
            def output(args,**kwargs):
                return "\n".join(files) if args[1]=="ls-tree" else files[args[-1].partition(":")[2]]
            with mock.patch.object(probe.subprocess,"check_output",side_effect=output),mock.patch.object(probe,"SOURCE_PATHS",[]):
                self.assertTrue(probe.verify_source_snapshot(source,"a"*40))
                (source/"src/lib.rs").write_bytes(b"modified")
                with self.assertRaises(RuntimeError):probe.verify_source_snapshot(source,"a"*40)
                (source/"src/lib.rs").write_bytes(b"library");(source/"src/extra.rs").write_text("extra")
                with self.assertRaises(RuntimeError):probe.verify_source_snapshot(source,"a"*40)

if __name__=="__main__":unittest.main()
