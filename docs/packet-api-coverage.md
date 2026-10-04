# Packet API coverage and remaining boundaries

This client-role inventory was reviewed after
[`761b5e9`](https://github.com/itarqos5/Rustwire-MC/commit/761b5e9388ead1d49456c183e5e97c7fe5848d41).
The [machine-readable table](packet-api-coverage.tsv) lists 267 distinct
state/direction/name bindings and their 3,233 exact protocol/packet-ID pairs
across protocols 763–776. Names shared across states or directions are separate.

The table inventories entrypoints. It does **not** prove complete wire
conformance, symmetric client/server codecs, live interoperability for every
packet, or a full Minecraft client. Public outgoing methods were manually
traced through their implementations/macros; incoming dispatch and control
routes were checked separately. Nested payload and application-policy limits
still apply.

## What the inventory means

| State | Clientbound versioned rows | Serverbound versioned rows |
|---|---:|---:|
| Handshake | 0 | 28, including 14 historical legacy-ping entries |
| Status | 28 | 28 |
| Login | 81 | 66 |
| Configuration | 220 | 110 |
| Play | 1,808 | 864 |

All 2,028 configuration/play incoming rows have a typed or connection-control
route. `Connection::next_typed_event_with_context` preserves complete raw packets
for unknown/unsupported layouts; malformed known bodies and resource-limit
violations remain errors. The three chunk families `map_chunk`, `update_light`
and `chunk_biomes` need the current dimension's section count. Refresh that
context after Join Game, Respawn or relevant configuration/registry changes.

Every modern serverbound family has an outbound builder or an input-driven
control-response path. The table distinguishes automatic keepalive/ping/state
acknowledgements from explicit application requests. Encryption completion needs
the optional `crypto` feature. `RawPacket` and `packet::named` remain low-level
building blocks; naming an arbitrary body does not validate its contents or
establish that sending it is appropriate in the current connection state.

`legacy_server_list_ping` is the explicit catalog exception. It names a
historical **unframed** compatibility exchange. A framed packet with ID 254 does
not implement it; no modern helper is claimed. Current status/ping is available
through `Connection::status`.

## Important API distinctions

- `packet::handshake` and `Connection::start_login` retain ordinary status/login
  intent. `handshake_with_intent` and `start_transfer_login` explicitly support
  transfer intent from protocol 766; see the [transfer audit](transfer-handshake-wire-audit.md)
- `SetSelectedSlot::encode` creates the clientbound body. Its `packet` method
  creates the distinct serverbound i16 form
- Incoming vehicle corrections and outgoing vehicle movement are separate
  codecs; only outgoing packets gain the ground-state field from protocol 769
- Before protocol 766, `chat_command` uses `chat::signed::SignedChatCommand`.
  From 766, `chat::unsigned_command` handles dedicated unsigned commands and
  signed commands use `chat_command_signed`. Signing providers and trust policy
  still belong to the application
- Configuration/play custom payloads and custom click actions have explicit
  state/direction-aware APIs. Reusing a packet name in another state is not
  sufficient to establish support
- Standalone body helpers can carry less context than packet/connection APIs.
  Prefer exact version/state/direction catalog lookup for arbitrary dispatch

## Reproducing the table

```sh
python3 tools/report_packet_api_coverage.py --check
python3 -m unittest discover -s tools -p 'test_report_packet_api_coverage.py'
```

The script reads only checked-in catalogs/source and needs no network or ignored
schema downloads. Running it without `--check` refreshes the table. Its outgoing
API mapping is deliberately explicit: a new packet name requires an audited
entrypoint rather than being labelled implemented merely because it exists in
a schema. Six self-contained checks cover the snapshot, separate states and
directions, dimension-context/legacy exceptions, removed incoming dispatch,
unknown outgoing APIs, wrong outgoing states, and malformed/duplicate catalog
entries. CI checks for table drift.

These guards are routing checks. They do not parse Rust semantically, prove every
conditional version branch, or replace the per-family wire fixtures, official
source audits and malformed/resource-budget tests. The earlier catalog
corrections for protocol 776's spectator tail and phantom serverbound
configuration metadata at 767–770 remain explicit and unchanged.

## Remaining semantic and validation boundaries

- Known vanilla item-component and metadata **outer wire layouts** are covered
  by the separate [nested-layout report](typed-coverage.json). Registry identity
  resolution, NBT-backed predicate semantics and many persistent component-hash
  forms remain limited. Unknown unframed nested kinds cannot be skipped safely
- Recipes now have explicit legacy declarations, modern property/stonecutter
  declarations, displays and control codecs. They transmit recipe data; they do
  not simulate crafting, implement server permissions or establish recipe-book
  behavior for every server
- Numeric world IDs, static block-state names, physics, entity/world simulation,
  rendering and full inventory prediction remain outside this protocol library
- Secure-chat certificate trust, signing/verification providers and session/index
  policy remain application responsibilities. Live account-backed Microsoft
  authentication still has not been established by the offline HTTP/crypto tests
- Resource-pack downloads/consent, code-of-conduct acceptance, transfers, cookie
  forwarding, custom-channel interpretation and URL navigation remain explicit
  application decisions
- Recent families use original synthetic fixtures and bounded in-memory tests.
  The published Paper/Grim evidence is historical and scenario/release specific;
  this inventory did not rerun it or test every patch alias. No comprehensive
  live replay of all recent packet families is claimed
- This is a client-oriented receive/send API. There is no unified typed
  serverbound dispatcher, complete server/proxy state machine, automatic SRV
  lookup, async-runtime adapter or reconnect policy

Future integration validation should target real workflows and known semantic
limits rather than treating a packet-name count as proof of a complete client.
