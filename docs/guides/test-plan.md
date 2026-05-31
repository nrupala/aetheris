# AETHERIS - TEST PLAN
## v1.0 - Production Validation

---

## TEST EXECUTION SUMMARY

| Category | Tests | Status |
|----------|-------|--------|
| Network | 3 | PENDING |
| Security | 6 | PENDING |
| Storage | 4 | PENDING |
| AI | 5 | PENDING |
| Monitoring | 4 | PENDING |
| Core API | 8 | PENDING |
| **TOTAL** | **27** | **PENDING** |

---

## CATEGORY 1: NETWORK TESTS

### NET-01: WireGuard Stealth Check
```bash
# Verify only UDP 51820 is visible
nmap -sU -p 51820 <HOST_IP>

# Expected: Filtered/Stealth response
# NOT: Open port indication
```

### NET-02: ICMP Silence
```bash
# Verify no ICMP response
ping -c 1 -W 1 <HOST_IP>

# Expected: 100% packet loss (no response)
# NOT: Reply from <HOST_IP>
```

### NET-03: Peer Handshake
```bash
# Verify active WireGuard peers
wg show

# Expected: Active peer with latest handshake
# NOT: No peers or handshake timeout
```

---

## CATEGORY 2: SECURITY TESTS

### SEC-01: OPA Default Deny
```bash
# Test unauthorized access
curl -X POST http://localhost:8181/v1/data/aetheris/authz/allow \
  -H "Content-Type: application/json" \
  -d '{"input": {"user_role": "unknown"}}'

# Expected: {"result": false}
```

### SEC-02: JWT Validation
```bash
# Test with valid admin token
curl -X POST http://localhost:8181/v1/data/aetheris/authz/allow \
  -H "Content-Type: application/json" \
  -d '{"input": {"user_role": "admin", "token": "valid_jwt"}}'

# Expected: {"result": true}
```

### SEC-03: Auto-Ban Trigger
```bash
# Trigger 5 failed authentications
for i in {1..5}; do
  curl -X POST http://localhost:8181/v1/data/aetheris/authz/allow \
    -H "Content-Type: application/json" \
    -d '{"input": {"user_role": "banned", "peer_id": "test-ban-peer"}}'
done

# Verify peer is banned
curl http://localhost:8080/check-ban?peer=test-ban-peer

# Expected: {"banned": true, "expires_at": "..."}
```

### SEC-04: Ghost Shell Redirect
```bash
# From banned peer, access core
curl -H "X-Aetheris-Peer: test-ban-peer" \
     http://localhost:8080/

# Expected: Redirected to honeypot (8081)
```

### SEC-05: Directory Traversal Block
```bash
# Attempt path traversal attack
curl http://localhost:8080/download/../../../etc/passwd

# Expected: 403 Forbidden
# NOT: File contents
```

### SEC-06: Kill-Switch Dry-Run
```bash
# Execute kill-switch in dry-run mode
./scripts/killswitch.sh --dry-run

# Expected: Commands displayed but not executed
# Verify vault still mounted
zfs mount | grep aetheris_vault
```

---

## CATEGORY 3: STORAGE TESTS

### STR-01: ZFS Encryption Verify
```bash
# Check encryption status
zfs get encryption aetheris_vault/secure_data

# Expected: aes-256-gcm
```

### STR-02: Snapshot Existence
```bash
# List snapshots
zfs list -t snapshot | grep aetheris_vault

# Expected: At least 1 @zrepl_* snapshot
# NOT: Empty list
```

### STR-03: Snapshot Recovery
```bash
# Create test file
echo "test content for rollback" > /opt/aetheris/vault/test_recovery.txt

# Trigger snapshot
zfs snapshot aetheris_vault/secure_data@test-manual

# Delete file
rm /opt/aetheris/vault/test_recovery.txt

# Rollback
./scripts/vault_rollback.sh <<< "aetheris_vault/secure_data@test-manual"

# Verify file restored
cat /opt/aetheris/vault/test_recovery.txt

# Expected: "test content for rollback"
```

### STR-04: Offsite Replication
```bash
# Execute sync script (if offsite configured)
./scripts/vault_sync.sh

# Verify last sent snapshot
ssh <OFFSITE_HOST> "zfs list -t snapshot | grep aetheris_vault"

# Expected: Recent snapshot present on remote
```

---

## CATEGORY 4: AI TESTS

### AI-01: Ollama Health
```bash
# Check Ollama is running
curl http://localhost:11434/api/tags

# Expected: {"models": [...]}
```

### AI-02: Text Generation
```bash
# Test LLM inference
curl -X POST http://localhost:11434/api/generate \
  -H "Content-Type: application/json" \
  -d '{"model": "mistral", "prompt": "What is 2+2?", "stream": false}'

# Expected: {"response": "4"}
```

### AI-03: Embedding Generation
```bash
# Test vector encoding
curl -X POST http://localhost:11434/api/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model": "nomic-embed-text", "prompt": "test document"}'

# Expected: {"embedding": [...]} - 768 floats
```

### AI-04: Semantic Search Integration
```bash
# Create and upload test file
echo "Aetheris is a sovereign AI cloud" > /tmp/semantic_test.txt
curl -X POST -F "file=@/tmp/semantic_test.txt" \
     http://localhost:8080/upload

# Wait for indexing
sleep 3

# Search
curl "http://localhost:8080/search?q=sovereign+cloud"

# Expected: {"results": [{"filename": "semantic_test.txt", ...}]}
```

### AI-05: AI Sentinel Analysis
```bash
# Run sentinel analysis
docker exec aetheris_sentinel python /scripts/ai_sentinel.py

# Expected: "AI Sentinel Prediction: SAFE"
# NOT: CRITICAL without cause
```

---

## CATEGORY 5: MONITORING TESTS

### MON-01: VictoriaMetrics Health
```bash
# Check VM is running
curl http://localhost:8428/health

# Expected: {"status": "ok"}
```

### MON-02: Metrics Endpoint
```bash
# Get Prometheus metrics
curl http://localhost:8080/metrics

# Expected: aetheris_* metrics in Prometheus format
```

### MON-03: Grafana Dashboard
```bash
# Import dashboard JSON and verify
# (Manual verification via Grafana UI)

# Expected: 3 panels visible
# - Security Status
# - AI Prediction
# - Vault Usage
```

### MON-04: Metrics Retention
```bash
# Query historical metrics (if retention > 0)
curl "http://localhost:8428/api/v1/query?query=aetheris_vault_usage_bytes"

# Expected: Metrics data returned
```

---

## CATEGORY 6: CORE API TESTS

### CORE-01: File Upload
```bash
# Upload test file
echo "test content" > /tmp/upload_test.txt
curl -X POST -F "file=@/tmp/upload_test.txt" \
     http://localhost:8080/upload

# Expected: {"status": "uploaded", ...}
# Verify file exists
ls /opt/aetheris/vault/upload_test.txt
```

### CORE-02: File Download
```bash
# Download previously uploaded file
curl -O http://localhost:8080/download/upload_test.txt

# Expected: File contents match original
diff /tmp/upload_test.txt upload_test.txt
```

### CORE-03: Search
```bash
# Semantic search
curl "http://localhost:8080/search?q=test+content"

# Expected: upload_test.txt in results
```

### CORE-04: Status Endpoint
```bash
# Get system status
curl http://localhost:8080/status | jq

# Expected: All components healthy
```

### CORE-05: Metrics Endpoint
```bash
# Prometheus format
curl http://localhost:8080/metrics | head -20

# Expected: Prometheus text format
```

### CORE-06: Dashboard HTML
```bash
# Verify zero-JS dashboard
curl http://localhost:8080/ | grep -c "<script>"

# Expected: 0 (no script tags)
```

### CORE-07: Health Check
```bash
# Container health
docker exec aetheris_core curl -s http://localhost:8080/status

# Expected: {"status": "ok", ...}
```

### CORE-08: Concurrent Uploads
```bash
# Test parallel uploads
for i in {1..5}; do
  echo "file $i" > /tmp/concurrent_$i.txt &
done
wait

for i in {1..5}; do
  curl -X POST -F "file=@/tmp/concurrent_$i.txt" \
       http://localhost:8080/upload &
done

# Expected: All uploads succeed
```

---

## TEST EXECUTION ORDER

```bash
#!/bin/bash
# Execute in order - parallel where safe

# Phase 1: Infrastructure (can run parallel)
echo "=== PHASE 1: INFRASTRUCTURE ==="
./tests/net-*.sh      # NET-01, NET-02, NET-03
./tests/storage-*.sh   # STR-01, STR-02

# Phase 2: Security (sequential - auto-ban test modifies state)
echo "=== PHASE 2: SECURITY ==="
./tests/sec-01.sh      # SEC-01: Default deny
./tests/sec-02.sh      # SEC-02: JWT validation
./tests/sec-05.sh      # SEC-05: Path traversal
./tests/sec-06.sh      # SEC-06: Kill-switch dry-run
./tests/sec-03.sh      # SEC-03: Auto-ban (run last in security)
./tests/sec-04.sh      # SEC-04: Ghost shell

# Phase 3: AI (can run parallel)
echo "=== PHASE 3: AI ==="
./tests/ai-01.sh       # Ollama health
./tests/ai-02.sh       # Text generation
./tests/ai-03.sh       # Embeddings

# Phase 4: Core API (sequential)
echo "=== PHASE 4: CORE API ==="
./tests/core-01.sh     # Upload
./tests/core-02.sh     # Download
./tests/core-03.sh     # Search
./tests/core-04.sh     # Status
./tests/core-05.sh     # Metrics
./tests/core-06.sh     # Zero-JS check
./tests/core-07.sh     # Health
./tests/core-08.sh     # Concurrent

# Phase 5: Monitoring (can run parallel)
echo "=== PHASE 5: MONITORING ==="
./tests/mon-01.sh      # VictoriaMetrics
./tests/mon-02.sh      # Metrics endpoint
./tests/mon-03.sh      # Grafana (manual)
./tests/mon-04.sh      # Retention

# Phase 6: Integration
echo "=== PHASE 6: INTEGRATION ==="
./tests/ai-04.sh       # Full semantic search flow
./tests/ai-05.sh       # Sentinel
./tests/storage-03.sh  # Rollback
./tests/storage-04.sh  # Offsite (if configured)

echo "=== COMPLETE ==="
```

---

## SUCCESS CRITERIA

| Metric | Target |
|--------|--------|
| Tests Passed | 27/27 (100%) |
| Critical Failures | 0 |
| Warnings | < 3 |
| Execution Time | < 30 minutes |

---

## FAILURE HANDLING

| Failure | Action |
|---------|--------|
| NET-01/02 Fail | Check firewall, verify WireGuard running |
| SEC-01/02 Fail | Check OPA policy, restart OPA |
| SEC-03 Fail | Check watcher.rs implementation |
| AI-01-03 Fail | Check Ollama container, pull models |
| CORE-01/02 Fail | Check vault mount, permissions |
| MON-01 Fail | Check VictoriaMetrics container |

---

**TEST PLAN VERSION:** 1.0
**STATUS:** APPROVED
**DATE:** 2026-04-15
