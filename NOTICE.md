# NOTICE — Telayd

Telayd is licensed under the Functional Source License 1.1 with MIT Future License (FSL-1.1-MIT).
See `LICENSE` for the full license text.

---

## Third-party Software Attributions

### cloudflared

- **License**: Apache License 2.0
- **Source**: https://github.com/cloudflare/cloudflared
- **Copyright**: Copyright (c) Cloudflare, Inc.
- **Usage in Telayd**: `telayd init` auto-downloads the cloudflared binary to
  `~/.local/bin/cloudflared` for Cloudflare Quick Tunnel support (L0 transport layer).
  The binary is NOT bundled in the source tree; it is downloaded at runtime.
- **SHA256 verification**: the binary SHA256 hash is verified against the official
  `SHA256SUMS` file published in the same GitHub release tag before installation.

A copy of the Apache License 2.0 is available at:
https://www.apache.org/licenses/LICENSE-2.0

---

## L0 Known Supply-Chain Limitations (IG6 — Phase 5 Round-1 diagnosis)

The following limitations are **explicitly accepted for L0** (single-user dogfooding scope)
and are scheduled for remediation in L1:

| Item | L0 Status | L1 Plan |
|---|---|---|
| **cloudflared SHA256SUMS pinning** | SHA256SUMS fetched from same-tag release at install time. Hash is NOT hardcoded/pinned in source. | Pin verified hash in source + Homebrew tap formula auto-update on release. |
| **cargo audit / cargo deny not in CI** | Manual sweep only. `cargo audit` is not run as a CI gate. Scope §7.1 security table lists L0 as "crate audit on release". | Add `cargo deny check` to CI gate in L1. |

To perform a manual audit: `cargo install cargo-audit --features=fix && cargo audit`
