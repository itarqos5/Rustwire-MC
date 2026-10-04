#!/usr/bin/env python3
"""Build the HUD/control probe and bind the binary to an unchanged source snapshot."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import validate_hud_controls as live


def artifact_executable(output, name="hud_control_probe"):
    paths = []
    for line in output.splitlines():
        artifact = json.loads(line)
        if (artifact.get("reason") == "compiler-artifact"
                and artifact.get("target", {}).get("name") == name
                and "example" in artifact.get("target", {}).get("kind", [])
                and artifact.get("executable")):
            paths.append(Path(artifact["executable"]))
    if len(paths) != 1 or not paths[0].is_absolute() or not paths[0].is_file():
        raise RuntimeError("Cargo did not identify exactly one existing probe executable")
    return paths[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--target-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    source, target = args.source_root.resolve(), args.target_dir.resolve()
    live.verify_source_snapshot(source, args.source_commit)
    before = live.source_hashes(source)
    command = ["cargo", "build", "--locked", "--all-features", "--example", "hud_control_probe",
               "--manifest-path", str(source / "Cargo.toml"), "--target-dir", str(target),
               "--message-format=json-render-diagnostics"]
    cargo = subprocess.check_output(["cargo", "--version"], text=True, cwd=source).strip()
    rustc = subprocess.check_output(["rustc", "--version", "--verbose"], text=True, cwd=source).strip()
    built = subprocess.run(command, cwd=source, check=True, stdout=subprocess.PIPE, text=True)
    if before != live.source_hashes(source):
        raise RuntimeError("source changed during probe build")
    binary = artifact_executable(built.stdout)
    receipt = {"method": "cargo build --locked --all-features --example hud_control_probe",
               "source_base_commit": args.source_commit, "source_sha256": before,
               "command": command, "binary_path": str(binary), "cargo_version": cargo, "rustc_version": rustc,
               "created_utc": live.base.utc_now(),
               "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
    args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Built {binary}; receipt {args.output}")


if __name__ == "__main__":
    main()
