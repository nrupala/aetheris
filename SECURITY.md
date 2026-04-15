# AETHERIS - SECURITY SPECIFICATION
## v1.0

---

## SECURITY PRINCIPLES

1. **Zero Trust**: Never trust, always verify
2. **Defense in Depth**: Multiple security layers
3. **Fail Closed**: Default deny all
4. **Least Privilege**: Minimal permissions required
5. **Encryption Everywhere**: E2EE at rest and in transit

---

## NETWORK SECURITY

### WireGuard Configuration
```
Protocol: UDP 51820
Encryption: ChaCha20-Poly1305
Key Exchange: Curve25519
Preshared Key: Optional post-quantum resistance
```

### Stealth Configuration
```bash
# No ICMP response
net.ipv4.icmp_echo_ignore_all = 1

# No response to unmatched packets
net.ipv4.conf.all.forwarding = 1
```

---

## ENCRYPTION

### ZFS Native Encryption
```
Algorithm: AES-256-GCM
Key Format: 32-byte hex
Key Location: /root/.aetheris_key
```

### Data-at-Rest
- All vault data encrypted
- Snapshots inherit encryption
- Key required to mount dataset

### Data-in-Transit
- WireGuard tunnel encryption
- TLS 1.3 (if HTTPS enabled)
- No plaintext network traffic

---

## ZERO-TRUST POLICY

### Default Policy
```rego
package aetheris.authz

default allow = false

allow {
    is_authenticated
    has_permission
    not is_blacklisted
}
```

### Role Definitions
| Role | Permissions |
|------|-------------|
| admin | Full access |
| analyst | Read + semantic search |
| system_agent | Indexing operations |

---

## AUTHENTICATION

### JWT Structure
```json
{
  "sub": "peer_id",
  "role": "admin",
  "iat": 1705312200,
  "exp": 1705315800,
  "aud": "aetheris-mesh"
}
```

### Token Lifetime
- Default: 1 hour
- Minimum: 5 minutes
- Maximum: 24 hours

---

## GHOST SHELL (HONEYPOT)

### Isolation
- Separate network namespace
- No access to real vault
- Read-only dummy files
- All actions logged

### Canary Tokens
- passwords.txt (fake credentials)
- network_map.json (fake topology)
- confidential_vault_keys.zfs (trap file)

---

## AUTO-BAN

### Thresholds
```
Failed Attempts: 5
Ban Duration: 1 hour
Ban Type: Peer ID (not IP)
```

### Logging
```rust
if failures >= MAX_FAILURES {
    log_security_event("PEER_BANNED", peer_id);
    add_to_blacklist(peer_id);
}
```

---

## KILL-SWITCH

### Triggers
1. Physical seizure
2. Remote compromise detected
3. Dead man's switch (cron)
4. Manual activation

### Actions
```bash
# 1. Stop all containers
docker compose down --volumes --timeout 0

# 2. Unmount vault
zfs unmount -f aetheris_vault/secure_data

# 3. Unload keys
zfs unload-key aetheris_vault/secure_data

# 4. Shred local key
shred -u -n 3 -z /root/.aetheris_key

# 5. Clear logs
echo > /var/log/auth.log
history -c
```

---

## AUDIT LOGGING

### Events Logged
- All OPA decisions
- File access attempts
- Authentication failures
- Peer connections/disconnections
- Admin actions

### Log Location
```
/opt/aetheris/vault/audit.log (encrypted)
```

---

## SECURITY CHECKLIST

- [ ] WireGuard only UDP 51820 exposed
- [ ] No ICMP response
- [ ] ZFS encryption enabled
- [ ] OPA policy deployed
- [ ] Default deny configured
- [ ] Auto-ban enabled
- [ ] Ghost shell armed
- [ ] Kill-switch tested
- [ ] Master key backed up
- [ ] Audit logging active

---

**SECURITY VERSION:** 1.0
**STATUS:** APPROVED
**DATE:** 2026-04-15
