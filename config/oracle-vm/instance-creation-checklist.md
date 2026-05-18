# 🚀 Oracle Cloud Instance Creation - Step by Step

## Phase 1: Console Navigation
1. ✅ Open: https://console.us-chicago-1.oraclecloud.com
2. Login with your Oracle Cloud credentials
3. Navigate to: Compute → Instances → Create Instance

## Phase 2: Basic Configuration
4. **Name**: `aetheris-ai-server`
5. **Compartment**: Root compartment (or choose your compartment)
6. **Placement**: 
   - Region: US Midwest (Chicago)
   - Availability Domain: AD-1
   - Capacity: On-demand

## Phase 3: Image & Shape (CRITICAL)
7. **Image**: Click "Edit"
   - Image source: Platform images
   - OS: Canonical Ubuntu 
   - Version: 22.04 LTS Minimal
   - Architecture: ARM64 (aarch64) ← MUST SELECT

8. **Shape**: Click "Edit"
   - Shape series: Virtual Machine
   - Shape: VM.Standard.A1.Flex
   - OCPU count: 4
   - Memory (GB): 24

## Phase 4: Networking
9. **Network**: 
   - Virtual cloud network: Create new VCN
   - Name: aetheris-vcn
   - Subnet: Create new public subnet
   - Public IP: Assign a public IPv4 address

## Phase 5: SSH Keys
10. **SSH Keys**:
    - Option: Generate a key pair for me
    - Key name: aetheris-ai-key
    - ⚠️ DOWNLOAD PRIVATE KEY: Click "Save Private Key"
    - ⚠️ SAVE KEY: Store in secure location

## Phase 6: Boot Volume
11. **Boot Volume**:
    - Size: 100 GB
    - Performance: Balanced
    - Encryption: Oracle-managed keys

## Phase 7: Create Instance
12. **Review**: Verify all settings
13. **Create**: Click "Create" button
14. **Wait**: 5-10 minutes for provisioning

## Phase 8: Post-Creation
15. **Note Public IP**: From instance details page
16. **Test SSH**: 
    ```bash
    ssh -i aetheris-ai-key ubuntu@<PUBLIC_IP>
    ```
17. **Upload Setup**:
    ```bash
    scp -i aetheris-ai-key config/oracle-vm/setup.sh ubuntu@<PUBLIC_IP>:~
    scp -i aetheris-ai-key config/oracle-vm/free-tier-check.sh ubuntu@<PUBLIC_IP>:~
    ```

## Phase 9: Run Setup
18. **On Server**:
    ```bash
    chmod +x setup.sh free-tier-check.sh
    ./free-tier-check.sh  # Verify free tier
    export CLOUDFLARE_TUNNEL_TOKEN="YOUR_ACTUAL_TOKEN_HERE"
    ./setup.sh
    ```

## 📊 Free Tier Validation
- ✅ 4 OCPU × 750 hours = 3,000 OCPU-hours (exact free limit)
- ✅ 24 GB × 750 hours = 18,000 GB-hours (exact free limit)  
- ✅ Monthly Cost: $0.00
- ✅ 24/7 operation: FULLY COVERED

## ⚠️ Critical Checks
- Architecture: MUST be ARM64 (not AMD64)
- Shape: MUST be VM.Standard.A1.Flex (not E4.Flex)
- OCPU: 4 (not more)
- Memory: 24GB (not more)
- Image: Ubuntu 22.04 LTS ARM64