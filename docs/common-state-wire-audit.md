# State-aware common packet dispatch

`CommonPacket::decode_in_state` now checks exact clientbound state/version/catalog
presence before parsing a known common envelope. `DecodedPacket` uses this entry
point. Unsupported tuples and unrelated names return `None`; valid tuples with
malformed bodies return an error. Non-common names take a cheap name check and
skip the catalog lookup, avoiding extra scans for ordinary high-volume entity
and chunk packets.

This corrects direct-call cases that previously interpreted a configuration
bundle/chunk-batch marker, a play code-of-conduct request, post-1.20.1 play feature
flags, or a pre-1.20.2 chunk-batch marker despite those tuples being absent from
the protocol. Normal `Connection` receive handling already resolves packet names
through the exact state/clientbound catalog, so it did not manufacture those
invalid tuples. The fix tightens the standalone semantic-dispatch API.

The existing `CommonPacket::decode` remains a name-only body helper for callers
that provide their own state context. Its chunk-batch start/finish paths now also
reject protocols before 764. Prefer the state-aware helper for arbitrary named
traffic. The earlier reset-chat-only guard is subsumed by the general check.

Three Rust regressions cover 980 combinations: fourteen common names, fourteen
release families and all five states. Expected presence is independently stated
in the tests and compared with both catalog lookup and common/typed dispatch.
Valid bodies reject trailing bytes; invalid tuples skip malformed/over-budget
bodies rather than attempting to decode them. Unknown names retain raw fallback.
The focused regression was run before the fix and failed on configuration
`bundle_delimiter`; the corrected implementation passes.

Release/state boundaries were checked against all fourteen unchanged hash-pinned
schemas and existing packet audits. This change adds no new packet layout,
side effect, network action or claim of live-server acceptance.
