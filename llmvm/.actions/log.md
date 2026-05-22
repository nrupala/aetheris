# Aetheris LLMVM — Action Log
# Track all deployment attempts, decisions, and outcomes

## Session: 2026-05-01

### Pre-flight
- [x] OCI credentials configured (root tenancy, API key at ~/.oci/oci_api_key.pem)
- [x] Cloudflare token verified (Account ID: 2edd59d09fd816187b47afbb9ea43af1)
- [x] SSH key generated: ~/.ssh/llmvm_key
- [x] Terraform OCI provider v8.6.0 installed
- [x] terraform.tfvars populated with credentials

### Region Attempts

#### 1. ca-montreal-1 (Montreal)
- Status: AUTH WORKS, CAPACITY FAILS
- AD prefix: cxxo:CA-MONTREAL-1-AD-1 (dynamic lookup via data.oci_identity_availability_domain)
- Error: 500-InternalError, Out of host capacity
- Tried: 4 OCPU/24GB RAM → 1 OCPU/6GB RAM (Always Free minimum)
- Both failed with same capacity error
- Decision: RETAIN — try again later, ARM capacity fluctuates
- Note: API key registered for this region only

#### 2. ap-mumbai-1 (Mumbai) — PENDING
- Status: AUTH NOT CONFIGURED
- Error: 401-NotAuthenticated on identity.ap-mumbai-1
- Decision: WAITING for user to register API key in Mumbai
- Note: Region already set in terraform.tfvars (line 5)

#### 3. ca-toronto-1 (Toronto) — NEXT TO TRY
- Status: NEEDS API KEY REGISTRATION
- Note: Multiple ADs, may have ARM capacity
- Action: User to confirm if API key is registered here

### Infrastructure State
- Montreal VCN/networking exists in OCI (5 resources) but removed from Terraform state
- No instances created anywhere yet
- Scripts saved: scripts/deploy.ps1

### Configuration
- Shape: VM.Standard.A1.Flex (ARM)
- Image: Canonical Ubuntu 22.04 ARM64
- Boot volume: 50GB (minimum)
- SSH: ED25519 key, pubkey in terraform.tfvars line 8
- Domain: nrupalakolkar.com
- Tunnel: llmvm-tunnel
- AI Bearer: aetheris-prod-2026

### Next Steps
1. Try Toronto if API key registered there
2. Wait for Mumbai API key registration
3. Retry Montreal periodically (capacity can open up)
