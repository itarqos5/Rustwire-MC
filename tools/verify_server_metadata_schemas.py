#!/usr/bin/env python3
"""Hash-verify metadata layouts/dispatch and independently encode wire fixtures.

Read-only by default; --write writes only the deterministic original TSV fixtures.
No release serializer, external runtime, server or network is used.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAMES = {"server_data", "custom_report_details", "chat_suggestions", "hide_message", "ping_response", "ping_request"}


def container(*fields):
    return ["container", [{"name": name, "type": kind} for name, kind in fields]]


def expected(protocol):
    component = "string" if protocol < 765 else "anonymousNbt"
    byte_array = ["buffer", {"countType": "varint"}]
    server = [("motd", component), ("iconBytes", ["option", byte_array if protocol < 766 else "ByteArray"])]
    if protocol < 766:
        server.append(("enforcesSecureChat", "bool"))
    result = {
        ("play", "toClient", "server_data"): ("packet_server_data", container(*server)),
        ("play", "toClient", "chat_suggestions"): ("packet_chat_suggestions", container(
            ("action", "varint"), ("entries", ["array", {"countType": "varint", "type": "string"}]))),
        ("play", "toClient", "hide_message"): ("packet_hide_message", container(
            ("id", "varint"), ("signature", ["switch", {"compareTo": "id", "fields": {"0": ["buffer", {"count": 256}]}, "default": "void"}]))),
    }
    if protocol >= 764:
        for direction, name in [("toClient", "ping_response"), ("toServer", "ping_request")]:
            result[("play", direction, name)] = ("packet_" + name, container(("id", "i64")))
    if protocol >= 767:
        details = container(("details", ["array", {"countType": "varint", "type": container(("key", "string"), ("value", "string"))}]))
        # Raw schemas erroneously advertise serverbound configuration metadata.
        # Verify the pinned anomaly here; generate_catalog removes those entries
        # using independently audited release registries, not a new wire helper.
        states = [("configuration", "toClient"), ("play", "toClient")]
        if protocol < 771:
            states.append(("configuration", "toServer"))
        for state, direction in states:
            result[(state, direction, "custom_report_details")] = ("packet_common_custom_report_details", details)
    return result


def inspect_schema(schema, protocol):
    expected_packets = expected(protocol)
    found = {}
    for state in ["handshaking", "status", "login", "configuration", "play"]:
        for direction in ["toClient", "toServer"]:
            local = schema.get(state, {}).get(direction, {}).get("types", {})
            if "packet" not in local:
                continue
            fields = local["packet"][1]
            mappings = fields[0]["type"][1]["mappings"]
            dispatch = fields[1]["type"][1]
            if dispatch["compareTo"] != "name":
                raise ValueError("Unexpected packet discriminator")
            for raw_id, name in mappings.items():
                if name not in NAMES:
                    continue
                key = (state, direction, name)
                if key not in expected_packets or key in found:
                    raise ValueError(f"Unexpected packet presence: {protocol} {key}")
                type_name, layout = expected_packets[key]
                if dispatch["fields"].get(name) != type_name:
                    raise ValueError(f"Unexpected packet dispatch: {protocol} {key}")
                if local.get(type_name, schema["types"].get(type_name)) != layout:
                    raise ValueError(f"Unexpected packet layout: {protocol} {key}")
                found[key] = int(raw_id, 0)
    if found.keys() != expected_packets.keys():
        raise ValueError(f"Missing packet presence: {protocol}")
    if protocol >= 766 and schema["types"].get("ByteArray") != ["buffer", {"countType": "varint"}]:
        raise ValueError(f"Unexpected ByteArray layout: {protocol}")
    return found


def varint(value):
    if not 0 <= value <= 0x7fffffff:
        raise ValueError("Fixture length/index out of range")
    result = bytearray()
    while value > 127:
        result.append((value & 127) | 128)
        value >>= 7
    result.append(value)
    return bytes(result)


def string(value):
    value = value.encode("utf-8")
    return varint(len(value)) + value


def bodies(protocol, name):
    if name == "server_data":
        component = b'\x0c{"text":"x"}' if protocol < 765 else b'\x08\x00\x01x'
        suffix = b'\x01' if protocol < 766 else b''
        return {"absent": component + b'\0' + suffix,
                "empty": component + b'\1\0' + suffix,
                "bytes": component + b'\1\3\x89\0\xff' + suffix}
    if name == "custom_report_details":
        return {"empty": b'\0', "duplicates": b'\2\1a\1x\1a\1y'}
    if name == "chat_suggestions":
        return {"add": b'\0\3\0\2hi' + string("😀"), "remove": b'\1\0', "set": b'\2\1\6/hello'}
    if name == "hide_message":
        return {"signature": b'\0' + bytes(range(256)), "first": b'\1',
                "last": b'\x80\1', "unresolved": b'\xff\xff\xff\xff\7'}
    return {case: struct.pack('>q', value) for case, value in [
        ("positive", 0x0102030405060708), ("negative", -1), ("minimum", -(1 << 63)),
        ("maximum", (1 << 63) - 1), ("zero", 0)]}


def generate(root=ROOT):
    records = json.loads((root / 'research/schema-hashes.json').read_text())
    if [r['protocol'] for r in records] != list(range(763, 777)):
        raise ValueError('Expected all fourteen explicit protocol families')
    lines = ['# Original synthetic schema-backed fixtures; not release-serializer outputs.',
             '# protocol\tstate\tdirection\tname\tcase\tpacket_id\tbody_hex']
    for row in records:
        raw = (root / 'research/protocols' / (row['schema'] + '.json')).read_bytes()
        if hashlib.sha256(raw).hexdigest() != row['sha256']:
            raise ValueError('Schema hash mismatch: ' + row['schema'])
        packets = inspect_schema(json.loads(raw), row['protocol'])
        for (state, direction, name), packet_id in packets.items():
            if direction == 'toServer' and name != 'ping_request':
                continue
            for case, body in bodies(row['protocol'], name).items():
                lines.append('\t'.join(map(str, [row['protocol'], state, direction, name, case, packet_id, body.hex()])))
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    output = generate()
    target = ROOT / 'tests/fixtures/server-metadata.tsv'
    if args.write:
        target.write_text(output)
    elif target.read_text() != output:
        raise SystemExit('Metadata fixtures do not match independent encoding/schema IDs')
    print(f"Verified all fourteen hash-pinned schemas and {len(output.splitlines()) - 2} fixtures")


if __name__ == '__main__':
    main()
