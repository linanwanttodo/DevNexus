# Logging Migration Plan: eprintln!/println! → tracing

## Completed Migrations

###  `src/lib.rs` (3 occurrences)
- Line 44-47: GDK_BACKEND setup → `tracing::info!`
- Line 54-57: WEBKIT_DISABLE_DMABUF_RENDERER → `tracing::info!`
- Line 139: Tray icon warning → `tracing::warn!`

###  `src/utils/mod.rs` (1 occurrence)
- Line 31-34: Data directory creation failure → `tracing::warn!`

## Remaining Files to Migrate

### Priority 1: High-Visibility Production Code

#### `src/utils/crypto.rs` (3 occurrences)
```rust
// Line 75: Keyring persistence failure
eprintln!("[CryptoVault] Failed to persist key to keyring: {}", e)
→ tracing::warn!(error = %e, "[CryptoVault] Failed to persist key to keyring")

// Line 81: Key persistence fallback
eprintln!("[CryptoVault] WARNING: unable to persist encryption key...")
→ tracing::error!("[CryptoVault] Unable to persist encryption key (keyring and file both unavailable)")

// Line 153: Key retrieval error
eprintln!("[CryptoVault] Failed to retrieve key from keyring: {}", e)
→ tracing::warn!(error = %e, "[CryptoVault] Failed to retrieve key from keyring")
```

#### `src/commands/system.rs` (1 occurrence - in test, can keep)
```rust
// Line 168: Debug output in test
eprintln!("cpu_usage after 2 samples = {usage:?}")
→ Keep as-is (test code)
```

#### `src/commands/mirror.rs` (1 occurrence)
```rust
// Line 144: Cache lock failure
eprintln!("[DevNexus] Warning: Failed to acquire LATENCY_CACHE read lock")
→ tracing::warn!("[DevNexus] Failed to acquire latency cache read lock")
```

#### `src/commands/island_bridge.rs` (4 occurrences)
```rust
// Line 174: HUD state save failure
// Line 514: Data dir creation for island_enabled
// Line 579: Data dir creation for deepseek key
// Line 591: DeepSeek key persistence failure
→ All should use tracing::error! or tracing::warn!
```

### Priority 2: SSH Module

#### `src/commands/ssh/terminal.rs` (1 occurrence)
```rust
// Line 53: Agent forwarding request failure
eprintln!("[agent] request agent forwarding failed: {e}")
→ tracing::warn!(error = %e, "[SSH] Agent forwarding request failed")
```

#### `src/commands/ssh/session.rs` (5 occurrences)
```rust
// Line 214: SSH_AUTH_SOCK connection failure
// Line 842: Forward channel open failure
// Line 847: Forward accept failure
// Line 965: SOCKS proxy error
// Line 970: SOCKS accept failure
→ All should use tracing::warn! with structured fields
```

### Priority 3: API Hub Module

#### `src/api_hub/mod.rs` (4 occurrences)
```rust
// Various initialization errors
→ Use tracing::error! for DB/init failures
```

#### `src/api_hub/fetch_models.rs` (1 occurrence)
```rust
// Line 31: Retry after error
eprintln!("[API Hub] fetch_models retry after error: {}", e)
→ tracing::warn!(error = %e, "[API Hub] Retrying model fetch after error")
```

#### `src/api_hub/usage.rs` (7 occurrences)
```rust
// Multiple logging failures and DB errors
→ Use tracing::warn! for non-critical, tracing::error! for critical
```

#### `src/api_hub/server.rs` & `src/api_hub/provider.rs`
- Similar pattern: log operational issues

## Migration Guidelines

### Log Level Selection
- **`tracing::error!`**: Critical failures that break functionality (DB init, crypto key loss)
- **`tracing::warn!`**: Recoverable issues (cache miss, fallback behavior)
- **`tracing::info!`**: Normal operational events (startup, config changes)
- **`tracing::debug!`**: Detailed debugging info (only if needed)

### Structured Fields
Always include relevant context:
```rust
//  Bad
tracing::warn!("Failed to do something: {}", e);

//  Good
tracing::warn!(
    operation = "persist_key",
    storage = "keyring",
    error = %e,
    "Failed to persist encryption key"
);
```

### Prefix Convention
Remove `[DevNexus]` prefix (already in log metadata). Keep module-specific prefixes like `[SSH]`, `[API Hub]` only if they add value.

## Estimated Effort
- **Remaining files**: ~13 files, ~30 occurrences
- **Time**: 30-45 minutes
- **Risk**: Very low (pure refactoring)
- **Testing**: Verify logs appear correctly in development mode

## Automation Script (Optional)
```bash
# Find all eprintln! outside tests
rg "eprintln!" src --type rust -l | grep -v test | while read file; do
    echo "Processing $file..."
    # Manual review recommended over automated replacement
done
```

## Benefits After Completion
1. **Unified logging**: Single source of truth for all logs
2. **Structured data**: Easier filtering and analysis
3. **Log levels**: Proper severity classification
4. **Performance**: Tracing can be disabled at compile time
5. **Observability**: Better integration with monitoring tools
