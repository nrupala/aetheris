# AETHERIS - USER ACCEPTANCE TESTING (UAT) SUITE
## v1.0 - PRODUCTION VALIDATION

---

## UAT PHILOSOPHY

> **"If it doesn't work in production, it's not done."**

Every test must verify ACTUAL BEHAVIOR, not just file existence.

---

## UAT TEST EXECUTION

### PRE-REQUISITES CHECK
```bash
# Run before any UAT
./uat_precheck.sh
```

### UAT EXECUTION ORDER
```bash
# 1. Infrastructure UAT
./uat_01_infrastructure.sh

# 2. Security UAT  
./uat_02_security.sh

# 3. File Operations UAT
./uat_03_file_operations.sh

# 4. AI/Semantic UAT
./uat_04_ai_semantic.sh

# 5. Network/Connectivity UAT
./uat_05_network.sh

# 6. Recovery UAT
./uat_06_recovery.sh
```

---

## UAT-01: INFRASTRUCTURE

### UAT-01.01: Docker Runtime
```gherkin
Feature: Docker Runtime
  Scenario: Docker daemon is operational
    Given the system has Docker installed
    When I check Docker status
    Then Docker should be running
    And Docker version should be >= 20.10
```

**Test Command:**
```bash
docker version
docker info | grep "Server Version"
```
**Pass Criteria:** Server Version returned, no errors

### UAT-01.02: Container Network
```gherkin
Feature: Container Networking
  Scenario: Internal network is created
    Given docker-compose is configured
    When I inspect network aetheris_internal
    Then network should exist
    And subnet should be accessible
```

**Test Command:**
```bash
docker network inspect aetheris_internal
```
**Pass Criteria:** Network found, correct driver

### UAT-01.03: Volume Mounts
```gherkin
Feature: Volume Mounts
  Scenario: All volumes are accessible
    Given vault directory exists
    And data directories exist
    When I check permissions
    Then directories should be writable
```

**Test Command:**
```bash
ls -la /opt/aetheris/vault
ls -la /opt/aetheris/data
```
**Pass Criteria:** Directories exist, correct permissions

---

## UAT-02: SECURITY

### UAT-02.01: OPA Policy - Default Deny
```gherkin
Feature: Zero-Trust Default Deny
  Scenario: Unauthenticated request is denied
    Given no JWT token provided
    When I send a request to /download/test.txt
    Then response should be 403 Forbidden
    And error message should indicate denial
```

**Test Command:**
```bash
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/download/test.txt
# Expected: 403
```
**Pass Criteria:** 403 returned, not 200

### UAT-02.02: OPA Policy - Valid Admin
```gherkin
Feature: Admin Access
  Scenario: Admin with valid JWT can access
    Given valid admin JWT token
    When I send authenticated request
    Then access should be granted
```

**Test Command:**
```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" http://localhost:8080/download/test.txt
```
**Pass Criteria:** 200 returned

### UAT-02.03: Path Traversal Prevention
```gherkin
Feature: Path Traversal Attack Prevention
  Scenario: Attacker tries directory traversal
    Given attacker sends ../etc/passwd
    When I process the request
    Then request should be rejected
    And response should be 403
```

**Test Command:**
```bash
curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/download/../../../etc/passwd"
# Expected: 403
```
**Pass Criteria:** 403 returned, not file contents

### UAT-02.04: Auto-Ban Mechanism
```gherkin
Feature: Automatic Peer Banning
  Scenario: 5 failed auth attempts triggers ban
    Given suspicious peer ID
    When I trigger 5 failed authentications
    Then peer should be banned
    And subsequent requests should fail
```

**Test Command:**
```bash
for i in {1..5}; do
  curl -H "X-Peer-ID: test-ban" http://localhost:8080/trigger-fail 2>/dev/null
done
curl -H "X-Peer-ID: test-ban" http://localhost:8080/status
# Expected: BANNED response or 403
```
**Pass Criteria:** Ban recorded, subsequent requests denied

### UAT-02.05: Encryption Verification
```gherkin
Feature: ZFS Encryption
  Scenario: Vault is encrypted with AES-256-GCM
    Given ZFS pool exists
    When I check encryption status
    Then encryption should be AES-256-GCM
```

**Test Command:**
```bash
sudo zfs get encryption aetheris_vault/secure_data
# Expected: aes-256-gcm
```
**Pass Criteria:** Encryption type correct

---

## UAT-03: FILE OPERATIONS

### UAT-03.01: File Upload
```gherkin
Feature: File Upload
  Scenario: User uploads a file to vault
    Given authenticated user
    And valid file exists
    When I upload the file via API
    Then file should appear in vault
    And success response should be returned
```

**Test Command:**
```bash
echo "Test file content $(date)" > /tmp/uat_upload.txt
curl -X POST -F "file=@/tmp/uat_upload.txt" http://localhost:8080/upload
ls -la /opt/aetheris/vault/uat_upload.txt
```
**Pass Criteria:** File exists in vault, size matches

### UAT-03.02: File Download
```gherkin
Feature: File Download
  Scenario: User downloads existing file
    Given file exists in vault
    And user is authenticated
    When I request download
    Then file content should be returned
    And content should match original
```

**Test Command:**
```bash
curl -o /tmp/uat_download.txt http://localhost:8080/download/uat_upload.txt
diff /tmp/uat_upload.txt /tmp/uat_download.txt
# Expected: No differences
```
**Pass Criteria:** Files identical

### UAT-03.03: File Delete Prevention
```gherkin
Feature: No Direct File Deletion
  Scenario: User cannot delete files directly
    Given authenticated user
    When I attempt DELETE request
    Then method should be rejected
    And 405 returned
```

**Test Command:**
```bash
curl -X DELETE -o /dev/null -w "%{http_code}" http://localhost:8080/delete/uat_upload.txt
# Expected: 405 Method Not Allowed
```
**Pass Criteria:** 405 returned

### UAT-03.04: Large File Upload
```gherkin
Feature: Large File Handling
  Scenario: Upload 10MB file succeeds
    Given authenticated user
    When I upload 10MB file
    Then upload should complete
    And file integrity should be maintained
```

**Test Command:**
```bash
dd if=/dev/urandom of=/tmp/large_test.bin bs=1M count=10
curl -X POST -F "file=@/tmp/large_test.bin" http://localhost:8080/upload
ls -la /opt/aetheris/vault/large_test.bin
# Expected: 10485760 bytes
```
**Pass Criteria:** File size matches (within tolerance)

---

## UAT-04: AI SEMANTIC SEARCH

### UAT-04.01: Ollama Service
```gherkin
Feature: Ollama AI Service
  Scenario: Ollama API is responsive
    Given Ollama container is running
    When I query /api/tags
    Then list of models should be returned
```

**Test Command:**
```bash
curl -s http://localhost:11434/api/tags | jq '.models[].name'
```
**Pass Criteria:** Valid JSON, models listed

### UAT-04.02: Text Generation
```gherkin
Feature: AI Text Generation
  Scenario: Ollama generates text response
    Given Ollama is healthy
    When I send generation request
    Then text should be generated
    And response should be coherent
```

**Test Command:**
```bash
curl -s -X POST http://localhost:11434/api/generate \
  -H "Content-Type: application/json" \
  -d '{"model":"mistral","prompt":"What is 2+2?","stream":false}'
```
**Pass Criteria:** Response contains "4"

### UAT-04.03: Embedding Generation
```gherkin
Feature: Semantic Embeddings
  Scenario: Document embeddings are generated
    Given Ollama is healthy
    When I request embeddings
    Then vector array should be returned
    And vector should have expected dimensions
```

**Test Command:**
```bash
curl -s -X POST http://localhost:11434/api/embeddings \
  -H "Content-Type: application/json" \
  -d '{"model":"nomic-embed-text","prompt":"test document"}' | jq '.embedding | length'
# Expected: 768
```
**Pass Criteria:** Vector length = 768

### UAT-04.04: End-to-End Semantic Search
```gherkin
Feature: Complete Semantic Search Flow
  Scenario: File is uploaded, indexed, and searchable
    Given authenticated user
    When I upload document about "budget 2024"
    And wait for indexing
    Then searching "budget" should find the document
```

**Test Command:**
```bash
echo "This document contains budget information for 2024 fiscal year" > /tmp/semantic_test.txt
curl -X POST -F "file=@/tmp/semantic_test.txt" http://localhost:8080/upload
sleep 5  # Wait for AI indexing
curl "http://localhost:8080/search?q=budget+2024" | jq '.results[].filename'
# Expected: semantic_test.txt in results
```
**Pass Criteria:** Upload succeeds, search returns relevant file

---

## UAT-05: NETWORK/CONNECTIVITY

### UAT-05.01: Internal API Connectivity
```gherkin
Feature: Internal Service Communication
  Scenario: Core can reach AI service
    Given containers are running
    When Core pings Ollama
    Then response should be received
```

**Test Command:**
```bash
docker exec aetheris_core curl -s -o /dev/null -w "%{http_code}" http://ai-engine:11434/api/tags
# Expected: 200
```
**Pass Criteria:** 200 returned from inside container

### UAT-05.02: ChromaDB Connectivity
```gherkin
Feature: Vector Database Connectivity
  Scenario: ChromaDB heartbeat is healthy
    Given ChromaDB is running
    When I check heartbeat
    Then healthy status should be returned
```

**Test Command:**
```bash
curl -s http://localhost:8000/api/v1/heartbeat | jq '.nanosecond heartbeat'
# Expected: timestamp
```
**Pass Criteria:** Valid heartbeat response

### UAT-05.03: Service Restart Resilience
```gherkin
Feature: Service Recovery
  Scenario: Services recover after container restart
    Given all services are running
    When I restart aetheris_core container
    Then container should restart
    And services should become healthy again
```

**Test Command:**
```bash
docker restart aetheris_core
sleep 10
curl -s http://localhost:8080/status | jq '.components.core.status'
# Expected: "ready"
```
**Pass Criteria:** Service recovers, status healthy

---

## UAT-06: RECOVERY & BACKUP

### UAT-06.01: Snapshot Creation
```gherkin
Feature: ZFS Snapshot
  Scenario: Manual snapshot can be created
    Given ZFS is operational
    When I create manual snapshot
    Then snapshot should appear in list
```

**Test Command:**
```bash
sudo zfs snapshot aetheris_vault/secure_data@uat-manual-test
sudo zfs list -t snapshot | grep uat-manual-test
# Expected: Snapshot exists
```
**Pass Criteria:** Snapshot created and listed

### UAT-06.02: Snapshot Rollback
```gherkin
Feature: Data Recovery from Snapshot
  Scenario: File can be restored from snapshot
    Given snapshot exists
    And file was deleted
    When I rollback to snapshot
    Then file should be restored
```

**Test Command:**
```bash
# Create and delete test file
echo "to be recovered" > /opt/aetheris/vault/recovery_test.txt
rm /opt/aetheris/vault/recovery_test.txt
# Rollback
sudo zfs rollback aetheris_vault/secure_data@uat-manual-test
cat /opt/aetheris/vault/recovery_test.txt
# Expected: "to be recovered"
```
**Pass Criteria:** File content restored

### UAT-06.03: Kill-Switch Dry-Run
```gherkin
Feature: Emergency Protocol
  Scenario: Kill-switch dry-run executes safely
    Given vault is mounted
    When I run kill-switch --dry-run
    Then no destructive changes should occur
    And vault should remain mounted
```

**Test Command:**
```bash
sudo /opt/aetheris/scripts/killswitch.sh --dry-run
sudo zfs get mountpoint aetheris_vault/secure_data
# Expected: Still mounted
```
**Pass Criteria:** Vault still mounted, no keys shredded

---

## UAT SUMMARY REPORT

| UAT ID | Category | Test | Result | Date |
|--------|----------|------|--------|------|
| UAT-01.01 | Infra | Docker Runtime | PENDING | - |
| UAT-01.02 | Infra | Container Network | PENDING | - |
| UAT-01.03 | Infra | Volume Mounts | PENDING | - |
| UAT-02.01 | Security | Default Deny | PENDING | - |
| UAT-02.02 | Security | Admin Access | PENDING | - |
| UAT-02.03 | Security | Path Traversal | PENDING | - |
| UAT-02.04 | Security | Auto-Ban | PENDING | - |
| UAT-02.05 | Security | Encryption | PENDING | - |
| UAT-03.01 | Files | Upload | PENDING | - |
| UAT-03.02 | Files | Download | PENDING | - |
| UAT-03.03 | Files | Delete Prevention | PENDING | - |
| UAT-03.04 | Files | Large File | PENDING | - |
| UAT-04.01 | AI | Ollama Service | PENDING | - |
| UAT-04.02 | AI | Text Generation | PENDING | - |
| UAT-04.03 | AI | Embeddings | PENDING | - |
| UAT-04.04 | AI | Semantic Search E2E | PENDING | - |
| UAT-05.01 | Network | Internal API | PENDING | - |
| UAT-05.02 | Network | ChromaDB | PENDING | - |
| UAT-05.03 | Network | Restart Recovery | PENDING | - |
| UAT-06.01 | Recovery | Snapshot Creation | PENDING | - |
| UAT-06.02 | Recovery | Rollback | PENDING | - |
| UAT-06.03 | Recovery | Kill-Switch Dry-Run | PENDING | - |

**Total Tests:** 21
**Pass Required:** 21/21 (100%)

---

## PRODUCTION GATE CRITERIA

System is PRODUCTION READY only if:
- [ ] All 21 UAT tests pass
- [ ] No security vulnerabilities found
- [ ] Performance meets SLAs (>100 req/s)
- [ ] Recovery tested and documented
- [ ] Monitoring operational

**Without ALL tests passing, system is NOT production ready.**

---

**UAT VERSION:** 1.0
**STATUS:** READY FOR EXECUTION
**EXECUTED BY:** Autonomous Build Agent
**DATE:** 2026-04-15
