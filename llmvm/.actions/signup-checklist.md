# OCI Signup Checklist — us-chicago-1 & eu-jovanovac-1

## Pre-Signup Requirements
- [ ] New email: `nrupalakolkar@gmail.com`
- [ ] New phone number (not used with previous OCI account)
- [ ] New credit/debit card (different from Montreal account)
- [ ] Different browser profile (or incognito) to avoid cookie conflicts
- [ ] Different billing address if possible

## Step-by-Step Signup

### Step 1: Start
1. Go to **https://www.oracle.com/cloud/free/**
2. Click **"Start for free"**
3. Use `nrupalakolkar@gmail.com` for email

### Step 2: Account Creation
1. Fill in account details (name, email, password)
2. Verify email (check inbox for Oracle verification code)
3. Fill in home address

### Step 3: Payment Verification
1. Enter credit/debit card (identity verification only, no charge)
2. Complete phone verification (SMS or call)

### Step 4: CRITICAL — Select Home Region
**You will see a region selector. Pick ONE:**

- **Account 1 (Chicago):** Select `US Midwest (Chicago) — us-chicago-1`
- **Account 2 (Jovanovac):** Select `Europe Southeast (Jovanovac) — eu-jovanovac-1`

**WARNING:** This cannot be changed later. Double-check before confirming.

### Step 5: Complete Signup
1. Accept terms
2. Wait for account provisioning (1-5 minutes)
3. You'll receive login credentials via email

### Step 6: Post-Signup — Upgrade to PAYG (Recommended)
1. Log into OCI Console
2. Go to **Billing & Cost Management**
3. Click **Upgrade to Pay As You Go**
4. This prevents idle instance reclamation while staying $0 within Always Free limits

### Step 7: Generate API Key
1. In Console, click profile icon → **My Profile**
2. Go to **API Keys** → **Add API Key**
3. Select **"Generate API Key Pair"**
4. Download the private key (save to `~/.oci/oci_api_key.pem`)
5. Copy the **Configuration File Preview** — extract:
   - `user_ocid`
   - `fingerprint`
   - `tenancy_ocid`
   - `region`

### Step 8: Record Credentials
Fill in `terraform.tfvars.chicago` or `terraform.tfvars.jovanovac`:
```
tenancy_ocid        = "ocid1.tenancy.oc1.."
user_ocid           = "ocid1.user.oc1.."
fingerprint         = "xx:xx:..."
private_key_path    = "~/.oci/oci_api_key.pem"
region              = "us-chicago-1"  OR "eu-jovanovac-1"
compartment_ocid    = "same as tenancy_ocid for new accounts"
ssh_public_key      = "ssh-ed25519 AAAA..."
```

## Troubleshooting
- **"Account already exists"**: Oracle's identity system detected you. Use different card + phone.
- **Region selector not showing desired region**: May be grayed out for free tier in some countries. Try from a VPN.
- **CAPTCHA loop**: Clear browser cache, use incognito, or try different browser.

## Capacity Notes
- us-chicago-1 has 3 Availability Domains — spreads load
- eu-jovanovac-1 has 1 Availability Domain — newer, fewer users
- After signup, create the ARM instance IMMEDIATELY before capacity fills
