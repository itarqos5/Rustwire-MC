# Explicit transfer handshake and login

`packet::handshake_with_intent` distinguishes status (wire intent 1), ordinary
login (2), and transfer (3). Transfer is available in protocols 766–776,
Minecraft 1.20.5–26.2. The existing `packet::handshake` and
`Connection::start_login` retain their original status/login behavior.

`Connection::start_transfer_login` starts the ordinary login pipeline on a
caller-created destination connection using intent 3. It requires the initial
Handshake state. It does not follow a received transfer or create a transport,
copy cookies/resource packs, clear application state, or approve a destination.
The application decides whether to transfer and which state may cross servers.
The destination server may reject transfers. Its encryption request still
controls `should_authenticate`; transfer intent does not suppress authentication.

## Sources and exact boundary

Mojang's [1.20.5 release notes](https://feedback.minecraft.net/hc/en-us/articles/26136167989005-Minecraft-Java-Edition-1-20-5-Armored-Paws)
introduce transfer intent 3 and explain that destination servers reject incoming
transfers by default. Static inspection of the official 1.20.6 and 26.2
`ClientIntent` and `ClientIntentionPacket` codecs confirms IDs 1/2/3 and the
unchanged envelope: protocol VarInt, String(255) hostname, unsigned 16-bit port,
intent VarInt. Both login and transfer enter the Login protocol state.

Inspected inner server JAR SHA-256 values:

- 1.20.6: `bb610daa96b3784645b78fd7c0b887291414c9605cd913fc79cf985f523f4f5b`, from the [official bundle](https://piston-data.mojang.com/v1/objects/145ff0858209bcfc164859ba735d4199aafa1eea/server.jar)
- 26.2: `183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`, from the [official bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)

The [official 1.20.6 server mappings](https://piston-data.mojang.com/v1/objects/9e96100f573a46ef44caab3e716d5eb974594bb7/server.txt)
(SHA-256 `4e5068e40a89c673b5541192ad7eab96289f6dcae704f577f8992bdf7c984aa3`).
They resolve ClientIntent to `aiw` and ClientIntentionPacket to `aix`.
The named 26.2 classes need no deobfuscation. The independent public cross-check
is MCProtocolLib commit `f4004310698f9e3793e6619a6db89ae0a7784a4d`:
[HandshakeIntent](https://github.com/GeyserMC/MCProtocolLib/blob/f4004310698f9e3793e6619a6db89ae0a7784a4d/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/data/handshake/HandshakeIntent.java) and
[ClientIntentionPacket](https://github.com/GeyserMC/MCProtocolLib/blob/f4004310698f9e3793e6619a6db89ae0a7784a4d/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/handshake/serverbound/ClientIntentionPacket.java).
No game classes were executed or copied into this repository.

## Preflight and tests

Both login entrypoints validate the handshake and login-start body, then both
frame encodings, before writing either packet. Unsupported transfer versions,
invalid host/user fields and packet/frame budget failures leave the connection
in Handshake without output. An I/O failure may have written bytes; the existing
poisoned-stream rule prevents unsafe retries rather than claiming rollback.

Seven regression tests cover:

- Literal handshake bytes for status/login in all fourteen families and transfer
  in all eleven eligible families, including the precise 765/766 boundary
- UTF-16 hostname length and full unsigned port width
- Exact two-frame output; ordinary login remains intent 2
- Unsupported version, wrong state, field and first/second-frame preflight errors
- Preservation of both values of the server's authentication-request flag
- Existing login-success/configuration acknowledgements through protocol 776
- Partial-write poisoning with no retry output

These are bounded synthetic in-memory wire tests. No live server transfer,
account-backed authentication, server acceptance, cookie forwarding or retained
resource-pack behavior is claimed.
