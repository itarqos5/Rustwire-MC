#!/usr/bin/env python3
"""Bounded live command/world-effect checks on preapproved disposable Paper worlds.

Does not accept legal terms, add plugins, OP players, or contact account services.
Uses only local console fixtures and a non-executing suggestion request.
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
COMMAND_SOURCES = ["https://www.minecraft.net/en-us/article/minecraft-java-edition-1-20-5",
                   "https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-2",
                   "https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-9"]
SOURCE_PATHS = ["examples/surface_probe.rs", "tools/paper/validate_surfaces.py",
                "tools/paper/test_validate_surfaces.py", "tools/paper/validate_gameplay.py"]


def scenario_commands(version):
    p = PROTOCOLS[version]
    dust = 'minecraft:dust 1 0 0 1.5' if p < 766 else 'minecraft:dust{color:[1.0,0.0,0.0],scale:1.5}'
    transition = 'minecraft:dust_color_transition 1 0 0 1.5 0 1 0' if p < 766 else 'minecraft:dust_color_transition{from_color:[1.0,0.0,0.0],to_color:[0.0,1.0,0.0],scale:1.5}'
    item = 'minecraft:item minecraft:stone' if p < 766 else 'minecraft:item{item:{id:"minecraft:stone",count:2}}'
    commands = [
        ("platform", "execute at Rustwire run fill -2 199 -2 6 199 2 minecraft:stone replace", 0.5),
        ("position", "tp Rustwire 0.5 200 0.5", 1.5),
        ("suggestion_request", 'tellraw Rustwire {"text":"RUSTWIRE_SURFACE_REQUEST"}', 1.5),
        ("sound", "playsound rustwire:surface_fixture master Rustwire 1.25 200.5 -1.75 0.75 1.25", 0.75),
        ("stop_both", "stopsound Rustwire master rustwire:surface_fixture", 0.5),
        ("stop_name", "stopsound Rustwire * rustwire:surface_fixture", 0.5),
        ("stop_source", "stopsound Rustwire music", 0.5),
        ("stop_all", "stopsound Rustwire", 0.5),
    ]
    if p >= 771:
        commands.append(("sound_ui", "playsound rustwire:surface_ui ui Rustwire 1.25 200.5 -1.75 0.75 1.25", 0.5))
    commands.extend(("particle_" + kind, f"particle {spec} 0.5 201 0.5 0.25 0.5 0.75 0.125 7 force Rustwire", 0.75)
                    for kind, spec in [("flame", "minecraft:flame"), ("dust", dust), ("transition", transition), ("item", item)])
    commands.extend([
        ("world_event_prepare", "execute at Rustwire run setblock 2 200 2 minecraft:stone replace", 0.5),
        ("world_event", "execute at Rustwire run setblock 2 200 2 minecraft:air destroy", 0.75),
        ("entity_prepare", 'execute at Rustwire run summon minecraft:pig 2.5 200 0.5 {Tags:["rustwire_surface"],NoAI:1b,NoGravity:1b}', 0.75),
        ("entity_sound", "execute at Rustwire run damage @e[type=minecraft:pig,tag=rustwire_surface,distance=..8,limit=1] 2 minecraft:generic", 0.75),
        ("entity_remove", "execute at Rustwire run kill @e[type=minecraft:pig,tag=rustwire_surface,distance=..8,limit=1]", 0.5),
        ("explosion", 'execute at Rustwire run summon minecraft:tnt 4.5 200 0.5 {Tags:["rustwire_surface_tnt"],NoGravity:1b,Motion:[0d,0d,0d]}', 6.0),
        ("finish", 'tellraw Rustwire {"text":"RUSTWIRE_SURFACE_DONE"}', 0.1),
    ])
    return commands


def required_values(version):
    p = PROTOCOLS[version]
    result = {
        "command_tree": {"root_kind": "true", "valid_refs": "true", "msg": "true"},
        "command_redirect": {"name": "tell", "target": "msg"},
        "command_target": {"single": "false", "players_only": "true", "ask_server": "true" if p < 769 else "false", "message_child": "true"},
        "suggestions": {"transaction": "1701", "start": "5", "length": "5", "rustwire": "true", "requested": "true"},
        "sound": {"sound": "inline:rustwire:surface_fixture", "source": "Master", "x": "10", "y": "1604", "z": "-14", "volume": "0.75", "pitch": "1.25"},
        "world_event": {"data": "1", "id": "2001", "x": "2", "y": "200", "z": "2", "global": "false"},
        "item_data": {"id": "1", "layout": "template" if p >= 775 else "slot", "count": "1" if p < 775 else "2"},
        "explosion_layout": {"layout": "legacy" if p < 768 else "modern"},
    }
    for kind in ["flame", "dust", "dust_color_transition", "item"]:
        result["particle_" + kind] = {"long": "true", "always": "false" if p >= 769 else "absent", "x": "0.5", "y": "201", "z": "0.5", "dx": "0.25", "dy": "0.5", "dz": "0.75", "speed": "0.125", "count": "7"}
    result["dust_data"] = {"layout": "rgb", "r": "1", "g": "0", "b": "0", "scale": "1.5"} if p < 768 else {"layout": "packed", "color": "-65536", "scale": "1.5"}
    result["transition_data"] = {"layout": "rgb", "from": "1,0,0", "to": "0,1,0", "scale": "1.5"} if p < 768 else {"layout": "packed", "from": "-65536", "to": "-16711936", "scale": "1.5"}
    if p < 768:
        result["explosion_layout"].update(radius="4", effects="true" if p >= 765 else "false")
    else:
        result["explosion_layout"].update(knockback="true", particle="explosion_emitter", block_effects="true" if p >= 773 else "false")
    if 765 <= p < 768:
        result["explosion_effects"] = {"interaction": "Destroy", "small": "explosion", "large": "explosion_emitter"}
    if p == 765:
        result["explosion_effects"]["sound"] = "inline:minecraft:entity.generic.explode"
    if p >= 773:
        result["explosion_blocks"] = {"radius": "4", "positive_weights": "true", "finite_parameters": "true"}
    return result


def assess_transcript(version, lines):
    observations = {}
    for line in lines:
        if line.startswith("VALUE "):
            fields = dict(re.findall(r"([a-z_]+)=([^\s]+)", line))
            observations.setdefault(fields.pop("category", ""), []).append(fields)
    def rows(category, expected):
        return [row for row in observations.get(category, []) if all(row.get(k) == v for k, v in expected.items())]
    missing = []
    for category, expected in required_values(version).items():
        matches = rows(category, expected)
        if category == "command_tree":
            matches = [r for r in matches if r.get("nodes", "").isdigit() and int(r["nodes"]) > 3 and r.get("arguments", "").isdigit() and int(r["arguments"]) > 0 and r.get("root", "").isdigit() and int(r["root"]) < int(r["nodes"])]
        if category == "suggestions":
            matches = [r for r in matches if r.get("matches", "").isdigit() and int(r["matches"]) > 0]
        if category == "item_data":
            matches = [r for r in matches if r.get("id", "").isdigit() and int(r["id"]) > 0]
        if category == "world_event":
            matches = [r for r in matches if r.get("data", "").isdigit() and int(r["data"]) > 0]
        if category == "explosion_blocks":
            matches = [r for r in matches if r.get("count", "").isdigit() and int(r["count"]) > 0 and r.get("particles", "").isdigit() and int(r["particles"]) > 0]
        if not matches:
            missing.append(category)
    for name in ["declare_commands", "tab_complete", "world_particles", "explosion", "sound_effect", "stop_sound", "world_event"]:
        if not rows("wire_roundtrip", {"packet": name, "equal": "true"}):
            missing.append("roundtrip_" + name)
    if any(r.get("equal") != "true" for r in observations.get("wire_roundtrip", [])):
        missing.append("roundtrip_mismatch")
    if PROTOCOLS[version] >= 773:
        weighted = [dict(re.findall(r"([a-z_]+)=([^\s]+)", line)) for line in lines if line.startswith("EXPLOSION_PARTICLE ")]
        expected_weighted = [{"kind": "poof", "scaling": "0.5", "speed": "1", "weight": "1"}, {"kind": "smoke", "scaling": "1", "speed": "1", "weight": "1"}]
        if not all(any(all(row.get(k) == v for k, v in expected.items()) for row in weighted) for expected in expected_weighted):
            missing.append("explosion_weighted_particle_values")
        observations["explosion_weighted_particles"] = weighted
    stop_values = [{"source": "Master", "sound": "rustwire:surface_fixture"}, {"source": "none", "sound": "rustwire:surface_fixture"}, {"source": "Music", "sound": "none"}, {"source": "none", "sound": "none"}]
    for i, expected in enumerate(stop_values):
        if not rows("stop_sound", expected):
            missing.append("stop_sound_flags_" + str([3, 2, 1, 0][i]))
    if PROTOCOLS[version] >= 771 and not rows("sound", {"sound": "inline:rustwire:surface_ui", "source": "Ui", "x": "10", "y": "1604", "z": "-14", "volume": "0.75", "pitch": "1.25"}):
        missing.append("sound_ui")
    def explosion_center(row):
        try:
            return abs(float(row["x"]) - 4.5) < 0.01 and 200 <= float(row["y"]) <= 201 and abs(float(row["z"]) - 0.5) < 0.01
        except (ValueError, KeyError):
            return False
    if not any(explosion_center(row) for row in observations.get("explosion", [])):
        missing.append("explosion_center")
    errors = [line for line in lines if line.startswith(("UNSUPPORTED ", "RAW_SCENARIO ", "DECODE_ERROR ", "Error:"))]
    entity_exercised = bool(rows("entity_sound", {"fixture": "true", "source": "Neutral", "volume": "1"}))
    unexercised = [] if entity_exercised else ["entity_sound_effect (tagged pig damage/death did not yield a value-matched EntitySound packet)"]
    complete = "PROBE_READY" in lines and any(line.startswith("PROBE_DONE ") for line in lines) and any(line.startswith("ACTION kind=suggestion_request transaction=1701 ") for line in lines)
    return {"passed": not missing and not errors and complete, "observations": observations,
            "expected": required_values(version), "stop_sound_expected": stop_values,
            "missing_or_wrong": sorted(missing), "protocol_errors": errors,
            "client_lifecycle_complete": complete, "explicitly_unexercised": unexercised}


def run_version(args, run_dir, client_bin, version):
    root = args.validation_dir
    world = base.prepare_world(root, run_dir, version)
    jar = root / "downloads" / f"paper-{version}-{BUILDS[version]}.jar"
    expected = next(x["download"]["checksums"]["sha256"] for x in json.loads(base.MANIFEST.read_text())["paper"] if x["version"] == version)
    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    if jar_sha != expected:
        raise RuntimeError("pinned Paper artifact hash mismatch")
    java = str(root / "jdk25/bin/java") if version.startswith("26.") else "java"
    command = [java, "-Xms256M", "-Xmx1024M", "-XX:ActiveProcessorCount=2", "-jar", str(jar), "--nogui"]
    record = {"version": version, "protocol": PROTOCOLS[version], "paper_build": BUILDS[version], "started_utc": base.utc_now(), "server_jar_sha256": jar_sha,
              "client_binary_sha256": hashlib.sha256(client_bin.read_bytes()).hexdigest(), "server_command": command, "commands": [], "passed": False}
    server = client = None
    readers, server_lines, client_lines = [], [], []
    ready, joined, probe_ready = (threading.Event() for _ in range(3))
    started = time.monotonic()
    print(f"=== SURFACES START {version} ===", flush=True)
    try:
        server = subprocess.Popen(command, cwd=world, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        def read_server():
            with (world / "surface-server.log").open("w") as log:
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
            with (world / "surface-client.log").open("w") as log:
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
        print(f"SURFACES FAIL {version}: {record['error']}", flush=True)
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
        record["passed"] &= record["server_exit"] == 0 and record["value_checks"]["passed"] and not record["console_failures"] and record["commands_complete"] and record["listener_closed"]
        (world / "surface-result.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"=== SURFACES STOP {version}: passed={record['passed']} missing={record['value_checks']['missing_or_wrong']} ===", flush=True)
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
        run_dir = root / "surfaces/runs" / stamp; run_dir.mkdir(parents=True)
        client_bin = run_dir / "surface_probe"; shutil.copyfile(args.client, client_bin); client_bin.chmod(0o755)
        source = args.source_root
        result = {"source_base_commit": args.source_commit, "source_snapshot_matches_commit": True, "scenario": "bounded-surfaces", "created_utc": base.utc_now(), "command_sources": COMMAND_SOURCES,
                  "library_source_sha256": {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((source / "src").rglob("*.rs"))},
                  "cargo_source_sha256": {n: hashlib.sha256((source / n).read_bytes()).hexdigest() for n in ["Cargo.toml", "Cargo.lock"]},
                  "source_sha256": {n: hashlib.sha256((source / n).read_bytes()).hexdigest() for n in SOURCE_PATHS}, "records": []}
        for version in args.versions:
            result["records"].append(run_version(args, run_dir, client_bin, version))
            (run_dir / "surface-results.json").write_text(json.dumps(result, indent=2) + "\n")
        (root / "surfaces/latest-run.txt").write_text(str(run_dir) + "\n")
        print("Surface evidence: " + str(run_dir / "surface-results.json"), flush=True)
        return 0 if all(r["passed"] for r in result["records"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
