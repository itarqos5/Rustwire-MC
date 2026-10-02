# Rustwire

A Rust Minecraft Java protocol library in active initial development.

Cargo package: `rustwire-mc`. This repository is being built in small, testable commits. The target is release protocols from Java 1.20 through 26.2. A packet catalog alone is not full protocol or gameplay support; verified feature coverage and test results will be documented as each component lands.

The first implementation will prioritize bounded binary codecs, framing and compression, connection state, authentication helpers, semantic chunks and registries, and a lean default build. No crates.io publication has occurred.
