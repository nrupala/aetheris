# AETHERIS PHASE 2 - ARCHITECTURE ROADMAP
## Resilient, Isolated, and Sovereign AI-Native Cloud

**Created:** 2026-04-15
**Status:** PARKED (Phase 1 Complete)
**Priority:** High
**Estimated Duration:** 6-8 weeks

---

## Executive Summary

Phase 1 established Aetheris as a sovereign personal cloud with Rust core, WireGuard mesh, OPA zero-trust, ZFS encryption, and Ghost Shell honeypot. Phase 2 elevates the architecture to **zero-leak, crash-resilient** with micro-enclaves, client-side E2EE, hardware-aware inference, and autonomous threat response.

---

## Current Architecture (Phase 1)

```
┌─────────────────────────────────────────────────────────────┐
│                     Aetheris Core (Rust)                     │
├─────────────┬─────────────┬─────────────┬──────────────────┤
│   Gateway   │   Storage   │   Identity  │   AI Policy      │
│   (Axum)   │   (ZFS)     │   (OpenID)  │   (OPA Bridge)   │
├─────────────┴─────────────┴─────────────┴──────────────────┤
│                    WireGuard Mesh (UDP 51820)                │
├─────────────────────────────────────────────────────────────┤
│              Ollama + ChromaDB (Tightly Coupled)             │
├─────────────────────────────────────────────────────────────┤
│              Ghost Shell Honeypot                            │
└─────────────────────────────────────────────────────────────┘
```

### Identified Risks
- **Cascade Failure:** OOM in AI inference → crash entire Rust Core
- **Side-Channel Exposure:** Monolithic memory space increases data exfiltration risk
- **No Client-Side E2EE:** Data encrypted at rest but not E2E
- **Homogeneous Compute:** No hardware-aware task routing

---

## Phase 2 Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                        CLIENT TIER (PWA + Web Crypto)                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │   Android    │  │     iOS      │  │   Windows    │  ...           │
│  │  E2EE PWA   │  │   E2EE PWA   │  │   E2EE PWA   │               │
│  │  Argon2id   │  │   IndexedDB  │  │   Local Key  │               │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘               │
│         │                  │                  │                       │
│         └──────────────────┼──────────────────┘                       │
│                            │                                          │
│              WireGuard Mesh (UDP 51820)                                │
│                            │                                          │
├────────────────────────────┼─────────────────────────────────────────┤
│                     RUST CORE ENCLAVE                                  │
│  ┌──────────────────────────────────────────────────────────────┐     │
│  │              Firecracker MicroVM / cgroups                     │     │
│  │  ┌─────────────────────────────────────────────────────────┐  │     │
│  │  │              Rust Core (Axum)                            │  │     │
│  │  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────────┐  │  │     │
│  │  │  │Gateway  │ │Storage  │ │Identity │ │OPA Bridge   │  │  │     │
│  │  │  └─────────┘ └─────────┘ └─────────┘ └─────────────┘  │  │     │
│  │  └─────────────────────────────────────────────────────────┘  │     │
│  └──────────────────────────────────────────────────────────────┘     │
│                            │                                          │
│  ┌─────────────────────────┼──────────────────────────────────────┐  │
│  │              AI GATEWAY (OpenClaw)                              │  │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────────┐  │  │
│  │  │ Timeout Guard │  │Fallback Logic │  │  Auto-Restart     │  │  │
│  │  └───────────────┘  └───────────────┘  └───────────────────┘  │  │
│  └─────────────────────────┬──────────────────────────────────────┘  │
│                            │                                          │
│  ┌─────────────────────────┼──────────────────────────────────────┐  │
│  │           MICRO-ENCLAVE 1        │     MICRO-ENCLAVE 2          │  │
│  │  ┌─────────────────────┐   │   ┌─────────────────────┐        │  │
│  │  │   Ollama (GGUF)     │   │   │    ChromaDB         │        │  │
│  │  │   q4_K_M Quantized  │   │   │   Vector Store      │        │  │
│  │  │   GPU-Pinned        │   │   │   Isolated Memory   │        │  │
│  │  └─────────────────────┘   │   └─────────────────────┘        │  │
│  └─────────────────────────────┴──────────────────────────────────┘  │
│                            │                                          │
│  ┌─────────────────────────┼──────────────────────────────────────┐  │
│  │              ZFS (AES-256-GCM)                                   │  │
│  │              + Key Sealing                                      │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                            │                                          │
│  ┌─────────────────────────┼──────────────────────────────────────┐  │
│  │              Ghost Shell 2.0 (High-Interaction Honeypot)        │  │
│  │              + Autonomous Threat Response                        │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────────────┘
```

---

## WORK PACKAGES

### WORK PACKAGE 1: Component Isolation & Crash Containment

**Objective:** Prevent cascade failures from AI/database crashes

| Task | Description | Difficulty | Risk |
|------|-------------|------------|------|
| 1.1 | Migrate to Firecracker microVMs | High | Low |
| 1.2 | Implement cgroups memory limits per service | Medium | Medium |
| 1.3 | Create AI Gateway (OpenClaw abstraction) | High | High |
| 1.4 | Add timeout guards (30s default, configurable) | Medium | Low |
| 1.5 | Implement fallback response logic | Medium | Medium |
| 1.6 | Auto-restart crashed AI engine | Medium | Low |
| 1.7 | Health check ping-pong between enclaves | Low | Low |

**Acceptance Criteria:**
- Ollama OOM does not crash Rust Core
- AI Gateway returns graceful error within 100ms
- Service auto-restart within 5 seconds

**Dependencies:** WP2, WP3

---

### WORK PACKAGE 2: Advanced Cryptographic Sovereignty

**Objective:** True zero-knowledge with client-side E2EE

| Task | Description | Difficulty | Risk |
|------|-------------|------------|------|
| 2.1 | Design E2EE protocol (X25519 + ChaCha20-Poly1305) | High | High |
| 2.2 | Implement Web Crypto API client library | High | Medium |
| 2.3 | Create E2EE PWA (Progressive Web App) | High | Medium |
| 2.4 | Implement Argon2id key derivation | Medium | Medium |
| 2.5 | Encrypted IndexedDB storage layer | Medium | Low |
| 2.6 | Key rotation mechanism | High | High |
| 2.7 | Secure key escrow (optional recovery) | High | High |

**Acceptance Criteria:**
- Server never sees plaintext data
- Client key derivation takes <2 seconds
- Key rotation without data loss

**Dependencies:** WP1

---

### WORK PACKAGE 3: Hardware-Aware Efficiency

**Objective:** Optimize inference without OOM or throttling

| Task | Description | Difficulty | Risk |
|------|-------------|------------|------|
| 3.1 | Hardware profiling (CPU/GPU detection) | Medium | Low |
| 3.2 | Dynamic task routing engine | High | High |
| 3.3 | GPU pinning for Ollama (CUDA/Vulkan) | High | Medium |
| 3.4 | OPA/Network → CPU threads affinity | Low | Low |
| 3.5 | Model quantization pipeline (q4_K_M) | Medium | Medium |
| 3.6 | KV cache size limits | Medium | Low |
| 3.7 | Thermal throttling detection | Medium | Low |

**Acceptance Criteria:**
- LLM inference on discrete GPU only
- OPA evaluations <10ms on CPU
- Zero OOM during concurrent requests

**Dependencies:** WP1

---

### WORK PACKAGE 4: Autonomous Threat Response

**Objective:** Scorched Earth 2.0 - Instantaneous defense

| Task | Description | Difficulty | Risk |
|------|-------------|------------|------|
| 4.1 | Expand OPA policies for inter-enclave traffic | Medium | Medium |
| 4.2 | Ghost Shell 2.0 detection engine | High | High |
| 4.3 | Closed-loop kill-switch integration | High | High |
| 4.4 | WireGuard tunnel severance automation | Medium | Medium |
| 4.5 | ZFS dataset forced lock | Medium | Low |
| 4.6 | RAM key purge (mlock/munmap) | High | High |
| 4.7 | Physical re-authorization flow | Medium | Medium |

**Acceptance Criteria:**
- Threat detection → response <1 second
- All keys purged from RAM
- Physical key required to restore

**Dependencies:** WP1, WP2

---

## SEQUENCING

```
Week 1-2:   WP1 (Isolation) - Foundation
Week 3-4:   WP2 (E2EE) - Parallel track
Week 5-6:   WP3 (Hardware) - Requires WP1
Week 7-8:   WP4 (Threat Response) - Requires WP1, WP2
Week 9-10:  Integration Testing
Week 11-12: Security Audit + Hardening
Week 13-14: Documentation + Release Prep
```

---

## RESOURCE REQUIREMENTS

### Hardware
- Discrete GPU (RTX 3080+ or equivalent) for inference
- 32GB+ RAM for microVMs
- 1TB+ NVMe for ZFS pool

### Software
- Firecracker (latest)
- OpenClaw or custom gateway
- Ollama with CUDA support
- Chromium for PWA testing

### Skills
- Rust (required)
- Go/Python (OPA policies)
- GPU programming (CUDA/Vulkan)
- Cryptography (X25519, ChaCha20-Poly1305)
- Web Crypto API

---

## SUCCESS METRICS

| Metric | Target | Measurement |
|--------|--------|--------------|
| Cascade Failure Rate | 0% | AI crash should not affect core |
| Response Time (P99) | <100ms | Graceful error response |
| E2EE Coverage | 100% | Zero plaintext on wire |
| OOM Events | 0/month | During concurrent load test |
| Threat Response Time | <1s | Detect to execute |
| Key Purge Completeness | 100% | Verified by memory dump |

---

## RISK REGISTER

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Firecracker complexity | Medium | High | Start with cgroups, migrate later |
| E2EE key management | High | Critical | Phased rollout, manual fallback |
| GPU driver instability | Low | High | Separate driver microVM |
| Performance regression | Medium | Medium | Comprehensive benchmarking |
| Breaking protocol changes | Medium | Medium | Versioned API, backward compat |

---

**STATUS:** PARKING LOT - Ready for Phase 2 initiation after Phase 1 production deployment

**NEXT STEP:** Execute Phase 1 deployment, validate baseline, then initiate Phase 2 work package 1

---

*Last Updated: 2026-04-15*
*Parked By: Autonomous Build Agent*
