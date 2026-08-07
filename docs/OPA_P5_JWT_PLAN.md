# OPA P5 — JWT Identity Verification (LIVE: enforced via CF_JWT_VERIFY=1)

> Status: **Enforced (CF_JWT_VERIFY=1 since 2026-08-06)**. P5.2 shadow (log-only) soaked
> clean for 48h; P5.4 made the flag functional — the verified `Cf-Access-Jwt-Assertion`
> email is the authoritative identity on sensitive routes; unverifiable → `unknown`
> (denied), closing the plaintext-header `Cf-Access-Authenticated-User-Email` spoof gap.
> Shadow (CF_JWT_VERIFY=0) remains available as rollback.

## §0 — Resolved values (locked)
- **Team / issuer:** `https://nrupal.cloudflareaccess.com`
- **JWKS:** `https://nrupal.cloudflareaccess.com/cdn-cgi/access/certs` → fetched to
  `/etc/aetheris/cf_access_jwks.json` (0644); hourly systemd timer refresh, keep last-good on failure.
- **Signing algorithm:** RS256 (`jsonwebtoken` crate).
- **`CF_ACCESS_AUD`** (comma-list of the Access Application AUDs whose hostnames route to
  **core `127.0.0.1:8080`** — ai/dev/rag/oracle; **EXCLUDE agents+mgmt** which are separate Python services):
  ```
  ai    = 03bbed24857c478bab5404901b443308631d881bc5fd68fa1894ca2b1df3e756
  dev   = 358ec69acf19657a591085df7f5382915491746df9afbe8328d3e02e0974a717
  rag   = c142cabc3ce3ff0938508da8974a523af5919888d19407201bc5f4b18e808651
  oracle= 7d0506af81682ff17518a336c443415ae37e3f8a99f98e3df53d592217b2ca03
  ```
- **Flags (in `/etc/aetheris/core.env`):**
  ```
  CF_ACCESS_TEAM_DOMAIN=nrupal.cloudflareaccess.com
  CF_ACCESS_AUD=<comma-list above>
  CF_ACCESS_JWKS_PATH=/etc/aetheris/cf_access_jwks.json
  CF_JWT_VERIFY=0        # default OFF (shadow). Rollback = set 0 + restart.
  ```

## Problem
Plaintext-email header-trust depends on the loopback+iptables boundary holding. Anyone
who reaches core directly could spoof `Cf-Access-Authenticated-User-Email` → `admin`.

## Goal — verify the signed assertion (shadow → opt-in enforce)
- Verify the signed `Cf-Access-Jwt-Assertion` (RS256, pinned JWKS, `iss`, `aud ∈ CF_ACCESS_AUD`,
  `exp`/`iat` with small leeway). On success, the `email` claim is the authoritative identity.
- **Shadow (CF_JWT_VERIFY=0):** on sensitive routes, verify and LOG header-email vs jwt-email
  mismatches + verify failures. **Identity still comes from the plaintext header. Block nothing.**
- **Enforce (CF_JWT_VERIFY=1, a later flip):** only a verified JWT (email == owner) maps to
  `admin`; unverifiable/spoofed → `unknown`, denied on sensitive routes.

## Design
- New crate: `jsonwebtoken` (RS256).
- New module `src/auth/cf_jwt.rs`: `verify_assertion(headers) -> Result<VerifiedIdentity{email, sub}, JwtError>`
  — read `Cf-Access-Jwt-Assertion`; decode header → `kid`; match against pinned JWKS
  (loaded from `CF_ACCESS_JWKS_PATH`); verify RS256 signature; validate
  `iss=https://nrupal.cloudflareaccess.com`, `aud ∈ CF_ACCESS_AUD`, `exp`/`iat` leeway.
- Config: flags above read in `Config::from_env` (mirroring OPA_ENFORCE pattern).
- Bootstrap: on install, `curl` JWKS → `CF_ACCESS_JWKS_PATH` (0644); new hourly systemd
  timer to refresh, keeping last-good on fetch failure.
- Wire into `opa_gate` **shadow only**: on `is_sensitive(method, path)` (the PR#14
  method-aware helper) call `verify_assertion`; log:
  - verify failure (bad sig / wrong aud / expired / missing on sensitive),
  - header-email vs jwt-email mismatch.
  Identity used by OPA remains the plaintext header. **No blocking.**

## Acceptance
- Valid JWT → `VerifiedIdentity{email, sub}`.
- Bad signature → Err; wrong `aud` → Err; expired → Err; missing-on-sensitive → shadow
  log-only; missing-on-nonsensitive → ignored (no log, no work).

## Rollback / flip
- **Flip enforce:** `CF_JWT_VERIFY=1` + `systemctl restart aetheris-core` (independent of
  `OPA_ENFORCE`).
- **Rollback:** `CF_JWT_VERIFY=0` + restart. No rebuild.

## References
- `AGENTS.md` → "Security posture — OPA authorization (LIVE)".
- `core/src/main.rs` → `opa_gate`, `access_role`, `is_sensitive`.
- Ingress evidence note: cloudflared tunnel is token/dashboard-managed (no on-box ingress
  file); `CF_ACCESS_AUD` set above follows ai/dev/rag/oracle → core, agents/mgmt → Python.
