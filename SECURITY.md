# AETHERIS - SECURITY SPECIFICATION

## Supported Versions

Version 0.1.0 is currently supported.

## Reporting a Vulnerability

Please report vulnerabilities via GitHub's private vulnerability reporting.

## Deployed Security Posture

- Loopback bind on 127.0.0.1:8080 behind Cloudflare Tunnel
- Cloudflare Access RS256 JWT verification enforced with 5 audiences
- OPA default-deny enforced
- Systemd-hardened native service
- Append-only deploy audit (audit.jsonl)
- Gated deploy-runner trust model (git-ancestry + sha256 + confirm-token)

Note: `/metrics` + `/dev/metrics` intentionally unauthenticated but loopback-only behind the tunnel (documented exception; further hardening tracked separately).