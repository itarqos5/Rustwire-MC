#!/usr/bin/env python3
"""Verify border-duration units from cached release APIs; no servers or downloads.

Only semantic facts and input hashes are retained, never implementation text.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

RELEASES = ['1.20.1', '1.20.2', '1.20.4', '1.20.6', '1.21.1', '1.21.3',
            '1.21.4', '1.21.5', '1.21.6', '1.21.8', '1.21.10', '1.21.11',
            '26.1.2', '26.2']


def classify(command, extent, protocol):
    facts = {
        'command_uses_tick_time_argument': 'TimeArgument.time:' in command,
        'command_multiplies_by_1000': '// long 1000l' in command,
        'extent_uses_wall_clock_milliseconds': 'Util.getMillis:' in extent or 'SystemUtils.b:()J' in extent,
        'extent_retains_tick_progress': 'lerpProgress:J' in extent,
    }
    ticks = protocol >= 774
    assert facts == {
        'command_uses_tick_time_argument': ticks,
        'command_multiplies_by_1000': not ticks,
        'extent_uses_wall_clock_milliseconds': not ticks,
        'extent_retains_tick_progress': ticks,
    }, (protocol, facts)
    return {'duration_unit': 'ticks' if ticks else 'milliseconds', **facts}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    records = []
    for protocol, version in enumerate(RELEASES, 763):
        jars = list((args.runtime_root / 'servers' / version / 'versions').glob('**/*.jar'))
        if len(jars) != 1:
            raise ValueError(f'exactly one prepared release jar required: {version}')
        jar = jars[0]
        def inspect(name):
            return subprocess.check_output([str(args.runtime_root / 'jdk25/bin/javap'),
                '-p', '-c', '-classpath', str(jar), name], text=True)
        command = inspect('net.minecraft.server.commands.' + ('CommandWorldBorder' if protocol < 766 else 'WorldBorderCommand'))
        extent = inspect('net.minecraft.world.level.border.WorldBorder$' + ('b' if protocol < 766 else 'MovingBorderExtent'))
        records.append({'version': version, 'protocol': protocol,
            'prepared_server_sha256': hashlib.sha256(jar.read_bytes()).hexdigest(),
            **classify(command, extent, protocol)})
    args.output.write_text(json.dumps({'verifier_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'scope': 'cached release command/extent unit facts; no live-server claim',
        'records': records}, indent=2) + '\n')
    print(f'PASS: exact border-duration units in {len(records)} release families')


if __name__ == '__main__':
    main()
