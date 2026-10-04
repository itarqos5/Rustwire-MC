# Configuration chat-reset wire audit

`packet::server_metadata::ResetChat` (also re-exported from `packet::common`)
represents the clientbound configuration `reset_chat` notification in protocols
766–776, from 1.20.5 through 26.2. The packet ID is `0x06` and its body is empty.
It is absent from protocols 763–765 and all other states/directions in the pinned
fourteen-family catalogs. Its packet builder always selects configuration.

The body codec supplies the same exact-length decode and transactional
reader/writer helpers as the other server metadata envelopes. An empty packet
is valid with a zero-byte packet limit. Any trailing byte is rejected by decode;
a streaming body read consumes zero bytes and leaves the next body untouched.
Version checks apply even to the empty body. State-aware typed dispatch returns
`DecodedPacket::Common(CommonPacket::ResetChat(ResetChat))` only in configuration.
The name-only `CommonPacket::decode` helper, like its existing siblings, has no
state argument; callers using that lower-level entry point supply catalog context.

This is an application notification. Parsing does not clear chat history,
signature caches, acknowledgements or authentication state, and does not send
anything. Applications that maintain chat state decide how to apply the request.

## Evidence and reproducibility

- `tools/check_reset_chat_fixtures.py` checks the SHA-256-pinned schemas,
  type/dispatch/ID/presence boundaries and eleven committed empty-body fixtures
- Four self-contained Python tests reject changed IDs, duplicates, nonempty
  bodies, redirected dispatch and incorrect version/state/direction presence;
  they do not need downloaded schemas or network access
- Rust tests check all supported fixture IDs, all 256 single-byte trailing
  values, unsupported families/states/direction, zero-byte budgets and atomic
  body helpers. A framed in-memory connection test verifies typed delivery
  without an outbound response or leaving configuration
- Independently of the schemas, static inspection of the official 1.20.6 and
  26.2 server classes found `StreamCodec.unit(INSTANCE)` for the packet body.
  The 1.20.6 official mappings resolve the obfuscated packet and unit-codec
  method. Immutable artifact locations and SHA-256 hashes are recorded in
  [the source facts](validation/reset-chat-wire-facts.json)

Recheck the schema evidence with `python3 tools/check_reset_chat_fixtures.py`
after fetching the pinned schemas. The official classes were inspected using
an existing JDK's `jdk.jdeps/com.sun.tools.javap.Main`; no game classes were
executed and no server or account session was started. No proprietary class
contents or mappings are distributed here. This evidence proves the envelope
layout, not application chat-reset behavior or live server acceptance.
