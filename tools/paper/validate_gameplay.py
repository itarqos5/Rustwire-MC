#!/usr/bin/env python3
"""Exercise a bounded gameplay scenario against isolated, pinned Paper servers.

This creates new disposable worlds outside the project. It never accepts the
Minecraft EULA: selected baseline server directories must already contain an
explicitly approved eula=true. No RCON, accounts, or public listener are used.
"""

import argparse
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time


PROJECT = Path(__file__).resolve().parents[2]
BUILDS = {"1.20.1": 196, "1.21.1": 133, "26.2": 129}
ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
EVENT = re.compile(r"\bEVENT\s+category=([a-zA-Z0-9_]+)")
ACTION = re.compile(r"\bACTION\s+(?:kind|category)=([a-zA-Z0-9_]+)")
DEFAULT_CATEGORIES = ["entity_spawn", "entity_move", "entity_remove", "block_update",
                      "health", "inventory", "system_chat", "respawn", "keepalive",
                      "item_damage", "item_custom", "chat_marker", "metadata",
                      "death", "respawn_requested", "movement_sent", "selected_slot_sent"]
COMMAND_FAILURE = re.compile(
    r"Unknown or incomplete command|Incorrect argument|<--\[HERE\]|"
    r"No (?:entity|player) was found|Unable to summon|Target is invulnerable|"
    r"Could not set the block|Nothing changed|(?:Expected|Invalid).+at position",
    re.IGNORECASE,
)


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=["bounded-gameplay"])
    parser.add_argument("--versions", nargs="+", choices=list(BUILDS), default=list(BUILDS))
    parser.add_argument("--validation-dir", type=Path,
                        default=Path(os.environ.get("RUSTWIRE_VALIDATION_DIR", str(PROJECT.parent / "rustwire-server-validation"))))
    parser.add_argument("--client", type=Path,
                        default=PROJECT / "target/debug/examples/gameplay_probe")
    parser.add_argument("--min-seconds", type=int, default=45)
    parser.add_argument("--client-timeout", type=int, default=120)
    parser.add_argument("--ready-marker", default="PROBE_READY")
    parser.add_argument("--required-category", action="append", default=DEFAULT_CATEGORIES.copy(),
                        help="Require an additional packet/action marker beyond the bounded scenario baseline")
    args = parser.parse_args()
    if not 20 <= args.min_seconds <= 120:
        parser.error("--min-seconds must be between 20 and 120")
    if args.client_timeout <= args.min_seconds or args.client_timeout > 180:
        parser.error("--client-timeout must exceed --min-seconds and be at most 180")
    args.validation_dir = args.validation_dir.resolve()
    if args.validation_dir == PROJECT or PROJECT in args.validation_dir.parents:
        parser.error("validation evidence and worlds must remain outside the Rustwire project")
    if not args.client.is_file():
        parser.error("the gameplay probe binary does not exist; build it before running")
    return args


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def property_lines(path):
    return path.read_text().splitlines()


def prepare_world(root, run_dir, version):
    baseline = root / "servers" / version
    if "eula=true" not in property_lines(baseline / "eula.txt"):
        raise RuntimeError(f"{version}: explicit prior EULA approval is required")
    lines = property_lines(baseline / "server.properties")
    required = ["server-ip=127.0.0.1", "online-mode=false", "enable-rcon=false", "enable-query=false"]
    if not all(line in lines for line in required):
        raise RuntimeError(f"{version}: baseline is not the isolated offline test configuration")
    world = run_dir / version
    world.mkdir()
    shutil.copyfile(baseline / "eula.txt", world / "eula.txt")
    replacements = {
        "level-name": "gameplay-world", "server-port": "25565",
        "server-ip": "127.0.0.1", "online-mode": "false",
        "enforce-secure-profile": "false", "enable-rcon": "false",
        "enable-query": "false", "enable-jmx-monitoring": "false",
        "view-distance": "2", "simulation-distance": "2",
        "max-players": "2", "gamemode": "creative", "difficulty": "peaceful",
        "motd": f"Rustwire isolated gameplay probe {version}",
    }
    updated = [line for line in lines if line.partition("=")[0] not in replacements]
    updated.extend(f"{key}={value}" for key, value in replacements.items())
    (world / "server.properties").write_text("\n".join(updated) + "\n")
    for name in ["bukkit.yml", "spigot.yml"]:
        if (baseline / name).exists():
            shutil.copyfile(baseline / name, world / name)
    if (baseline / "config").exists():
        shutil.copytree(baseline / "config", world / "config")
    # Only immutable boot caches are reused. All world/player data is new.
    for name in ["cache", "libraries", "versions"]:
        if (baseline / name).is_dir():
            (world / name).symlink_to(baseline / name, target_is_directory=True)
    return world


def listeners():
    found = []
    for file in ["/proc/net/tcp", "/proc/net/tcp6"]:
        for line in Path(file).read_text().splitlines()[1:]:
            fields = line.split()
            if fields[1].endswith(":63DD") and fields[3] == "0A":
                found.append(fields[1])
    if not found or not all(value in ["0100007F:63DD", "0000000000000000FFFF00000100007F:63DD"] for value in found):
        raise RuntimeError(f"The actual server listener is not exclusively loopback: {found}")
    return found


def wait_for_event(event, processes, timeout, description):
    until = time.monotonic() + timeout
    while not event.wait(0.1):
        if any(process.poll() is not None for process in processes):
            raise RuntimeError(f"A process exited before {description}")
        if time.monotonic() >= until:
            raise RuntimeError(f"Timed out waiting for {description}")


def stop_process(process, polite=None):
    if process is None or process.poll() is not None:
        return
    if polite is not None:
        try:
            process.stdin.write(polite + "\n")
            process.stdin.flush()
        except (BrokenPipeError, OSError):
            pass
    else:
        process.terminate()
    try:
        process.wait(timeout=45 if polite else 5)
    except subprocess.TimeoutExpired:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def scenario_commands(version):
    if version == "1.20.1":
        sword = "minecraft:diamond_sword{Damage:3}"
        stone = "minecraft:stone{rustwire_probe:1}"
    else:
        sword = "minecraft:diamond_sword[minecraft:damage=3]"
        stone = "minecraft:stone[minecraft:custom_data={rustwire_probe:1}]"
    return [
        ("verify_client_position", "data get entity Rustwire Pos", 0.5),
        ("verify_client_selected_slot", "data get entity Rustwire SelectedItemSlot", 0.5),
        ("inventory_give", "give Rustwire minecraft:stone 3", 1.25),
        ("inventory_damaged_sword", f"give Rustwire {sword} 1", 1.25),
        ("inventory_custom_data", f"give Rustwire {stone} 1", 1.25),
        ("system_chat", 'tellraw Rustwire {"text":"Rustwire gameplay probe","color":"gold"}', 1.25),
        ("survival_mode", "gamemode survival Rustwire", 1.25),
        ("entity_spawn", 'execute at Rustwire run summon minecraft:armor_stand ~2 ~ ~ {Tags:["rustwire_probe"],NoGravity:1b}', 1.25),
        ("entity_move", "execute at Rustwire as @e[type=minecraft:armor_stand,tag=rustwire_probe,distance=..8,limit=1,sort=nearest] at @s run tp @s ~0.25 ~ ~", 1.25),
        ("block_update", "execute at Rustwire run setblock ~2 ~ ~2 minecraft:stone replace", 1.25),
        ("entity_remove", "execute at Rustwire run kill @e[type=minecraft:armor_stand,tag=rustwire_probe,distance=..8,limit=1]", 1.25),
        ("health_damage", "damage Rustwire 2 minecraft:generic", 1.75),
        ("death_respawn", "kill Rustwire", 1.0),
    ]


def client_action_confirmation(server_lines, commands):
    initial = None
    observed = None
    slot_confirmed = False
    numeric = r"([-+\d.eE]+)"
    spawn_pattern = re.compile(r"at \(\[(?:gameplay-world|minecraft:overworld)\]" + numeric + r",\s*" + numeric + r",\s*" + numeric + r"\)")
    position_pattern = re.compile(r"following entity data: \[" + numeric + r"d,\s*" + numeric + r"d,\s*" + numeric + r"d\]")
    for entry in server_lines:
        match = spawn_pattern.search(entry["line"])
        if match and initial is None:
            initial = [float(value) for value in match.groups()]
    for command in commands:
        for line in command["server_output_in_window"]:
            if command["name"] == "verify_client_position":
                match = position_pattern.search(line)
                if match:
                    observed = [float(value) for value in match.groups()]
            if command["name"] == "verify_client_selected_slot":
                slot_confirmed |= bool(re.search(r"following entity data: 1\s*$", line))
    delta = None if initial is None or observed is None else [b - a for a, b in zip(initial, observed)]
    movement_confirmed = delta is not None and all(abs(a - b) < 1e-8 for a, b in zip(delta, [0.125, 0.0, 0.0]))
    return {"initial_position": initial, "server_queried_position": observed,
            "position_delta": delta, "movement_confirmed": movement_confirmed,
            "selected_slot_1_confirmed": slot_confirmed}


def run_version(args, run_dir, client_bin, version):
    root = args.validation_dir
    world = prepare_world(root, run_dir, version)
    java = str(root / "jdk25/bin/java") if version == "26.2" else "java"
    jar = root / "downloads" / f"paper-{version}-{BUILDS[version]}.jar"
    manifest_path = root / "report/download-provenance.json"
    if not manifest_path.is_file():
        manifest_path = PROJECT / "docs/validation/download-provenance.json"
    provenance = json.loads(manifest_path.read_text())
    expected = next(item["download"]["checksums"]["sha256"]
                    for item in provenance["paper"] if item["version"] == version)
    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    if jar_sha != expected:
        raise RuntimeError(f"{version}: Paper JAR SHA-256 differs from the pinned official artifact")
    server_command = [java, "-Xms256M", "-Xmx1024M", "-XX:ActiveProcessorCount=2", "-jar", str(jar), "--nogui"]
    record = {"version": version, "paper_build": BUILDS[version], "started_utc": utc_now(),
              "scenario": args.mode, "server_command": server_command,
              "server_jar_sha256": jar_sha,
              "client_binary_sha256": hashlib.sha256(client_bin.read_bytes()).hexdigest(),
              "commands": [], "observed_categories": [], "passed": False}
    ready, joined, probe_ready = threading.Event(), threading.Event(), threading.Event()
    server_lines, client_lines, client_timeline, categories = [], [], [], set()
    actions = set()
    started = time.monotonic()
    server = client = None
    readers = []
    print(f"=== GAMEPLAY START {version} ===", flush=True)
    try:
        server = subprocess.Popen(server_command, cwd=world, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)

        def read_server():
            with (world / "gameplay-server.log").open("w") as log:
                for raw in server.stdout:
                    log.write(raw)
                    log.flush()
                    line = ANSI.sub("", raw).strip()
                    server_lines.append({"seconds": round(time.monotonic() - started, 3), "line": line})
                    if "Done (" in line:
                        ready.set()
                    if "Rustwire joined the game" in line:
                        joined.set()
                    if any(text in line for text in ["Starting Minecraft server on", "joined the game", "lost connection"]):
                        print(line, flush=True)

        readers.append(threading.Thread(target=read_server, daemon=True))
        readers[-1].start()
        wait_for_event(ready, [server], 150, "Paper startup")
        record["listeners"] = listeners()
        client_command = [str(client_bin), "127.0.0.1", "25565", version, str(args.min_seconds)]
        record["client_command"] = client_command
        client_started = time.monotonic()
        client = subprocess.Popen(client_command, cwd=world, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, text=True, bufsize=1)

        def read_client():
            with (world / "gameplay-client.log").open("w") as log:
                for raw in client.stdout:
                    log.write(raw)
                    log.flush()
                    line = ANSI.sub("", raw).strip()
                    client_lines.append(line)
                    client_timeline.append({"seconds": round(time.monotonic() - started, 3), "line": line})
                    line_categories = EVENT.findall(line)
                    first_observation = any(category not in categories for category in line_categories)
                    for category in line_categories:
                        categories.add(category)
                        if category in ["movement_sent", "selected_slot_sent", "respawn_requested"]:
                            actions.add(category)
                    actions.update(ACTION.findall(line))
                    if args.ready_marker in line:
                        probe_ready.set()
                    if not line_categories or first_observation:
                        print(f"client {version}: {line}", flush=True)

        readers.append(threading.Thread(target=read_client, daemon=True))
        readers[-1].start()
        wait_for_event(joined, [server, client], 45, "Rustwire server-side world entry")
        wait_for_event(probe_ready, [server, client], 45, "typed probe spawn readiness")
        time.sleep(0.75)
        commands = scenario_commands(version)
        for name, command, pause in commands:
            if server.poll() is not None or client.poll() is not None:
                raise RuntimeError(f"A process exited before scenario step {name}")
            record["commands"].append({"name": name, "command": command,
                                       "sent_seconds": round(time.monotonic() - started, 3)})
            print(f"console {version}: {command}", flush=True)
            server.stdin.write(command + "\n")
            server.stdin.flush()
            time.sleep(pause)
        remaining = args.client_timeout - (time.monotonic() - client_started)
        if remaining <= 0:
            raise RuntimeError("Scenario exceeded the bounded client timeout")
        client.wait(timeout=remaining)
        record["client_seconds"] = round(time.monotonic() - client_started, 3)
        readers[-1].join(timeout=5)
        record["observed_categories"] = sorted(categories)
        record["missing_categories"] = sorted(set(args.required_category) - categories)
        record["scenario_commands_sent"] = len(record["commands"]) == len(commands)
        record["passed"] = client.returncode == 0 and not record["missing_categories"] and record["scenario_commands_sent"]
        if not record["passed"]:
            record["error"] = f"Probe exit {client.returncode}; missing categories {record['missing_categories']}"
    except Exception as exc:
        record["error"] = f"{type(exc).__name__}: {exc}"
        print(f"GAMEPLAY FAIL {version}: {record['error']}", flush=True)
    finally:
        stop_process(client)
        stop_process(server, "stop")
        for reader in readers:
            reader.join(timeout=5)
        record["server_exit"] = None if server is None else server.returncode
        record["client_exit"] = None if client is None else client.returncode
        record["observed_categories"] = sorted(categories)
        record["observed_client_actions"] = sorted(actions)
        record["client_output"] = client_lines
        record["client_timeline"] = client_timeline
        record["unsupported_packet_observations"] = [line for line in client_lines if line.startswith("UNSUPPORTED ")]
        record["seconds"] = round(time.monotonic() - started, 3)
        if record["server_exit"] != 0:
            record["passed"] = False
        for index, command in enumerate(record["commands"]):
            end = record["commands"][index + 1]["sent_seconds"] if index + 1 < len(record["commands"]) else record["seconds"]
            command["server_output_in_window"] = [item["line"] for item in server_lines
                                                   if command["sent_seconds"] <= item["seconds"] < end and item["line"]]
            command["console_failure_lines"] = [line for line in command["server_output_in_window"]
                                                if COMMAND_FAILURE.search(line)]
        record["console_failures"] = [
            {"command": command["name"], "lines": command["console_failure_lines"]}
            for command in record["commands"] if command["console_failure_lines"]
        ]
        if record["console_failures"]:
            record["passed"] = False
            record["error"] = "One or more scenario commands failed; inspect console_failures"
        record["client_action_server_confirmation"] = client_action_confirmation(server_lines, record["commands"])
        confirmations = record["client_action_server_confirmation"]
        if not confirmations["movement_confirmed"] or not confirmations["selected_slot_1_confirmed"]:
            record["passed"] = False
            record.setdefault("error", "The server did not confirm both requested client movement and selected-slot actions")
        (world / "gameplay-result.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"=== GAMEPLAY STOP {version}: passed={record['passed']}, server exit={record['server_exit']} ===", flush=True)
    return record


def main():
    args = arguments()
    root = args.validation_dir
    root.mkdir(parents=True, exist_ok=True)
    with (root / "one-server.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        run_dir = root / "gameplay" / "runs" / stamp
        run_dir.mkdir(parents=True)
        binary_dir = run_dir / "client-bin"
        binary_dir.mkdir()
        client_bin = binary_dir / "gameplay_probe"
        shutil.copyfile(args.client, client_bin)
        client_bin.chmod(0o755)
        results = {"scenario": args.mode, "created_utc": utc_now(),
                   "required_categories": args.required_category, "records": []}
        for version in args.versions:
            results["records"].append(run_version(args, run_dir, client_bin, version))
            (run_dir / "gameplay-results.json").write_text(json.dumps(results, indent=2) + "\n")
        (root / "gameplay" / "latest-run.txt").write_text(str(run_dir) + "\n")
        print(f"Gameplay evidence: {run_dir / 'gameplay-results.json'}", flush=True)
        return 0 if all(record["passed"] for record in results["records"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
