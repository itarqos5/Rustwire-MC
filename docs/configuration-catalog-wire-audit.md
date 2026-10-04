# Configuration metadata direction correction

The four raw pinned schemas for protocols 767–770 (1.21–1.21.5) erroneously
append `custom_report_details` and `server_links` to the serverbound configuration
registry at IDs 8 and 9. Rustwire previously reproduced those raw entries. They
are now removed from the generated catalogs. Clientbound configuration/play
metadata packets, IDs 0–7 serverbound, and all other release/state/direction
registries are preserved.

This is a catalog correction, not two missing outbound packet builders. Both
metadata families are sent by the server. Callers using `packet::named` or
`Version::packet_id` now receive an unsupported-packet error for the nonexistent
outbound names. Raw packet construction remains available but does not confer
validity on arbitrary packet IDs.

## Independent evidence

[The source manifest](validation/configuration-catalog-sources.json) pins six
primary source files by immutable commit URL and SHA-256:

- MCProtocolLib release bindings explicitly declare protocols 767, 768, 769 and
  770 with their matching Minecraft versions. Each complete configuration
  registry contains exactly eight serverbound entries, ending with known-pack
  selection, and registers the report/link packets clientbound
- PacketEvents v2.13.0 selects its 1.20.5 serverbound configuration map until the
  1.21.6 boundary. That exact map likewise contains only the same eight entries;
  report details and server links appear only in the clientbound definitions

The implementations independently agree on both direction and ordered IDs.
This check inspects source declarations; it is not a new official server runtime
or captured-traffic test. Only protocol facts/hashes are included here, not
upstream implementation contents.

## Guarded regeneration and regressions

`tools/generate_catalog.py::corrected_mapping` applies the correction only to the
four affected protocol/state/direction tuples. It first requires an exact match
for all ten historical input entries; any different input stops regeneration
for review. `research/protocols` and `research/schema-hashes.json` are unchanged.
The existing protocol-776 spectator/action correction is preserved separately.

Four self-contained Python tests exercise affected/unaffected tuples, exact-input
drift guards and the existing 776 correction without loading schemas or running
generation side effects. The Rust catalog regression checks all fourteen
families, both preserved clientbound registrations and rejection of the phantom
outbound names/IDs. The server-metadata schema verifier intentionally still
checks the raw anomaly, while its synthetic fixtures remain clientbound.
