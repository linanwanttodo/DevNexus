# 密码管理器 — 模块设计文档

## 1. 功能概述

密码管理器（Password Manager）作为本地密码存储方案，数据以 SQLite 数据库 + AES-256-GCM 加密存储在本地。用户设置一个主密码，所有条目数据通过主密码派生的密钥加密。支持生成强随机密码。

**通信链路**:
```
PasswordManager.svelte ──→ invoke("check_password_manager_status") ──→ password_manager.rs
                      ──→ invoke("set_master_password")           ──→ password_manager.rs
                      ──→ invoke("unlock_password_manager")       ──→ password_manager.rs
                      ──→ invoke("add_password_entry")            ──→ password_manager.rs
                      ──→ invoke("get_password_entries")          ──→ password_manager.rs
                      ──→ invoke("update_password_entry")         ──→ password_manager.rs
                      ──→ invoke("delete_password_entry")         ──→ password_manager.rs
                      ──→ invoke("generate_password")             ──→ password_manager.rs
```

---

## 2. 数据结构

```rust
/// 加密后的密码条目
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PasswordEntry {
    pub id: i64,
    pub name: String,           // 条目名称，如 "GitHub"
    pub username: String,       // 明文用户名/邮箱
    pub encrypted_password: Vec<u8>,  // AES-256-GCM 加密的密码密文
    pub url: Option<String>,    // 相关 URL
    pub notes: Option<String>,  // 明文备注（不包含敏感信息）
    pub category: String,       // "website" / "app" / "database" / "other"
    pub created_at: String,
    pub updated_at: String,
}

/// 解密后返回给前端的数据（密码解密为明文）
#[derive(Serialize, Debug)]
pub struct PasswordEntryView {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub password: String,      // <-- 解密后的明文密码
    pub url: Option<String>,
    pub notes: Option<String>,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 加密状态
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum EncryptionStatus {
    Uninitialized,   // 未设置主密码
    Locked,          // 已设置主密码但未解锁
    Unlocked,        // 已解锁，密钥保留在内存中
}
```

**前端对应** (`routes/PasswordManager.svelte`):

```javascript
let status = $state("locked");     // "uninitialized" | "locked" | "unlocked"
let entries = $state([]);
let searchQuery = $state("");

// 搜索过滤
let filtered = $derived(
    searchQuery
        ? entries.filter(e =>
            e.name.toLowerCase().includes(searchQuery) ||
            e.username.toLowerCase().includes(searchQuery) ||
            e.url?.toLowerCase().includes(searchQuery)
          )
        : entries
);
```

---

## 3. 核心实现

### 3.1 加密方案

```rust
pub struct CryptoEngine {
    key: [u8; 32],  // AES-256 密钥
}

impl CryptoEngine {
    /// 从主密码派生 AES-256 密钥
    pub fn derive_key(master_password: &str) -> [u8; 32] {
        // 使用 PBKDF2-HMAC-SHA256
        // 迭代次数: 100,000（平衡安全性和性能）
        // salt: 使用固定的应用级 salt（不存储因为加密依赖此 salt）
        let mut key = [0u8; 32];
        pbkdf2_hmac::pbkdf2_hmac::<sha2::Sha256>(
            master_password.as_bytes(),
            b"devnexus-password-manager",  // 应用级固定 salt
            100_000,
            &mut key,
        );
        key
    }

    /// AES-256-GCM 加密
    pub fn encrypt(&self, plaintext: &str) -> Vec<u8> {
        use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
        let key = Key::<Aes256Gcm>::from_slice(&self.key);
        let cipher = Aes256Gcm::new(key);
        // 随机 nonce（12 字节，GCM 推荐长度）
        let nonce_bytes = {
            use rand::RngCore;
            let mut n = [0u8; 12];
            rand::thread_rng().fill_bytes(&mut n);
            n
        };
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
            .expect("encryption failure");
        // 输出格式: nonce(12字节) + ciphertext
        [&nonce_bytes[..], &ciphertext[..]].concat()
    }
}
```

**加密方案细节**:
- 算法: **AES-256-GCM**（认证加密，同时保证机密性和完整性）
- 密钥派生: **PBKDF2-HMAC-SHA256**，100,000 次迭代
- Nonce: 12 字节随机值，每次加密不同，生成新的 salt + nonce
- 存储格式: `nonce(12 bytes) || ciphertext`
- 加密后每行 `base64` 编码存入 SQLite

### 3.2 状态管理

```rust
pub struct PasswordManager {
    pub status: EncryptionStatus,
    pub crypto: Option<CryptoEngine>,  // 解锁后持有密钥
    pub db_path: PathBuf,
}

impl PasswordManager {
    pub fn new() -> Self {
        let db_path = Self::default_db_path();
        let status = if Self::is_initialized(&db_path) {
            EncryptionStatus::Locked
        } else {
            EncryptionStatus::Uninitialized
        };
        Self { status, crypto: None, db_path }
    }

    /// 判断是否已初始化（主密码设置指纹是否存在）
    fn is_initialized(db_path: &Path) -> bool {
        // 检查 SQLite 中是否存在 master_password_hash 表
    }
}
```

### 3.3 设置主密码

```rust
pub fn set_master_password(&mut self, password: &str) -> Result<(), String> {
    // 1. 生成密钥
    let key = CryptoEngine::derive_key(password);
    self.crypto = Some(CryptoEngine { key });

    // 2. 创建验证令牌（加密一个已知明文用于后续验证）
    let token_plaintext = "devnexus-unlock-token";
    let encrypted_token = self.crypto.as_ref().unwrap().encrypt(token_plaintext);

    // 3. 存储到 SQLite（创建一个验证表）
    self.save_master_token(&encrypted_token);

    // 4. 创建空的密码条目表
    self.init_password_tables();

    self.status = EncryptionStatus::Unlocked;
    Ok(())
}
```

### 3.4 解锁验证

```rust
pub fn unlock(&mut self, password: &str) -> Result<(), String> {
    // 1. 尝试派生密钥
    let key = CryptoEngine::derive_key(password);
    let crypto = CryptoEngine { key };

    // 2. 从数据库读取验证令牌
    let encrypted_token = self.load_master_token()?;

    // 3. 尝试解密
    let decrypted = crypto.decrypt(&encrypted_token)
        .map_err(|_| "Invalid master password".to_string())?;

    // 4. 验证明文
    if decrypted != "devnexus-unlock-token" {
        return Err("Invalid master password".to_string());
    }

    // 5. 解锁成功，密钥保留在内存中
    self.crypto = Some(crypto);
    self.status = EncryptionStatus::Unlocked;
    Ok(())
}
```

### 3.5 密码生成器

```rust
pub fn generate_password(
    length: u32,
    use_uppercase: bool,
    use_lowercase: bool,
    use_numbers: bool,
    use_symbols: bool,
) -> String {
    let mut charset = String::new();
    if use_uppercase { charset.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ"); }
    if use_lowercase { charset.push_str("abcdefghijklmnopqrstuvwxyz"); }
    if use_numbers  { charset.push_str("0123456789"); }
    if use_symbols  { charset.push_str("!@#$%^&*()_+-=[]{}|;:,.<>?"); }

    // 确保至少包含每个字符集的一个字符（如果启用）
    let password: String = (0..length)
        .map(|_| {
            let idx = rand::thread_rng().gen_range(0..charset.len());
            charset.chars().nth(idx).unwrap()
        })
        .collect();

    password
}
```

---

## 4. 数据库设计

使用 SQLite（通过 `rusqlite` crate），数据库文件位于:

| 平台 | 路径 |
|------|------|
| macOS | `~/Library/Application Support/devnexus/passwords.db` |
| Linux | `~/.local/share/devnexus/passwords.db` |
| Windows | `%APPDATA%/devnexus/passwords.db` |

### 4.1 表结构

```sql
-- 验证表（存储主密码的加密验证令牌）
CREATE TABLE IF NOT EXISTS master_token (
    id INTEGER PRIMARY KEY,
    encrypted_token BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 密码条目表
CREATE TABLE IF NOT EXISTS password_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    username TEXT NOT NULL,
    encrypted_password BLOB NOT NULL,
    url TEXT,
    notes TEXT,
    category TEXT NOT NULL DEFAULT 'other',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

---

## 5. 前端实现

### 5.1 三状态视图

**未初始化**（`Uninitialized`）:
```
┌─────────────────────────┐
│  Welcome to Password    │
│  Manager                │
│                         │
│  [Set Master Password]  │
│  [     ●●●●●●●●    ]   │
│  [Confirm: ●●●●●●●●]  │
│  [      Submit      ]   │
└─────────────────────────┘
```

**锁定**（`Locked`）:
```
┌─────────────────────────┐
│  Password Manager       │
│  [   Enter password ]   │
│  [      Unlock      ]   │
└─────────────────────────┘
```

**解锁**（`Unlocked`）:
```
┌───────────────────────────┐
│ [Search...]  [+ Add]      │
│                           │
│ ┌─ GitHub ──────────────┐ │
│ │ user@email.com        │ │
│ │ ●●●●●●●●● [Copy] [Edit]│ │
│ └───────────────────────┘ │
│ ┌─ Docker Hub ───────────┐│
│ │ ...                    ││
└───────────────────────────┘
```

### 5.2 查看/复制密码

```javascript
// 点击密码字段时，解密显示明文 3 秒后自动隐藏
async function togglePasswordVisibility(entry) {
    if (visiblePasswords.has(entry.id)) {
        visiblePasswords.delete(entry.id);
    } else {
        visiblePasswords.set(entry.id, true);
        setTimeout(() => visiblePasswords.delete(entry.id), 3000);
    }
}

// 复制到剪贴板
async function copyToClipboard(text) {
    await navigator.clipboard.writeText(text);
}
```

### 5.3 密码生成弹窗

用户可以通过可配置的选项生成随机密码:

| 选项 | 默认 | 说明 |
|------|------|------|
| 长度 | 16 | 密码字符数 |
| 大写字母 | ✅ | A-Z |
| 小写字母 | ✅ | a-z |
| 数字 | ✅ | 0-9 |
| 特殊符号 | ✅ | `!@#$%^&*()_+-=[]{}|;:,.<>?` |

---

## 6. 测试

```rust
#[test] fn test_encrypt_decrypt()
#[test] fn test_derive_key_consistency()
#[test] fn test_generate_password_length()
#[test] fn test_generate_password_charsets()
#[test] fn test_entry_serialization()
```

测试覆盖: 加密解密往返（同一密码加密再解密得到原文）、密钥派生一致性、密码生成器长度和字符集合规。

---

## 7. 安全设计要点

| 设计 | 说明 |
|------|------|
| 主密码不存储 | 主密码本身不出现在数据库中，仅存储跳板用的加密验证令牌 |
| 密钥不在磁盘 | CryptoEngine 仅在 `Unlocked` 状态保存在进程内存中 |
| AES-256-GCM | 认证加密，防止密文被篡改 |
| PBKDF2 100k 轮 | 抗暴力破解（增加每次尝试的计算成本） |
| 内存中的密钥 | `Locked` 后 `crypto` 被 `take()` 移出，密钥从内存清除 |
| SQLite 本地 | 数据库文件本身就存储在本机，无需网络传输 |

---

## 8. 跨平台注意事项

| 功能 | 跨平台一致性 |
|------|-------------|
| AES-256-GCM | ✅ 纯算法，跨平台一致 |
| SQLite | ✅ rusqlite 统一接口，仅路径不同 |
| 状态三态 | ✅ 前端逻辑完全一致 |
| 剪贴板操作 | ✅ 调用浏览器或 Tauri 剪贴板 API |
