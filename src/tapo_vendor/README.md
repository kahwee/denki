# Bundled Tapo transport

This is the default-feature Rust source of `tapo` 0.12.2 from crates.io
(upstream: https://github.com/mihai-dinculescu/tapo), under the adjacent MIT
`LICENSE`. Original crates.io archive SHA-256:
`bff4e7a5ac30410bd928f2b2b75f77e515e0cd88e3b67567c20cf6d9e4708498`. It is private implementation code, not a new supported Denki API.
Upstream device-support claims in comments are not Denki hardware verification.

Why bundled: upstream constructs its own reqwest clients and offers no response
body limit or client-injection hook. A normal dependency cannot enforce a cap
before upstream buffers responses. A Cargo path override would be lost when
publishing Denki to crates.io. Keeping this code under `src/` ensures source
packages and installed binaries use the same bounded implementation.

Security changes:

- `api/protocol/bounded_response.rs` enforces a 1 MiB limit, checking advertised
  length before reading and actual chunks before extending the response buffer.
- All buffered HTTP control responses in KLAP, AES SSL and TPAP use that reader.
  Non-success responses and unused KLAP handshake2 bodies are dropped without
  buffering. Oversized discovery fails rather than triggering protocol fallback.
- SHA-crypt credential transforms enforce 1,000–100,000 rounds (the standard
  default and upstream device fixture use 5,000; 10,000-round vectors remain
  supported). Malformed present round counts are rejected, not defaulted. Both
  credential transformation and PBKDF2 run on the blocking pool with independent
  work limits, because caller timeouts cannot cancel started blocking work.
  SHA-crypt salts must use the crypt ASCII alphabet and are truncated to 16 bytes.
  These are local defensive budgets, not claims of hardware verification.
- JSON HTTP parse errors do not echo device-controlled input.
- Control and discovery logs omit raw requests/responses, device addresses,
  configuration values, session sequences and free-form errors. Only static
  phases, HTTP statuses, fixed sub-method names and numeric error codes remain.
- KLAP checks response signatures in constant time before decryption, binding
  ciphertext to its request sequence; handshake hash comparison is constant time.
- Auto HTTP clients disable redirects and environment proxies so device
  authentication stays at the selected endpoint. Device TLS still permits
  self-signed certificates as upstream requires; this is not PKI identity
  verification and protocol authentication remains essential.
- Offline loopback tests cover missing length, chunked responses, conflicting
  chunked/false length, truncated declared bodies, exact boundary, and overflow.

Maintenance: RSA discovery uses the stable rsa 0.10 `PrivateKey`/`PublicKey`
names rather than their deprecated aliases.

Mechanical embedding changes: `crate::` paths refer to `crate::tapo_vendor::`;
upstream optional feature predicates are fixed to their original default (off);
upstream examples are ignored because they reference an external `tapo` crate;
unused implementation items are allowed. Root Cargo dependencies replace the
upstream crate's default dependency graph. Upstream protocol logic and device
handler interfaces otherwise remain intact.

Maintenance: compare this directory against future upstream releases, preserve
these limits and tests, and remove the copy only when an upstream release offers
an equivalent enforced streaming cap on every control response. Do not replace
this with a versioned path dependency, which would silently discard the fix in
registry packages. No live device validation was performed for this patch.
