# Debug samples and subscription envelopes

`packet::debug` adds three bounded Play envelopes:

| Packet | Direction | Protocol families | Ordered body |
| --- | --- | --- | --- |
| `DebugSample` / `debug_sample` | Clientbound | 766–776 | VarInt count, signed i64 samples, sample-kind VarInt |
| `DebugSampleSubscription` / `debug_sample_subscription` | Serverbound | 766–772 | sample-kind VarInt |
| `DebugSubscriptionRequest` / `debug_subscription_request` | Serverbound | 773–776 | VarInt count, debug-subscription registry-ID VarInts |

The legacy request disappears at 773; it is not an alias for the modern list.
The sample kind is the closed `TickTime = 0` enum in the inspected releases.
Unknown or negative enum values are rejected. Samples retain their entire signed
i64 domain, with no units, timing computation or fixed sample-count requirement.
Incoming typed dispatch returns `DecodedPacket::DebugSample`.

Modern subscription IDs are explicitly **unresolved registry references**, not a
new closed enum. Nonnegative i32-domain IDs are retained, including values outside
the pinned default registry's 0–15 range; callers must resolve them against the
appropriate registry before expecting server acceptance. Wire order and duplicate
entries are preserved. The inspected vanilla implementation resolves the request
into a set, so wire preservation does not imply duplicate application effects.

No packet is sent by decoding. There is no automatic subscription, timer,
telemetry collection, handler invocation or debug rendering. Outbound builders
require explicit application use and do not imply server permission.

## Resource and parser policy

- Sample arrays obey the caller's collection and full-body byte budgets. The
  required eight bytes per sample plus at least one enum byte are checked before
  allocation. Huge counts with tiny bodies fail without reserving those arrays
- Subscription requests obey the caller's collection budget and a conservative
  hard cap of 32 entries for all four supported modern families. The cap is
  independently confirmed in the official 26.2 codec; earlier-family shape
  evidence does not independently establish their receiver allocation caps
- All output fields/counts/byte sizes are validated before reserving one output
  buffer. Fixed enum-only requests consume no collection budget
- Missing data, overflowing/negative counts, invalid enum values, negative or
  out-of-i32-domain registry references, excessive collections and trailing bytes
  are rejected. Accepted nonminimal VarInts canonicalize when re-encoded
- Body readers/writers are transactional on failure. `read` consumes one body
  and can leave enclosing bytes; `decode` requires exact completion

The more complex debug block/entity/chunk updates, debug events and game-test
packets remain outside this slice. Their names in the catalog do not imply that
this module interprets their nested payloads.

## Evidence and tests

The unchanged SHA-256-pinned fourteen-family schemas establish 22 present
packet/version combinations and exact IDs, directions, replacement boundaries,
array element widths and modern registry-ID representation.
`tools/check_debug_sample_fixtures.py` independently encodes 75 original fixtures:
empty/zero/signed-edge/multi-byte-count samples, the legacy enum request, and
modern empty/default-registry/duplicate/unresolved/max-count requests. Six
self-contained Python regressions test schema presence, dispatch, width and
registry drift without downloaded schemas or network access.

Rust tests compare original expected semantics, exact bytes/IDs and typed
routing, all 12,006 strict fixture prefixes, trailing bytes, exact/undersized
packet budgets, transactional I/O, maximum counts, malformed VarInts/enums and
invalid registry references. A framed in-memory connection checks that incoming
samples produce no subscription or response.

[Immutable source and artifact facts](validation/debug-samples-wire-facts.json)
record independent PacketEvents readers/writers plus static official 1.20.6 and
26.2 class hashes. Official 1.20.6 mappings bind the obfuscated sample, legacy
subscription and one-member sample enum; official 26.2 also binds the modern
registry collection and its 32-entry cap. PacketEvents' legacy-wrapper version
annotation is narrower than its actual protocol presence, so the earlier
boundary comes from the official 1.20.6 class and exact pinned schema matrix,
not that annotation. No upstream serializer or game class was executed and no
proprietary source, class content or mappings are redistributed.

Reproduce the schema/fixture audit with
`python3 tools/check_debug_sample_fixtures.py` after fetching the pinned schemas.
This is source/static/fixture evidence, not live debug-subscription acceptance.
