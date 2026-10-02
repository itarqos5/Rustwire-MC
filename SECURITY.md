# Security and trust boundaries

This is an early protocol library. Do not assume an independent security audit.

## Incoming bytes

The default limits cap frames at 2,097,151 bytes and decompressed packets at 8 MiB. Lengths, IDs, UTF-8/UTF-16 lengths, NBT recursion and node budgets are checked. Compressed output must exactly match its advertised size and consume exactly one complete zlib stream. Malformed lengths are rejected before large allocations where possible. The decoder accepts narrowly identified Paper 1.20.1 and 1.21.5 zero-padding patterns only for protocols 763 and 770, respectively.

A failure during network reading/writing poisons the connection, because resuming a partially read frame or advanced cipher state is unsafe. Create a new connection instead. High-level protocol errors should also be treated as terminal by applications.

Raw packet payloads and server strings are untrusted. Render them safely; do not execute commands, open supplied URLs, accept transfers, or download server resource packs automatically. The example capture environment variable is explicit opt-in; captured chunks may contain private world information and must not be committed or shared unintentionally.

## Authentication

- Never log, serialize casually, or commit access/refresh tokens
- Use an application-owned, properly authorized client ID
- Device authorization is initiated only by the caller; polling honors interval and slow_down
- HTTP requests use fixed HTTPS authentication endpoints, TLS validation, bounded bodies, timeouts and disabled redirects
- Tokens have redacted Debug and zeroizing owned storage. Third-party HTTP/JSON internals can create transient copies; perfect zeroization is not claimed
- Session join is explicitly separate from completing the encryption response
- Offline-mode usernames/UUIDs establish no real identity

AES-CFB8, SHA-1, RSA PKCS#1 v1.5 and MD5 offline UUIDs are Minecraft compatibility algorithms. Do not reuse these helpers as a general-purpose security design. The RSA helper only encrypts with a server public key; no private RSA key is used in production library code. RSA private-key operations appear only in offline tests.

### Known RSA dependency advisory

As reviewed on 2026-10-02, the pinned `rsa` 0.9.10 dependency is covered by
[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html), with no
patched release listed. The advisory concerns private-key recovery through
observable timing of private RSA operations. Rustwire's production helper uses
only public-key encryption, so it does not expose that private-key operation;
test-only private keys are synthetic and never serve network requests. The
signed-chat helper exposes canonical bytes to a caller-owned provider and does
not select a cryptographic implementation or handle its private key. This
scope assessment is not a clean dependency-audit claim or a promise that the
dependency is safe for other uses. Do not extend this implementation to network
private-key decryption/signing without a separate cryptographic review and an
appropriate provider.

## Reporting

Avoid posting tokens, private server addresses, captured worlds or exploit-ready private data in public issues. Use GitHub private vulnerability reporting if the repository has it enabled; otherwise contact the maintainer through an existing private channel. This document does not claim private reporting is enabled.

Authentication HTTP regression tests use only loopback mock endpoints and
explicitly synthetic credentials. Their private routing hook is compiled only
under `cfg(test)`; it cannot redirect production authentication requests. Tests
cover credential-redacted errors, redirect refusal, deadlines and body limits.
They are not an independent security audit or a live eligible-account validation.
