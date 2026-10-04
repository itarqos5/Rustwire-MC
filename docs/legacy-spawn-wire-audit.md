# Legacy spawn packet wire audit

`packet::entity` now exposes `SpawnExperienceOrb` and `SpawnPlayer`, carried by
`EntityPacket::ExperienceOrb` / `PlayerSpawn` and `DecodedPacket::Entity`.
The existing generic `SpawnEntity` and movement codecs are unchanged.

## Exact release scope

The fourteen unchanged hash-pinned schemas in `research/schema-hashes.json`
contain eight present packet/version combinations and twenty absent combinations:

| Packet | Supported protocol families | Clientbound Play ID | Body |
|---|---|---|---|
| `spawn_entity_experience_orb` | 763–769 (1.20–1.21.4) | `0x02` | signed VarInt entity ID; three f64 coordinates; signed i16 experience value |
| `named_entity_spawn` | 763 (1.20/1.20.1) | `0x03` | signed VarInt entity ID; 16 UUID bytes; three f64 coordinates; yaw byte; pitch byte |

For every family, the verifier checks the packet type, ID mapper and body-switch
binding, including exact absence after removal. No neighboring-version fallback
is used. Direct encoders/decoders check the exact catalog before reading or
writing fields. The family envelope and typed Play dispatcher enforce the same
boundaries. Other states do not dispatch these names, and unrelated unknown
names retain the existing raw fallback behavior.

The orb schema calls its final field `count`; independent implementations and
Mojang's mapped `value` field identify it as the orb's experience value. It is not
a collection length. `SpawnPlayer` corresponds to the legacy named-entity packet,
not player-list updates. Its yaw precedes pitch, unlike generic spawn's ordering.
This increment does not construct equivalent generic-spawn packets for releases
where either legacy packet was removed.

## Scalar policy and bounds

- Signed entity references and the entire signed-short experience-value domain
  are retained; these codecs do not impose inferred gameplay validity
- UUID bytes and all byte angles are retained, using existing `Angle(u8)` for
  modulo-turn interpretation without float-angle conversion
- All f64 bits are retained, including negative zero, infinities, subnormals and
  NaN payloads. This does not assert those values are accepted by a game client
- Strict completion rejects trailing bytes; every strict prefix is rejected
- The shared VarInt reader rejects overflow and excessive continuation, accepts
  non-minimal encodings and canonicalizes them when re-encoding
- Decode checks `max_packet` before parsing. Encode returns no partial packet on
  failure and checks the completed fixed-size body against `max_packet`; at most
  31 orb-body or 47 player-body bytes are produced before that check
- No collection allocation or collection-budget consumption occurs. A zero
  `max_collection` is valid for these fixed-size bodies

The raw-value policy is scoped to these additions. Existing generic spawn's
nonnegative-ID and finite-coordinate checks remain unchanged.

## Independent source evidence

The [source-facts manifest](validation/legacy-spawn-wire-facts.json) records
immutable URLs, SHA-256 content hashes, Git blob hashes and declared protocol
numbers for sixteen inspected MCProtocolLib source files. The independent
implementation sources cover **every present protocol family**, plus the first
absent boundaries:

| Protocol | Source reference | Immutable commit |
|---|---|---|
| 763 | 1.20-1 | `caa35c7be4ff6dea3a8b9bc05558cd0bc18b2c3c` |
| 764 | 1.20.2-1 | `a0cd1e1cef2e4ef3987381376ddaaacc8e20cab8` |
| 765 | release 1.20.4-1 PR #772 merge | `5103ededc8c4d9d1f65a00b023579cee77edd35b` |
| 766 | 1.20.6-1 | `8dbdffc0004746122efd1fbe63d1c1620a8336db` |
| 767 | 1.21-1 | `e71d7df8c127b7196fd81dd392034d7d97acbb4e` |
| 768 | 1.21.2-1 (declares 1.21.3/protocol 768) | `667e02d38cd4a79d46640312cd599123609633ac` |
| 769 | 1.21.4-1 | `7cc247026c6bff700c644d545942f7733ebb6ad0` |
| 770 | 1.21.5-1 | `f4004310698f9e3793e6619a6db89ae0a7784a4d` |

The source readers/writers agree on the field sequence and primitive widths.
MCProtocolLib converts player angles through degree-valued floats; Rustwire
retains the original byte values. Each selected `MinecraftCodec` declaration
explicitly identifies the expected protocol. The 764–770 source trees lack the
legacy player class/registration; 770 also lacks the legacy orb class/registration.
Later absent families remain schema-verified, not separately source-audited here.
The protocol-765 reference is an immutable merge commit, not an assumed tag.

Primary source entry points are the
[1.20 release](https://github.com/GeyserMC/MCProtocolLib/releases/tag/1.20-1),
[protocol-765 release change](https://github.com/GeyserMC/MCProtocolLib/pull/772),
[1.21.4 orb codec](https://github.com/GeyserMC/MCProtocolLib/blob/7cc247026c6bff700c644d545942f7733ebb6ad0/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/packet/ingame/clientbound/entity/spawn/ClientboundAddExperienceOrbPacket.java)
and [1.21.5 registration](https://github.com/GeyserMC/MCProtocolLib/blob/f4004310698f9e3793e6619a6db89ae0a7784a4d/protocol/src/main/java/org/geysermc/mcprotocollib/protocol/codec/MinecraftCodec.java).
All inspected files have exact links in the manifest.

A narrow static inspection of Mojang's official **1.20.6** inner server JAR also
confirms the orb constructor/writer order: VarInt, three doubles, signed short.
The inner JAR SHA-256 is
`bb610daa96b3784645b78fd7c0b887291414c9605cd913fc79cf985f523f4f5b`;
class and official mapping hashes/URLs are recorded in the manifest. Inspection
used the JDK's `jdk.jdeps/com.sun.tools.javap.Main` entry point, not game execution.
There is no official static player inspection or official all-release claim.

## Fixtures and evidence limits

`tools/check_legacy_spawn_fixtures.py` verifies the exact schema hashes,
presence/layouts, IDs and switch bindings and independently generates **62**
original synthetic bodies with Python's standard library. No Rustwire encoder,
MCProtocolLib serializer or Mojang serializer generates those bytes.
The Rust tests cover direct decode/encode, family dispatch and typed dispatch,
all **1,910** strict body prefixes, trailing data, exact and undersized packet
budgets, zero collection limits, signed scalar domains, UUID order, float bits,
angle-byte order/range, malformed VarInts and every version/state boundary.

Fourteen Python regression tests construct small synthetic schemas to exercise
hash, shape, mapper/switch and absence drift; they require no downloaded schemas
or networking. These are source/static comparisons and independent synthetic
fixtures, **not executed upstream-serializer outputs, captures, live-server
interoperability or gameplay validation**. Only original Rust/Python/test bytes,
factual documentation and source-location/hash metadata are published; upstream
implementation text, official JARs/mappings/disassembly and private captures are
not included.

## Reproduction and focused validation

```sh
python3 tools/check_legacy_spawn_fixtures.py
python3 -m unittest discover -s tools -p 'test_check_legacy_spawn_fixtures.py' -v
cargo test --locked --offline --no-default-features \
  --test legacy_spawn --test entities --test typed
cargo clippy --locked --offline --no-default-features \
  --lib --test legacy_spawn --test entities --test typed -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline --no-default-features --no-deps
cargo fmt --all -- --check
```

Focused validation used Rust 1.99, two build jobs, a separate target directory,
zero incremental compilation and zero dev/test debug information. Six new and
nine existing Rust tests, strict Clippy/rustdoc/format checks, exact pinned-schema
checks and all fourteen isolated schema-free/network-disabled Python tests passed.
Full integrated feature matrices, exact Rust 1.88/MSRV and cross-platform CI are
separate integration gates; this focused result does not assert they ran here.
