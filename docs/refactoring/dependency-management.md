# Dependency Management Guide

## Overview
This document explains the dependency organization in DevNexus and provides guidelines for maintenance.

## Recent Improvements

### 1. Unified Tauri Plugin Versions 
**Before:**
```toml
tauri-plugin-process = "2"
tauri-plugin-updater = "2"
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2.5.1"  # Inconsistent version format
```

**After:**
```toml
tauri-plugin-process = "2"
tauri-plugin-updater = "2"
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"     # Unified to major version
```

**Benefit:** Consistent version management, easier updates, clearer compatibility expectations.

### 2. Organized Cryptography Libraries 
**Rationale for each crypto library:**

- **`aes-gcm` (0.10)**: Primary encryption for API keys and passwords
  - Provides AES-256-GCM authenticated encryption
  - Used in `api_hub/crypto.rs` for secure storage
  
- **`cbc` (0.1)**: Legacy cookie decryption support
  - Required for Chrome cookie decryption on Linux
  - Uses AES-128-CBC with integrity checking
  
- **`aes` (0.8)**: AES block cipher primitive
  - Base implementation used by both GCM and CBC modes
  - Not directly used in application code
  
- **`sha2` (0.10)**: SHA-256 hashing
  - Integrity verification for encrypted data
  - Cookie decryption integrity checks
  
- **`sha1` (0.10)**: SHA-1 for legacy compatibility
  - Required for older systems that still use SHA-1
  - Not recommended for new implementations
  
- **`pbkdf2` (0.12)**: Key derivation function
  - Derives encryption keys from user passwords
  - Used in password manager
  
- **`base64` (0.21)**: Encoding/decoding
  - Stores encrypted data as text
  - Interoperability with web APIs
  
- **`rand` (0.8)**: Random number generation
  - Generates nonces for encryption
  - Salt generation for key derivation

**Assessment:** No redundancy detected. Each library serves a specific purpose:
- Modern encryption (aes-gcm) for new features
- Legacy support (cbc, sha1) for browser cookie compatibility
- Supporting primitives (aes, sha2, pbkdf2, base64, rand)

### 3. Added Development Dependencies 
```toml
[dev-dependencies]
tokio-test = "0.4"  # Async testing utilities
```

## Security Audit Setup

### Install cargo-audit
```bash
cargo install cargo-audit
```

### Run Security Audit
```bash
# Check for known vulnerabilities
cargo audit

# Auto-fix when possible
cargo audit fix
```

### Recommended: Add to CI
Create `.github/workflows/security.yml`:
```yaml
name: Security Audit
on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly on Sunday
  push:
    paths:
      - '**/Cargo.toml'
      - '**/Cargo.lock'

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/audit-check@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

### Optional: cargo-deny for Advanced Analysis
```bash
cargo install cargo-deny
cargo deny check
```

Provides:
- License compliance checking
- Duplicate dependency detection
- Advisory database integration
- Custom policy enforcement

## Version Update Strategy

### Regular Updates (Monthly)
```bash
# Check for outdated dependencies
cargo outdated

# Update patch versions (safe)
cargo update

# Update minor versions (usually safe)
cargo update --aggressive
```

### Major Version Updates (Quarterly)
1. Review changelog for breaking changes
2. Update one dependency at a time
3. Run full test suite after each update
4. Commit separately for easy rollback

### Critical Security Updates (Immediate)
1. Monitor RustSec advisories
2. Apply patches immediately
3. Test critical paths
4. Deploy hotfix if production-impacting

## Dependency Health Metrics

### Current Status
- **Total dependencies**: ~50 direct + ~200 transitive
- **Tauri plugins**: 5 (all v2, unified)
- **Crypto libraries**: 8 (all necessary, no redundancy)
- **Last audit**: [Date of last cargo audit run]

### Targets
- Zero known vulnerabilities (cargo audit clean)
- All Tauri plugins on same major version 
- No duplicate dependencies
- License compliance (MIT/Apache-2.0 preferred)

## Troubleshooting

### Common Issues

**Problem:** Version conflict between Tauri plugins
**Solution:** Use consistent major versions (all "2")

**Problem:** Build fails after updating crypto library
**Solution:** Check API changes in changelog, update usage accordingly

**Problem:** cargo audit reports vulnerability
**Solution:** 
1. Check if it affects your usage
2. Update to patched version
3. If no patch available, consider alternative library
4. Document risk if mitigation not possible

## References
- [RustSec Advisory Database](https://rustsec.org/)
- [cargo-audit Documentation](https://github.com/rustsec/cargo-audit)
- [Tauri Plugin Ecosystem](https://tauri.app/plugin/)
- [Are We Learning Yet? - Dependency Management](https://www.arewelearningyet.com/)
