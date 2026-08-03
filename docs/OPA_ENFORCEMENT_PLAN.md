# Aetheris OPA Enforcement — Wiring Plan

Status: Phase 0 (plan). Build starts after this merges. Branches off `main` post RAG-prod (PR #9).

## 0. Purpose & framing

OPA is the last hardening item on the native Aetheris box. **Today it is decorative, not enforcing.** The `SecurityBridge`/`OpaBridge` exists, but it is not a route gate, it has two correctness bugs, the input it sends does not match the policy, OPA itself was scaffolded as a Docker service, and there is no version-controlled policy or native service.

Framing that drives every decision: **Cloudflare Access already gates identity at the edge** (email-OTP allow-list, live on all six subdomains). OPA is *defense-in-depth* — fine-grained, local, microsecond authorization behind CF Access — **not the primary gate.** Hence the failure posture is fail-open-with-alert, not fail-closed: an OPA outage must never lock Nrupal out of his own box.

This touches the **live auth path** — the highest-risk change in the stack. It proceeds strictly in phases; enforcement is gated behind an env flag with instant rollback.

## 1. Grounded findings (source @ `edb46c3`)

- **F1 — Input contract mismatch.** `OpaBridge::authorize` sends `input: { peer_id, action }`; the bootstrap-generated `policy.rego` decides on `input.user_role` / `input.method`. With `default allow = false`, the policy never matches → deny-all. Wiring as-is = self-lockout.
- **F2 — Decision-parse bug (fail-open on explicit deny).** `authorize` reads `body.get("result").is_some()`. OPA returns `{"result": false}` for a DENY; `.is_some()` is `true` whenever the key exists → **explicit deny is read as allow**. Must be `body["result"].as_bool().unwrap_or(false)`.
- **F3 — Fail-closed on unreachable.** `Err(_) => false`. As a global gate, OPA down = total lockout.
- **F4 — No enforce flag; stale endpoint default.** `Config` has no enforcement toggle → no safe staged rollout. `opa_endpoint` defaults to `http://opa:8181` (Docker hostname); a native run without `OPA_ENDPOINT` dials an unresolvable host.
- **F5 — No version-controlled policy; OPA was Dockerized.** Rego exists only as a heredoc in `scripts/bootstrap.sh`; docs list `openpolicyagent/opa` as a Docker image. Native needs a systemd `opa.service` loading real `config/policy/*.rego`.
- **F6 — Identity source is stale.** `peer_id` is the old WireGuard concept. Under CF Access, identity arrives as `Cf-Access-Authenticated-User-Email` + `Cf-Access-Jwt-Assertion`. The gate must derive identity from CF Access, not `peer_id`.

Net today: F1 (deny-all) and F2 (deny-read-as-allow) cancel into "allow everything." OPA enforces nothing.

## 2. Adopted decisions (recommended defaults; override at review)

- **D1 — OPA unreachable → fail-open + loud log + `aetheris_security_violations_total` bump.** CF Access is the primary gate; no-lockout is absolute. An explicit `deny` from a *reachable* OPA is always honored.
- **D2 — Enforce first on mutating/sensitive routes** (vault write, upload, admin/bridge, model proxy). Static panels stay CF-Access-only. Every route gets a shadow pass before it blocks.
- **D3 — Roles:** `nrupalakolkar@gmail.com` → `admin` (full); CF Access service token → `analyst` (GET-only); else deny.
- **D4 — Policy source of truth:** version `config/policy/aetheris.authz.rego` in-repo; stop generating it in `bootstrap.sh`.

## 3. Phased build

Each phase = branch + PR + green CI. No direct-to-main. No Docker. Box changes start at Phase 2, only after the prior phase merges and Nrupal says go.

- **Phase 0 — Plan-doc PR** (this document).
- **Phase 1 — Correctness + contract (no enforcement).** Fix F2 parse; redefine OPA input to `{ identity, role, method, path, action }`; add `opa_enforce` (default `false`) + `opa_fail_open` (default `true`) to `Config`; fix `opa_endpoint` default to `http://127.0.0.1:8181`; add versioned `config/policy/aetheris.authz.rego` matching the new input; unit + integration tests (allow admin, deny unknown, GET-only analyst, explicit-deny-honored, unreachable→fail-open). Bridge still not a gate.
- **Phase 2 — Native OPA service.** `opa.service` systemd unit: `opa run --server --addr 127.0.0.1:8181 --set decision_logs.console=true /etc/aetheris/policy/`, loopback-only. Deploy + healthcheck; strip Docker-OPA remnants. Acceptance: `curl 127.0.0.1:8181/health` + decision probe.
- **Phase 3 — Shadow enforcement (observe, never block).** Axum `from_fn_with_state` middleware after CF Access identity extraction; calls the bridge, logs the decision + bumps the metric, returns Allow regardless (`opa_enforce=false`). Soak against real traffic. Verify own identity decides `allow`; would-be denies logged.
- **Phase 4 — Enforce.** Flip `OPA_ENFORCE=true` on sensitive routes (D2). Real `403` on deny; fail-open on unreachable (D1). **Rollback = flip env flag off (instant, no redeploy).**
- **Phase 5 — Harden + close.** Rotate the `dev_user` basic-auth credential; tighten policy (path/sensitivity); update `SECURITY.md` + compliance checklist so "OPA default-deny returns false for unauthorized actions" is actually true; acceptance sign-off.

## 4. No-lockout guarantees

- Enforcement behind an env flag → instant rollback without rebuild.
- Fail-open on OPA-unreachable → an OPA outage never locks Nrupal out (CF Access still gates).
- Shadow phase proves own identity is allowed **before** anything can block.
- OPA bound to `127.0.0.1:8181` → never exposed.
- Every phase: branch + PR + green CI; no direct-to-main; no Docker.

## 5. Confirm at build kickoff (not blockers)

- Exact `main.rs` seam: current `authorize` call appears to sit on the `/bridge/security/*` endpoint, not a route gate — confirm the precise middleware insertion point when Phase 3 starts.
- Verify CF Access passes the JWT/identity header through to origin (default) — confirm on-box.
