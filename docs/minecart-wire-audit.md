# Modern minecart movement wire audit

`MoveMinecart` and `MinecartStep` extend `packet::entity_control` with clientbound
`move_minecart` for protocols 768–776, using `EntityControlPacket::Minecart` and
ordinary Play-only typed dispatch. Earlier versions fail the exact catalog gate.
This is a movement envelope, not rail physics, interpolation or movement policy.

## Corrected wire layout

The body is entity-ID VarInt, step-count VarInt, then fixed-size steps:

- position: three big-endian f64 values
- velocity: three big-endian f64 values
- yaw and pitch: one raw angle byte each
- weight: one big-endian f32

Each step occupies exactly **54 bytes**. Angles use the existing lossless `Angle`
byte wrapper. Signed entity references, signed zero, infinities and NaN payload
bits are retained. No coordinates/weights are clamped and no state is applied.

### Explicit schema disagreement

All nine pinned schemas incorrectly describe the two vectors as `vec3f`
(three f32s each) and yaw/pitch as f32. That would give 36-byte steps and is not
the implemented wire format. The `movement` field name becomes `velocity` at
773; that spelling change does not change the wire body.

`tools/check_minecart_fixtures.py` checks the complete **actual pinned layout**,
including those discrepant aliases, packet mapping/dispatch and the five older
families where the packet is absent. It does not rewrite source schemas or
present their wrong field widths as corroborating evidence. The corrected widths
come from independent implementations and narrow official static bindings:

- [MCProtocolLib 1.21.2-1](https://github.com/GeyserMC/MCProtocolLib/blob/667e02d38cd4a79d46640312cd599123609633ac/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/entity/ClientboundMoveMinecartPacket.java)
  explicitly reads/writes six doubles, two bytes and a float. The matching
  MinecraftCodec at this immutable commit identifies protocol 768
- [PacketEvents v2.13.0](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerMoveMinecart.java)
  uses its double-vector and byte-rotation helpers for the same step shape,
  without a later-family layout override
- Static inspection of the official 26.2 `ClientboundMoveMinecartPacket` composes
  a VarInt entity and a list of `NewMinecartBehavior.MinecartStep`. The step
  composes two `Vec3.STREAM_CODEC` values, two `ROTATION_BYTE` values and FLOAT.
  `Vec3.STREAM_CODEC` is `Vec3$1`, whose codec uses three `readDouble` and three
  `writeDouble` bindings. No game codec was executed

The [source manifest](validation/minecart-source-audit.json) records independently
inspected immutable URLs and hashes. The official 26.2 bundle and inner JAR have
SHA-256 hashes `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`
and `183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
The [official bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)
contains `META-INF/versions/26.2/server-26.2.jar`. The inner version metadata
identifies protocol 776. This establishes static endpoint evidence, not executed
release-API results across all nine families.

To reproduce the narrow static checks, inspect the named classes with
`javap -c -p -classpath server-26.2.jar`, quoting `$` in nested class names.
The JDK module launcher described in the [slot audit](slot-count-wire-audit.md)
works when the javap launcher is absent. No server, installation, account or
license-acceptance operation is needed for this read-only inspection. No game
binary, disassembly or copied implementation is included in the repository.

## Bounds and tests

Decode checks the count against both the caller collection budget and
`remaining_bytes / 54` before reserving a vector. Encode preflights checked
`54 * count + VarInt-prefix lengths` before growing output. Whole-packet byte
limits and strict trailing-data checks apply; version availability is checked
before either codec runs.

The **27 original Python fixtures** include empty/multiple steps, independent
f64/float bit patterns, extreme signed IDs and high angle bytes. Rust checks
exact decode/encode/packet-ID behavior and every **1,593 strict prefix**, plus
trailing bytes, exact/undersized limits, negative/overflow/impossible counts,
wrong-version use, typed state boundaries, bounded mutations and explicit
rejection of a stale-schema 36-byte step. No NaN equality assertion is used to
stand in for a bit-preservation check. Four Python regression tests construct
small synthetic schemas and need no downloaded inputs.

```sh
python3 tools/check_minecart_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_minecart_fixtures.py'
cargo test --locked --offline --no-default-features --test minecart
```

Fixtures are synthetic, not captures or runtime serializer output. No new live
minecart/receiving-client acceptance, rail simulation or physics test is claimed.
