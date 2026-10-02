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
MANIFEST = PROJECT / "docs/validation/matrix-download-provenance.json"
BUILDS = {item["version"]: item["build"] for item in json.loads(MANIFEST.read_text())["paper"]}
PROTOCOLS = dict(zip(BUILDS, range(763, 777)))
COMMAND_SOURCES = [
    "https://www.minecraft.net/en-us/article/minecraft-java-edition-1-20-5",
    "https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21",
    "https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-2",
    "https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-5",
]
ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
EVENT = re.compile(r"\bEVENT\s+category=([a-zA-Z0-9_]+)")
ACTION = re.compile(r"\bACTION\s+(?:kind|category)=([a-zA-Z0-9_]+)")
DEFAULT_CATEGORIES = ["entity_spawn", "entity_move", "entity_remove", "block_update",
                      "health", "inventory", "system_chat", "respawn", "keepalive",
                      "item_damage", "item_custom", "chat_marker", "metadata",
                      "death", "respawn_requested", "movement_sent", "movement_after_ready", "selected_slot_sent"]
COMMAND_FAILURE = re.compile(
    r"Unknown or incomplete command|Incorrect argument|<--\[HERE\]|"
    r"No (?:entity|player) was found|Unable to summon|Target is invulnerable|"
    r"Could not set the block|Nothing changed|has no modifier|Malformed .+ component|(?:Expected|Invalid).+at position",
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
    parser.add_argument("--source-commit", help="Exact archived source commit used to build the frozen client")
    parser.add_argument("--extended-components", action="store_true", help="Also validate typed ordinary components, particle metadata, and modern hash controls")
    parser.add_argument("--expanded-state", action="store_true", help="Validate roster lifecycle, equipment, modifiers, effects and newly completed component layouts")
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
    if args.source_commit is not None and not re.fullmatch(r"[0-9a-f]{40}", args.source_commit):
        parser.error("--source-commit must be an exact 40-character commit SHA")
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


def scenario_commands(version, extended=False, expanded=False):
    protocol = PROTOCOLS[version]
    if protocol < 766:
        sword = "minecraft:diamond_sword{Damage:3}"
        stone = "minecraft:stone{rustwire_probe:1}"
    else:
        sword = "minecraft:diamond_sword[minecraft:damage=3]"
        stone = "minecraft:stone[minecraft:custom_data={rustwire_probe:1}]"
    commands = [
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

    if extended and protocol >= 766:
        extra = []
        if protocol >= 770:
            extra.extend([
                ("hash_bad", 'tellraw Rustwire {"text":"RUSTWIRE_HASH_BAD"}', 1.25),
                ("hash_bad_end", 'tellraw Rustwire {"text":"RUSTWIRE_HASH_BAD_END"}', 0.25),
                ("verify_hash_bad_move", 'data get entity Rustwire Inventory', 0.5),
                ("hash_good", 'tellraw Rustwire {"text":"RUSTWIRE_HASH_GOOD"}', 1.25),
                ("hash_good_end", 'tellraw Rustwire {"text":"RUSTWIRE_HASH_GOOD_END"}', 0.25),
                ("verify_hash_good_move", 'data get entity Rustwire Inventory', 0.5),
            ])
        items = {
            "food": 'minecraft:apple[minecraft:food={nutrition:5,saturation:0.3,can_always_eat:true}]',
            "potion": 'minecraft:potion[minecraft:potion_contents={custom_color:123,custom_effects:[{id:"minecraft:speed",duration:100,amplifier:1}]}]',
            "stew": 'minecraft:suspicious_stew[minecraft:suspicious_stew_effects=[{id:"minecraft:speed",duration:100}]]',
            "writable_book": 'minecraft:writable_book[minecraft:writable_book_content={pages:["Rustwire page"]}]',
            "written_book": 'minecraft:written_book[minecraft:written_book_content={title:"Rustwire",author:"Rustwire",pages:["Rustwire written"]}]',
            "fireworks": 'minecraft:firework_rocket[minecraft:fireworks={flight_duration:1,explosions:[{shape:"small_ball",colors:[123],has_trail:true}]}]',
        }
        if protocol >= 775:
            items.update({
                "template_remainder": 'minecraft:apple[minecraft:use_remainder={id:"minecraft:stone",count:2}]',
                "template_projectiles": 'minecraft:crossbow[minecraft:charged_projectiles=[{id:"minecraft:arrow",count:1}]]',
                "template_bundle": 'minecraft:bundle[minecraft:bundle_contents=[{id:"minecraft:stone",count:2}]]',
                "template_container": 'minecraft:shulker_box[minecraft:container=[{slot:2,item:{id:"minecraft:stone",count:2}}]]',
            })
            if version == "26.2":
                items["template_sulfur"] = 'minecraft:stone[minecraft:sulfur_cube_content={id:"minecraft:stone",count:2}]'
        if protocol < 770:
            # Pre-1.21.5 item commands use JSON-encoded text-component strings.
            page = json.dumps(json.dumps({"text": "Rustwire written"}, separators=(",", ":")))
            items["written_book"] = 'minecraft:written_book[minecraft:written_book_content={title:"Rustwire",author:"Rustwire",pages:[' + page + ']}]'
        extra.extend(("component_"+kind, "give Rustwire "+item+" 1", 0.75) for kind,item in items.items())
        extra.extend([
            ("particle_effect", "effect give Rustwire minecraft:speed 2 0 false", 1.0),
            ("wolf_holder", 'execute at Rustwire run summon minecraft:wolf ~3 ~ ~ {NoAI:1b,Tags:["rustwire_holder"]}', 1.0),
            ("wolf_remove", 'execute at Rustwire run kill @e[type=minecraft:wolf,tag=rustwire_holder,distance=..8,limit=1]', 0.75),
        ])
        commands[5:5] = extra
    if expanded:
        attribute = "minecraft:generic.movement_speed" if protocol < 768 else "minecraft:movement_speed"
        modifier = '00000000-0000-0000-0000-000000000001 "rustwire_probe" 0.125 ' + ('add' if protocol < 766 else 'add_value') if protocol < 767 else 'rustwire:probe 0.125 add_value'
        removal = "00000000-0000-0000-0000-000000000001" if protocol < 767 else "rustwire:probe"
        extra = [
            ("attribute_modifier_add", f"attribute Rustwire {attribute} modifier add {modifier}", 0.75),
            ("attribute_modifier_remove", f"attribute Rustwire {attribute} modifier remove {removal}", 0.5),
            ("effect_add", "effect give Rustwire minecraft:speed 60 2 false", 0.75),
            ("effect_remove", "effect clear Rustwire minecraft:speed", 0.75),
        ]
        if protocol >= 766:
            predicate = '{blocks:"minecraft:oak_log",state:{axis:"x"},nbt:"{rustwire_probe:1}"}'
            if protocol < 770:
                predicate = '{predicates:[' + predicate + '],show_in_tooltip:false}'
            items = {
                "block_predicate": 'minecraft:stick[minecraft:can_break=' + predicate + ']',
                "block_tag": 'minecraft:stick[minecraft:can_place_on={blocks:"#minecraft:logs"}]',
                "trim": 'minecraft:diamond_chestplate[minecraft:trim={material:"minecraft:quartz",pattern:"minecraft:sentry"}]',
                "banner": 'minecraft:white_banner[minecraft:banner_patterns=[{pattern:"minecraft:stripe_top",color:"red"}]]',
                # An explicit property-only profile on stone never requests account/skin resolution.
                "profile": 'minecraft:stone[minecraft:profile={properties:[{name:"rustwire_probe",value:"local"}]}]',
                "instrument": 'minecraft:stone[minecraft:instrument="minecraft:ponder_goat_horn"]',
            }
            instrument = '{sound_event:{sound_id:"rustwire:probe",range:12.0},range:32.0,use_duration:'
            instrument += '140}' if protocol < 768 else '3.5,description:' + (json.dumps(json.dumps({"text": "Rustwire horn"}, separators=(",", ":"))) if protocol < 770 else '{text:"Rustwire horn"}') + '}'
            items["instrument"] = 'minecraft:stone[minecraft:instrument=' + instrument + ']'
            if protocol >= 767:
                items["jukebox"] = 'minecraft:music_disc_13[minecraft:jukebox_playable=' + ('{song:"minecraft:cat"}' if protocol < 770 else '"minecraft:cat"') + ']'
            if protocol >= 768:
                items.update({
                    "consumable": 'minecraft:apple[minecraft:consumable={consume_seconds:3.0,animation:"eat",has_consume_particles:false,on_consume_effects:[{type:"minecraft:clear_all_effects"}]}]',
                    "equippable": 'minecraft:stone[minecraft:equippable={slot:"head",dispensable:false,swappable:false,damage_on_hurt:false}]',
                    "use_cooldown": 'minecraft:stone[minecraft:use_cooldown={seconds:1.5,cooldown_group:"rustwire:probe"}]',
                    "death_protection": 'minecraft:stick[minecraft:death_protection={death_effects:[{type:"minecraft:clear_all_effects"}]}]',
                })
            if protocol >= 770:
                items.update({
                    "component_matchers": 'minecraft:stick[minecraft:can_break={blocks:"minecraft:chest",components:{"minecraft:custom_data":{rustwire_probe:1}},predicates:{"minecraft:custom_data":{rustwire_probe:1}}}]',
                    "weapon": 'minecraft:stick[minecraft:weapon={item_damage_per_attack:2,disable_blocking_for_seconds:3.5}]',
                    "blocks_attacks": 'minecraft:stick[minecraft:blocks_attacks={block_delay_seconds:0.75,disable_cooldown_scale:1.5}]',
                })
            extra.extend(("component_" + kind, "give Rustwire " + item + " 1", 0.5) for kind, item in items.items())
        commands[5:5] = extra
        helmet = "minecraft:diamond_helmet{Damage:7}" if protocol < 766 else "minecraft:diamond_helmet[minecraft:damage=7]"
        target = "@e[type=minecraft:armor_stand,tag=rustwire_probe,distance=..8,limit=1,sort=nearest]"
        spawn = next(i for i, c in enumerate(commands) if c[0] == "entity_spawn")
        commands.insert(spawn + 1, ("entity_equipment", f"execute at Rustwire run item replace entity {target} armor.head with {helmet} 1", 1.0))
    commands.insert(0, ("client_move_after_ready", 'tellraw Rustwire {"text":"RUSTWIRE_MOVE_AFTER_READY"}', 1.0))
    return commands


def required_values(version):
    p = PROTOCOLS[version]
    values = {
        "movement_after_ready": {"delta_x": "0.125", "server_ready": "true"},
        "player_self_add": {"name": "Rustwire", "listed": "true", "mode": "Creative"},
        "player_peer_add": {"name": "RustwirePeer", "listed": "true"},
        "player_peer_remove": {"name": "RustwirePeer", "matched_added_uuid": "true"},
        "player_mode_update": {"name": "Rustwire", "mode": "Survival"},
        "entity_equipment": {"slot": "Head", "damage": "7", "count": "1"},
        "entity_attribute_modifier": {"amount": "0.125", "operation": "AddValue", "own_entity": "true"},
        "entity_effect_add": {"amplifier": "2", "visible": "true", "icon": "true", "own_entity": "true"},
        "entity_effect_remove": {"matched_added_effect": "true", "own_entity": "true"},
    }
    if p >= 766:
        values.update({
            "item_block_predicate": {"component": "can_break", "axis": "x", "nbt_marker": "1", "block_ids": "1"},
            "item_block_tag": {"component": "can_place_on", "tag": "minecraft:logs"},
            "item_trim": {"material": "minecraft:quartz", "pattern": "minecraft:sentry"},
            "item_banner": {"pattern": "minecraft:stripe_top", "color": "14"},
            "item_profile": {"property": "rustwire_probe", "value": "local", "signed": "false"},
            "item_instrument": {"inline": "true", "duration": "140" if p < 768 else "3.5", "unit": "ticks" if p < 768 else "seconds", "range": "32", "sound": "rustwire:probe", "sound_range": "12"},
        })
    if p >= 767:
        values["item_jukebox"] = {"song": "minecraft:cat"}
    if p >= 768:
        values.update({
            "item_consumable": {"seconds": "3", "animation": "Eat", "particles": "false", "effect": "ClearAllEffects"},
            "item_equippable": {"slot": "Head", "dispensable": "false", "swappable": "false", "damage_on_hurt": "false"},
            "item_use_cooldown": {"seconds": "1.5", "group": "rustwire:probe"},
            "item_death_protection": {"effect": "ClearAllEffects"},
        })
    if p >= 770:
        values.update({
            "item_component_matchers": {"exact_custom_data": "1", "partial_custom_data": "1"},
            "item_weapon": {"damage_per_attack": "2", "disable_seconds": "3.5"},
            "item_blocks_attacks": {"delay_seconds": "0.75", "cooldown_scale": "1.5"},
        })
    return values


def assess_values(version, lines):
    observations = {}
    for line in lines:
        if line.startswith("VALUE "):
            fields = dict(re.findall(r"([a-z_]+)=([^\s]+)", line))
            observations.setdefault(fields.pop("category", ""), []).append(fields)
    required = required_values(version)
    missing = []
    for category, expected in required.items():
        matches = [row for row in observations.get(category, []) if all(row.get(k) == v for k, v in expected.items())]
        if category == "entity_effect_add":
            matches = [row for row in matches if row.get("duration", "").isdigit() and 1100 < int(row["duration"]) <= 1200
                       and row.get("effect_id", "").isdigit()]
        if category == "entity_equipment":
            matches = [row for row in matches if row.get("entity", "").isdigit() and row.get("item_id", "").isdigit()]
        if not matches:
            missing.append(category)
    return {"observations": observations, "expected": required, "missing_or_wrong": sorted(missing), "passed": not missing}


def assess_protocol_transcript(version, lines, expanded=False, peer_lines=(), peer_exit=None):
    result = {
        "unsupported_packet_observations": [line for line in lines if line.startswith("UNSUPPORTED ")],
        "raw_scenario_packet_observations": [line for line in lines if line.startswith("RAW_SCENARIO ")],
    }
    result["passed"] = not result["unsupported_packet_observations"] and not result["raw_scenario_packet_observations"]
    if expanded:
        result["value_checks"] = assess_values(version, lines)
        result["peer_lifecycle_passed"] = (peer_exit == 0 and "PEER_READY" in peer_lines and "PEER_DONE" in peer_lines
                                           and not any(line.startswith(("UNSUPPORTED ", "RAW_SCENARIO ", "Error:")) for line in peer_lines))
        result["passed"] &= result["value_checks"]["passed"] and result["peer_lifecycle_passed"]
    return result


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
    java = str(root / "jdk25/bin/java") if version.startswith("26.") else "java"
    jar = root / "downloads" / f"paper-{version}-{BUILDS[version]}.jar"
    manifest_path = PROJECT / "docs/validation/matrix-download-provenance.json"
    if not manifest_path.is_file():
        manifest_path = PROJECT / "docs/validation/download-provenance.json"
    provenance = json.loads(manifest_path.read_text())
    expected = next(item["download"]["checksums"]["sha256"]
                    for item in provenance["paper"] if item["version"] == version)
    jar_sha = hashlib.sha256(jar.read_bytes()).hexdigest()
    if jar_sha != expected:
        raise RuntimeError(f"{version}: Paper JAR SHA-256 differs from the pinned official artifact")
    server_command = [java, "-Xms256M", "-Xmx1024M", "-XX:ActiveProcessorCount=2", "-jar", str(jar), "--nogui"]
    record = {"version": version, "protocol": PROTOCOLS[version], "paper_build": BUILDS[version], "started_utc": utc_now(),
              "scenario": args.mode, "extended_components": args.extended_components, "expanded_state": args.expanded_state, "server_command": server_command,
              "server_jar_sha256": jar_sha,
              "client_binary_sha256": hashlib.sha256(client_bin.read_bytes()).hexdigest(),
              "commands": [], "observed_categories": [], "passed": False}
    ready, joined, probe_ready, movement_ready = (threading.Event() for _ in range(4))
    server_lines, client_lines, client_timeline, categories = [], [], [], set()
    actions = set()
    started = time.monotonic()
    server = client = peer = None
    peer_lines = []
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
                    if "movement_after_ready" in line_categories:
                        movement_ready.set()
                    if not line_categories or first_observation:
                        print(f"client {version}: {line}", flush=True)

        client_reader = threading.Thread(target=read_client, daemon=True)
        readers.append(client_reader)
        client_reader.start()
        wait_for_event(joined, [server, client], 45, "Rustwire server-side world entry")
        wait_for_event(probe_ready, [server, client], 45, "typed probe spawn readiness")
        time.sleep(0.75)
        if args.expanded_state:
            peer_ready = threading.Event()
            peer = subprocess.Popen([str(client_bin), "127.0.0.1", "25565", version, "120", "--peer"], cwd=world,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
            def read_peer():
                with (world / "gameplay-peer.log").open("w") as log:
                    for raw in peer.stdout:
                        log.write(raw)
                        log.flush()
                        line = ANSI.sub("", raw).strip()
                        peer_lines.append(line)
                        if line == "PEER_READY":
                            peer_ready.set()
            peer_reader = threading.Thread(target=read_peer, daemon=True)
            readers.append(peer_reader)
            peer_reader.start()
            wait_for_event(peer_ready, [server, client, peer], 45, "loopback peer readiness")
            time.sleep(0.5)
            server.stdin.write('tellraw RustwirePeer {"text":"RUSTWIRE_PEER_DONE"}\n')
            server.stdin.flush()
            peer.wait(timeout=15)
            peer_reader.join(timeout=5)
            if peer.returncode != 0 or "PEER_DONE" not in peer_lines:
                raise RuntimeError("Roster peer did not complete its bounded lifecycle")
            time.sleep(0.5)
        commands = scenario_commands(version, args.extended_components, args.expanded_state)
        for name, command, pause in commands:
            if server.poll() is not None or client.poll() is not None:
                raise RuntimeError(f"A process exited before scenario step {name}")
            record["commands"].append({"name": name, "command": command,
                                       "sent_seconds": round(time.monotonic() - started, 3)})
            print(f"console {version}: {command}", flush=True)
            server.stdin.write(command + "\n")
            server.stdin.flush()
            if name == "client_move_after_ready":
                wait_for_event(movement_ready, [server, client], 10, "the client movement-sent marker after setup")
            if name == "verify_client_position":
                # Console execution and socket processing are asynchronous. Poll
                # only this test player's position, with a strict total bound.
                deadline = time.monotonic() + 8
                retry_at = time.monotonic() + 0.5
                entry = record["commands"][-1]
                entry["retry_sent_seconds"] = []
                while True:
                    observed = [item["line"] for item in server_lines if item["seconds"] >= entry["sent_seconds"]]
                    confirmation = client_action_confirmation(server_lines, [{"name": name, "server_output_in_window": observed}])
                    if confirmation["movement_confirmed"]:
                        entry["movement_poll_confirmed"] = True
                        break
                    if server.poll() is not None or client.poll() is not None:
                        raise RuntimeError("A process exited while confirming client movement")
                    if time.monotonic() >= deadline:
                        raise RuntimeError("The server did not confirm the post-readiness client movement within eight seconds")
                    if time.monotonic() >= retry_at:
                        entry["retry_sent_seconds"].append(round(time.monotonic() - started, 3))
                        server.stdin.write(command + "\n")
                        server.stdin.flush()
                        retry_at = time.monotonic() + 0.5
                    time.sleep(0.05)
            else:
                time.sleep(pause)
        remaining = args.client_timeout - (time.monotonic() - client_started)
        if remaining <= 0:
            raise RuntimeError("Scenario exceeded the bounded client timeout")
        client.wait(timeout=remaining)
        record["client_seconds"] = round(time.monotonic() - client_started, 3)
        client_reader.join(timeout=5)
        record["observed_categories"] = sorted(categories)
        required=set(args.required_category)
        protocol = PROTOCOLS[version]
        if args.extended_components and protocol >= 766:
            required.update(["item_food", "item_potion", "item_stew", "item_writable_book", "item_written_book", "item_fireworks", "metadata_particles", "metadata_particles_nonempty"])
            if protocol >= 770:
                required.update(["hash_negative_control", "hash_positive_control"])
            if protocol >= 775:
                required.update(["item_template_remainder", "item_template_projectiles", "item_template_bundle", "item_template_container"])
            if version == "26.2":
                required.add("item_template_sulfur")
        if args.expanded_state:
            required.update(required_values(version))
        record["required_categories"] = sorted(required)
        record["missing_categories"] = sorted(required - categories)
        record["scenario_commands_sent"] = len(record["commands"]) == len(commands)
        record["passed"] = client.returncode == 0 and not record["missing_categories"] and record["scenario_commands_sent"]
        if not record["passed"]:
            record["error"] = f"Probe exit {client.returncode}; missing categories {record['missing_categories']}"
    except Exception as exc:
        record["error"] = f"{type(exc).__name__}: {exc}"
        print(f"GAMEPLAY FAIL {version}: {record['error']}", flush=True)
    finally:
        stop_process(peer)
        stop_process(client)
        stop_process(server, "stop")
        for reader in readers:
            reader.join(timeout=5)
        record["server_exit"] = None if server is None else server.returncode
        record["peer_exit"] = None if peer is None else peer.returncode
        record["peer_output"] = peer_lines
        record["client_exit"] = None if client is None else client.returncode
        record["observed_categories"] = sorted(categories)
        record["observed_client_actions"] = sorted(actions)
        record["client_output"] = client_lines
        record["client_timeline"] = client_timeline
        transcript = assess_protocol_transcript(version, client_lines, args.expanded_state, peer_lines, record["peer_exit"])
        for key, value in transcript.items():
            if key != "passed":
                record[key] = value
        if not transcript["passed"]:
            record["passed"] = False
            record.setdefault("error", "Missing/incorrect decoded values, unsupported/Raw scenario packets, or incomplete roster peer lifecycle")
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
        source_paths = ["examples/gameplay_probe.rs", "examples/gameplay_probe/expanded.rs", "examples/gameplay_probe/hash_probe.rs", "tools/paper/validate_gameplay.py"]
        results = {"source_base_commit": args.source_commit or subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=PROJECT, text=True).strip(),
                   "library_source_sha256": {str(path.relative_to(PROJECT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted((PROJECT / "src").rglob("*.rs"))},
                   "cargo_source_sha256": {name: hashlib.sha256((PROJECT / name).read_bytes()).hexdigest() for name in ["Cargo.toml", "Cargo.lock"]},
                   "source_sha256": {path: hashlib.sha256((PROJECT / path).read_bytes()).hexdigest() for path in source_paths},
                   "scenario": args.mode, "created_utc": utc_now(),
                   "required_categories": args.required_category, "command_sources": COMMAND_SOURCES, "records": []}
        for version in args.versions:
            results["records"].append(run_version(args, run_dir, client_bin, version))
            (run_dir / "gameplay-results.json").write_text(json.dumps(results, indent=2) + "\n")
        (root / "gameplay" / "latest-run.txt").write_text(str(run_dir) + "\n")
        print(f"Gameplay evidence: {run_dir / 'gameplay-results.json'}", flush=True)
        return 0 if all(record["passed"] for record in results["records"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
