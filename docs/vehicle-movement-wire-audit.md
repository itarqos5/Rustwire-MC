# Serverbound vehicle movement wire audit

## Scope and exact boundary

`packet::movement::VehicleMovement` is an original, bounded serverbound Play
`vehicle_move` envelope. It reports coordinates/angles and, from protocol 769,
explicit ground state. It performs no mounting, simulation, collision checking,
input synthesis or network transmission. `packet()` returns the exact-version
serverbound packet ID. Existing `world_control::VehicleMove` and its clientbound
typed dispatch remain unchanged.

| Direction | Protocol families | Ordered body | Bytes |
| --- | --- | --- | --- |
| Serverbound | 763–768 | X/Y/Z f64, yaw/pitch f32 | 32 |
| Serverbound | 769–776 | X/Y/Z f64, yaw/pitch f32, on-ground boolean | 33 |
| Clientbound | 763–776 | X/Y/Z f64, yaw/pitch f32 | 32 |

All fixed scalars are big endian. `on_ground: Option<bool>` describes version
presence: `None` is required before 769; `Some(false)` or `Some(true)` is required
from 769. No option discriminator is emitted. Incompatible representations fail
with `Unsupported`, rather than inventing ground state or silently dropping it.
The byte is a boolean, not the player-movement flags introduced at protocol 768.

## Independently checked sources

The hash-pinned schemas establish all 28 directional layouts/IDs. They are not
the sole authority for the 768/769 boundary:

- MCProtocolLib `1.21.2-1`, commit
  [`667e02d38cd4a79d46640312cd599123609633ac`](https://github.com/GeyserMC/MCProtocolLib/blob/667e02d38cd4a79d46640312cd599123609633ac/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/serverbound/level/ServerboundMoveVehiclePacket.java),
  independently reads/writes three doubles and two floats. Its
  [MinecraftCodec](https://github.com/GeyserMC/MCProtocolLib/blob/667e02d38cd4a79d46640312cd599123609633ac/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/codec/MinecraftCodec.java)
  identifies protocol 768 / Minecraft 1.21.3.
- MCProtocolLib `1.21.4-1`, commit
  [`7cc247026c6bff700c644d545942f7733ebb6ad0`](https://github.com/GeyserMC/MCProtocolLib/blob/7cc247026c6bff700c644d545942f7733ebb6ad0/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/serverbound/level/ServerboundMoveVehiclePacket.java),
  independently appends one boolean. Its
  [MinecraftCodec](https://github.com/GeyserMC/MCProtocolLib/blob/7cc247026c6bff700c644d545942f7733ebb6ad0/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/codec/MinecraftCodec.java)
  identifies protocol 769 / Minecraft 1.21.4.
- Static inspection of the official 1.20.6 release's
  `ServerboundMoveVehiclePacket` and `ClientboundMoveVehiclePacket` confirms
  three f64s followed by two f32s, with no boolean in either direction.
  Official mappings identify obfuscated classes `ahv` and `aed` respectively.
- Static inspection of official 26.2 confirms serverbound composition of
  `Vec3.STREAM_CODEC`, `FLOAT`, `FLOAT`, `BOOL`; the clientbound composition has
  no `BOOL`. Inspection of `Vec3$1` confirms the vector is three fixed-width
  f64s, not the separately defined low-precision vector codec.

Only protocol facts and independently authored code/fixtures are included.
No game class was executed; no server was started. These static checks are not
release-API-generated fixture bytes or live interoperability observations.
No game binary, mapping file, source implementation or disassembly is distributed.

### Artifact identities

| Evidence | SHA-256 |
| --- | --- |
| [Official 1.20.6 bundler](https://piston-data.mojang.com/v1/objects/145ff0858209bcfc164859ba735d4199aafa1eea/server.jar) | `c6d01d018ca782e506f0ec60652d47fd565078be9122b625c1681bc86c29c7ec` |
| Inner `META-INF/versions/1.20.6/server-1.20.6.jar` | `bb610daa96b3784645b78fd7c0b887291414c9605cd913fc79cf985f523f4f5b` |
| [Official 1.20.6 mappings](https://piston-data.mojang.com/v1/objects/9e96100f573a46ef44caab3e716d5eb974594bb7/server.txt) | `4e5068e40a89c673b5541192ad7eab96289f6dcae704f577f8992bdf7c984aa3` |
| [Official 26.2 bundler](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar) | `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5` |
| Inner `META-INF/versions/26.2/server-26.2.jar` | `183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8` |
| MCProtocolLib 768 packet source | `241c9ec95487be32cdf67d0c51f5c956dd51dbf581f0b3789e20ca6309d6961a` |
| MCProtocolLib 768 version declaration | `93ae69a2e3698c61a799427449e97d4d527deafbe44f9e655282a4f896664434` |
| MCProtocolLib 769 packet source | `5014a2b163d494697cda5465ca5c891af57c59109672c4f2e93b16688ebafd7e` |
| MCProtocolLib 769 version declaration | `eda2f5113352461ed6d72b5065e1ea433818037e76701fb805e90e65c4b6aac5` |

## Semantics and resource bounds

- This envelope preserves exact floating-point bits, including signed zero,
  subnormals, finite extrema, infinities and NaN payloads. It does not normalize
  rotations, clamp coordinates, or promise a server will accept them. The
  existing `PlayerMovement` finite-value policy is unchanged.
- Ground booleans use the crate's strict canonical 0/1 policy. All 254 other
  byte values return `Invalid`; this is deliberately stricter than Netty's
  nonzero-is-true primitive read behavior.
- A fixed 32/33-byte size check happens before allocation or field parsing.
  Packet-byte budgets are enforced by direct read/decode, encode/write and
  packet construction. Unrelated collection/string/NBT budgets are not charged.
- Reader operations consume exactly one body and are transactional on failure,
  even when the reader has already advanced. Writer operations append only a
  complete body and leave the destination unchanged on failure. `decode` rejects
  trailing bytes; `read` intentionally leaves subsequent enclosing bytes.

## Fixtures and verification

`tools/check_vehicle_movement_fixtures.py` independently encodes 108 synthetic
rows with Python `struct`: 66 serverbound and 42 clientbound. Each body has a
catalog ID and explicit finite/extrema/raw-bit semantic case. Ground false/true
are both encoded in every modern family. The committed clientbound rows preserve
and test the pre-existing 32-byte behavior across all fourteen families.

Fixture SHA-256: `4f0cf4c08c5faabb98575417a5c0913822f1f4ad915d752e44db64fda32a9fb8`.

Nine Rust tests cover all rows, semantic values independently constructed from
constants, complete uncompressed outbound frames, exact directional packet IDs,
all 3,504 strict prefixes, trailing bytes, every invalid boolean, each version's
missing/extra ground-state representation, every smaller packet budget,
transactional I/O and all five floating fields' special bit domains. Modern
outbound bodies must fail clientbound direct and named dispatch as trailing data.
Ten offline Python regressions use minimal synthetic schemas and the committed
fixture table, so they run in a clean checkout without research downloads. They
check hash enforcement, exact family inventory, directional presence, scalar
width/order/aliases, dispatcher linkage, duplicate/missing IDs and fixture bytes.
The separate verifier command checks actual downloaded hash-pinned schemas.

```sh
python3 tools/check_vehicle_movement_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_vehicle_movement_fixtures.py' -v
cargo test --locked --offline --no-default-features \
  --test vehicle_movement --test movement --test world_control --test world_control_typed
cargo clippy --locked --offline --no-default-features --lib \
  --test vehicle_movement --test movement --test world_control --test world_control_typed -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --no-default-features --no-deps
cargo fmt --all -- --check
```

These focused checks passed: 26 Rust tests (9 new and 17 existing movement/world-control
regressions), 10 offline Python verifier tests, the 28-layout schema check,
clippy with warnings denied, rustdoc with warnings denied and formatting.
They are not the full feature/MSRV or live server matrix and introduce no
benchmark or uncontended timing claims.

## Exact packet IDs

| Protocol | Serverbound | Clientbound |
| --- | --- | --- |
| 763 | 0x18 | 0x2e |
| 764 | 0x1a | 0x2f |
| 765 | 0x1b | 0x2f |
| 766 | 0x1e | 0x31 |
| 767 | 0x1e | 0x31 |
| 768 | 0x20 | 0x33 |
| 769 | 0x20 | 0x33 |
| 770 | 0x20 | 0x32 |
| 771 | 0x21 | 0x32 |
| 772 | 0x21 | 0x32 |
| 773 | 0x21 | 0x37 |
| 774 | 0x21 | 0x37 |
| 775 | 0x22 | 0x39 |
| 776 | 0x22 | 0x39 |
