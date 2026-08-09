# 密码管理器 — 模块设计文档

## 1. 功能概述

密码管理器（Password Manager）作为本地密码存储方案，数据以 SQLite 数据库 + AES-256-GCM 加密存储在本地。用户设置一个主密码，所有条目数据通过主密码派生的密钥加密。支持生成强随机密码。

**通信链路**:
```
PasswordManager.vue ──→ invoke("is_locked")                ──→ password_manager.rs
                    ──→ invoke("has_master_password")       ──→ password_manager.rs
                    ──→ invoke("set_master_password")       ──→ password_manager.rs
                    ──→ invoke("unlock")                    ──→ password_manager.rs
                    ──→ invoke("lock")                      ──→ password_manager.rs
                    ──→ invoke("add_password")              ──→ password_manager.rs
                    ──→ invoke("list_passwords")            ──→ password_manager.rs
                    ──→ invoke("get_password")              ──→ password_manager.rs
                    ──→ invoke("update_password")           ──→ password_manager.rs
                    ──→ invoke("delete_password")           ──→ password_manager.rs
                    ──→ invoke("export_chrome_csv")        ──→ password_manager.rs
                    ──→ invoke("import_chrome_csv")         ──→ password_manager.rs
```

---

## 2. 数据结构

```rust
/// 密码条目（密码字段为加密后的字符串，明文只在解锁后于内存中解密）
#[derive(Serialize, Deserialize, Clone)]
pub struct PasswordEntry {
    pub id: u32,                  // 自增主键
    pub name: String,             // 条目名称，如 "GitHub"
    pub username: String,         // 明文用户名/邮箱
    pub password_encrypted: String, // AES-256-GCM 加密后的密文（base64）
    pub url: Option<String>,      // 相关 URL
    pub notes: Option<String>,    // 明文备注
    pub created_at: String,       // 创建时间（RFC3339）
}

/// 主密码验证器状态（用于判断是否需要解锁）
pub struct PasswordManager {
    pub entries: Arc<Mutex<Vec<PasswordEntry>>>,
    pub next_id: Arc<Mutex<u32>>,
    encryption_key: Arc<Mutex<[u8; 32]>>, // AES-256 密钥（解锁后驻留内存）
    pub locked: Arc<Mutex<bool>>,          // 是否处于锁定状态
    password_verifier: Arc<Mutex<Option<(Vec<u8>, Vec<u8>)>>>, // (salt, hash)
}
```

**前端对应** (`views/PasswordManager.vue`):

```javascript
import { ref } from "vue";

const locked = ref(true);        // 是否锁定（未设置主密码时自动解锁）
const entries = ref([]);
const searchQuery = ref("");

// 搜索过滤（使用 computed）
const filtered = computed(() =>
    searchQuery.value
        ? entries.value.filter(e =>
            e.name.toLowerCase().includes(searchQuery.value) ||
            e.username.toLowerCase().includes(searchQuery.value) ||
            (e.url && e.url.toLowerCase().includes(searchQuery.value))
          )
        : entries.value
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

> 注：密码生成在**前端**完成（纯客户端工具函数，不经由 Tauri 命令），生成的明文仅在用户点击「复制」时短暂存在内存中，不会写入加密存储。下方为生成算法的示意逻辑：

```javascript
function generatePassword({ length, uppercase, lowercase, numbers, symbols }) {
    let charset = "";
    if (uppercase) charset += "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if (lowercase) charset += "abcdefghijklmnopqrstuvwxyz";
    if (numbers)  charset += "0123456789";
    if (symbols)  charset += "!@#$%^&*()_+-=[]{}|;:,.<>?";

    let password = "";
    const crypto = window.crypto || window.msCrypto;
    for (let i = 0; i < length; i++) {
        const idx = Math.floor((crypto.getRandomValues(new Uint32Array(1))[0] / 4294967296) * charset.length);
        password += charset[idx];
    }
    return password;
}
```

---

## 4. 存储设计

所有密码条目序列化（JSON）后经 AES-256-GCM 加密，整体写入单个文件 `entries.enc`，位于应用数据目录（由 `dirs::data_dir()` 提供，再拼接 `devnexus/`）：

| 平台 | 路径 |
|------|------|
| macOS | `~/Library/Application Support/devnexus/entries.enc` |
| Linux | `~/.local/share/devnexus/entries.enc` |
| Windows | `%APPDATA%/devnexus/entries.enc` |

- 主密码本身不落盘，仅保存由随机 `salt` + 派生 `hash` 组成的验证器（`password_verifier`）
- 未设置主密码时应用自动解锁，首次设置后会进入锁定态
- 文件为「密文整体」而非逐条 BLOB，解密后在内存中以 `Vec<PasswordEntry>` 形式持有

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
#[test] fn test_entry_serialization()
```

测试覆盖: 加密解密往返（同一密码加密再解密得到原文）、密钥派生一致性、条目序列化完整。

---

## 7. 安全设计要点

| 设计 | 说明 |
|------|------|
| 主密码不存储 | 主密码本身不落盘，仅保存 (salt, hash) 验证器用于校验 |
| 密钥不在磁盘 | AES-256 密钥仅在解锁后驻留进程内存（`encryption_key`），锁定即清除 |
| AES-256-GCM | 认证加密，防止密文被篡改 |
| PBKDF2 派生 | 由主密码派生密钥，增加暴力破解成本 |
| 整体加密文件 | 所有条目密文汇总为单个 `entries.enc`，本地存储，无网络传输 |
| 自动解锁 | 未设置主密码时自动解锁便于首次使用；设置后进入锁定态 |

---

## 8. 跨平台注意事项

| 功能 | 跨平台一致性 |
|------|-------------|
| AES-256-GCM | ✅ 纯算法，跨平台一致 |
| SQLite | ✅ rusqlite 统一接口，仅路径不同 |
| 状态三态 | ✅ 前端逻辑完全一致 |
| 剪贴板操作 | ✅ 调用浏览器或 Tauri 剪贴板 API |
