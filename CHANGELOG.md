# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-08-30

### Added
- Gated blue-green deploy-runner (validate → stage → scratch-smoke → cutover → rollback → audit) with git-ancestry + sha256 + confirm-token trust gates.

### Changed
- Docker→native cutover complete; the core now runs as a native musl systemd binary on 127.0.0.1:8080 behind Cloudflare Tunnel. Ollama trimmed to 5 models.

### Fixed
- `/api/*` sensitive routes are now correctly gated by OPA (previously reachable) — commit 80bb8243.

### Removed
- Repository hygiene: 27 loose root files archived; 19 MB vendored installer dropped from HEAD.

### Security
- OPA authorization is default-deny and fully enforced (OPA_ENFORCE=1). Cloudflare Access JWT verification enforced (CF_JWT_VERIFY=1, RS256, `Cf-Access-Jwt-Assertion`) with 5 configured audiences.
