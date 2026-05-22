# OCI Region Selection Strategy — PDCA Analysis
# Date: 2026-05-01

## Q1: Penalty for recreating tenancy after deletion?
- Oracle docs: Deletion is IRREVERSIBLE. Tenancy suspended immediately, permanently deleted after 30 days.
- Community reports: Re-signing up with same email/card often blocked — Oracle ties to identity (card hash, phone, address).
- Reddit case: User deleted account, waited 6+ months, STILL blocked from creating new account with different emails/cards.
- Risk: HIGH. Oracle appears to enforce one Free Tier account per real-world identity.
- Mitigation: After tenancy deletion completes, try signing up immediately. If blocked, may need different card/email.

## Q2: Why did Oracle give Montreal with 0 ARM capacity?
- Oracle's signup picks the home region based on your signup location/selection, NOT on capacity availability.
- Montreal was a "recommended" region (less popular than Mumbai/Ashburn) when Oracle wrote docs.
- Capacity changed after your signup — free-tier users flocked to Montreal because it was recommended.
- Oracle does NOT check capacity during region selection. It's a first-come-first-served model.

## OCI Free Tier Reality Check
- NO GPU in Always Free — confirmed. GPU shapes are paid only (A10, V100, A100, H100, etc.)
- ARM Always Free: 4 OCPU, 24GB RAM total (VM.Standard.A1.Flex) — max capacity
- AMD Always Free: 2x VM.Standard.E2.1.Micro (1 OCPU, 1GB RAM each) — backup option
- Always Free compute ONLY in home region — cannot change after signup
- $300 trial credits: 30 days, can be used for GPU but burns fast

## Recommended Signup Regions (lowest free-tier demand)
1. eu-jovanovac-1 (Serbia) — newest European region
2. eu-turin-1 (Italy) — very new
3. us-chicago-1 (Chicago) — Oracle officially recommends, 3 ADs
4. eu-stockholm-1 (Sweden) — Nordic, low adoption
5. sa-bogota-1 (Colombia) — newer, low free-tier adoption

## Recommended Strategy
1. Wait for Montreal tenancy deletion confirmation (30 days per Oracle docs)
2. Sign up at oracle.com/cloud/free with DIFFERENT email + card if possible
3. Pick eu-jovanovac-1 or us-chicago-1 as home region
4. Immediately upgrade to Pay As You Go (free within Always Free limits, prevents idle reclamation)
5. Deploy ARM instance with Terraform
