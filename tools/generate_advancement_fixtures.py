#!/usr/bin/env python3
"""Original hand-specified advancement fixture construction, not an oracle.

No Rustwire encoder is used. Packet IDs are looked up in hash-checked schemas;
body bytes below are independently assembled from audited field order.
"""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def varint(n):
    data = bytearray()
    n &= 0xffffffff
    while n > 127:
        data.append((n & 127) | 128)
        n >>= 7
    data.append(n)
    return bytes(data)


def string(value):
    data = value.encode("utf-8")
    return varint(len(data)) + data


def full(protocol):
    data = bytearray(b"\x01\x02" + string("a:root") + b"\x01" + string("a:p") + b"\x01")
    data += string('"T"') + string('"D"') if protocol < 765 else b"\x08\x00\x01T\x08\x00\x01D"
    nbt = b"\x0a" + (b"\x00\x00" if protocol == 763 else b"") + b"\x08\x00\x01k\x00\x01v\x00"
    if protocol <= 765:
        data += b"\x01\xac\x02\x03" + nbt
    elif protocol < 775:
        data += b"\x03\xac\x02\x01\x00\x00" + nbt
    else:
        data += b"\xac\x02\x00\x01\x00\x00" + nbt
    data += b"\x01\x80\x00\x00\x07" + string("a:bg") + struct.pack(">ff", 1.5, -2.0)
    if protocol == 763:
        data += b"\x03" + string("First Criterion!?") + string("b") + string("c")
    data += b"\x03\x01" + string("First Criterion!?") + b"\x02" + string("b") + string("c") + b"\x00\x01"
    data += string("a:child") + b"\x00\x00" + (b"\x00" if protocol == 763 else b"") + b"\x00\x00"
    data += b"\x02" + string("a:old") + string("a:old")
    data += b"\x02" + string("a:root") + b"\x03"
    data += string("First Criterion!?") + b"\x01" + struct.pack(">q", 300)
    data += string("b") + b"\x00" + string("First Criterion!?") + b"\x01" + struct.pack(">q", -1)
    data += string("a:child") + b"\x00"
    if protocol >= 770:
        data += b"\x01"
    return bytes(data)


def main():
    rows = ["# Evidence: original hand-specified fixtures, independent of Rustwire encoder; not captures or release-runtime oracle", "# protocol\tname\tdirection\tpacket_id\tcase\thex"]
    for row in json.loads((ROOT / "research/schema-hashes.json").read_text()):
        data = (ROOT / "research/protocols" / f"{row['schema']}.json").read_bytes()
        if hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise SystemExit(f"Schema hash mismatch: {row['schema']}")
        play = json.loads(data)["play"]
        protocol = row["protocol"]
        for direction, name, label, body in [
            ("toClient", "advancements", "full", full(protocol)),
            ("toClient", "advancements", "empty", b"\x00\x00\x00\x00" + (b"\x00" if protocol >= 770 else b"")),
            ("toClient", "select_advancement_tab", "selected", b"\x01" + string("a:root")),
            ("toClient", "select_advancement_tab", "clear", b"\x00"),
            ("toServer", "advancement_tab", "opened", b"\x00" + string("a:root")),
            ("toServer", "advancement_tab", "closed", b"\x01"),
        ]:
            mapping = play[direction]["types"]["packet"][1][0]["type"][1]["mappings"]
            packet_id = next(int(key, 16) for key, value in mapping.items() if value == name)
            rows.append(f"{protocol}\t{name}\t{direction}\t{packet_id}\t{label}\t{body.hex()}")
    (ROOT / "tests/fixtures/advancements.tsv").write_text("\n".join(rows) + "\n")


if __name__ == "__main__":
    main()
