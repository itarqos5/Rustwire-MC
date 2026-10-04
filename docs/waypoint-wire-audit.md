# Tracked waypoint packet audit

`packet::waypoint::TrackedWaypoint` provides the clientbound `tracked_waypoint`
body for protocols 771–776 (1.21.6 through 26.2), with `DecodedPacket::Waypoint`
and normal Connection typed dispatch. All earlier families fail the exact catalog
gate. No waypoint tracking state, rendering or entity/style lookup is performed.

## Exact body and semantic distinctions

The outer operation is track=0, untrack=1 or update=2, followed by a complete
waypoint. **Untrack still carries the complete identity, icon and location**;
there is no special short UUID-only removal wire body.

A boolean selects a raw 16-byte UUID (true) or an ordinary string (false).
Named identities may contain spaces, uppercase letters and Unicode; they are not
resource-location keys. The icon then carries a resource identifier style and a
boolean-prefixed optional RGB triple, one unsigned byte per channel, no alpha.
Style spelling is validated/preserved without normalization or registry lookup.

The location discriminator is a VarInt:

- 0: empty, no further fields
- 1: three signed coordinate VarInts
- 2: signed chunk-X and chunk-Z VarInts
- 3: raw f32 azimuth

Coordinates are not packed block positions and have no gameplay range clamp.
Azimuth values preserve signed-zero/infinity/NaN bits without unit conversion or
normalization. Unknown positive location kinds are unsupported unframed payloads;
Connection retains their **complete original RawPacket**, without pretending to
validate unread bytes. Negative kinds and invalid operation IDs are malformed.
Known booleans, identifiers, UTF-8, truncated/trailing data and resource-limit
violations remain errors rather than opaque fallback.

## Evidence levels

1. `tools/check_waypoint_fixtures.py` hashes all fourteen pinned inputs, checks
   all **six present/eight absent** packet bodies, complete nested field order,
   discriminator maps, identity/color switches, primitive/Vec3i aliases and exact
   packet mapping/dispatch. Schemas agree on the implemented body in this slice
2. The [independent source manifest](validation/waypoint-source-audit.json)
   records immutable PacketEvents v2.13.0 URLs/hashes for the packet, waypoint,
   icon, all location variants, vector and color helpers. Source was inspected,
   not executed or copied into Rustwire
3. Narrow static inspection of the official 26.2 class bindings independently
   confirms the outer operation plus full waypoint codec, UUID/string identity,
   icon style/optional RGB codec and location-type dispatch. This is not an
   executed release serializer or live-server result
4. An original Python encoder produces **306 fixtures**, spanning all operations,
   identity kinds, color presence and location variants in every supported
   family, with additional exact NaN/infinity bit patterns. These are synthetic,
   not captured packets or game-generated fixture output

Representative independently inspected sources:

- [Packet wrapper](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/wrapper/play/server/WrapperPlayServerWaypoint.java)
- [TrackedWaypoint](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/world/waypoint/TrackedWaypoint.java)
- [WaypointIcon](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/world/waypoint/WaypointIcon.java)
- [RGB helper](https://github.com/retrooper/packetevents/blob/1063d4e71cff72b9fe022f37b51faad9536815b7/api/src/main/java/com/github/retrooper/packetevents/protocol/color/Color.java)

The [official 26.2 bundle](https://piston-data.mojang.com/v1/objects/823e2250d24b3ddac457a60c92a6a941943fcd6a/server.jar)
has SHA-256 `cdacdfb25898de5e4b4b0e5ddcc2722f77067e46605709c2d886c000ebb63ec5`.
The inner `META-INF/versions/26.2/server-26.2.jar` has SHA-256
`183c0499c5f855570ee487dd38e141a53f0121f83a0b07a3bac2d8b6698823e8`.
For static reproduction, use `javap -c -p -classpath <inner.jar>` on
`net.minecraft.network.protocol.game.ClientboundTrackedWaypointPacket`,
`net.minecraft.world.waypoints.TrackedWaypoint` and `Waypoint$Icon` (quote `$`
in a shell). The packet composes operation and waypoint codecs regardless of
operation; the waypoint always serializes identity, icon, type and its contents.
No game binary, disassembly or copied implementation is redistributed.

## Bounds and verification

Both strings use the shared UTF-16 limit and 32,767-unit wire ceiling. Whole-body
byte caps bound input before parsing, and string writes preflight prefix/payload
size before growing output. Scalar fields are fixed-size or bounded VarInts;
there are no lists or nested NBT payloads. Public one-body reads and writes leave
their caller unchanged on failure. The color/position fixed arrays do not consume
a collection budget intended for variable-sized wire arrays.

Six Rust tests cover all **12,582 strict fixture prefixes**, decoded meanings,
exact re-encoding and packet IDs, undersized/exact byte caps, per-string limits,
known malformed values, explicit unsupported-kind fallback, old-version/state
gates, complete Untrack bodies, read/write rollback and bounded mutations.
An in-memory Connection test distinguishes decoded events, full raw unknown-kind
fallback, and errors for malformed known bodies. Four Python verifier regression
tests construct their own small schemas and do not need the ignored schema cache
or network access.

```sh
python3 tools/check_waypoint_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_waypoint_fixtures.py'
cargo test --locked --offline --no-default-features --test waypoint --test waypoint_typed
```

No live waypoint renderer/client acceptance, tracking simulation or server
location-sharing behavior is exercised or claimed by this codec increment.
