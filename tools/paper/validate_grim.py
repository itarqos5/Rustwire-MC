#!/usr/bin/env python3
"""Run bounded, non-OP headless-client scenarios on isolated Paper with Grim.

Use only disposable offline loopback servers. Prior explicit Minecraft EULA
acceptance is required in the baseline workspace. This script never accepts
terms, disables checks, grants exemptions, or uploads anti-cheat debug logs.
"""

import argparse
import datetime
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import threading
import time
import zipfile


PROJECT = Path(__file__).resolve().parents[2]
VERSIONS = {"1.21.1": "1FIGlM6Q", "1.21.5": "1FIGlM6Q", "26.1.2": "YJEwvStg", "26.2": "YJEwvStg"}
ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]|\u00a7[0-9A-FK-ORa-fk-or]")
FLAG = re.compile(r"\bRustwire\s+failed\s+(?P<check>.+?)\s+\(x(?P<vl>\d+)\)\s*(?P<verbose>.*)")
ACTION = re.compile(r"\bGRIM_ACTION\s+phase=([a-z_]+)")
COMMAND_FAILURE = re.compile(
    r"Unknown or incomplete command|Incorrect argument|<--\[HERE\]|"
    r"No (?:entity|player) was found|Unable to summon|Could not set the block|"
    r"(?:Expected|Invalid).+at position|Player is exempt or offline", re.IGNORECASE,
)
RUNTIME_FAILURE = re.compile(
    r"Error (?:occurred while|loading|enabling|disabling)|Exception in thread|"
    r"An error occurred while processing a packet|caught an unhandled exception|Failed to (?:load|enable) Grim|"
    r"\b(?:DecoderException|EncoderException|NoClassDefFoundError|UnsupportedClassVersionError)\b", re.IGNORECASE,
)


def utc_now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def digest(path, algorithm="sha256"):
    return hashlib.new(algorithm, path.read_bytes()).hexdigest()


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=["bounded-grim"])
    parser.add_argument("--versions", nargs="+", choices=list(VERSIONS), default=["1.21.1", "1.21.5", "26.2"])
    parser.add_argument("--validation-dir", type=Path, default=Path(os.environ.get("RUSTWIRE_VALIDATION_DIR", str(PROJECT.parent / "rustwire-server-validation"))))
    parser.add_argument("--grim-dir", type=Path, default=PROJECT.parent / "rustwire-grim-validation")
    parser.add_argument("--client", type=Path, default=PROJECT / "target/release/examples/grim_probe")
    parser.add_argument("--prepare-only", action="store_true", help="Verify artifacts and prepare new worlds without launching any process")
    parser.add_argument("--client-timeout", type=int, default=100)
    args = parser.parse_args()
    args.validation_dir = args.validation_dir.resolve()
    args.grim_dir = args.grim_dir.resolve()
    args.client = args.client.resolve()
    for path in [args.validation_dir, args.grim_dir]:
        if path == PROJECT or PROJECT in path.parents:
            parser.error("worlds and runtime evidence must be outside the source repository")
    if args.grim_dir == args.validation_dir or args.validation_dir in args.grim_dir.parents:
        parser.error("Grim fixtures must remain separate from the baseline workspace")
    if not args.prepare_only and not args.client.is_file():
        parser.error("client binary is missing; build grim_probe first or use --prepare-only")
    if not 60 <= args.client_timeout <= 180:
        parser.error("--client-timeout must be between 60 and 180 seconds")
    if len(set(args.versions)) != len(args.versions):
        parser.error("duplicate versions are not allowed")
    return args


def verified_plugin(args, pin):
    """Download only the exact official Modrinth CDN artifact in our manifest."""
    destination = args.grim_dir / "downloads" / pin["download_url"].rsplit("/", 1)[1]
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.is_file():
        if not pin["download_url"].startswith("https://cdn.modrinth.com/data/LJNGWSvH/versions/"):
            raise RuntimeError("The Grim artifact is not on the pinned official project CDN")
        temporary = destination.with_suffix(".jar.part")
        subprocess.run([
            "curl", "--fail", "--location", "--silent", "--show-error", "--retry", "2",
            "--user-agent", "Rustwire-Compatibility-Validation/0.1 (isolated testing)",
            pin["download_url"], "--output", str(temporary),
        ], check=True)
        if digest(temporary) != pin["local_sha256"]:
            raise RuntimeError("Downloaded Grim artifact SHA-256 mismatch")
        temporary.replace(destination)
    if destination.stat().st_size != pin["bytes"] or digest(destination) != pin["local_sha256"]:
        raise RuntimeError("Pinned Grim artifact size/SHA-256 mismatch")
    for algorithm, expected in pin["publisher_hashes"].items():
        if digest(destination, algorithm) != expected:
            raise RuntimeError(f"Grim publisher {algorithm} mismatch")
    return destination


def prepare_server(args, run_dir, version, paper_manifest, grim_manifest):
    baseline = args.validation_dir / "servers" / version
    eula = baseline / "eula.txt"
    if "eula=true" not in eula.read_text().splitlines():
        raise RuntimeError(f"{version}: prior explicit Minecraft EULA acceptance is required")
    lines = (baseline / "server.properties").read_text().splitlines()
    for required in ["server-ip=127.0.0.1", "online-mode=false", "enable-rcon=false", "enable-query=false"]:
        if required not in lines:
            raise RuntimeError(f"{version}: baseline is missing isolated setting {required}")
    paper = next(row for row in paper_manifest["paper"] if row["version"] == version)
    paper_jar = args.validation_dir / "downloads" / paper["download"]["name"]
    if digest(paper_jar) != paper["download"]["checksums"]["sha256"]:
        raise RuntimeError("Paper SHA-256 differs from the pinned official artifact")
    mojang = next(row for row in paper_manifest["mojang"] if row["version"] == version)
    if digest(baseline / "cache" / mojang["name"]) != mojang["sha256"]:
        raise RuntimeError("Mojang cache SHA-256 differs from the pinned artifact")
    pin = next(row for row in grim_manifest["records"] if row["version_id"] == VERSIONS[version])
    if version not in pin["supported_test_versions"] or "paper" not in pin["loaders"]:
        raise RuntimeError("Grim does not advertise support for the selected native version")
    plugin = verified_plugin(args, pin)
    world = run_dir / version
    world.mkdir(exist_ok=False)
    shutil.copyfile(eula, world / "eula.txt")
    replacements = {
        "server-ip": "127.0.0.1", "server-port": "25565", "online-mode": "false",
        "enforce-secure-profile": "false", "enable-rcon": "false", "enable-query": "false",
        "enable-jmx-monitoring": "false", "gamemode": "survival", "force-gamemode": "true",
        "allow-flight": "false", "level-name": "grim-test-world", "level-seed": "731031",
        "max-players": "2", "spawn-animals": "false", "spawn-monsters": "false", "spawn-npcs": "false",
        "motd": f"Rustwire isolated headless Grim compatibility {version}",
    }
    updated = [line for line in lines if line.partition("=")[0] not in replacements]
    updated.extend(f"{key}={value}" for key, value in replacements.items())
    (world / "server.properties").write_text("\n".join(updated) + "\n")
    for name in ["bukkit.yml", "spigot.yml"]:
        if (baseline / name).is_file():
            shutil.copyfile(baseline / name, world / name)
    if (baseline / "config").is_dir():
        shutil.copytree(baseline / "config", world / "config")
    for name in ["cache", "libraries", "versions"]:
        if (baseline / name).is_dir():
            (world / name).symlink_to(baseline / name, target_is_directory=True)
    (world / "ops.json").write_text("[]\n")
    (world / "permissions.yml").write_text("{}\n")
    plugin_dir = world / "plugins" / "GrimAC"
    plugin_dir.mkdir(parents=True)
    shutil.copyfile(plugin, world / "plugins" / plugin.name)
    with zipfile.ZipFile(plugin) as jar:
        default_config = jar.read("config/en.yml")
        old, new = b"verbose:\n    print-to-console: false", b"verbose:\n    print-to-console: true"
        if default_config.count(old) != 1:
            raise RuntimeError("Unexpected upstream verbose configuration")
        config = default_config.replace(old, new)
        (plugin_dir / "config.yml").write_bytes(config)
        punishments = jar.read("punishments/en.yml")
        for embedded, local in [("punishments/en.yml", "punishments.yml"), ("messages/en.yml", "messages.yml"), ("discord/en.yml", "discord.yml")]:
            (plugin_dir / local).write_bytes(jar.read(embedded))
        properties = jar.read("grimac.properties").decode()
    java = str(args.validation_dir / "jdk25/bin/java") if version.startswith("26.") else "java"
    return world, {
        "version": version, "paper_build": paper["build"], "paper_channel": paper["channel"],
        "paper_jar_sha256": digest(paper_jar), "grim_version": pin["version"],
        "grim_channel": pin["release_type"], "grim_version_url": pin["version_page"],
        "grim_jar_sha256": digest(plugin), "grim_publisher_hashes": pin["publisher_hashes"],
        "grim_build_properties": properties, "config_changes": {"verbose.print-to-console": {"from": False, "to": True}},
        "config_sha256": hashlib.sha256(config).hexdigest(),
        "default_config_sha256": hashlib.sha256(default_config).hexdigest(),
        "punishments_sha256": hashlib.sha256(punishments).hexdigest(),
        "server_directory": str(world), "default_checks_and_thresholds_preserved": True,
        "server_command": [java, "-Xms256M", "-Xmx1024M", "-XX:ActiveProcessorCount=2", "-jar", str(paper_jar), "--nogui"],
        "baseline_kind": "headless Rustwire client on ordinary native Paper; no graphical vanilla comparison",
        "negative_control_kind": "bounded duplicate sprint state (BadPacketsF), not an invalid-movement simulation control",
        "started_utc": utc_now(), "launched": False, "passed": False, "commands": [],
    }


def listeners(require_present=True):
    found = []
    for path in ["/proc/net/tcp", "/proc/net/tcp6"]:
        for line in Path(path).read_text().splitlines()[1:]:
            fields = line.split()
            if fields[1].endswith(":63DD") and fields[3] == "0A":
                found.append(fields[1])
    allowed = {"0100007F:63DD", "0000000000000000FFFF00000100007F:63DD"}
    if any(value not in allowed for value in found) or (require_present and not found):
        raise RuntimeError(f"Server listener is not exclusively loopback: {found}")
    return found


def ensure_alive(processes, description):
    if any(process.poll() is not None for process in processes):
        raise RuntimeError(f"A process exited before {description}")


def wait_for(event, processes, timeout, description):
    deadline = time.monotonic() + timeout
    while not event.wait(0.1):
        ensure_alive(processes, description)
        if time.monotonic() >= deadline:
            raise RuntimeError(f"Timed out waiting for {description}")


def stop_process(process, polite=None):
    if process is None or process.poll() is not None:
        return
    if polite:
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


def scenario(version="1.21.1"):
    """Each tuple is name, console command, delay, evidence phase."""
    marker = lambda text: "tellraw Rustwire " + json.dumps({"text": text}, separators=(",", ":"))
    # 1.21.5 moved player armor/offhand serialization out of Inventory.
    offhand_path = "Inventory" if version == "1.21.1" else "equipment.offhand"
    return [
        ("paper_version", "version", 0.4, "setup"),
        ("plugins", "plugins", 0.4, "setup"),
        ("grim_version", "grim version", 0.5, "setup"),
        ("grim_profile", "grim profile Rustwire", 0.5, "setup"),
        ("grim_prediction_debug", "grim consoledebug Rustwire", 0.3, "setup"),
        ("floor", "fill -16 -61 -16 16 -61 16 minecraft:stone replace", 0.3, "setup"),
        ("clear_above_floor", "fill -16 -60 -16 16 -54 16 minecraft:air replace", 0.3, "setup"),
        ("teleport", "tp Rustwire 0.5 -60 0.5 0 0", 2.0, "settling"),
        ("wind_charge_inventory", "item replace entity Rustwire hotbar.0 with minecraft:wind_charge 8", 0.5, "settling"),
        ("initial_position", "data get entity Rustwire Pos", 0.3, "settling"),
        ("initial_inventory", "data get entity Rustwire Inventory", 0.3, "settling"),
        ("initial_survival", "data get entity Rustwire playerGameType", 0.3, "settling"),
        ("valid_begin", marker("GRIM_VALID_BEGIN"), 5.0, "valid_walk"),
        ("walk_position", "data get entity Rustwire Pos", 0.3, "valid_walk"),
        ("inventory_begin", marker("GRIM_INVENTORY"), 4.0, "valid_inventory"),
        ("swapped_inventory", f"data get entity Rustwire {offhand_path}", 0.3, "valid_inventory"),
        ("wind_begin", marker("GRIM_WIND"), 1.25, "valid_wind"),
        ("wind_entities", "execute at Rustwire if entity @e[type=minecraft:wind_charge,distance=..64]", 0.3, "valid_wind"),
        ("wind_entity_positions", "execute at Rustwire as @e[type=minecraft:wind_charge,distance=..64] run data get entity @s Pos", 0.3, "valid_wind"),
        ("wind_inventory", f"data get entity Rustwire {offhand_path}", 3.15, "valid_wind"),
        ("valid_end", marker("GRIM_VALID_END"), 2.0, "valid_tail"),
        ("negative_begin", marker("GRIM_NEGATIVE"), 2.0, "negative"),
        ("done", marker("GRIM_DONE"), 0.0, "done"),
    ]


def command_output(record, name):
    return "\n".join(next((row["server_output_in_window"] for row in record["commands"] if row["name"] == name), []))


def position(text):
    number = r"([-+\d.eE]+)"
    match = re.search(r"following entity data: \[" + number + r"d,\s*" + number + r"d,\s*" + number + r"d\]", text)
    return [float(value) for value in match.groups()] if match else None


def wind_inventory(text, offhand_query=False):
    """Plain wind-charge stacks have no nested custom components in this test."""
    entries = []
    for compound in re.findall(r"\{[^{}]*\}", text):
        if '"minecraft:wind_charge"' not in compound:
            continue
        slot = re.search(r"\bSlot:\s*(-?\d+)b", compound)
        count = re.search(r"\b(?:count|Count):\s*(\d+)b?", compound)
        if count and (slot or offhand_query):
            # Normalize the explicitly queried equipment.offhand to its legacy
            # inventory slot identity for cross-version result comparison.
            entries.append({"slot": int(slot.group(1)) if slot else -106, "count": int(count.group(1))})
    return entries


def assess(record, server_lines, client_lines):
    for index, command in enumerate(record["commands"]):
        end = record["commands"][index + 1]["sent_seconds"] if index + 1 < len(record["commands"]) else record["seconds"]
        command["server_output_in_window"] = [row["line"] for row in server_lines if command["sent_seconds"] <= row["seconds"] < end]
        command["console_failure_lines"] = [line for line in command["server_output_in_window"] if COMMAND_FAILURE.search(line)]
    flags = []
    for row in server_lines:
        match = FLAG.search(row["line"])
        if match:
            prior = [cmd for cmd in record["commands"] if cmd["sent_seconds"] <= row["seconds"]]
            phase = prior[-1]["phase"] if prior else "startup"
            flags.append({**row, **match.groupdict(), "vl": int(match.group("vl")), "phase": phase})
    record["all_flags"] = flags
    record["flags_by_phase"] = {phase: [flag for flag in flags if flag["phase"] == phase] for phase in sorted({flag["phase"] for flag in flags} | {"valid_walk", "valid_inventory", "valid_wind", "valid_tail", "negative"})}
    record["legitimate_action_flags"] = [flag for flag in flags if flag["phase"].startswith("valid_")]
    # Setup and startup are ordinary client behavior too. Never hide their
    # flags by starting the acceptance window later, or by dropping late flags.
    record["legitimate_flags"] = [flag for flag in flags if flag["phase"] != "negative"]
    record["negative_flags"] = [flag for flag in flags if flag["phase"] == "negative"]
    record["console_failures"] = [{"command": cmd["name"], "lines": cmd["console_failure_lines"]} for cmd in record["commands"] if cmd["console_failure_lines"]]
    record["runtime_failure_lines"] = [row for row in server_lines if RUNTIME_FAILURE.search(row["line"])]
    record["observed_client_actions"] = sorted({phase for row in client_lines for phase in ACTION.findall(row["line"])})
    start = position(command_output(record, "initial_position"))
    end = position(command_output(record, "walk_position"))
    delta = None if start is None or end is None else [b - a for a, b in zip(start, end)]
    initial_inventory = wind_inventory(command_output(record, "initial_inventory"))
    offhand_query = record.get("version", "1.21.1") != "1.21.1"
    swapped_inventory = wind_inventory(command_output(record, "swapped_inventory"), offhand_query)
    after_wind_inventory = wind_inventory(command_output(record, "wind_inventory"), offhand_query)
    count_match = re.search(r"Test passed[,.]\s+count:\s*(\d+)", command_output(record, "wind_entities"), re.IGNORECASE)
    wind_count = int(count_match.group(1)) if count_match else None
    profile_output = command_output(record, "grim_profile")
    version_output = command_output(record, "grim_version")
    record["server_action_confirmation"] = {
        "initial_position": start, "walk_position": end, "walk_delta": delta,
        "initial_inventory": initial_inventory, "swapped_inventory": swapped_inventory,
        "after_wind_inventory": after_wind_inventory, "wind_entity_count": wind_count,
        "offhand_nbt_location": "equipment.offhand" if offhand_query else "Inventory[{Slot:-106b}]",
    }
    checks = {
        "server_clean_exit": record["server_exit"] == 0,
        "client_clean_exit": record["client_exit"] == 0,
        "ready_seen": record.get("ready_seen", False),
        "all_scenario_commands_sent": len(record["commands"]) == len(scenario(record.get("version", "1.21.1"))),
        "plugin_version_confirmed": "Grim Version:" in version_output and record["grim_version"] in version_output,
        "player_tracked": "Profile for Rustwire" in profile_output and "Version:" in profile_output and "exempt or offline" not in profile_output,
        "prediction_debug_enabled": "Console output for Rustwire is now enabled" in command_output(record, "grim_prediction_debug"),
        "survival_confirmed": bool(re.search(r"following entity data:\s*0\s*$", command_output(record, "initial_survival"), re.MULTILINE)),
        "no_ops_or_permission_grants": record.get("no_ops_or_permission_grants", False),
        "check_configuration_unchanged": record.get("check_configuration_unchanged", False),
        "no_flags_outside_negative_control": not record["legitimate_flags"],
        "negative_control_flagged": any(flag["check"].replace(" ", "").rstrip("*") == "BadPacketsF" for flag in record["negative_flags"]),
        "all_client_actions_observed": {"walk", "inventory", "wind", "negative"}.issubset(record["observed_client_actions"]),
        "walking_confirmed": delta is not None and all(math.isfinite(value) for value in delta) and math.hypot(delta[0], delta[2]) > 0.05 and abs(delta[1]) < 0.1,
        "initial_wind_stack_confirmed": {"slot": 0, "count": 8} in initial_inventory,
        "offhand_swap_confirmed": {"slot": -106, "count": 8} in swapped_inventory,
        "two_wind_uses_confirmed": {"slot": -106, "count": 6} in after_wind_inventory,
        "wind_entities_confirmed": wind_count is not None and wind_count >= 2,
        "no_console_command_errors": not record["console_failures"],
        "no_runtime_errors": not record["runtime_failure_lines"],
    }
    record["acceptance_checks"] = checks
    record["failed_checks"] = [name for name, passed in checks.items() if not passed]
    record["passed"] = not record.get("error") and not record["failed_checks"]


def run_server(args, world, record, client_binary):
    server = client = None
    readers, server_lines, client_lines = [], [], []
    server_ready, joined, probe_ready = threading.Event(), threading.Event(), threading.Event()
    unexpected_flag = threading.Event()
    started = time.monotonic()
    record["client_binary_sha256"] = digest(client_binary)
    record["client_command"] = [str(client_binary), "127.0.0.1", "25565", record["version"]]

    def read_output(process, label, timeline):
        with (world / f"grim-{label}.log").open("w") as output:
            for raw in process.stdout:
                output.write(raw)
                output.flush()
                line = ANSI.sub("", raw).strip()
                timeline.append({"seconds": round(time.monotonic() - started, 6), "utc": utc_now(), "line": line})
                if label == "server":
                    if "Done (" in line:
                        server_ready.set()
                    if "Rustwire joined the game" in line:
                        joined.set()
                    if FLAG.search(line) or "joined the game" in line or "lost connection" in line:
                        print(f"{record['version']} {line}", flush=True)
                    if FLAG.search(line):
                        phase = record["commands"][-1]["phase"] if record["commands"] else "startup"
                        if phase != "negative":
                            unexpected_flag.set()
                else:
                    if "GRIM_PROBE_READY" in line:
                        probe_ready.set()
                    if "GRIM_ACTION" in line or "GRIM_PROBE" in line:
                        print(f"{record['version']} {line}", flush=True)

    def start_reader(process, label, timeline):
        reader = threading.Thread(target=read_output, args=(process, label, timeline), daemon=True)
        reader.start()
        readers.append(reader)

    def check_running(description):
        if unexpected_flag.is_set():
            raise RuntimeError("Unexpected Grim flag outside the negative-control window; stopping for diagnosis")
        ensure_alive([server, client], description)

    try:
        if listeners(require_present=False):
            raise RuntimeError("Port 25565 already has a listener; refusing to overlap tests")
        java = record["server_command"][0]
        resolved_java = Path(shutil.which(java) or java).resolve()
        runtime = subprocess.run([str(resolved_java), "-version"], capture_output=True, text=True, timeout=10, check=True)
        record["java_runtime"] = {
            "executable": str(resolved_java), "executable_sha256": digest(resolved_java),
            "version_output": (runtime.stdout + runtime.stderr).strip(),
        }
        java_release = resolved_java.parent.parent / "release"
        if java_release.is_file():
            record["java_runtime"]["release_metadata"] = java_release.read_text()
        print(f"=== GRIM START {record['version']}: {record['grim_version']} ({record['grim_channel']}) ===", flush=True)
        server = subprocess.Popen(record["server_command"], cwd=world, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        record["launched"] = True
        start_reader(server, "server", server_lines)
        wait_for(server_ready, [server], 180, "Paper/Grim startup")
        record["listeners"] = listeners()
        client = subprocess.Popen(record["client_command"], cwd=world, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
        client_started = time.monotonic()
        start_reader(client, "client", client_lines)
        wait_for(joined, [server, client], 40, "server-side player join")
        wait_for(probe_ready, [server, client], 40, "initial teleport acknowledgement")
        record["ready_seen"] = True
        for name, command, delay, phase in scenario(record["version"]):
            check_running(name)
            record["commands"].append({"name": name, "command": command, "phase": phase, "sent_seconds": round(time.monotonic() - started, 6), "sent_utc": utc_now()})
            print(f"console {record['version']}: {command}", flush=True)
            server.stdin.write(command + "\n")
            server.stdin.flush()
            deadline = time.monotonic() + delay
            while time.monotonic() < deadline:
                check_running(name)
                if time.monotonic() - client_started > args.client_timeout:
                    raise RuntimeError("Bounded client timeout exceeded")
                time.sleep(min(0.1, max(0, deadline - time.monotonic())))
        remaining = args.client_timeout - (time.monotonic() - client_started)
        if remaining <= 0:
            raise RuntimeError("Bounded client timeout exceeded")
        client.wait(timeout=remaining)
        record["client_seconds"] = round(time.monotonic() - client_started, 3)
        time.sleep(0.5)
    except Exception as exc:
        record["error"] = f"{type(exc).__name__}: {exc}"
        print(f"GRIM FAIL {record['version']}: {record['error']}", flush=True)
    finally:
        stop_process(client)
        stop_process(server, "stop")
        for reader in readers:
            reader.join(timeout=5)
        record["server_exit"] = None if server is None else server.returncode
        record["client_exit"] = None if client is None else client.returncode
        record["seconds"] = round(time.monotonic() - started, 6)
        record["server_timeline"] = server_lines
        record["client_timeline"] = client_lines
        record["no_ops_or_permission_grants"] = json.loads((world / "ops.json").read_text()) == [] and (world / "permissions.yml").read_text().strip() in ("", "{}")
        config_dir = world / "plugins/GrimAC"
        record["check_configuration_unchanged"] = digest(config_dir / "config.yml") == record["config_sha256"] and digest(config_dir / "punishments.yml") == record["punishments_sha256"]
        assess(record, server_lines, client_lines)
        (world / "grim-result.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"=== GRIM STOP {record['version']}: passed={record['passed']}, failed={record['failed_checks']} ===", flush=True)
    return record


def main():
    args = arguments()
    if not args.validation_dir.is_dir():
        raise RuntimeError("Prepare the approved baseline Paper workspace first")
    paper = json.loads((PROJECT / "docs/validation/matrix-download-provenance.json").read_text())
    grim = json.loads((PROJECT / "docs/validation/grim-download-provenance.json").read_text())
    with (args.validation_dir / "one-server.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
        run_dir = args.grim_dir / "runs" / stamp
        run_dir.mkdir(parents=True)
        results = {"scenario": args.mode, "created_utc": utc_now(), "prepare_only": args.prepare_only, "records": []}
        (run_dir / "grim-download-provenance.json").write_text(json.dumps(grim, indent=2) + "\n")
        client_binary = None
        if not args.prepare_only:
            (run_dir / "client-bin").mkdir()
            client_binary = run_dir / "client-bin/grim_probe"
            shutil.copyfile(args.client, client_binary)
            client_binary.chmod(0o755)
        for version in args.versions:
            try:
                world, record = prepare_server(args, run_dir, version, paper, grim)
                if not args.prepare_only:
                    record = run_server(args, world, record, client_binary)
                else:
                    record["prepared"] = True
                    print(f"Prepared {version}; no server launched", flush=True)
            except Exception as exc:
                record = {"version": version, "passed": False, "prepared": False, "error": f"{type(exc).__name__}: {exc}"}
                print(f"GRIM PREPARATION FAIL {version}: {record['error']}", flush=True)
            results["records"].append(record)
            (run_dir / "grim-results.json").write_text(json.dumps(results, indent=2) + "\n")
        (args.grim_dir / "latest-run.txt").write_text(str(run_dir) + "\n")
        print(f"Grim evidence: {run_dir / 'grim-results.json'}", flush=True)
        key = "prepared" if args.prepare_only else "passed"
        return 0 if all(record.get(key, False) for record in results["records"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
