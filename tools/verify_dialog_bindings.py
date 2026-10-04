#!/usr/bin/env python3
"""Read-only verification of existing official 26.2 dialog class bindings.

Requires an already acquired inner server JAR and a JDK with jdk.jdeps. This
checks hashes and runs javap only; it never launches game/server classes, fetches
artifacts or persists disassembly. It is separate from clean-checkout unit tests.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def verify(jar, java='java'):
    facts = json.loads((ROOT / 'docs/validation/dialog-wire-facts.json').read_text())['official_26_2']
    if hashlib.sha256(jar.read_bytes()).hexdigest() != facts['inner_sha256']:
        raise ValueError('Official inner JAR hash mismatch')
    with zipfile.ZipFile(jar) as archive:
        for name, digest in facts['class_sha256'].items():
            if hashlib.sha256(archive.read(name.replace('.', '/') + '.class')).hexdigest() != digest:
                raise ValueError(f'Official class hash mismatch: {name}')
    required = {
        'net.minecraft.network.protocol.common.ClientboundClearDialogPacket': ['StreamCodec.unit:'],
        'net.minecraft.network.protocol.common.ClientboundShowDialogPacket': ['Dialog.STREAM_CODEC:', 'Dialog.CONTEXT_FREE_STREAM_CODEC:'],
        'net.minecraft.network.protocol.common.ServerboundCustomClickActionPacket': ['ByteBufCodecs.optionalTagCodec:', 'ByteBufCodecs.lengthPrefixed:', 'int 65536', 'long 32768l', 'bipush        16'],
        'net.minecraft.network.codec.ByteBufCodecs$15': ['FriendlyByteBuf.readNbt:', 'FriendlyByteBuf.writeNbt:', 'Optional.ofNullable:', 'Optional.orElse:'],
        'net.minecraft.network.codec.ByteBufCodecs$27': ['VarInt.read:', 'ByteBuf.slice:'],
        'net.minecraft.network.FriendlyByteBuf': ['EndTag.INSTANCE:', 'NbtIo.writeAnyTag:', 'NbtIo.readAnyTag:'],
        'net.minecraft.nbt.NbtAccounter': ['public void pushDepth();', 'Field maxDepth:I'],
        'net.minecraft.server.dialog.Dialog': ['ByteBufCodecs.holder:', 'ByteBufCodecs.fromCodecTrusted:', 'ByteBufCodecs.fromCodecWithRegistriesTrusted:'],
        'net.minecraft.network.protocol.configuration.ConfigurationProtocols': ['ClientboundShowDialogPacket.CONTEXT_FREE_STREAM_CODEC:', 'ServerboundCustomClickActionPacket.STREAM_CODEC:'],
        'net.minecraft.network.protocol.game.GameProtocols': ['ClientboundShowDialogPacket.STREAM_CODEC:', 'ServerboundCustomClickActionPacket.STREAM_CODEC:'],
    }
    for name, tokens in required.items():
        text = subprocess.run([java, '--module', 'jdk.jdeps/com.sun.tools.javap.Main', '-p', '-c', '-classpath', str(jar), name],
                              check=True, capture_output=True, text=True).stdout
        if any(token not in text for token in tokens):
            raise ValueError(f'Static dialog codec binding differs: {name}')
        if name.endswith('ByteBufCodecs$15') and 'readBoolean' in text:
            raise ValueError('Optional tag unexpectedly includes a boolean')
        if name.endswith('ConfigurationProtocols'):
            pattern = r'CLIENTBOUND_SHOW_DIALOG[^\n]*\n[^\n]*ClientboundShowDialogPacket.CONTEXT_FREE_STREAM_CODEC:'
            if not re.search(pattern, text):
                raise ValueError('Configuration show-dialog binding differs')
    print('Verified official 26.2 JAR, 11 class hashes and static dialog/framing bindings; no game code executed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True, help='Existing official 26.2 inner server JAR')
    parser.add_argument('--java', default='java', help='Existing JDK java command')
    args = parser.parse_args()
    verify(args.jar, args.java)


if __name__ == '__main__':
    main()
