#!/usr/bin/env python3
"""Bounded local Paper HUD/control replay. Prior EULA approval is required.

Uses disposable loopback-only offline worlds and console commands. Never logs
into an account, installs plugins, grants OP, or changes anti-cheat settings.
"""
import argparse
import datetime
import fcntl
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import threading
import time

import validate_gameplay as base

PROJECT = Path(__file__).resolve().parents[2]
BUILDS, PROTOCOLS = base.BUILDS, base.PROTOCOLS
COMMAND_SOURCES = ["https://www.minecraft.net/en-us/article/minecraft-java-edition-1-20-3",
                   "https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-2"]
SOURCE_PATHS = ["examples/hud_control_probe.rs", "tools/paper/validate_hud_controls.py",
                "tools/paper/test_validate_hud_controls.py", "tools/paper/validate_gameplay.py",
                "tools/paper/build_hud_probe.py"]


def scenario_commands(version):
    p = PROTOCOLS[version]
    commands = [
        ("platform", "execute at Rustwire run fill -2 199 -2 6 199 2 minecraft:stone replace", 0.5),
        ("face", "tp Rustwire 0.5 200 0.5 facing 10.5 200 0.5", 0.75),
        ("title_time", "title Rustwire times 7 31 11", 0.25),
        ("subtitle", 'title Rustwire subtitle {"text":"RUSTWIRE_SUBTITLE"}', 0.25),
        ("title", 'title Rustwire title {"text":"RUSTWIRE_TITLE"}', 0.25),
        ("actionbar", 'title Rustwire actionbar {"text":"RUSTWIRE_ACTIONBAR"}', 0.25),
        ("clear", "title Rustwire clear", 0.25),
        ("reset", "title Rustwire reset", 0.25),
        ("experience_levels", "experience set Rustwire 12 levels", 0.25),
        ("experience_points", "experience set Rustwire 3 points", 0.75),
    ]
    if p >= 768:
        commands.append(("rotation", "rotate Rustwire 37 -12", 0.5))
    if p >= 765:
        commands.extend([
            ("tick_rate", "tick rate 25", 0.5),
            ("tick_freeze", "tick freeze", 0.5),
            ("tick_step", "tick step 7t", 0.5),
            ("tick_unfreeze", "tick unfreeze", 0.5),
            ("tick_restore", "tick rate 20", 0.5),
        ])
    commands.extend([
        ("death", "kill Rustwire", 2.0),
        ("finish", 'tellraw Rustwire {"text":"RUSTWIRE_HUD_DONE"}', 0.1),
    ])
    return commands


def required_values(version):
    result = {
        "title": {"fixture": "true"}, "subtitle": {"fixture": "true"},
        "actionbar": {"fixture": "true"},
        "title_time": {"fade_in": "7", "stay": "31", "fade_out": "11"},
        "experience": {"level": "12", "total": "0", "bar_bits": struct.pack(">f", 3 / 31).hex()},
        "death": {"own_entity": "true", "player_text": "true"},
        "face": {"anchor": "Feet", "x": "10.5", "y": "200", "z": "0.5", "entity": "false"},
    }
    if PROTOCOLS[version] >= 768:
        result["rotation"] = {"yaw": "37", "pitch": "-12", "relative_yaw": "false", "relative_pitch": "false"}
    if PROTOCOLS[version] >= 765:
        result["step"] = {"ticks": "7"}
    return result


def required_packets(version):
    names = ["clear_titles", "action_bar", "set_title_text", "set_title_subtitle", "set_title_time",
             "experience", "death_combat_event", "face_player"]
    if PROTOCOLS[version] >= 765:
        names.extend(["set_ticking_state", "step_tick"])
    if PROTOCOLS[version] >= 768:
        names.append("player_rotation")
    return names


def assess_transcript(version, lines):
    observations, scenario_observations = {}, {}
    malformed = []
    ready_position = lines.index("PROBE_READY") if lines.count("PROBE_READY") == 1 else -1
    reserved = {"PROBE_READY", "ACTION kind=respawn", "LIFECYCLE event=death_received",
                "LIFECYCLE event=respawn_received", "LIFECYCLE event=respawn_position_acknowledged"}
    for index, line in enumerate(lines):
        if line.startswith(("PROBE_READY", "ACTION", "LIFECYCLE")) and line not in reserved:
            malformed.append(line)
        if line.startswith("VALUE"):
            if not line.startswith("VALUE "):
                malformed.append(line)
                continue
            fields = {}
            for token in line.removeprefix("VALUE ").split():
                key, sep, value = token.partition("=")
                if not sep or not key or not value or key in fields:
                    malformed.append(line)
                    break
                fields[key] = value
            category = fields.pop("category", "")
            if not category:
                malformed.append(line)
            observations.setdefault(category, []).append(fields)
            if index > ready_position:
                scenario_observations.setdefault(category, []).append(fields)
    def matches(category, expected):
        return any(all(row.get(k) == v for k, v in expected.items()) for row in scenario_observations.get(category, []))
    missing = [category for category, expected in required_values(version).items() if not matches(category, expected)]
    for reset in ["false", "true"]:
        if not matches("clear", {"reset": reset}): missing.append("clear_" + reset)
    if PROTOCOLS[version] >= 765:
        for rate, frozen in [("25", "false"), ("25", "true"), ("20", "false")]:
            if not matches("ticking", {"rate": rate, "frozen": frozen}): missing.append("ticking_" + rate + "_" + frozen)
    for name in required_packets(version):
        if not matches("wire_roundtrip", {"packet": name, "equal": "true"}): missing.append("roundtrip_" + name)
    if any(row.get("equal") != "true" for row in observations.get("wire_roundtrip", [])):
        missing.append("roundtrip_mismatch")
    errors = [line for line in lines if line.startswith(("UNSUPPORTED", "RAW_SCENARIO", "DECODE_ERROR", "Error:"))]
    done = [line for line in lines if line.startswith("PROBE_DONE")]
    lifecycle = ["PROBE_READY", "LIFECYCLE event=death_received", "ACTION kind=respawn",
                 "LIFECYCLE event=respawn_received", "LIFECYCLE event=respawn_position_acknowledged"]
    lifecycle_positions = [lines.index(marker) for marker in lifecycle if lines.count(marker) == 1]
    expected_done = f"PROBE_DONE protocol={PROTOCOLS[version]} roundtrips={len(observations.get('wire_roundtrip', []))}"
    complete = (len(lifecycle_positions) == len(lifecycle)
                and lifecycle_positions == sorted(lifecycle_positions)
                and len(done) == 1 and done[0] == expected_done and lines[-1] == done[0]
                and len(observations.get("wire_roundtrip", [])) > 0
                and lifecycle_positions[-1] < lines.index(done[0]))
    if PROTOCOLS[version] >= 765:
        transitions = ["VALUE category=ticking rate=25 frozen=false",
                       "VALUE category=ticking rate=25 frozen=true",
                       "VALUE category=step ticks=7",
                       "VALUE category=ticking rate=25 frozen=false",
                       "VALUE category=ticking rate=20 frozen=false"]
        cursor = max(0, ready_position + 1)
        for marker in transitions:
            try:
                cursor = lines.index(marker, cursor) + 1
            except ValueError:
                missing.append("ordered_ticking_transitions")
                break
    return {"passed": not missing and not errors and not malformed and complete,
            "observations": observations, "expected": required_values(version),
            "missing_or_wrong": sorted(missing), "protocol_errors": errors,
            "malformed_lines": malformed, "client_lifecycle_complete": complete,
            "explicitly_unexercised": ["HUD open_book, enter_combat_event, end_combat_event",
                                      "World-control packets outside face/rotation/ticking/step", "Rendering and player physics"]}


def stop_processes(client, server):
    errors = []
    for process, polite in [(client, None), (server, "stop")]:
        try:
            base.stop_process(process, polite)
        except Exception as exc:
            errors.append(f"{type(exc).__name__}: {exc}")
    return errors


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
    print(f"=== HUD/CONTROL START {version} ===", flush=True)
    try:
        server = subprocess.Popen(command, cwd=world, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        def read_server():
            with (world / "hud-control-server.log").open("w") as log:
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
            with (world / "hud-control-client.log").open("w") as log:
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
        print(f"HUD/CONTROL FAIL {version}: {record['error']}", flush=True)
    finally:
        cleanup_errors = stop_processes(client, server)
        record["cleanup_errors"] = cleanup_errors
        for reader in readers: reader.join(timeout=5)
        record.update(server_exit=None if server is None else server.returncode, client_exit=None if client is None else client.returncode, client_output=client_lines, seconds=round(time.monotonic() - started, 3))
        record["value_checks"] = assess_transcript(version, client_lines)
        for i, entry in enumerate(record["commands"]):
            end = record["commands"][i + 1]["sent_seconds"] if i + 1 < len(record["commands"]) else record["seconds"]
            entry["server_output_in_window"] = [x["line"] for x in server_lines if entry["sent_seconds"] <= x["seconds"] < end]
        record["console_failures"] = [{"command": e["name"], "lines": [s for s in e["server_output_in_window"] if base.COMMAND_FAILURE.search(s)]} for e in record["commands"] if any(base.COMMAND_FAILURE.search(s) for s in e["server_output_in_window"])]
        record["commands_complete"] = len(record["commands"]) == len(scenario_commands(version))
        record["listener_closed"] = not any(line.split()[1].endswith(":63DD") and line.split()[3] == "0A" for path in ["/proc/net/tcp", "/proc/net/tcp6"] for line in Path(path).read_text().splitlines()[1:])
        record["passed"] &= not cleanup_errors and record["server_exit"] == 0 and record["value_checks"]["passed"] and not record["console_failures"] and record["commands_complete"] and record["listener_closed"]
        (world / "hud-control-result.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"=== HUD/CONTROL STOP {version}: passed={record['passed']} missing={record['value_checks']['missing_or_wrong']} ===", flush=True)
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


def source_hashes(source):
    names = {str(p.relative_to(source)) for p in (source / "src").rglob("*.rs")}
    names.update(["Cargo.toml", "Cargo.lock", *SOURCE_PATHS])
    return {name: hashlib.sha256((source / name).read_bytes()).hexdigest() for name in sorted(names)}


def verify_build_receipt(receipt, source, client, commit):
    if receipt.get("method") != "cargo build --locked --all-features --example hud_control_probe":
        raise RuntimeError("unsupported build receipt method")
    if receipt.get("source_base_commit") != commit or receipt.get("source_sha256") != source_hashes(source):
        raise RuntimeError("build receipt source mismatch")
    if receipt.get("binary_sha256") != hashlib.sha256(client.read_bytes()).hexdigest():
        raise RuntimeError("build receipt binary mismatch")
    if not receipt.get("cargo_version") or not receipt.get("rustc_version"):
        raise RuntimeError("missing build toolchain receipt")
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--versions", nargs="+", choices=list(BUILDS), default=list(BUILDS))
    parser.add_argument("--validation-dir", type=Path, default=PROJECT.parent / "rustwire-server-validation")
    parser.add_argument("--client", required=True, type=Path)
    parser.add_argument("--build-receipt", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-root", required=True, type=Path, help="Frozen archived source used to build the probe")
    args = parser.parse_args()
    args.validation_dir = args.validation_dir.resolve(); args.source_root = args.source_root.resolve()
    if not re.fullmatch("[0-9a-f]{40}", args.source_commit): parser.error("exact source SHA required")
    if args.validation_dir == PROJECT or PROJECT in args.validation_dir.parents: parser.error("worlds must be outside the project")
    if not args.client.is_file(): parser.error("build the probe first")
    verify_source_snapshot(args.source_root, args.source_commit)
    receipt = json.loads(args.build_receipt.read_text())
    verify_build_receipt(receipt, args.source_root, args.client, args.source_commit)
    root = args.validation_dir
    with (root / "one-server.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        run_dir = root / "hud-controls/runs" / stamp; run_dir.mkdir(parents=True)
        client_bin = run_dir / "hud_control_probe"; shutil.copyfile(args.client, client_bin); client_bin.chmod(0o755)
        source = args.source_root
        result = {"build_receipt": receipt, "source_base_commit": args.source_commit, "library_snapshot_matches_base_commit": True, "harness_attribution": "Probe/harness are separately hash-pinned in source_sha256; source_base_commit attributes only the library and Cargo files", "scenario": "bounded-hud-controls", "created_utc": base.utc_now(), "command_sources": COMMAND_SOURCES,
                  "library_source_sha256": {str(p.relative_to(source)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((source / "src").rglob("*.rs"))},
                  "cargo_source_sha256": {n: hashlib.sha256((source / n).read_bytes()).hexdigest() for n in ["Cargo.toml", "Cargo.lock"]},
                  "source_sha256": {n: hashlib.sha256((source / n).read_bytes()).hexdigest() for n in SOURCE_PATHS}, "records": []}
        for version in args.versions:
            result["records"].append(run_version(args, run_dir, client_bin, version))
            (run_dir / "hud-control-results.json").write_text(json.dumps(result, indent=2) + "\n")
        (root / "hud-controls/latest-run.txt").write_text(str(run_dir) + "\n")
        print("HUD/control evidence: " + str(run_dir / "hud-control-results.json"), flush=True)
        return 0 if all(r["passed"] for r in result["records"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
