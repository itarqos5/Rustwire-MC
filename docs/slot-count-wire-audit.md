# Component-slot count boundary correction

Component slots use a **VarInt count from protocol 766 (1.20.5/1.20.6)**,
not from 767. Classic slots through 765 keep their signed-byte count.
The pinned protocol-766 schema labels this field `i8`; that annotation is stale.
The shared inventory reader/writer now uses the release framing, including in
merchant results, advancement icons, nested component items and recipe results.

## Direct official release evidence

Read-only static inspection on 2026-10-04 used the official 1.20.6 release:

- [Release metadata](https://piston-meta.mojang.com/v1/packages/84c5b02d36a9be4560597687f9e2d19d69fada5d/1.20.6.json)
- [Server bundle](https://piston-data.mojang.com/v1/objects/145ff0858209bcfc164859ba735d4199aafa1eea/server.jar)
  SHA-256 `c6d01d018ca782e506f0ec60652d47fd565078be9122b625c1681bc86c29c7ec`
- Inner `META-INF/versions/1.20.6/server-1.20.6.jar`
  SHA-256 `bb610daa96b3784645b78fd7c0b887291414c9605cd913fc79cf985f523f4f5b`
- [Official server mappings](https://piston-data.mojang.com/v1/objects/9e96100f573a46ef44caab3e716d5eb974594bb7/server.txt)
  SHA-256 `4e5068e40a89c673b5541192ad7eab96289f6dcae704f577f8992bdf7c984aa3`

The inner archive's `version.json` identifies protocol 766. Official mappings
identify ItemStack as `cur`, its OPTIONAL_STREAM_CODEC as `h`, its anonymous
codec as `cur$1`, RegistryFriendlyByteBuf as `xa`, and FriendlyByteBuf as `wm`.
The ItemStack initializer assigns the anonymous codec to that field. The codec's
first decoder integer read calls `xa.l:()I`, which maps to `readVarInt`; its count
encoder calls `xa.c:(I)Lwm;`, which maps to `writeVarInt`. Those method bindings
establish the count width, rather than inferring it from small-count examples.

To reproduce, download the linked bundle and mappings, verify the above hashes,
extract the named inner archive, inspect its `version.json`, and inspect the
mapped classes with a JDK's `javap -c -p -classpath server-1.20.6.jar 'cur$1'`
and `cur`. When the `javap` launcher is absent but the JDK module is present,
`java --module jdk.jdeps/com.sun.tools.javap.Main` accepts the same arguments.
Check method names against the official mapping file. This inspects class data;
it does not load or execute the game's serializers or start a server.

Release-era ViaVersion and PacketEvents implementations independently use
VarInt for this same boundary. The official static bindings above are the
primary evidence. Game binaries, mapping text and disassembly are not included
in this repository.

## Regression coverage and limits

Original explicit fixtures for counts 128 (`80 01`) and 300 (`ac 02`) now exercise
both decode and exact encode for every component-slot family, 766–776, with
truncation and trailing-byte rejection. Advancement icon tests also exercise
count 128 in protocol 766. Existing limits, negative-count rejection and classic
signed-byte checks remain in place; this does not relax item semantics or impose
vanilla stack-size gameplay rules.

This is a static release-binding audit plus Rust fixture coverage, not a newly
executed serializer oracle or live-server result. Earlier ordinary-count live
checks did not distinguish one-byte VarInts from signed bytes and cannot prove
this boundary. No Microsoft account authentication or gameplay validation is
claimed by this correction.
