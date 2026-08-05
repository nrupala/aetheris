# OPA P5 — JWT Identity Verification (HARDENING PLAN, not yet enabled)

> Status: **Planned**. Today's OPA identity is a plaintext `Cf-Access-Authenticated-User-Email`
> header injected by Cloudflare Access. That is only safe because the core binds
> `127.0.0.1:8080` (loopback) and iptables refuses external NEW connections to 8080 —
> so the only ingress is the cloudflared tunnel, which fronts CF-Access-protected hosts.
> This forwards a *signed* `Cf-Access-Jwt-Assertion` token that we currently ignore.

## Problem
Relying on the plaintext email header means header-trust depends on the loopback+iptables
boundary staying intact. Anyone who can reach core directly (or misroutes) could spoof
`Cf-Access-Authenticated-User-Email: nrupalakolkar@gmail.com` → `admin`.

## Goal — verify the signed assertion
- Accept the plaintext email header today (backward compatible), but when hardened, verify
  the signed `Cf-Access-Jwt-Assertion` before trusting the identity.
- CF Access validates the JWT (HS256 keyed by your Access Application AUD + the account's
  `teamdomain`/cert), with a short expiry + nonce. On verify success, extract the `email`
  claim as the authoritative identity.

## Design
- New env flag **`CF_JWT_VERIFY`**, default **off** (matching today's behavior).
- When on, `access_role(email, client_id)` path in `opa_gate` verifies the
  `Cf-Access-Jwt-Assertion` first; only a verified JWT whose `email` claim equals
  `nrupalakolkar@gmail.com` maps to `admin`. Unverifiable assertions → `unknown` (deny on
  sensitive routes) rather than trusting the plaintext header.
- Keep the plaintext header as a fast-path fallback ONLY while `CF_JWT_VERIFY=off`.

## JWT verification details (CF Access)
- Headers (typical): `Cf-Access-Jwt-Assertion`, `Cf-Access-Client-Id`,
  `Cf-Access-Client-Secret` (service tokens), `Cf-Access-Authenticated-User-Email`.
- Key: the JWT is signed with the applied-to Application's AUD secret. Must not be confused
  with the origin security secret; use a verified copy obtained via the Cloudflare
  dashboard (`Team → Applications → <app> → Keys`).
- Claims: `aud` == the Access application AUD; `email` == the authenticated user; `exp`
  within a short window; reject on any mismatch/expiry.

## Acceptance (when implemented)
- With `CF_JWT_VERIFY=1`: valid JWT `(aud, exp, email=nrupalakolkar@gmail.com)` → `admin`, allow.
- Forged / expired / wrong-aud JWT → `unknown`, denied on sensitive routes.
- Malformed header → `unknown`, never `admin`.
- Plaintext-email spoof (no valid JWT) → `unknown`, denied on sensitive routes (closes the
  spoof gap).

## Rollback
- `CF_JWT_VERIFY=0` (or unset) restores today's header-trust behavior. No rebuild.

## References
- `AGENTS.md` → "Security posture — OPA authorization (LIVE)".
- `core/src/main.rs` → `opa_gate`, `access_role`, `identity_to_role`.
