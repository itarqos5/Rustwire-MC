#!/usr/bin/env python3
"""Bounded streamed-world checks on preapproved disposable loopback Paper worlds.

No EULA acceptance, plugins, OP, accounts, or external server connections.
"""
import argparse
import datetime
import fcntl
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time
import validate_gameplay as base

PROJECT = Path(__file__).resolve().parents[2]
BUILDS, PROTOCOLS = base.BUILDS, base.PROTOCOLS
COMMAND_SOURCES = ["https://www.minecraft.net/en-us/article/minecraft-java-edition-1-19-4"]
SOURCE_PATHS = ["examples/chunk_update_probe.rs", "tools/paper/validate_chunk_updates.py", "tools/paper/test_validate_chunk_updates.py", "tools/paper/validate_gameplay.py"]
ROUNDTRIPS = ["map_chunk", "update_light", "tile_entity_data", "chunk_biomes", "update_view_position", "update_view_distance", "simulation_distance"]


def scenario_commands(version):
    commands = [
        ("platform", "execute at Rustwire run fill -2 199 -2 6 199 6 minecraft:stone replace", 0.5),
        ("position", "tp Rustwire 0.5 200 0.5", 2.0),
        ("sign", "execute at Rustwire run setblock 2 200 2 minecraft:oak_sign replace", 0.75),
        ("sign_nbt", 'execute at Rustwire run data merge block 2 200 2 {is_waxed:1b,front_text:{color:"red",has_glowing_text:1b}}', 1.0),
    ]
    for name, command in [
        ("light_on", "execute at Rustwire run fill 1 200 1 10 203 1 minecraft:glowstone replace"),
        ("light_off", "execute at Rustwire run fill 1 200 1 10 203 1 minecraft:air replace"),
        ("biome_desert", "execute at Rustwire run fillbiome 0 -64 0 15 63 15 minecraft:desert"),
        ("biome_desert", "execute at Rustwire run fillbiome 0 64 0 15 191 15 minecraft:desert"),
        ("biome_desert", "execute at Rustwire run fillbiome 0 192 0 15 319 15 minecraft:desert"),
        ("biome_mixed", "execute at Rustwire run fillbiome 0 192 0 7 207 7 minecraft:plains"),
    ]:
        commands.extend([(name + "_marker", 'tellraw Rustwire {"text":"RUSTWIRE_CHUNK_' + name + '"}', 0.2), (name, command, 2.0)])
    commands.extend([
        ("view_move", "tp Rustwire 128.5 200 0.5", 3.0),
        ("view_return", "tp Rustwire 0.5 200 0.5", 3.0),
        ("finish", 'tellraw Rustwire {"text":"RUSTWIRE_CHUNK_DONE"}', 0.1),
    ])
    return commands


def required_values(version):
    return {
        "context": {"source": "registry_join", "sections": "24", "min_y": "-64", "height": "384"},
        "full_chunk": {"x": "0", "z": "0", "sections": "24", "optional_compounds": "true"},
        "light": { "source": "0", "neighbor": "0", "sky": "15", "section": "17", "sample_index": "2065", "neighbor_index": "2064", "sky_bit": "true", "block_empty": "true", "valid_lengths": "true"},
        "embedded_sign": {"x": "2", "y": "200", "z": "2", "kind": "7", "red": "true", "glow": "true", "wax": "true"},
        "block_entity": {"kind": "7", "x": "2", "y": "200", "z": "2", "compound": "true", "red": "true", "glow": "true", "wax": "true"},
        "view_position": {"x": "8", "z": "0"},
        "view_distance": {"distance": "3"},
        "simulation_distance": {"distance": "2"},
    }


def assess_transcript(version, lines):
    observations = {}
    for line in lines:
        if line.startswith("VALUE "):
            fields = dict(re.findall(r"([a-z_]+)=([^\s]+)", line))
            observations.setdefault(fields.pop("category", ""), []).append(fields)
    def rows(category, expected):
        return [r for r in observations.get(category, []) if all(r.get(k) == v for k,v in expected.items())]
    missing = []
    for category, expected in required_values(version).items():
        if not rows(category, expected): missing.append(category)
    for stage, desert, plains in [("biome_desert","1536","0"),("biome_mixed","1520","16")]:
        if not rows("biomes", {"stage":stage,"x":"0","z":"0","sections":"24","desert":desert,"plains":plains,"exact_pattern":"true"}): missing.append(stage)
    for name in ROUNDTRIPS:
        if not rows("wire_roundtrip", {"packet":name,"semantic":"true"}): missing.append("roundtrip_" + name)
    for row in observations.get("wire_roundtrip", []):
        padding = row.get("packet") == "map_chunk" and PROTOCOLS[version] in (763,770) or row.get("packet") == "chunk_biomes" and PROTOCOLS[version] == 770
        if row.get("semantic") != "true" or row.get("equal") not in ("true", "false") or row.get("equal") == "false" and not padding: missing.append("roundtrip_mismatch")
        if row.get("packet") == "chunk_biomes" and row.get("equal") == "false" and row.get("canonical_delta") != "24": missing.append("biome_padding_size")
    for name in ["map_chunk", "update_light", "chunk_biomes"]:
        if not rows("context_dispatch", {"packet":name,"no_context_raw":"true","contextual":"true"}): missing.append("context_"+name)
    if any(r.get("no_context_raw") != "true" or r.get("contextual") != "true" for r in observations.get("context_dispatch", [])): missing.append("context_mismatch")
    errors=[line for line in lines if line.startswith(("UNSUPPORTED ", "RAW_SCENARIO ", "DECODE_ERROR ", "Error:"))]
    complete="PROBE_READY" in lines and any(line.startswith("PROBE_DONE protocol="+str(PROTOCOLS[version])+" ") for line in lines)
    dynamic_light = all(rows("light", {"stage":stage,"x":"0","z":"0","source":source,"neighbor":neighbor,"valid_lengths":"true"}) for stage,source,neighbor in [("light_on","15","14"),("light_off","0","0")])
    return {"dynamic_light_transition_observed":dynamic_light, "passed":not missing and not errors and complete, "observations":observations, "expected":required_values(version), "missing_or_wrong":sorted(set(missing)), "protocol_errors":errors, "client_lifecycle_complete":complete,
            "explicitly_unexercised":([] if dynamic_light else ["glowstone source/neighbor 15/14 to 0/0 standalone update transition (no value-matched packet emitted by the bounded fixture)"])+["embedded TAG_End sentinel NBT", "standalone null block-entity NBT (legal only 763–765)","malformed/limit rejection paths and non-overworld dimensions (fixture-tested, not live)"]}


def audit_plugins(world, server_lines):
    """Recognize only Paper's exact generated remap classpath and empty indexes."""
    initialized = [line for line in server_lines if re.search(r"Initialized \d+ plugins", line)]
    zero = any(re.search(r"Initialized 0 plugins", line) for line in initialized)
    nonzero = any(re.search(r"Initialized [1-9]\d* plugins", line) for line in initialized)
    generated, unexpected = [], []
    for jar in sorted((world / "plugins").rglob("*.jar")):
        name = jar.relative_to(world / "plugins").as_posix()
        info = {"path": "plugins/" + name, "sha256": hashlib.sha256(jar.read_bytes()).hexdigest()}
        mapping = world / "plugins/.paper-remapped/mappings/reversed" / (jar.stem + ".tiny")
        indexes = []
        for relative in ["index.json", "extra-plugins/index.json", "unknown-origin/index.json", "libraries/index.json"]:
            index = world / "plugins/.paper-remapped" / relative
            try:
                value = json.loads(index.read_text())
            except (OSError, ValueError):
                continue
            if value == {"hashes": {}, "skippedHashes": [], "mappingsHash": jar.stem}:
                indexes.append({"path": str(index.relative_to(world)), "sha256": hashlib.sha256(index.read_bytes()).hexdigest(), "contents": value})
        route = "zero_plugin_startup_log" if zero else "four_empty_generated_plugin_indexes" if not initialized and len(indexes) == 4 else None
        if re.fullmatch(r"\.paper-remapped/remap-classpath/[0-9A-F]{64}\.jar", name) and mapping.is_file() and route and not nonzero:
            info.update(mapping_path=str(mapping.relative_to(world)), mapping_sha256=hashlib.sha256(mapping.read_bytes()).hexdigest(), evidence_route=route, empty_indexes=indexes)
            generated.append(info)
        else:
            unexpected.append(info)
    return {"gate_version": "paper-generated-remap-classpath-v2", "passed": not unexpected and not nonzero,
            "installed_or_unrecognized_plugin_jars": unexpected, "generated_remap_classpath_jars": generated,
            "plugin_initialization_evidence": initialized}


def run_version(args, run_dir, client_bin, version):
    root = args.validation_dir
    world = base.prepare_world(root, run_dir, version)
    approved_eula_sha = hashlib.sha256((world / "eula.txt").read_bytes()).hexdigest()
    jar = root / "downloads" / f"paper-{version}-{BUILDS[version]}.jar"
    expected = next(x["download"]["checksums"]["sha256"] for x in json.loads(base.MANIFEST.read_text())["paper"] if x["version"] == version)
    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    if jar_sha != expected:
        raise RuntimeError("pinned Paper artifact hash mismatch")
    java = str(root / "jdk25/bin/java") if version.startswith("26.") else "java"
    command = [java, "-Xms256M", "-Xmx1024M", "-XX:ActiveProcessorCount=2", "-jar", str(jar), "--nogui"]
    record = {"version": version, "protocol": PROTOCOLS[version], "paper_build": BUILDS[version], "started_utc": base.utc_now(), "server_jar_sha256": jar_sha,
              "client_binary_sha256": hashlib.sha256(client_bin.read_bytes()).hexdigest(), "server_command": command, "java_version": subprocess.check_output([java, "-version"], stderr=subprocess.STDOUT, text=True).splitlines(), "approved_eula_sha256": approved_eula_sha, "commands": [], "passed": False}
    server = client = None
    readers, server_lines, client_lines = [], [], []
    ready, joined, probe_ready = (threading.Event() for _ in range(3))
    started = time.monotonic()
    print(f"=== CHUNK_UPDATES START {version} ===", flush=True)
    try:
        server = subprocess.Popen(command, cwd=world, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        def read_server():
            with (world / "chunk-update-server.log").open("w") as log:
                for raw in server.stdout:
                    log.write(raw); log.flush()
                    line = base.ANSI.sub("", raw).strip()
                    server_lines.append({"seconds": round(time.monotonic() - started, 3), "line": line})
                    if "Done (" in line: ready.set()
                    if "Rustwire joined the game" in line: joined.set()
                    if "Starting Minecraft server on" in line: print(line, flush=True)
        readers.append(threading.Thread(target=read_server, daemon=True)); readers[-1].start()
        base.wait_for_event(ready, [server], 150, "Paper startup")
        record["listeners"] = base.listeners()
        client = subprocess.Popen([str(client_bin), "127.0.0.1", "25565", version], cwd=world, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        def read_client():
            with (world / "chunk-update-client.log").open("w") as log:
                for raw in client.stdout:
                    log.write(raw); log.flush()
                    line = base.ANSI.sub("", raw).strip(); client_lines.append(line)
                    if line == "PROBE_READY": probe_ready.set()
                    if not line.startswith("NODE "): print(f"client {version}: {line}", flush=True)
        readers.append(threading.Thread(target=read_client, daemon=True)); readers[-1].start()
        base.wait_for_event(joined, [server, client], 45, "server-side entry")
        base.wait_for_event(probe_ready, [server, client], 45, "client spawn readiness")
        time.sleep(0.75)
        for name, command, delay in scenario_commands(version):
            if server.poll() is not None or client.poll() is not None: raise RuntimeError(f"process exited before {name}")
            record["commands"].append({"name": name, "command": command, "sent_seconds": round(time.monotonic() - started, 3)})
            print(f"console {version}: {command}", flush=True)
            server.stdin.write(command + "\n"); server.stdin.flush(); time.sleep(delay)
        client.wait(timeout=20)
        record["passed"] = client.returncode == 0
    except Exception as exc:
        record["error"] = f"{type(exc).__name__}: {exc}"
        print(f"CHUNK_UPDATES FAIL {version}: {record['error']}", flush=True)
    finally:
        base.stop_process(client); base.stop_process(server, "stop")
        for reader in readers: reader.join(timeout=5)
        record.update(server_exit=None if server is None else server.returncode, client_exit=None if client is None else client.returncode, client_output=client_lines, seconds=round(time.monotonic() - started, 3))
        record["value_checks"] = assess_transcript(version, client_lines)
        for i, entry in enumerate(record["commands"]):
            end = record["commands"][i + 1]["sent_seconds"] if i + 1 < len(record["commands"]) else record["seconds"]
            entry["server_output_in_window"] = [x["line"] for x in server_lines if entry["sent_seconds"] <= x["seconds"] < end]
        record["console_failures"] = [{"command": e["name"], "lines": [s for s in e["server_output_in_window"] if base.COMMAND_FAILURE.search(s)]} for e in record["commands"] if any(base.COMMAND_FAILURE.search(s) for s in e["server_output_in_window"])]
        record["commands_complete"] = len(record["commands"]) == len(scenario_commands(version))
        record["listener_closed"] = not any(line.split()[1].endswith(":63DD") and line.split()[3] == "0A" for path in ["/proc/net/tcp", "/proc/net/tcp6"] for line in Path(path).read_text().splitlines()[1:])
        record["log_sha256"] = {name: hashlib.sha256((world / name).read_bytes()).hexdigest() for name in ["chunk-update-client.log", "chunk-update-server.log"] if (world / name).exists()}
        record["configuration_sha256"] = {str(p.relative_to(world)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [world / "server.properties", world / "bukkit.yml", world / "spigot.yml", *sorted((world / "config").glob("*.yml"))] if p.exists()}
        record["eula_unchanged"] = hashlib.sha256((world / "eula.txt").read_bytes()).hexdigest() == approved_eula_sha
        record["no_operators"] = not json.loads((world / "ops.json").read_text()) if (world / "ops.json").exists() else True
        record["plugin_safety"] = audit_plugins(world, [row["line"] for row in server_lines])
        record["passed"] &= record["eula_unchanged"] and record["no_operators"] and record["plugin_safety"]["passed"] and record["server_exit"] == 0 and record["value_checks"]["passed"] and not record["console_failures"] and record["commands_complete"] and record["listener_closed"]
        (world / "chunk-update-result.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"=== CHUNK_UPDATES STOP {version}: passed={record['passed']} missing={record['value_checks']['missing_or_wrong']} ===", flush=True)
    return record


def verify_source_snapshot(source, commit):
    """Refuse attribution to a commit when the compiled source tree differs."""
    tracked = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", commit, "src", "Cargo.toml", "Cargo.lock"], cwd=PROJECT, text=True).splitlines()
    expected_names = {name for name in tracked if name.endswith(".rs") or name in ["Cargo.toml", "Cargo.lock"]}
    actual_names = {str(p.relative_to(source)) for p in (source / "src").rglob("*.rs")} | {"Cargo.toml", "Cargo.lock"}
    if expected_names != actual_names:
        raise RuntimeError("frozen source file set differs from attributed commit")
    for name in expected_names:
        expected = subprocess.check_output(["git", "show", commit + ":" + name], cwd=PROJECT)
        if expected != (source / name).read_bytes():
            raise RuntimeError("frozen source differs from attributed commit: " + name)
    for name in SOURCE_PATHS:
        if (source / name).read_bytes() != (PROJECT / name).read_bytes():
            raise RuntimeError("frozen probe/harness differs from invoked source: " + name)
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--versions", nargs="+", choices=list(BUILDS), default=list(BUILDS))
    parser.add_argument("--validation-dir", type=Path, default=PROJECT.parent / "rustwire-server-validation")
    parser.add_argument("--client", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-root", required=True, type=Path, help="Frozen archived source used to build the probe")
    args = parser.parse_args()
    args.validation_dir = args.validation_dir.resolve(); args.source_root = args.source_root.resolve()
    if not re.fullmatch("[0-9a-f]{40}", args.source_commit): parser.error("exact source SHA required")
    if args.validation_dir == PROJECT or PROJECT in args.validation_dir.parents: parser.error("worlds must be outside the project")
    if not args.client.is_file(): parser.error("build the probe first")
    verify_source_snapshot(args.source_root, args.source_commit)
    root = args.validation_dir
    with (root / "one-server.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        run_dir = root / "chunk-updates/runs" / stamp; run_dir.mkdir(parents=True)
        client_bin = run_dir / "chunk_update_probe"; shutil.copyfile(args.client, client_bin); client_bin.chmod(0o755)
        source = args.source_root
        result = {"source_base_commit": args.source_commit, "library_source_snapshot_matches_commit": True, "probe_harness_attribution": "exact hashes recorded separately from library commit", "scenario": "bounded-chunk-updates", "created_utc": base.utc_now(), "command_sources": COMMAND_SOURCES,
                  "library_source_sha256": {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((source / "src").rglob("*.rs"))},
                  "cargo_source_sha256": {n: hashlib.sha256((source / n).read_bytes()).hexdigest() for n in ["Cargo.toml", "Cargo.lock"]},
                  "source_sha256": {n: hashlib.sha256((source / n).read_bytes()).hexdigest() for n in SOURCE_PATHS}, "records": []}
        for version in args.versions:
            result["records"].append(run_version(args, run_dir, client_bin, version))
            (run_dir / "chunk-update-results.json").write_text(json.dumps(result, indent=2) + "\n")
        (root / "chunk-updates/latest-run.txt").write_text(str(run_dir) + "\n")
        print("Chunk update evidence: " + str(run_dir / "chunk-update-results.json"), flush=True)
        return 0 if all(r["passed"] for r in result["records"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
