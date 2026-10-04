#!/usr/bin/env python3
"""Build the bounded Grim probe with a checked source/binary receipt."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from build_hud_probe import artifact_executable

PROJECT = Path(__file__).resolve().parents[2]
METHOD = "cargo build --release --locked --example grim_probe; default features; lto=false; codegen-units=16"
HARNESS = ["tools/paper/build_grim_probe.py", "tools/paper/build_hud_probe.py",
           "tools/paper/validate_grim.py", "tools/paper/test_validate_grim.py",
           "docs/validation/grim-download-provenance.json", "docs/validation/matrix-download-provenance.json"]


def sources(root):
    names = {str(p.relative_to(root)) for directory in [root / "src", root / "examples/grim_probe"] for p in directory.rglob("*.rs")}
    names.update(["Cargo.toml", "Cargo.lock", "examples/grim_probe.rs", *HARNESS])
    return {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in sorted(names)}


def verify_library(root, commit):
    names = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", commit, "src", "Cargo.toml", "Cargo.lock"], cwd=PROJECT, text=True).splitlines()
    actual = {str(p.relative_to(root)) for p in (root / "src").rglob("*.rs")} | {"Cargo.toml", "Cargo.lock"}
    if set(names) != actual:
        raise RuntimeError("library snapshot file set differs from commit")
    for name in names:
        if subprocess.check_output(["git", "show", commit + ":" + name], cwd=PROJECT) != (root / name).read_bytes():
            raise RuntimeError("library snapshot differs from commit: " + name)


def verify_harness(root):
    for name in HARNESS:
        if (root / name).read_bytes() != (PROJECT / name).read_bytes():
            raise RuntimeError("invoked harness/manifest differs from source snapshot: " + name)


def verify_receipt(receipt, root, binary, commit):
    verify_library(root, commit)
    verify_harness(root)
    if receipt.get("method") != METHOD or receipt.get("source_base_commit") != commit:
        raise RuntimeError("build receipt method/commit mismatch")
    if receipt.get("source_sha256") != sources(root):
        raise RuntimeError("build receipt source mismatch")
    if receipt.get("binary_sha256") != hashlib.sha256(binary.read_bytes()).hexdigest():
        raise RuntimeError("build receipt binary mismatch")
    if not receipt.get("cargo_version") or not receipt.get("rustc_version"):
        raise RuntimeError("build receipt toolchain missing")
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--target-dir", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    root, target = args.source_root.resolve(), args.target_dir.resolve()
    verify_library(root, args.source_commit)
    verify_harness(root)
    before = sources(root)
    command = ["cargo", "build", "--release", "--locked", "--example", "grim_probe",
               "--config", "profile.release.lto=false", "--config", "profile.release.codegen-units=16",
               "--manifest-path", str(root / "Cargo.toml"), "--target-dir", str(target),
               "--message-format=json-render-diagnostics"]
    cargo = subprocess.check_output(["cargo", "--version"], cwd=root, text=True).strip()
    rustc = subprocess.check_output(["rustc", "--version", "--verbose"], cwd=root, text=True).strip()
    built = subprocess.run(command, cwd=root, check=True, stdout=subprocess.PIPE, text=True)
    if sources(root) != before:
        raise RuntimeError("source changed during build")
    binary = artifact_executable(built.stdout, "grim_probe")
    receipt = {"method": METHOD, "source_base_commit": args.source_commit, "source_sha256": before,
               "command": command, "binary_path": str(binary), "cargo_version": cargo, "rustc_version": rustc,
               "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}
    args.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"Built {binary}; receipt {args.output}")


if __name__ == "__main__":
    main()
