# Issues exposed by real Paper testing

The first run exposed two legacy-protocol interoperability cases that synthetic roundtrip tests had not caught:

1. **1.20.1 settings timing.** Sending Play client settings immediately after Login Success caused Paper to report `Bad packet id 8`. Waiting for the Play Join Game packet before sending settings resolved the disconnect and allowed world entry.
2. **1.20.1 chunk section padding.** Paper 1.20.1 build 196 wrote a declared 2,268-byte section-data array with 2,245 bytes of actual section data plus 23 zero padding bytes. The disposable flat overworld had one nonempty section and 23 singleton-air sections. The strict decoder originally rejected the padding. The repaired decoder allows only protocol 763's exact, all-zero, singleton-section padding pattern; arbitrary trailing section bytes and trailing whole-packet data remain rejected. A synthetic fixture can reproduce this shape without distributing captured Minecraft/world data.

The repaired example also reads typed Join Game metadata and reports the world dimension. The tested overworlds report minimum Y -64 and height 384 (24 sections).

The accompanying final test results record whether each repaired client completed the real-server test. A successful semantic decode does not assert byte-identical reconstruction of vendor padding.
