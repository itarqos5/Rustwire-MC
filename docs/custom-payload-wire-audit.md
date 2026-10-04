# Custom-payload envelope wire audit

`packet::custom_payload::{CustomPayload, CustomPayloadRef}` supplies owned and
borrowed plugin/mod-channel envelopes for clientbound and serverbound play in
protocols 763–776, and configuration in 764–776. There are 54 supported
version/state/direction combinations. The complete body is a VarInt-byte-length
resource identifier followed by all remaining bytes; there is **no inner payload
length prefix**. Known and unknown channel data remains opaque.

`CustomPayloadRef::decode` borrows both the channel and data directly from the
already delimited packet body. Owned decoding checks all bounds before copying.
Encoding checks channel, data and whole-body sizes before scanning channel syntax
or reserving one output buffer. Oversized caller-supplied values therefore fail
at their cheap length checks.
Both types expose an atomic writer and exact state/direction packet builder.
Incoming typed dispatch exposes `DecodedPacket::CustomPayload` in configuration
and play. No channel registration, handler execution, plugin negotiation or
reply is triggered. Login plugin requests remain a separate control protocol.

The existing `packet::custom_payload` serverbound convenience function now uses
this encoder. Its 32,767-byte payload cap and wire format are preserved. It now
also rejects malformed resource identifiers and nonexistent states explicitly;
callers relying on invalid identifiers should correct them. Raw packet access
remains available for application-specific extension protocols.

## Bounds and interpretation

- Clientbound payload cap: 1,048,576 bytes; serverbound: 32,767 bytes, excluding
  the channel header. These are conservative envelope policies matching the
  inspected vanilla **unknown-channel fallback** caps
- `Limits::max_packet` includes the complete channel header and payload;
  `Limits::max_string_chars` also bounds the channel, up to 32,767 UTF-16 units
- The shared resource-identifier validator preserves omitted/empty namespace
  spellings and empty paths without normalization. The namespace `..` is rejected
  from protocol 775, as in the other audited identifier codecs
- Opaque bytes can contain NUL, invalid UTF-8 or arbitrary binary values. They
  are not interpreted as an item, component, chat message, command or executable
- Missing/invalid channel prefixes, malformed UTF-8, invalid identifiers,
  excessive channel/data/body sizes and invalid state/version combinations fail
- The payload extends to the packet boundary. Shortening it after the complete
  channel is valid, and appended bytes become payload data. A generic envelope
  cannot detect channel-specific truncation or trailing-data errors
- Accepted overlong, nonoverflowing string-length VarInts are canonicalized when
  re-encoded, following the shared codec policy

A valid envelope does not prove a known channel's internal codec will accept its
bytes. For example, a brand payload has its own string layout; Rustwire does not
parse or validate it here. Mod loaders may define larger or different limits.

## Evidence and reproducibility

`tools/check_custom_payload_fixtures.py` verifies all fourteen SHA-256-pinned
schemas, exact packet IDs, state/direction presence, discriminator bindings and
the string-plus-rest layout. It independently encodes 324 original fixtures
covering empty/binary data, identifier spellings and a multi-byte channel prefix.
Five Python regression tests use synthetic schemas and need neither network nor
ignored downloaded inputs.

Rust tests compare semantic values, exact bytes/IDs, owned and borrowed APIs and
typed dispatch; pointer equality verifies the zero-copy view. They check every
channel-prefix truncation and every valid shortened-payload prefix, both exact
direction caps and one byte over, whole-body budgets, malformed prefixes/UTF-8,
identifier/version boundaries, atomic writer failure and state rejection. A
framed in-memory connection checks configuration/play delivery without replies.

Independent primary evidence is pinned in
[the source facts](validation/custom-payload-wire-facts.json):

- MCProtocolLib's exact protocol-763 / Minecraft-1.20 release binding and both
  packet implementations confirm the string followed by unprefixed remaining
  bytes, independently of the schema and Rust implementation
- Read-only inspection of official 1.20.6 and 26.2 packet/fallback bindings
  confirms the direction-specific caps and identifier-first channel dispatch
- Artifact, class and release-mapping SHA-256 hashes identify inspected inputs;
  no proprietary class bodies or mapping contents are distributed

Run `python3 tools/check_custom_payload_fixtures.py` after fetching pinned schemas.
This is source/static/fixture evidence, not a live plugin/mod-loader acceptance
run, and does not claim typed semantics for individual channel payloads.
