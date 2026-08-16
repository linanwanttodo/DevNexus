# SSH 模块与通用导航上下文 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 DevNexus 新增 SSH 模块（连接管理 + 交互式终端 + SFTP），并将侧边栏改造成"图标轨 + 上下文面板"的通用导航上下文机制。

**Architecture:** 后端新增 `commands/ssh/`（connections/session/terminal/sftp 四个文件），基于 `russh` 0.62（纯 Rust 异步，与既有 tokio 栈契合）+ `russh-sftp` 2.4；凭据复用从 `PasswordManager` 抽出的共享 `utils/crypto.rs`（AES-256-GCM，密钥存 OS keyring）。前端侧边栏改为双栏：左侧图标轨展示所有主模块，右侧上下文面板按激活模块展示子导航项，由单一配置源 `src/lib/nav-config.js` 驱动。

**Tech Stack:** russh 0.62.6、russh-sftp 2.4.0、@xterm/xterm 6.x、@xterm/addon-fit、Tauri 2 `emit`/`invoke` 事件流、Vue 3 + vue-router（hash）。

**Spec:** `docs/superpowers/specs/2026-08-16-ssh-module-design.md`

## Global Constraints

- 凭据/私钥明文**只在 Rust 侧解密使用**，绝不回传前端；前端只拿 `SshConnectionInfo`（无密文字段）
- 终端/文件数据一律 base64 编码传输（终端输出可能含任意字节，不能用 `from_utf8_lossy`）
- 现有命令风格：Rust 用 `#[tauri::command]` + `Result<T, String>`；前端用 `invoke` + `@tauri-apps/api/event` 的 `listen`
- Rust 事件用 `use tauri::Emitter;` + `app.emit("event-name", payload)`（主窗口）
- i18n 键需同步三个语言文件 `src/locales/{zh,en,ru}.json`，键格式 `nav.ssh` / `ssh.*`
- 新增 lucide 图标必须先加 `icon-map.js` 映射 + `AppIcon.vue` 的按需导入
- 前端无单测框架：用 `pnpm check`（vite build）验证编译，交互逻辑按步骤手动验证
- Rust 单测：`cargo test --manifest-path src-tauri/Cargo.toml`
- 每次任务结束跑全量门禁：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`pnpm check`
- 每次提交前确保 pre-commit 钩子通过（项目已配置，提交即触发）

---

## 文件结构

**新建（Rust 后端）：**
- `src-tauri/src/utils/crypto.rs` — 共享加密 vault（AES-256-GCM + keyring）
- `src-tauri/src/commands/ssh/mod.rs` — 模块声明 + 命令转发
- `src-tauri/src/commands/ssh/connections.rs` — 连接配置 CRUD（加密存储）
- `src-tauri/src/commands/ssh/session.rs` — 连接池、认证、host key 校验
- `src-tauri/src/commands/ssh/terminal.rs` — PTY 终端会话与数据流
- `src-tauri/src/commands/ssh/sftp.rs` — SFTP 操作

**修改（Rust 后端）：**
- `src-tauri/src/commands/password_manager.rs` — 改用共享 `CryptoVault`
- `src-tauri/src/commands/mod.rs` — `pub mod ssh;`
- `src-tauri/src/utils/mod.rs` — `pub mod crypto;`
- `src-tauri/src/lib.rs` — 注册 `ssh::*` 命令 + manage `SshSessionManager`
- `src-tauri/Cargo.toml` — 新增 `russh`、`russh-sftp`、`bytes`

**新建（前端）：**
- `src/lib/nav-config.js` — 导航配置单一事实来源
- `src/views/SSHConnections.vue` — 连接管理页
- `src/views/SSHTerminal.vue` — 终端页（多标签）
- `src/views/SSHSftp.vue` — SFTP 页

**修改（前端）：**
- `src/router.js` — 新增 3 条懒加载路由
- `src/components/Sidebar.vue` — 双栏改造
- `src/components/AppIcon.vue` — 新增图标按需导入
- `src/lib/icon-map.js` — 新增图标映射
- `src/locales/{zh,en,ru}.json` — 新增 i18n 键
- `package.json` — 新增 xterm 依赖

---

### Task 1: 共享加密模块 `utils/crypto.rs` 抽取

**Files:**
- Create: `src-tauri/src/utils/crypto.rs`
- Modify: `src-tauri/src/utils/mod.rs`
- Modify: `src-tauri/src/commands/password_manager.rs`

**Interfaces:**
- Produces: `pub struct CryptoVault { .. }`，方法 `CryptoVault::new() -> Self`、`fn encrypt(&self, data: &str) -> Result<String, String>`、`fn decrypt(&self, encrypted_data: &str) -> Result<String, String>`。密钥服务名 `com.devnexus.app` / 用户 `encryption-key`，回退文件 `password_key.bin`，与现有 vault 完全一致（**保证既有加密数据可继续解密**）。
- Consumes: 无。

- [ ] **Step 1: 写失败测试**

创建 `src-tauri/src/utils/crypto.rs`，先写测试（`#[cfg(test)]`）：

```rust
use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine as _};
use rand::Rng;
use std::fs;
use std::sync::{Arc, Mutex};

/// 共享加密保险库：AES-256-GCM，密钥存 OS keyring（回退 data_dir/password_key.bin）。
/// 与 password_manager 共用同一密钥，保证既有数据可解密。
pub struct CryptoVault {
    key: Arc<Mutex<[u8; 32]>>,
}

#[allow(clippy::new_without_default)]
impl CryptoVault {
    pub fn new() -> Self {
        Self { key: Arc::new(Mutex::new(Self::load_or_create_key())) }
    }

    fn key_file_path() -> std::path::PathBuf {
        crate::utils::data_dir().join("password_key.bin")
    }

    fn load_or_create_key() -> [u8; 32] {
        // TODO: 由 Step 3 填充
        unimplemented!()
    }

    fn read_key_file() -> Option<[u8; 32]> {
        let data = std::fs::read(Self::key_file_path()).ok()?;
        if data.len() != 32 { return None; }
        let mut key = [0u8; 32];
        key.copy_from_slice(&data);
        Some(key)
    }

    fn write_key_file(key: &[u8; 32]) -> bool {
        let path = Self::key_file_path();
        if let Some(parent) = path.parent() {
            if std::fs::create_dir_all(parent).is_err() { return false; }
        }
        if std::fs::write(&path, key).is_err() { return false; }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }
        true
    }

    pub fn encrypt(&self, data: &str) -> Result<String, String> {
        let key = self.key.lock().map_err(|e| format!("Encryption lock error: {}", e))?;
        let cipher = Aes256Gcm::new_from_slice(&key[..]).map_err(|e| format!("Encryption error: {}", e))?;
        let nonce_bytes: [u8; 12] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, data.as_bytes()).map_err(|e| format!("Encryption error: {}", e))?;
        let mut combined = nonce_bytes.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(general_purpose::STANDARD.encode(&combined))
    }

    pub fn decrypt(&self, encrypted_data: &str) -> Result<String, String> {
        let combined = general_purpose::STANDARD.decode(encrypted_data).map_err(|e| format!("Decoding error: {}", e))?;
        if combined.len() < 12 { return Err("Invalid encrypted data".to_string()); }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let key = self.key.lock().map_err(|e| format!("Decryption lock error: {}", e))?;
        let cipher = Aes256Gcm::new_from_slice(&key[..]).map_err(|e| format!("Decryption error: {}", e))?;
        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| format!("Decryption error: {}", e))?;
        String::from_utf8(plaintext).map_err(|e| format!("UTF-8 error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vault() -> CryptoVault {
        // 用固定密钥构造，避免触碰 keyring（CI 无 Secret Service）
        CryptoVault { key: Arc::new(Mutex::new([7u8; 32])) }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let v = test_vault();
        let plain = "ssh secret \u{4f60}\u{597d} 123";
        let enc = v.encrypt(plain).unwrap();
        assert_ne!(enc, plain);
        assert_eq!(v.decrypt(&enc).unwrap(), plain);
    }

    #[test]
    fn test_encrypt_produces_randomized_nonce() {
        let v = test_vault();
        let a = v.encrypt("same").unwrap();
        let b = v.encrypt("same").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_decrypt_invalid_base64_fails() {
        let v = test_vault();
        assert!(v.decrypt("!!!not-base64!!!").is_err());
    }

    #[test]
    fn test_decrypt_truncated_data_fails() {
        let v = test_vault();
        assert!(v.decrypt("AAAA").is_err());
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml utils::crypto`
Expected: FAIL（`unimplemented!()` panic + `mod crypto not found` 编译错误）。先将 `src-tauri/src/utils/mod.rs` 加上 `pub mod crypto;` 解决 `mod crypto not found`。

- [ ] **Step 3: 实现 `load_or_create_key`**

从 `password_manager.rs` 的 `load_or_create_key` / `read_key_file` / `write_key_file` / `migrate_from_keyfile` / `try_remove_old_keyfile`（第 99–271 行）整体迁移到 `CryptoVault`，保留相同 SERVICE_NAME/KEYRING_USER 与回退链。复制后把其中的 `Self::` 路径保持（方法名相同），删掉 password_manager 中的重复实现。

```rust
fn load_or_create_key() -> [u8; 32] {
    const SERVICE_NAME: &str = "com.devnexus.app";
    const KEYRING_USER: &str = "encryption-key";

    let entry = keyring::Entry::new(SERVICE_NAME, KEYRING_USER).ok();
    if let Some(ref entry) = entry {
        if let Ok(pw) = entry.get_password() {
            if let Ok(decoded) = general_purpose::STANDARD.decode(&pw) {
                if decoded.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&decoded);
                    Self::try_remove_old_keyfile();
                    return key;
                }
            }
        }
    }

    if let Some(key) = Self::read_key_file() {
        Self::try_remove_old_keyfile();
        return key;
    }

    if let Some(key) = Self::migrate_from_keyfile(entry.as_ref()) {
        Self::try_remove_old_keyfile();
        return key;
    }

    let mut key = [0u8; 32];
    rand::thread_rng().fill(&mut key);

    let mut persisted = false;
    if let Some(ref entry) = entry {
        let encoded = general_purpose::STANDARD.encode(key);
        match entry.set_password(&encoded) {
            Ok(_) => persisted = true,
            Err(e) => eprintln!("[CryptoVault] Failed to persist key to keyring: {}", e),
        }
    }
    if !persisted {
        persisted = Self::write_key_file(&key);
        if !persisted {
            eprintln!("[CryptoVault] WARNING: unable to persist encryption key (keyring and file both unavailable).");
        }
    }
    if persisted {
        Self::try_remove_old_keyfile();
    }
    key
}

fn migrate_from_keyfile(entry: Option<&keyring::Entry>) -> Option<[u8; 32]> {
    // 从 password_manager.rs 原样迁移（旧版 key.bin 迁移逻辑）
    let base = if cfg!(target_os = "macos") {
        std::env::var("HOME").map(|h| std::path::PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else {
        std::env::var("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    };
    let key_path = base.join("devnexus").join("key.bin");
    let data = std::fs::read(&key_path).ok()?;
    let key = if data.len() == 48 {
        let mut k = [0u8; 32];
        k.copy_from_slice(&data[16..]);
        k
    } else if data.len() == 32 {
        let mut k = [0u8; 32];
        k.copy_from_slice(&data);
        k
    } else {
        return None;
    };
    if let Some(e) = entry {
        let encoded = general_purpose::STANDARD.encode(key);
        if let Err(err) = e.set_password(&encoded) {
            eprintln!("[CryptoVault] Failed to persist master password to keyring: {}", err);
        }
    }
    Some(key)
}

fn try_remove_old_keyfile() {
    let base = if cfg!(target_os = "macos") {
        let Ok(home) = std::env::var("HOME") else { return; };
        std::path::PathBuf::from(home).join("Library/Application Support")
    } else if cfg!(target_os = "windows") {
        let Ok(appdata) = std::env::var("APPDATA") else { return; };
        std::path::PathBuf::from(appdata)
    } else {
        let Ok(home) = std::env::var("HOME") else { return; };
        std::path::PathBuf::from(home).join(".config")
    };
    let _ = std::fs::remove_file(base.join("devnexus").join("key.bin"));
}
```

- [ ] **Step 4: 重构 `PasswordManager` 使用共享 vault**

在 `password_manager.rs` 中：
- 删除重复的 `load_or_create_key`/`read_key_file`/`write_key_file`/`migrate_from_keyfile`/`try_remove_old_keyfile`/`encrypt`/`decrypt` 方法及其辅助函数（第 99–322 行）
- `PasswordManager` 增加字段 `crypto: CryptoVault`，`new()` 中 `crypto: crate::utils::crypto::CryptoVault::new()`
- 把内部 `self.encrypt(&json)` / `self.decrypt(&encrypted)` 改为 `self.crypto.encrypt(&json)` / `self.crypto.decrypt(&encrypted)`

```rust
// password_manager.rs 顶部结构变化
use crate::utils::crypto::CryptoVault;
use std::sync::{Arc, Mutex};

pub struct PasswordManager {
    pub entries: Arc<Mutex<Vec<PasswordEntry>>>,
    pub next_id: Arc<Mutex<u32>>,
    crypto: CryptoVault,
}
```

- [ ] **Step 5: 运行全部测试验证通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 全部 PASS（含 password_manager 既有 7 个测试 + crypto 新 4 个测试）。

- [ ] **Step 6: 门禁 + 提交**

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml && cargo clippy --all-targets -- -D warnings
git add src-tauri/src/utils/crypto.rs src-tauri/src/utils/mod.rs src-tauri/src/commands/password_manager.rs
git commit -m "refactor: 抽取共享 CryptoVault，SSH 与密码管理器复用 AES-GCM 加密"
```

---

### Task 2: SSH 依赖与连接配置 CRUD

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/commands/ssh/mod.rs`
- Create: `src-tauri/src/commands/ssh/connections.rs`
- Modify: `src-tauri/src/commands/mod.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone)] pub struct SshConnection { pub id: String, pub name: String, pub host: String, pub port: u16, pub username: String, pub auth_type: String /* "password"|"private_key" */, pub encrypted_secret: String, pub key_passphrase_encrypted: Option<String>, pub created_at: i64, pub updated_at: i64 }`
  - `#[derive(Serialize, Deserialize, Clone)] pub struct SshConnectionInfo { pub id: String, pub name: String, pub host: String, pub port: u16, pub username: String, pub auth_type: String }`（无密文）
  - `#[tauri::command] pub fn ssh_list_connections(state: tauri::State<SshStore>) -> Result<Vec<SshConnectionInfo>, String>`
  - `#[tauri::command] pub fn ssh_save_connection(state: tauri::State<SshStore>, connection: SshConnectionInput) -> Result<SshConnectionInfo, String>`，`SshConnectionInput` 含 `secret: String`（明文密码/私钥，前端一次提交）与可选 `key_passphrase: Option<String>`
  - `#[tauri::command] pub fn ssh_delete_connection(state: tauri::State<SshStore>, id: String) -> Result<(), String>`
  - `pub struct SshStore { pub vault: CryptoVault, pub conns: Arc<Mutex<Vec<SshConnection>>> }`，`impl SshStore { pub fn new() -> Self }`（启动即加载 `ssh_connections.json`）
- Consumes: Task 1 的 `CryptoVault`。

- [ ] **Step 1: 加依赖**

`src-tauri/Cargo.toml` 的 `[dependencies]` 增加：

```toml
russh = "0.62"
russh-sftp = "2.4"
bytes = "1"
```

Run: `cargo check --manifest-path src-tauri/Cargo.toml` 确认依赖解析成功。

- [ ] **Step 2: 写连接序列化/加解密测试**

创建 `src-tauri/src/commands/ssh/connections.rs`，先写测试与结构（`SshStore` 构造带固定密钥 vault）：

```rust
use crate::utils::crypto::CryptoVault;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone)]
pub struct SshConnection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub encrypted_secret: String,
    pub key_passphrase_encrypted: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SshConnectionInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct SshConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: String,
    pub secret: String,
    pub key_passphrase: Option<String>,
}

pub struct SshStore {
    pub vault: CryptoVault,
    pub conns: Arc<Mutex<Vec<SshConnection>>>,
    pub next_id: Arc<Mutex<u64>>,
}

impl SshStore {
    pub fn new() -> Self { /* Step 4 实现：CryptoVault::new() + load */ }

    fn conns_path() -> std::path::PathBuf {
        crate::utils::data_dir().join("ssh_connections.json")
    }

    fn encrypt_secret(&self, input: &SshConnectionInput) -> Result<(String, Option<String>), String> {
        let enc = self.vault.encrypt(&input.secret)?;
        let pass = match &input.key_passphrase {
            Some(p) if !p.is_empty() => Some(self.vault.encrypt(p)?),
            _ => None,
        };
        Ok((enc, pass))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> SshStore {
        SshStore {
            vault: CryptoVault::for_test(),
            conns: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    #[test]
    fn test_encrypt_secret_roundtrip() {
        let store = test_store();
        let input = SshConnectionInput {
            id: None, name: "prod".into(), host: "10.0.0.1".into(), port: 22,
            username: "root".into(), auth_type: "password".into(),
            secret: "s3cret".into(), key_passphrase: None,
        };
        let (enc, pass) = store.encrypt_secret(&input).unwrap();
        assert_ne!(enc, "s3cret");
        assert!(pass.is_none());
        assert_eq!(store.vault.decrypt(&enc).unwrap(), "s3cret");
    }

    #[test]
    fn test_info_excludes_secret() {
        let conn = SshConnection {
            id: "1".into(), name: "prod".into(), host: "h".into(), port: 22,
            username: "u".into(), auth_type: "password".into(),
            encrypted_secret: "TOP-SECRET".into(), key_passphrase_encrypted: None,
            created_at: 0, updated_at: 0,
        };
        let info: SshConnectionInfo = conn.into();
        assert_eq!(info.id, "1");
        assert!(!serde_json::to_string(&info).unwrap().contains("TOP-SECRET"));
    }
}
```

说明：`CryptoVault::for_test()` 是本任务临时加的测试辅助——在 `crypto.rs` 增加 `pub fn for_test() -> Self { Self { key: Arc::new(Mutex::new([9u8; 32])) } }`。

- [ ] **Step 3: 运行测试验证失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ssh::connections`
Expected: FAIL（`SshStore` 等未实现、`CryptoVault::for_test` 不存在）。先创建 `src-tauri/src/commands/ssh/mod.rs`：`pub mod connections;`，并在 `commands/mod.rs` 加 `pub mod ssh;`。

- [ ] **Step 4: 实现 CRUD 命令**

补全 `SshStore` 与命令：

```rust
impl SshStore {
    pub fn new() -> Self {
        let store = Self {
            vault: CryptoVault::new(),
            conns: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        };
        let _ = store.load();
        store
    }

    fn load(&self) -> Result<(), String> {
        let path = Self::conns_path();
        if !path.exists() { return Ok(()); }
        let data = std::fs::read(&path).map_err(|e| format!("read: {}", e))?;
        let list: Vec<SshConnection> = serde_json::from_slice(&data).map_err(|e| format!("parse: {}", e))?;
        let mut conns = self.conns.lock().map_err(|e| e.to_string())?;
        *conns = list;
        let max = conns.iter().filter_map(|c| c.id.parse::<u64>().ok()).max().unwrap_or(0);
        let mut nid = self.next_id.lock().map_err(|e| e.to_string())?;
        *nid = max + 1;
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        let path = Self::conns_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conns = self.conns.lock().map_err(|e| e.to_string())?;
        let data = serde_json::to_vec(&*conns).map_err(|e| e.to_string())?;
        std::fs::write(&path, &data).map_err(|e| format!("write: {}", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn to_info(c: &SshConnection) -> SshConnectionInfo {
        SshConnectionInfo {
            id: c.id.clone(), name: c.name.clone(), host: c.host.clone(), port: c.port,
            username: c.username.clone(), auth_type: c.auth_type.clone(),
        }
    }

    pub fn find(&self, id: &str) -> Option<SshConnection> {
        self.conns.lock().ok()?.iter().find(|c| c.id == id).cloned()
    }

    /// 供 session.rs 使用：解密 secret（密码或私钥 PEM 文本）
    pub fn decrypt_secret(&self, conn: &SshConnection) -> Result<String, String> {
        self.vault.decrypt(&conn.encrypted_secret)
    }

    pub fn decrypt_passphrase(&self, conn: &SshConnection) -> Result<Option<String>, String> {
        match &conn.key_passphrase_encrypted {
            Some(enc) => Ok(Some(self.vault.decrypt(enc)?)),
            None => Ok(None),
        }
    }
}

impl From<SshConnection> for SshConnectionInfo {
    fn from(c: SshConnection) -> Self { SshStore::to_info(&c) }
}

fn now_ts() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[tauri::command]
pub fn ssh_list_connections(state: tauri::State<SshStore>) -> Result<Vec<SshConnectionInfo>, String> {
    let conns = state.conns.lock().map_err(|e| e.to_string())?;
    Ok(conns.iter().map(SshStore::to_info).collect())
}

#[tauri::command]
pub fn ssh_save_connection(state: tauri::State<SshStore>, connection: SshConnectionInput) -> Result<SshConnectionInfo, String> {
    let (enc, pass) = state.encrypt_secret(&connection)?;
    let ts = now_ts();
    let mut conns = state.conns.lock().map_err(|e| e.to_string())?;
    let conn = match &connection.id {
        Some(id) if conns.iter().any(|c| &c.id == id) => {
            let c = conns.iter_mut().find(|c| &c.id == id).unwrap();
            c.name = connection.name;
            c.host = connection.host;
            c.port = connection.port;
            c.username = connection.username;
            c.auth_type = connection.auth_type;
            c.encrypted_secret = enc;
            c.key_passphrase_encrypted = pass;
            c.updated_at = ts;
            c.clone()
        }
        _ => {
            let mut nid = state.next_id.lock().map_err(|e| e.to_string())?;
            let id = nid.to_string();
            *nid += 1;
            let c = SshConnection {
                id: id.clone(), name: connection.name, host: connection.host,
                port: connection.port, username: connection.username,
                auth_type: connection.auth_type, encrypted_secret: enc,
                key_passphrase_encrypted: pass, created_at: ts, updated_at: ts,
            };
            conns.push(c.clone());
            c
        }
    };
    drop(conns);
    state.save()?;
    Ok(SshStore::to_info(&conn))
}

#[tauri::command]
pub fn ssh_delete_connection(state: tauri::State<SshStore>, id: String) -> Result<(), String> {
    let mut conns = state.conns.lock().map_err(|e| e.to_string())?;
    conns.retain(|c| c.id != id);
    drop(conns);
    state.save()
}
```

- [ ] **Step 5: 运行测试验证通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ssh::connections`
Expected: PASS。

- [ ] **Step 6: 门禁 + 提交**

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml && cargo clippy --all-targets -- -D warnings
git add src-tauri/Cargo.toml src-tauri/src/commands/ssh src-tauri/src/commands/mod.rs src-tauri/src/utils/crypto.rs
git commit -m "feat(ssh): 连接配置加密存储与 CRUD 命令"
```

---

### Task 3: SSH 连接池与会话认证（`session.rs`）

**Files:**
- Create: `src-tauri/src/commands/ssh/session.rs`
- Modify: `src-tauri/src/commands/ssh/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct SshSessionManager { pub sessions: Mutex<HashMap<String, SessionEntry>> }`，`impl SshSessionManager { pub fn new() -> Self }`
  - `pub struct SessionEntry { pub client: russh::client::Handle<SshHandler>, pub connection_id: String }`
  - `pub struct SshHandler { pub server_key: Arc<Mutex<Option<russh::keys::PublicKey>>> }`
  - `pub async fn open(app: &tauri::AppHandle, store: &SshStore, manager: &SshSessionManager, connection_id: &str) -> Result<String, String>` — 返回 `session_id`；完成 host key 校验/首次提示、认证（密码或私钥）
  - `pub async fn close(manager: &SshSessionManager, session_id: &str)`
  - `pub fn fingerprint(server_key: &russh::keys::PublicKey) -> String` — SHA256 指纹，形如 `SHA256:xxxx`
  - 事件：`ssh-hostkey-prompt`（payload `{ host: String, fingerprint: String, session_id: String }`）
  - 命令：`#[tauri::command] pub async fn ssh_hostkey_accept(app: tauri::AppHandle, state: tauri::State<SshSessionManager>, session_id: String, host: String, fingerprint: String) -> Result<(), String>`、`ssh_hostkey_reject(...)`、`ssh_close(session_id)`、`ssh_test_connection(store, connection_id)`
- Consumes: Task 2 的 `SshStore`（`find`/`decrypt_secret`/`decrypt_passphrase`）。

- [ ] **Step 1: 写 host key 指纹与 known_hosts 测试**

先写纯逻辑测试（known_hosts 文件读写 + 指纹字符串化），不依赖真实网络：

```rust
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

pub struct SshSessionManager {
    pub sessions: Mutex<HashMap<String, SessionEntry>>,
    pub known_hosts: Mutex<HashMap<String, String>>, // host:port -> fingerprint
    pub pending_keys: Mutex<HashMap<String, PendingKey>>, // session_id -> pending server key
}

pub struct PendingKey {
    pub host: String,
    pub fingerprint: String,
    pub server_key: russh::keys::PublicKey,
}

pub struct SessionEntry {
    pub client: russh::client::Handle<SshHandler>,
    pub connection_id: String,
}

pub struct SshHandler {
    pub server_key: Arc<Mutex<Option<russh::keys::PublicKey>>>,
}

fn known_hosts_path() -> std::path::PathBuf {
    crate::utils::data_dir().join("known_hosts.json")
}

fn load_known_hosts() -> HashMap<String, String> {
    let path = known_hosts_path();
    if !path.exists() { return HashMap::new(); }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_known_hosts(map: &HashMap<String, String>) -> Result<(), String> {
    let path = known_hosts_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    fs::write(&path, &data).map_err(|e| format!("write known_hosts: {}", e))
}

/// 校验 host key：
/// - 已知且匹配 -> Ok(Accept)
/// - 已知但不匹配 -> Err(HostKeyMismatch)
/// - 未知 -> Ok(Prompt) 等待前端确认后落盘
pub enum HostKeyCheck { Known, Unknown }

pub fn check_host_key(known: &HashMap<String, String>, host: &str, fingerprint: &str) -> Result<HostKeyCheck, String> {
    match known.get(host) {
        Some(k) if k == fingerprint => Ok(HostKeyCheck::Known),
        Some(_) => Err(format!("HOST_KEY_MISMATCH: {host} fingerprint changed")),
        None => Ok(HostKeyCheck::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_sha256_format() {
        // russh PublicKey::fingerprint 返回 SHA256:<base64> 或 MD5:<hex>，这里只断言格式前缀由 russh 决定；
        // 我们封装只做展示。直接测 check_host_key 逻辑。
        let mut known = HashMap::new();
        known.insert("h:22".to_string(), "SHA256:abc".to_string());
        assert!(matches!(check_host_key(&known, "h:22", "SHA256:abc").unwrap(), HostKeyCheck::Known));
        assert!(check_host_key(&known, "h:22", "SHA256:xyz").is_err());
        assert!(matches!(check_host_key(&known, "new:22", "SHA256:zzz").unwrap(), HostKeyCheck::Unknown));
    }

    #[test]
    fn test_known_hosts_roundtrip() {
        let mut map = HashMap::new();
        map.insert("a:22".to_string(), "SHA256:x".to_string());
        save_known_hosts(&map).unwrap();
        let loaded = load_known_hosts();
        assert_eq!(loaded.get("a:22").map(String::as_str), Some("SHA256:x"));
        let _ = fs::remove_file(known_hosts_path());
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ssh::session`
Expected: FAIL（模块不存在）。在 `ssh/mod.rs` 加 `pub mod session;`。`save_known_hosts` 与 `check_host_key` 测试应该能跑过——把实现先补齐（上面代码已含），验证"未知→Prompt / 不匹配→Err"语义正确。`check_host_key` 的调用方（Step 4）是编译期验证点。

- [ ] **Step 3: 实现 `SshHandler` + 连接认证**

补全 handler 与 `open`：

```rust
use russh::client::Handler;
use russh::keys::PrivateKeyWithHashAlg;

#[async_trait::async_trait]
impl Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server_public_key: &russh::keys::PublicKey) -> Result<bool, Self::Error> {
        *self.server_key.lock().unwrap() = Some(server_public_key.clone());
        Ok(true) // 先接受，connect 完成后由上层统一校验
    }
}

/// 生成展示用指纹（SHA256 优先）
pub fn fingerprint(server_key: &russh::keys::PublicKey) -> String {
    server_key.fingerprint().to_string()
}

fn session_config() -> Arc<russh::client::Config> {
    let mut config = russh::client::Config::default();
    config.keepalive_interval = Some(std::time::Duration::from_secs(30));
    config.keepalive_count = 3;
    config.set_keepalive(true);
    Arc::new(config)
}

async fn authenticate(
    client: &mut russh::client::Handle<SshHandler>,
    store: &SshStore,
    conn: &SshConnection,
) -> Result<(), String> {
    let user = &conn.username;
    match conn.auth_type.as_str() {
        "password" => {
            let pass = store.decrypt_secret(conn)?;
            match client.authenticate_password(user, &pass).await {
                Ok(russh::client::AuthResult::Success) => Ok(()),
                Ok(_) => Err("AUTH_FAILED: authentication did not succeed".into()),
                Err(e) => Err(format!("AUTH_FAILED: {e}")),
            }
        }
        "private_key" => {
            let pem = store.decrypt_secret(conn)?;
            let passphrase = store.decrypt_passphrase(conn)?;
            let key = russh::keys::decode_secret_key(pem.as_bytes(), passphrase.as_deref())
                .map_err(|e| format!("KEY_INVALID: {e}"))?;
            let key_with_hash = PrivateKeyWithHashAlg { key, hash_alg: russh::keys::HashAlg::SHA2_256 };
            match client.authenticate_publickey(user, &key_with_hash).await {
                Ok(russh::client::AuthResult::Success) => Ok(()),
                Ok(_) => Err("AUTH_FAILED: public key auth did not succeed".into()),
                Err(e) => Err(format!("AUTH_FAILED: {e}")),
            }
        }
        other => Err(format!("INVALID_AUTH_TYPE: {other}")),
    }
}
```

- [ ] **Step 4: 实现 `open`（连接 + host key 提示 + 认证）**

```rust
use tauri::Emitter;
use std::collections::HashMap;

impl SshSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            known_hosts: Mutex::new(load_known_hosts()),
            pending_keys: Mutex::new(HashMap::new()),
        }
    }

    fn host_key(&self, host: &str) -> Option<String> {
        self.known_hosts.lock().ok()?.get(host).cloned()
    }

    fn record_host_key(&self, host: &str, fingerprint: &str) {
        if let Ok(mut kh) = self.known_hosts.lock() {
            kh.insert(host.to_string(), fingerprint.to_string());
            let _ = save_known_hosts(&kh);
        }
    }
}

pub async fn open(
    app: &tauri::AppHandle,
    store: &SshStore,
    manager: &SshSessionManager,
    connection_id: &str,
) -> Result<String, String> {
    let conn = store.find(connection_id).ok_or_else(|| format!("NOT_FOUND: connection {connection_id}"))?;
    let host_key = format!("{}:{}", conn.host, conn.port);

    // 1. TCP 连接
    let tcp = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::net::TcpStream::connect((conn.host.as_str(), conn.port)),
    )
    .await
    .map_err(|_| format!("TIMEOUT: connect to {}:{}", conn.host, conn.port))?
    .map_err(|e| format!("CONNECT_FAILED: {e}"))?;

    // 2. SSH 握手 + 捕获 server key
    let server_key: Arc<Mutex<Option<russh::keys::PublicKey>>> = Arc::new(Mutex::new(None));
    let handler = SshHandler { server_key: server_key.clone() };
    let config = session_config();
    let mut client = russh::client::connect(config, tcp, handler)
        .await
        .map_err(|e| format!("HANDSHAKE_FAILED: {e}"))?;

    let key = server_key.lock().unwrap().clone().ok_or_else(|| "NO_SERVER_KEY".to_string())?;
    let fp = fingerprint(&key);

    // 3. host key 校验
    let known = manager.host_key(&host_key);
    match known {
        Some(k) if k == fp => {}
        Some(_) => return Err(format!("HOST_KEY_MISMATCH: {} fingerprint changed", conn.host)),
        None => {
            // 首次连接：登记 pending，等待前端确认
            let session_id = uuid::Uuid::new_v4().to_string();
            manager.pending_keys.lock().unwrap().insert(
                session_id.clone(),
                PendingKey { host: host_key.clone(), fingerprint: fp.clone(), server_key: key.clone() },
            );
            let _ = app.emit("ssh-hostkey-prompt", serde_json::json!({
                "session_id": session_id,
                "host": host_key,
                "fingerprint": fp,
            }));
            // 等前端 ssh_hostkey_accept/reject（最长 30s）
            let start = std::time::Instant::now();
            loop {
                if start.elapsed() > std::time::Duration::from_secs(30) {
                    manager.pending_keys.lock().unwrap().remove(&session_id);
                    return Err("HOSTKEY_TIMEOUT: no confirmation".into());
                }
                let accepted = manager.pending_keys.lock().unwrap().get(&session_id).map(|p| p.fingerprint.clone());
                match accepted {
                    Some(f) if f == fp => {
                        manager.pending_keys.lock().unwrap().remove(&session_id);
                        manager.record_host_key(&host_key, &fp);
                        break;
                    }
                    Some(_) => {
                        manager.pending_keys.lock().unwrap().remove(&session_id);
                        return Err("HOSTKEY_REJECTED".into());
                    }
                    None => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
                }
            }
        }
    }

    // 4. 认证
    authenticate(&mut client, store, &conn).await?;

    // 5. 存入连接池
    let session_id = uuid::Uuid::new_v4().to_string();
    manager.sessions.lock().unwrap().insert(
        session_id.clone(),
        SessionEntry { client, connection_id: conn.id.clone() },
    );
    Ok(session_id)
}

pub async fn close(manager: &SshSessionManager, session_id: &str) {
    manager.sessions.lock().unwrap().remove(session_id);
}
```

- [ ] **Step 5: 实现 host key 确认与测试命令**

```rust
#[tauri::command]
pub fn ssh_hostkey_accept(
    state: tauri::State<SshSessionManager>,
    session_id: String,
    host: String,
    fingerprint: String,
) -> Result<(), String> {
    let mut pending = state.pending_keys.lock().map_err(|e| e.to_string())?;
    let p = pending.get_mut(&session_id).ok_or_else(|| "NO_PENDING_KEY".to_string())?;
    if p.host != host || p.fingerprint != fingerprint {
        return Err("HOSTKEY_MISMATCH".into());
    }
    Ok(())
}

#[tauri::command]
pub fn ssh_hostkey_reject(state: tauri::State<SshSessionManager>, session_id: String) -> Result<(), String> {
    state.pending_keys.lock().map_err(|e| e.to_string())?.remove(&session_id);
    Ok(())
}

#[tauri::command]
pub fn ssh_close(state: tauri::State<SshSessionManager>, session_id: String) -> Result<(), String> {
    state.sessions.lock().map_err(|e| e.to_string())?.remove(&session_id);
    Ok(())
}

/// 连接测试：复用 open 但立即关闭
#[tauri::command]
pub async fn ssh_test_connection(
    app: tauri::AppHandle,
    store: tauri::State<'_, SshStore>,
    manager: tauri::State<'_, SshSessionManager>,
    connection_id: String,
) -> Result<String, String> {
    let sid = open(&app, &store, &manager, &connection_id).await?;
    close(&manager, &sid).await;
    Ok("ok".into())
}
```

注意：`open` 里等待确认的循环依赖 `pending_keys` 中 fingerprint 变化来判断 accept（`ssh_hostkey_accept` 只是校验参数，不主动移除）。可改为在 `ssh_hostkey_accept` 校验通过后直接 `pending.remove(&session_id)`，循环里用 `pending.contains_key` 判断。实现时采用后者（语义更清晰）：accept → remove；循环里 `pending.get(&session_id)` 返回 None 且此前为 Some 则视为接受，超时判拒绝。

- [ ] **Step 6: 运行测试 + 编译**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ssh::session` → PASS；`cargo check --manifest-path src-tauri/Cargo.toml` → OK。

- [ ] **Step 7: 门禁 + 提交**

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml && cargo clippy --all-targets -- -D warnings
git add src-tauri/src/commands/ssh
git commit -m "feat(ssh): 连接池、认证与 host key 首连确认"
```

---

### Task 4: 终端 PTY 会话与数据流（`terminal.rs`）

**Files:**
- Create: `src-tauri/src/commands/ssh/terminal.rs`
- Modify: `src-tauri/src/commands/ssh/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct TerminalSession { pub session_id: String, pub connection_id: String, pub write: tokio::sync::Mutex<russh::ChannelWriteHalf<russh::client::Msg>> }`
  - `#[tauri::command] pub async fn ssh_terminal_open(app: tauri::AppHandle, store: tauri::State<'_, SshStore>, manager: tauri::State<'_, SshSessionManager>, connection_id: String, cols: u32, rows: u32) -> Result<String, String>` — 返回 terminal session_id
  - `#[tauri::command] pub async fn ssh_terminal_input(app: tauri::AppHandle, state: tauri::State<'_, SshSessionManager>, session_id: String, data: String) -> Result<(), String>` — `data` 为 base64
  - `#[tauri::command] pub async fn ssh_terminal_resize(state: tauri::State<'_, SshSessionManager>, session_id: String, cols: u32, rows: u32) -> Result<(), String>`
  - `#[tauri::command] pub async fn ssh_terminal_close(state: tauri::State<'_, SshSessionManager>, session_id: String) -> Result<(), String>`
  - 事件：`ssh-terminal-output`（payload `{ session_id: String, data: String /* base64 */ }`）、`ssh-terminal-closed`（`{ session_id: String, reason: String }`）
- Consumes: Task 3 的 `SshSessionManager`（`sessions` 的 `SessionEntry.client`）、`open`。

- [ ] **Step 1: 实现终端会话状态与 open**

`SessionEntry` 增加终端通道表。在 `session.rs` 的 `SessionEntry` 中加字段（Task 3 已建，这里补）：

```rust
pub struct SessionEntry {
    pub client: russh::client::Handle<SshHandler>,
    pub connection_id: String,
    pub terminals: tokio::sync::Mutex<HashMap<String, TerminalHandle>>,
}
// TerminalHandle 持有可写的 write half（读 half 已移入后台 task）
pub struct TerminalHandle {
    pub write: russh::ChannelWriteHalf<russh::client::Msg>,
}
```

新建 `terminal.rs`：

```rust
use crate::commands::ssh::session::{open, SshSessionManager, SessionEntry};
use crate::commands::ssh::connections::SshStore;
use russh::channels::ChannelMsg;
use russh::client::Handle;
use tauri::Emitter;
use tokio::sync::Mutex;
use std::collections::HashMap;

fn b64(data: &[u8]) -> String {
    use base64::{engine::general_purpose, Engine as _};
    general_purpose::STANDARD.encode(data)
}

#[tauri::command]
pub async fn ssh_terminal_open(
    app: tauri::AppHandle,
    store: tauri::State<'_, SshStore>,
    manager: tauri::State<'_, SshSessionManager>,
    connection_id: String,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    // 复用已存在的连接会话；否则新建
    let session_id = {
        let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
        sessions.values().find(|s| s.connection_id == connection_id).map(|s| s.connection_id.clone())
    };
    let sid = match session_id {
        Some(id) => {
            // 该连接已有会话，取它的 client 直接开通道（新逻辑在下方 open_channel 用）
            let mut sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
            let entry = sessions.get_mut(&id).ok_or("NO_SESSION")?;
            let client = entry.client.clone();
            open_channel(&app, manager, entry, &id, client, cols, rows).await?
        }
        None => {
            let sid = open(&app, &store, &manager, &connection_id).await?;
            let mut sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
            let entry = sessions.get_mut(&sid).ok_or("NO_SESSION")?;
            let client = entry.client.clone();
            open_channel(&app, manager, entry, &sid, client, cols, rows).await?
        }
    };
    Ok(sid)
}
```

- [ ] **Step 2: 实现通道开启 + 读任务 + 输入命令**

```rust
async fn open_channel<H: russh::client::Handler>(
    app: &tauri::AppHandle,
    manager: &SshSessionManager,
    entry: &mut SessionEntry,
    sid: &str,
    client: Handle<H>,
    cols: u32,
    rows: u32,
) -> Result<String, String> {
    let mut channel = client.channel_open_session().await.map_err(|e| format!("OPEN_FAILED: {e}"))?;
    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await
        .map_err(|e| format!("PTY_FAILED: {e}"))?;
    channel.request_shell(false).await.map_err(|e| format!("SHELL_FAILED: {e}"))?;

    let term_id = uuid::Uuid::new_v4().to_string();
    let (read_half, write_half) = channel.split();

    // 读任务：ChannelMsg -> emit
    let app_clone = app.clone();
    let tid = term_id.clone();
    tokio::spawn(async move {
        let mut read = read_half;
        let mut closed = false;
        while let Some(msg) = read.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    let _ = app_clone.emit("ssh-terminal-output", serde_json::json!({
                        "session_id": tid, "data": b64(&data),
                    }));
                }
                ChannelMsg::ExtendedData { data, ext: _ } => {
                    let _ = app_clone.emit("ssh-terminal-output", serde_json::json!({
                        "session_id": tid, "data": b64(&data),
                    }));
                }
                ChannelMsg::Close | ChannelMsg::Eof => { closed = true; break; }
                _ => {}
            }
        }
        let _ = app_clone.emit("ssh-terminal-closed", serde_json::json!({
            "session_id": tid, "reason": if closed { "closed" } else { "error" },
        }));
    });

    entry.terminals.lock().await.insert(
        term_id.clone(),
        crate::commands::ssh::session::TerminalHandle { write: write_half },
    );
    Ok(term_id)
}

#[tauri::command]
pub async fn ssh_terminal_input(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    use base64::{engine::general_purpose, Engine as _};
    let bytes = general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|e| format!("INVALID_B64: {e}"))?;
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut terms = entry.terminals.lock().await;
        if let Some(t) = terms.get(&session_id) {
            t.write.data_bytes(bytes).await.map_err(|e| format!("WRITE_FAILED: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_TERMINAL: {session_id}"))
}

#[tauri::command]
pub async fn ssh_terminal_resize(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut terms = entry.terminals.lock().await;
        if let Some(t) = terms.get(&session_id) {
            t.write.window_change(cols, rows, 0, 0).await.map_err(|e| format!("RESIZE_FAILED: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_TERMINAL: {session_id}"))
}

#[tauri::command]
pub async fn ssh_terminal_close(
    state: tauri::State<'_, SshSessionManager>,
    session_id: String,
) -> Result<(), String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut terms = entry.terminals.lock().await;
        if let Some(t) = terms.remove(&session_id) {
            let _ = t.write.eof().await;
            let _ = t.write.close().await;
            return Ok(());
        }
    }
    Ok(())
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: OK。若 `Handle<H>` 泛型在 `open_channel` 传参上有生命周期/泛型问题，将 `open_channel` 的参数类型改为 `russh::client::Handle<SshHandler>`（我们只用具体 handler）。

- [ ] **Step 4: 门禁 + 提交**

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml && cargo clippy --all-targets -- -D warnings
git add src-tauri/src/commands/ssh
git commit -m "feat(ssh): PTY 终端会话、输入输出数据流与缩放"
```

---

### Task 5: SFTP 文件操作（`sftp.rs`）

**Files:**
- Create: `src-tauri/src/commands/ssh/sftp.rs`
- Modify: `src-tauri/src/commands/ssh/mod.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize)] pub struct SftpEntry { pub name: String, pub is_dir: bool, pub size: u64, pub mode: u32, pub mtime: i64 }`
  - `#[tauri::command] pub async fn ssh_sftp_open(app: tauri::AppHandle, store: tauri::State<'_, SshStore>, manager: tauri::State<'_, SshSessionManager>, connection_id: String) -> Result<String, String>` — 返回 sftp_id
  - `#[tauri::command] pub async fn ssh_sftp_list_dir(manager: tauri::State<'_, SshSessionManager>, sftp_id: String, path: String) -> Result<Vec<SftpEntry>, String>`
  - `#[tauri::command] pub async fn ssh_sftp_read_file(manager: tauri::State<'_, SshSessionManager>, sftp_id: String, path: String, offset: u64, length: usize) -> Result<String, String>` — base64
  - `#[tauri::command] pub async fn ssh_sftp_write_file(manager: tauri::State<'_, SshSessionManager>, sftp_id: String, path: String, data: String, offset: u64) -> Result<(), String>` — `data` base64
  - `#[tauri::command] pub async fn ssh_sftp_mkdir / ssh_sftp_rename / ssh_sftp_delete / ssh_sftp_stat`（签名同列表）
- Consumes: Task 3 的 `SshSessionManager`、`open`。

- [ ] **Step 1: 实现 SFTP 会话与目录列举**

```rust
use crate::commands::ssh::connections::SshStore;
use crate::commands::ssh::session::{open, SshSessionManager};
use russh_sftp::client::SftpSession;
use serde::Serialize;
use tauri::Emitter;
use tokio::sync::Mutex;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct SftpEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mode: u32,
    pub mtime: i64,
}

/// SessionEntry 追加字段：sftp 会话表（sftp_id -> SftpSession）
pub struct SftpHandle { pub sftp: SftpSession }
// 在 session.rs 的 SessionEntry 追加：
//   pub sftp_sessions: tokio::sync::Mutex<HashMap<String, SftpHandle>>,

fn meta_to_entry(name: &str, meta: &russh_sftp::protocol::FileAttributes) -> SftpEntry {
    SftpEntry {
        name: name.to_string(),
        is_dir: meta.is_dir(),
        size: meta.size.unwrap_or(0),
        mode: meta.permissions.unwrap_or(0),
        mtime: meta.mtime.unwrap_or(0) as i64,
    }
}

#[tauri::command]
pub async fn ssh_sftp_open(
    app: tauri::AppHandle,
    store: tauri::State<'_, SshStore>,
    manager: tauri::State<'_, SshSessionManager>,
    connection_id: String,
) -> Result<String, String> {
    // 找既有连接会话，否则新建（与 terminal.rs 相同的取会话逻辑）
    let sid = {
        let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
        sessions.values().find(|s| s.connection_id == connection_id).map(|s| s.connection_id.clone())
    };
    let sid = match sid {
        Some(id) => id,
        None => open(&app, &store, &manager, &connection_id).await?,
    };

    let mut sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    let entry = sessions.get_mut(&sid).ok_or("NO_SESSION")?;
    let mut channel = entry.client.channel_open_session().await.map_err(|e| format!("OPEN_FAILED: {e}"))?;
    channel.request_subsystem(true, "sftp").await.map_err(|e| format!("SFTP_SUBSYSTEM_FAILED: {e}"))?;
    let sftp = SftpSession::new(channel.into_stream()).await.map_err(|e| format!("SFTP_INIT_FAILED: {e}"))?;

    let sftp_id = uuid::Uuid::new_v4().to_string();
    entry.sftp_sessions.lock().await.insert(sftp_id.clone(), crate::commands::ssh::session::SftpHandle { sftp });
    Ok(sftp_id)
}

#[tauri::command]
pub async fn ssh_sftp_list_dir(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
) -> Result<Vec<SftpEntry>, String> {
    let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let mut out = Vec::new();
            for f in h.sftp.read_dir(&path).await.map_err(|e| format!("SFTP_LIST: {e}"))? {
                out.push(meta_to_entry(&f.file_name().to_string_lossy(), f.metadata()));
            }
            out.sort_by(|a, b| (b.is_dir as u8).cmp(&(a.is_dir as u8)).then(a.name.cmp(&b.name)));
            return Ok(out);
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}
```

- [ ] **Step 2: 实现读写/增删改命令**

```rust
use base64::{engine::general_purpose, Engine as _};

async fn with_sftp<F, T>(manager: &SshSessionManager, sftp_id: &str, f: F) -> Result<T, String>
where
    F: for<'a> FnOnce(&'a mut SftpSession) -> futures_util::future::BoxFuture<'a, Result<T, String>>,
{
    // 简化：不用闭包抽象，直接在每个命令内联。此函数仅为占位说明——实际实现内联于各命令。
    unimplemented!()
}

#[tauri::command]
pub async fn ssh_sftp_read_file(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
    offset: u64,
    length: usize,
) -> Result<String, String> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let mut file = h.sftp.open(&path).await.map_err(|e| format!("SFTP_OPEN: {e}"))?;
            if offset > 0 {
                file.seek(tokio::io::SeekFrom::Start(offset)).await.map_err(|e| format!("SFTP_SEEK: {e}"))?;
            }
            let mut buf = vec![0u8; length];
            let n = file.read(&mut buf).await.map_err(|e| format!("SFTP_READ: {e}"))?;
            buf.truncate(n);
            return Ok(general_purpose::STANDARD.encode(&buf));
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_write_file(
    manager: tauri::State<'_, SshSessionManager>,
    sftp_id: String,
    path: String,
    data: String,
    offset: u64,
) -> Result<(), String> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let bytes = general_purpose::STANDARD.decode(data.as_bytes()).map_err(|e| format!("INVALID_B64: {e}"))?;
    let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let mut file = h.sftp.open_with_flags(
                &path,
                russh_sftp::protocol::OpenFlags::WRITE | russh_sftp::protocol::OpenFlags::CREATE,
            ).await.map_err(|e| format!("SFTP_OPEN: {e}"))?;
            if offset > 0 {
                file.seek(tokio::io::SeekFrom::Start(offset)).await.map_err(|e| format!("SFTP_SEEK: {e}"))?;
            }
            file.write_all(&bytes).await.map_err(|e| format!("SFTP_WRITE: {e}"))?;
            file.sync_all().await.map_err(|e| format!("SFTP_SYNC: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_mkdir(manager: tauri::State<'_, SshSessionManager>, sftp_id: String, path: String) -> Result<(), String> {
    let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            h.sftp.create_dir(&path).await.map_err(|e| format!("SFTP_MKDIR: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_rename(manager: tauri::State<'_, SshSessionManager>, sftp_id: String, from: String, to: String) -> Result<(), String> {
    let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            h.sftp.rename(&from, &to).await.map_err(|e| format!("SFTP_RENAME: {e}"))?;
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_delete(manager: tauri::State<'_, SshSessionManager>, sftp_id: String, path: String, is_dir: bool) -> Result<(), String> {
    let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            if is_dir {
                h.sftp.remove_dir(&path).await.map_err(|e| format!("SFTP_RMDIR: {e}"))?;
            } else {
                h.sftp.remove_file(&path).await.map_err(|e| format!("SFTP_RM: {e}"))?;
            }
            return Ok(());
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}

#[tauri::command]
pub async fn ssh_sftp_stat(manager: tauri::State<'_, SshSessionManager>, sftp_id: String, path: String) -> Result<SftpEntry, String> {
    let sessions = manager.sessions.lock().map_err(|e| e.to_string())?;
    for entry in sessions.values() {
        let mut map = entry.sftp_sessions.lock().await;
        if let Some(h) = map.get(&sftp_id) {
            let meta = h.sftp.stat(&path).await.map_err(|e| format!("SFTP_STAT: {e}"))?;
            let name = std::path::Path::new(&path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(path.clone());
            return Ok(meta_to_entry(&name, &meta));
        }
    }
    Err(format!("NO_SFTP: {sftp_id}"))
}
```

注意：`open_with_flags` 与 `create_dir`/`rename`/`remove_file`/`remove_dir`/`stat`/`open` 均为 `russh-sftp 2.x` 的 `SftpSession` 方法；若版本方法名不同（如 `create_dir` vs `create_dir`），以 `russh-sftp 2.4` 文档为准微调。`with_sftp` 占位函数若引入编译噪音则删除。

- [ ] **Step 3: 编译验证**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: OK。SFTP 相关 API 若签名不一致，参考 `russh-sftp` 2.4 的 `_autodocs/02-client-session.md` 与 `03-client-file-and-streams.md` 修正。

- [ ] **Step 4: 门禁 + 提交**

```bash
cargo fmt --check --manifest-path src-tauri/Cargo.toml && cargo clippy --all-targets -- -D warnings
git add src-tauri/src/commands/ssh
git commit -m "feat(ssh): SFTP 目录列举、读写、增删改"
```

---

### Task 6: 注册命令与状态（后端收尾）

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/ssh/mod.rs`

**Interfaces:**
- Consumes: Task 1–5 全部命令。
- Produces: 后端可被前端调用的完整命令集 + `manage` 的 `SshStore`/`SshSessionManager`。

- [ ] **Step 1: `ssh/mod.rs` 导出 + 状态构建**

```rust
pub mod connections;
pub mod session;
pub mod terminal;
pub mod sftp;
```

- [ ] **Step 2: `lib.rs` 注册状态与命令**

`lib.rs` 的 `setup` 或 builder 链中加：

```rust
.manage(commands::ssh::connections::SshStore::new())
.manage(commands::ssh::session::SshSessionManager::new())
```

在 `invoke_handler` 的 `generate_handler![]` 里追加：

```rust
commands::ssh::connections::ssh_list_connections,
commands::ssh::connections::ssh_save_connection,
commands::ssh::connections::ssh_delete_connection,
commands::ssh::session::ssh_hostkey_accept,
commands::ssh::session::ssh_hostkey_reject,
commands::ssh::session::ssh_close,
commands::ssh::session::ssh_test_connection,
commands::ssh::terminal::ssh_terminal_open,
commands::ssh::terminal::ssh_terminal_input,
commands::ssh::terminal::ssh_terminal_resize,
commands::ssh::terminal::ssh_terminal_close,
commands::ssh::sftp::ssh_sftp_open,
commands::ssh::sftp::ssh_sftp_list_dir,
commands::ssh::sftp::ssh_sftp_read_file,
commands::ssh::sftp::ssh_sftp_write_file,
commands::ssh::sftp::ssh_sftp_mkdir,
commands::ssh::sftp::ssh_sftp_rename,
commands::ssh::sftp::ssh_sftp_delete,
commands::ssh::sftp::ssh_sftp_stat,
```

- [ ] **Step 3: 编译 + 全量测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && cargo clippy --all-targets -- -D warnings`
Expected: PASS + 无警告。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/ssh/mod.rs
git commit -m "feat(ssh): 注册全部 SSH 命令与全局状态"
```

---

### Task 7: 前端基础 — 路由、导航配置、i18n、xterm 依赖

**Files:**
- Modify: `package.json`
- Modify: `src/router.js`
- Create: `src/lib/nav-config.js`
- Modify: `src/components/AppIcon.vue`
- Modify: `src/lib/icon-map.js`
- Modify: `src/locales/zh.json`, `src/locales/en.json`, `src/locales/ru.json`

**Interfaces:**
- Produces:
  - `export const navItems`（含 `context` 字段的结构，见下方）
  - `export function navForPath(path) -> { nav, sub }`（`sub` 为匹配的 context 子项或 null）
  - 新路由：`/ssh`、`/ssh/sessions`、`/ssh/sftp`
  - 新图标 key：`server`、`list`、`terminal`
- Consumes: 无。

- [ ] **Step 1: 加 xterm 依赖**

```bash
pnpm add @xterm/xterm@6 @xterm/addon-fit
```

- [ ] **Step 2: 建 `nav-config.js`**

```js
// src/lib/nav-config.js — 导航配置单一事实来源
export const navItems = [
  { id: "dashboard", route: "/dashboard", icon: "dashboard", labelKey: "nav.dashboard" },
  { id: "environments", route: "/environments", icon: "code", labelKey: "nav.environments" },
  { id: "migration", route: "/migration", icon: "swap", labelKey: "nav.migration" },
  { id: "software", route: "/software", icon: "apps", labelKey: "nav.software" },
  { id: "containers", route: "/containers", icon: "command", labelKey: "nav.containers" },
  { id: "mirrors", route: "/mirrors", icon: "sync", labelKey: "nav.mirrors" },
  { id: "processes", route: "/processes", icon: "thunderbolt", labelKey: "nav.processes" },
  { id: "passwords", route: "/passwords", icon: "lock", labelKey: "nav.passwords" },
  { id: "cookies", route: "/cookies", icon: "idcard", labelKey: "nav.cookies" },
  { id: "uninstall", route: "/uninstall", icon: "delete", labelKey: "nav.uninstall" },
  { id: "api-hub", route: "/api-hub", icon: "branch", labelKey: "nav.api_hub" },
  { id: "ssh", route: "/ssh", icon: "server", labelKey: "nav.ssh",
    context: {
      titleKey: "nav.ssh",
      items: [
        { route: "/ssh", icon: "list", labelKey: "ssh.connections" },
        { route: "/ssh/sessions", icon: "terminal", labelKey: "ssh.sessions" },
        { route: "/ssh/sftp", icon: "folder", labelKey: "ssh.sftp" },
      ],
    },
  },
  { id: "island", route: "/island", icon: "island", labelKey: "nav.island" },
  { id: "settings", route: "/settings", icon: "settings", labelKey: "nav.settings" },
];

/** 由当前路径推导激活的主导航与上下文子项 */
export function navForPath(path) {
  for (const nav of navItems) {
    if (path === nav.route || (nav.context && path.startsWith(nav.route))) {
      const sub = nav.context
        ? nav.context.items.find((i) => i.route === path) || null
        : null;
      return { nav, sub };
    }
  }
  return { nav: null, sub: null };
}

/** 取带上下文的导航项（供图标轨点击判断） */
export function navWithContext(id) {
  return navItems.find((n) => n.id === id && n.context) || null;
}
```

- [ ] **Step 3: 路由新增**

`router.js` 末尾（`/:pathMatch` 之前）追加：

```js
{
  path: "/ssh",
  component: () => import("./views/SSHConnections.vue"),
},
{
  path: "/ssh/sessions",
  component: () => import("./views/SSHTerminal.vue"),
},
{
  path: "/ssh/sftp",
  component: () => import("./views/SSHSftp.vue"),
},
```

- [ ] **Step 4: 图标映射与按需导入**

`icon-map.js` 增加：

```js
server: "Server",
list: "List",
terminal: "Terminal",
```

`AppIcon.vue` 的 import 与 `IconComp` 增加 `Server, List, Terminal`（来自 `@lucide/vue`）。

- [ ] **Step 5: i18n 键**

`zh.json`：
```json
"nav": { ...既有..., "ssh": "SSH" },
"ssh": {
  "connections": "连接",
  "sessions": "终端",
  "sftp": "文件",
  "add": "新建连接",
  "edit": "编辑连接",
  "delete": "删除连接",
  "test": "测试连接",
  "name": "名称",
  "host": "主机",
  "port": "端口",
  "username": "用户名",
  "auth_type": "认证方式",
  "password": "密码",
  "private_key": "私钥",
  "key_passphrase": "私钥口令",
  "connect": "连接",
  "hostkey_title": "确认服务器指纹",
  "hostkey_body": "是否信任 {host} 的指纹 {fingerprint}？",
  "accept": "信任",
  "reject": "取消",
  "open_terminal": "打开终端",
  "upload": "上传",
  "download": "下载",
  "refresh": "刷新",
  "new_folder": "新建文件夹",
  "rename": "重命名",
  "up": "上级目录",
  "empty": "暂无连接",
  "add_hint": "点击右上角新建 SSH 连接",
  "disconnected": "已断开",
  "reconnect": "重连"
}
```
`en.json` / `ru.json` 对应翻译（`nav.ssh` = "SSH"；ru 参考既有模块风格）。

- [ ] **Step 6: 编译验证**

Run: `pnpm check`
Expected: 构建通过（视图文件尚不存在会失败——先建三个最小占位视图，见 Task 9/10/11 的骨架，此处先建空壳导出即可）。

最小占位（三个文件先建后填）：
```vue
<template><div class="page"><h1>SSH</h1></div></template>
```

- [ ] **Step 7: 提交**

```bash
git add package.json pnpm-lock.yaml src/router.js src/lib/nav-config.js src/components/AppIcon.vue src/lib/icon-map.js src/locales src/views/SSHConnections.vue src/views/SSHTerminal.vue src/views/SSHSftp.vue
git commit -m "feat(ssh): 前端路由、导航上下文配置、i18n 与 xterm 依赖"
```

---

### Task 8: 侧边栏双栏改造

**Files:**
- Modify: `src/components/Sidebar.vue`

**Interfaces:**
- Consumes: Task 7 的 `navItems`/`navForPath`。
- Produces: 双栏侧边栏：图标轨（全部主模块）+ 上下文面板（激活模块的子项）。

- [ ] **Step 1: 替换 script 逻辑**

`Sidebar.vue` 的 `<script setup>` 中替换导航相关部分：

```js
import { computed } from "vue";
import { navItems, navForPath } from "../lib/nav-config.js";

const route = useRoute();

const active = computed(() => navForPath(route.path));

function handleNavClick(item) {
  router.push(item.route);
}

function handleSubClick(sub) {
  router.push(sub.route);
}
```

删除旧的 `navItems` 数组、`selectedKey` 计算（改用 `active`）。

- [ ] **Step 2: 重写模板为双栏**

`<aside class="sidebar">` 内替换 `<nav class="nav-menu">` 为：

```html
<div class="sidebar-body">
  <!-- 左：图标轨 -->
  <nav class="icon-rail" aria-label="Main navigation">
    <button
      v-for="item in navItems"
      :key="item.id"
      type="button"
      class="rail-item"
      :class="{ active: active.nav && active.nav.id === item.id }"
      :title="item.labelKey"
      @click="handleNavClick(item)"
    >
      <AppIcon :name="item.icon" class="rail-icon" />
    </button>
  </nav>

  <!-- 右：上下文面板 -->
  <nav v-if="active.nav && active.nav.context" class="context-panel">
    <div class="context-title">{{ t(active.nav.context.titleKey) }}</div>
    <button
      v-for="sub in active.nav.context.items"
      :key="sub.route"
      type="button"
      class="context-item"
      :class="{ active: active.sub && active.sub.route === sub.route }"
      @click="handleSubClick(sub)"
    >
      <AppIcon :name="sub.icon" class="context-icon" />
      <span>{{ t(sub.labelKey) }}</span>
    </button>
  </nav>
</div>
```

- [ ] **Step 3: 调整样式**

`.sidebar` 宽度改为：图标轨 52px + 上下文面板 172px（有上下文时），无上下文时仅 52px。新增：

```css
.sidebar-body { display: flex; flex: 1; min-height: 0; }
.icon-rail {
  width: 52px; flex-shrink: 0; padding: 8px 6px;
  display: flex; flex-direction: column; gap: 2px;
  border-right: 1px solid var(--color-border);
  overflow-y: auto;
}
.rail-item {
  display: flex; align-items: center; justify-content: center;
  height: 34px; border: none; border-radius: 6px;
  background: transparent; color: var(--color-sidebar-foreground);
  opacity: 0.72; cursor: pointer;
  transition: background-color 0.12s ease, opacity 0.12s ease;
}
.rail-item:hover { background-color: var(--color-sidebar-accent); opacity: 1; }
.rail-item.active { background-color: var(--color-sidebar-accent); opacity: 1; }
.rail-icon { width: 18px; height: 18px; }
.context-panel {
  flex: 1; min-width: 0; padding: 10px 8px;
  display: flex; flex-direction: column; gap: 2px;
  overflow-y: auto;
}
.context-title {
  padding: 4px 10px 8px; font-size: 12px; font-weight: 600;
  color: var(--color-muted-foreground); letter-spacing: 0.02em;
}
.context-item {
  display: flex; align-items: center; gap: 8px;
  width: 100%; padding: 6px 10px; border: none; border-radius: 6px;
  background: transparent; color: var(--color-sidebar-foreground);
  opacity: 0.72; font-size: 13px; text-align: left; cursor: pointer;
  transition: background-color 0.12s ease, opacity 0.12s ease;
}
.context-item:hover { background-color: var(--color-sidebar-accent); opacity: 1; }
.context-item.active { background-color: var(--color-sidebar-accent); opacity: 1; font-weight: 500; }
.context-icon { width: 15px; height: 15px; flex-shrink: 0; }
```

保留 logo 区、status-bar、footer 不动。`.sidebar` 容器样式更新为：`width: auto; min-width: 52px;` 由子栏撑开。

- [ ] **Step 4: 编译验证**

Run: `pnpm check`
Expected: 构建通过。

- [ ] **Step 5: 手动验证**

Run: `pnpm tauri dev`
验证：① 点击各主模块图标导航正常；② 点击 SSH 图标 → 右侧出现 SSH 上下文面板（连接/终端/文件）；③ 切到 `/ssh/sessions` 时图标轨 SSH 保持高亮、面板"终端"子项高亮；④ 点击无上下文的模块（如概览）→ 面板收起。

- [ ] **Step 6: 提交**

```bash
git add src/components/Sidebar.vue
git commit -m "feat(nav): 侧边栏双栏图标轨 + 上下文面板"
```

---

### Task 9: SSH 连接管理视图（`SSHConnections.vue`）

**Files:**
- Create: `src/views/SSHConnections.vue`
- Modify: `src/lib/api-ssh.js`（新建，封装 invoke）

**Interfaces:**
- Produces:
  - `src/lib/api-ssh.js`：`export async function listConnections()`、`saveConnection(input)`、`deleteConnection(id)`、`testConnection(id)`、`openTerminal(connectionId)`、`listenTerminalOutput(cb)`、`sendTerminalInput(sessionId, b64)`、`resizeTerminal(sessionId, cols, rows)`、`closeTerminal(sessionId)`
  - `SSHConnections.vue`：连接列表 + 新建/编辑表单（Dialog）+ 测试 + 打开终端跳转 `/ssh/sessions?open=<id>`
- Consumes: Task 6 的后端命令、Task 7 的路由。

- [ ] **Step 1: 建 `api-ssh.js`**

```js
// src/lib/api-ssh.js — SSH 后端命令封装
import { invoke } from "@tauri-apps/api/core";

export const listConnections = () => invoke("ssh_list_connections");
export const saveConnection = (connection) => invoke("ssh_save_connection", { connection });
export const deleteConnection = (id) => invoke("ssh_delete_connection", { id });
export const testConnection = (id) => invoke("ssh_test_connection", { connectionId: id });

export const openTerminal = (connectionId, cols, rows) =>
  invoke("ssh_terminal_open", { connectionId, cols, rows });
export const sendTerminalInput = (sessionId, data) =>
  invoke("ssh_terminal_input", { sessionId, data });
export const resizeTerminal = (sessionId, cols, rows) =>
  invoke("ssh_terminal_resize", { sessionId, cols, rows });
export const closeTerminal = (sessionId) =>
  invoke("ssh_terminal_close", { sessionId });
```

- [ ] **Step 2: 实现连接管理视图**

`SSHConnections.vue`：列表 + 对话框表单 + 操作。参考 `ContainerManager.vue` 的布局与 `ConfirmDialog`/`toast` 用法（`src/lib/toast.js` 提供 `toast.success/error`）。核心：

```vue
<script setup>
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { listConnections, saveConnection, deleteConnection, testConnection } from "../lib/api-ssh.js";
import { t } from "../lib/i18n.js";
import AppIcon from "../components/AppIcon.vue";
import { toast } from "../lib/toast.js";

const router = useRouter();
const conns = ref([]);
const loading = ref(true);
const editing = ref(null);   // 正在编辑的连接（null=关闭对话框）
const form = ref({ id: null, name: "", host: "", port: 22, username: "", auth_type: "password", secret: "", key_passphrase: "" });

async function refresh() {
  loading.value = true;
  try { conns.value = await listConnections(); } finally { loading.value = false; }
}
onMounted(refresh);

function openCreate() { form.value = { id: null, name: "", host: "", port: 22, username: "", auth_type: "password", secret: "", key_passphrase: "" }; editing.value = form.value; }
function openEdit(c) { form.value = { id: c.id, name: c.name, host: c.host, port: c.port, username: c.username, auth_type: c.auth_type, secret: "", key_passphrase: "" }; editing.value = form.value; }
function closeEdit() { editing.value = null; }

async function onSave() {
  if (!form.value.name || !form.value.host || !form.value.username || !form.value.secret) {
    toast.error(t("ssh.name") + " / " + t("ssh.host") + " / " + t("ssh.username") + " / " + t("ssh.password"));
    return;
  }
  try {
    await saveConnection({ ...form.value });
    toast.success("Saved");
    closeEdit(); refresh();
  } catch (e) { toast.error(String(e)); }
}

async function onDelete(id) { try { await deleteConnection(id); refresh(); } catch (e) { toast.error(String(e)); } }
async function onTest(id) { try { await testConnection(id); toast.success("OK"); } catch (e) { toast.error(String(e)); } }
function openTerminal(id) { router.push({ path: "/ssh/sessions", query: { open: id } }); }
</script>

<template>
  <div class="page">
    <div class="page-header">
      <h1>{{ t("nav.ssh") }}</h1>
      <button class="btn-primary" @click="openCreate">
        <AppIcon name="plus" /> {{ t("ssh.add") }}
      </button>
    </div>

    <div v-if="loading" class="muted">{{ t("common.loading") }}</div>
    <div v-else-if="conns.length === 0" class="empty">
      {{ t("ssh.empty") }}<br/><span class="muted">{{ t("ssh.add_hint") }}</span>
    </div>

    <div class="conn-grid">
      <div v-for="c in conns" :key="c.id" class="conn-card">
        <div class="conn-title">
          <AppIcon name="server" class="conn-icon" />
          <div>
            <div class="conn-name">{{ c.name }}</div>
            <div class="conn-sub">{{ c.username }}@{{ c.host }}:{{ c.port }}</div>
          </div>
        </div>
        <div class="conn-actions">
          <button class="btn" @click="openTerminal(c.id)">
            <AppIcon name="terminal" /> {{ t("ssh.open_terminal") }}
          </button>
          <button class="btn" @click="onTest(c.id)">
            <AppIcon name="check" /> {{ t("ssh.test") }}
          </button>
          <button class="btn" @click="openEdit(c)">
            <AppIcon name="edit" /> {{ t("ssh.edit") }}
          </button>
          <button class="btn btn-danger" @click="onDelete(c.id)">
            <AppIcon name="delete" />
          </button>
        </div>
      </div>
    </div>

    <!-- 新建/编辑对话框（复用 reka-ui Dialog 模式） -->
    <div v-if="editing" class="modal-mask" @click.self="closeEdit">
      <div class="modal">
        <h2>{{ editing.id ? t("ssh.edit") : t("ssh.add") }}</h2>
        <label>{{ t("ssh.name") }}<input v-model="form.name" /></label>
        <label>{{ t("ssh.host") }}<input v-model="form.host" /></label>
        <label>{{ t("ssh.port") }}<input v-model.number="form.port" type="number" /></label>
        <label>{{ t("ssh.username") }}<input v-model="form.username" /></label>
        <label>
          {{ t("ssh.auth_type") }}
          <select v-model="form.auth_type">
            <option value="password">{{ t("ssh.password") }}</option>
            <option value="private_key">{{ t("ssh.private_key") }}</option>
          </select>
        </label>
        <label v-if="form.auth_type === 'password'">{{ t("ssh.password") }}
          <input v-model="form.secret" type="password" :placeholder="form.id ? '••••••' : ''" /></label>
        <label v-else>{{ t("ssh.private_key") }}
          <textarea v-model="form.secret" :placeholder="'-----BEGIN OPENSSH PRIVATE KEY-----'" /></label>
        <label v-if="form.auth_type === 'private_key'">{{ t("ssh.key_passphrase") }}
          <input v-model="form.key_passphrase" type="password" /></label>
        <div class="modal-actions">
          <button class="btn" @click="closeEdit">{{ t("common.cancel") }}</button>
          <button class="btn-primary" @click="onSave">{{ t("common.save") }}</button>
        </div>
      </div>
    </div>
  </div>
</template>
```

配套 `.conn-grid`/`.conn-card`/`.modal` 等 scoped 样式（与 `ContainerManager.vue` 的卡片风格一致）。

- [ ] **Step 3: 编译验证**

Run: `pnpm check` → 构建通过。

- [ ] **Step 4: 手动验证**

Run: `pnpm tauri dev` → 连接页新增/编辑/删除/测试；测试不存在的 host 显示错误 toast。

- [ ] **Step 5: 提交**

```bash
git add src/views/SSHConnections.vue src/lib/api-ssh.js
git commit -m "feat(ssh): 连接管理视图（列表/增删改/测试）"
```

---

### Task 10: SSH 终端视图（`SSHTerminal.vue`）

**Files:**
- Create: `src/views/SSHTerminal.vue`
- Modify: `src/lib/api-ssh.js`

**Interfaces:**
- Produces: 多标签终端页（xterm.js）。每标签一个 `{ sessionId, connectionName, terminal, fitAddon }`。监听 `ssh-terminal-output`/`ssh-terminal-closed`/`ssh-hostkey-prompt`。
- Consumes: Task 6 命令、Task 9 的 `api-ssh.js`。

- [ ] **Step 1: `api-ssh.js` 补终端/事件封装**

```js
import { listen } from "@tauri-apps/api/event";

export const onTerminalOutput = (cb) => listen("ssh-terminal-output", (ev) => cb(ev.payload));
export const onTerminalClosed = (cb) => listen("ssh-terminal-closed", (ev) => cb(ev.payload));
export const onHostkeyPrompt = (cb) => listen("ssh-hostkey-prompt", (ev) => cb(ev.payload));
export const acceptHostkey = (sessionId, host, fingerprint) =>
  invoke("ssh_hostkey_accept", { sessionId, host, fingerprint });
export const rejectHostkey = (sessionId) => invoke("ssh_hostkey_reject", { sessionId });
export const listSftp = (sftpId, path) => invoke("ssh_sftp_list_dir", { sftpId, path });
```

- [ ] **Step 2: 实现终端视图**

`SSHTerminal.vue` 核心逻辑：

```vue
<script setup>
import { ref, onMounted, onBeforeUnmount, nextTick } from "vue";
import { useRoute } from "vue-router";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { listConnections, openTerminal, sendTerminalInput, resizeTerminal, closeTerminal,
         onTerminalOutput, onTerminalClosed, onHostkeyPrompt, acceptHostkey, rejectHostkey } from "../lib/api-ssh.js";
import { t } from "../lib/i18n.js";
import AppIcon from "../components/AppIcon.vue";

const route = useRoute();
const tabs = ref([]);       // { sessionId, connectionName, term, fit, el }
const activeId = ref(null);
const conns = ref([]);
let unlisten = [];

function decode(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

async function mountTerminal(sessionId, name, el) {
  const term = new Terminal({ cursorBlink: true, fontFamily: '"JetBrains Mono", monospace', fontSize: 13, theme: { background: "transparent" } });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(el);
  term.onData((d) => sendTerminalInput(sessionId, btoa(unescape(encodeURIComponent(d)))).catch(() => {}));
  term.onResize(({ cols, rows }) => resizeTerminal(sessionId, cols, rows).catch(() => {}));
  tabs.value.push({ sessionId, connectionName: name, term, fit, el });
  activeId.value = sessionId;
  await nextTick();
  fit.fit();
  return term;
}

async function openTab(connectionId) {
  const c = conns.value.find((x) => x.id === connectionId);
  if (!c) return;
  const sessionId = await openTerminal(connectionId, 80, 24);
  const container = document.createElement("div");
  container.className = "term-container";
  const holder = document.getElementById("term-holder");
  holder.appendChild(container);
  await mountTerminal(sessionId, c.name, container);
}

async function init() {
  conns.value = await listConnections();
  const toOpen = route.query.open;
  if (toOpen) await openTab(toOpen);
}

onMounted(async () => {
  unlisten = [
    await onTerminalOutput(({ session_id, data }) => {
      const tab = tabs.value.find((x) => x.sessionId === session_id);
      tab?.term.write(decode(data));
    }),
    await onTerminalClosed(({ session_id }) => {
      const tab = tabs.value.find((x) => x.sessionId === session_id);
      if (tab) { tab.term.writeln("\r\n\x1b[31m[disconnected]\x1b[0m"); }
    }),
    await onHostkeyPrompt(async ({ session_id, host, fingerprint }) => {
      // 简单 confirm；后续可换 ConfirmDialog
      if (window.confirm(`${t("ssh.hostkey_body")}\n\n${host}\n${fingerprint}`)) {
        await acceptHostkey(session_id, host, fingerprint);
      } else {
        await rejectHostkey(session_id);
      }
    }),
  ];
  await init();
});

onBeforeUnmount(async () => {
  for (const fn of unlisten) fn();
  for (const tab of tabs.value) closeTerminal(tab.sessionId).catch(() => {});
});
</script>

<template>
  <div class="page page-terminal">
    <div class="tabbar" v-if="tabs.length">
      <button v-for="tab in tabs" :key="tab.sessionId" class="tab"
              :class="{ active: tab.sessionId === activeId }"
              @click="activeId = tab.sessionId; tab.fit.fit();">
        <AppIcon name="terminal" class="tab-icon" />
        {{ tab.connectionName }}
        <span class="tab-close" @click.stop="closeTerminal(tab.sessionId); tabs.splice(tabs.indexOf(tab), 1); tab.term.dispose(); tab.el.remove(); activeId = tabs[tabs.length-1]?.sessionId ?? null;">
          <AppIcon name="close" />
        </span>
      </button>
    </div>
    <div id="term-holder" class="term-holder"></div>
  </div>
</template>
```

样式：`.tabbar`（水平滚动、圆角 tab）、`.term-holder { flex: 1; min-height: 0; padding: 4px; }`、`.term-container { height: 100%; }`；`.page-terminal { display: flex; flex-direction: column; height: 100%; }`。多标签切换时用 `v-show` 控制 `display`（xterm 在 display:none 时会丢布局，fit 时重新 fit）。实现中为每个 tab 挂到 `#term-holder`，切换时设置 `container.style.display = tab.sessionId === activeId ? 'block' : 'none'` 并 `fit.fit()`。

- [ ] **Step 3: 编译验证**

Run: `pnpm check` → 通过。

- [ ] **Step 4: 手动验证**

Run: `pnpm tauri dev` → 从连接页点"打开终端"跳转并连上；输入命令有响应；调整窗口大小终端自适应；断线显示 disconnected；首次连接弹出指纹确认。

- [ ] **Step 5: 提交**

```bash
git add src/views/SSHTerminal.vue src/lib/api-ssh.js
git commit -m "feat(ssh): 多标签 xterm 终端视图与事件流"
```

---

### Task 11: SSH SFTP 视图（`SSHSftp.vue`）

**Files:**
- Create: `src/views/SSHSftp.vue`
- Modify: `src/lib/api-ssh.js`

**Interfaces:**
- Produces: 双栏 SFTP 浏览器 + 拖拽上传/下载 + 进度条。
- Consumes: Task 6 命令（`ssh_sftp_*`）、Task 9 `api-ssh.js`。

- [ ] **Step 1: `api-ssh.js` 补 SFTP 封装**

```js
export const openSftp = (connectionId) => invoke("ssh_sftp_open", { connectionId });
export const readSftpFile = (sftpId, path, offset, length) => invoke("ssh_sftp_read_file", { sftpId, path, offset, length });
export const writeSftpFile = (sftpId, path, data, offset) => invoke("ssh_sftp_write_file", { sftpId, path, data, offset });
export const mkdirSftp = (sftpId, path) => invoke("ssh_sftp_mkdir", { sftpId, path });
export const renameSftp = (sftpId, from, to) => invoke("ssh_sftp_rename", { sftpId, from, to });
export const deleteSftp = (sftpId, path, isDir) => invoke("ssh_sftp_delete", { sftpId, path, isDir });
export const statSftp = (sftpId, path) => invoke("ssh_sftp_stat", { sftpId, path });
```

- [ ] **Step 2: 实现 SFTP 视图**

`SSHSftp.vue` 核心：

```vue
<script setup>
import { ref, onMounted } from "vue";
import { listConnections, openSftp, listSftp, readSftpFile, writeSftpFile, mkdirSftp, renameSftp, deleteSftp } from "../lib/api-ssh.js";
import { t } from "../lib/i18n.js";
import AppIcon from "../components/AppIcon.vue";
import { toast } from "../lib/toast.js";

const conns = ref([]);
const connId = ref("");
const sftpId = ref(null);
const cwd = ref("/");
const entries = ref([]);
const path = ref([]);           // 面包屑
const transfer = ref(null);     // { kind:'up'|'down', name, done, total }

async function connect() {
  if (!connId.value) return;
  try {
    sftpId.value = await openSftp(connId.value);
    await cd("/");
  } catch (e) { toast.error(String(e)); }
}

async function cd(p) {
  cwd.value = p;
  path.value = p.split("/").filter(Boolean);
  entries.value = await listSftp(sftpId.value, p);
}

function enter(entry) { if (entry.is_dir) cd(join(cwd.value, entry.name)); }
function up() { cd(cwd.value === "/" ? "/" : cwd.value.slice(0, cwd.value.lastIndexOf("/")) || "/"); }
function join(base, name) { return (base === "/" ? "" : base) + "/" + name; }

const CHUNK = 256 * 1024;
async function download(entry) {
  const local = await saveFilePicker(entry.name);   // 用 @tauri-apps/plugin-dialog 的 save 对话框
  if (!local) return;
  const total = entry.size;
  let offset = 0;
  transfer.value = { kind: "down", name: entry.name, done: 0, total };
  while (offset < total) {
    const b64 = await readSftpFile(sftpId.value, join(cwd.value, entry.name), offset, CHUNK);
    const bytes = decode(b64);
    await writeLocal(local, bytes, offset);
    offset += bytes.length;
    transfer.value.done = offset;
    if (bytes.length === 0) break;
  }
  transfer.value = null;
}

async function upload(file) {
  const data = await file.arrayBuffer();
  const total = data.byteLength;
  transfer.value = { kind: "up", name: file.name, done: 0, total };
  let offset = 0;
  while (offset < total) {
    const chunk = new Uint8Array(data.slice(offset, offset + CHUNK));
    await writeSftpFile(sftpId.value, join(cwd.value, file.name), btoa(String.fromCharCode(...chunk)), offset);
    offset += chunk.length;
    transfer.value.done = offset;
  }
  transfer.value = null;
  await cd(cwd.value);
}

function onDrop(e) {
  const files = [...(e.dataTransfer?.files || [])];
  files.forEach(upload);
}
</script>
```

模板：顶部连接选择 + 连接按钮；面包屑 + 上级目录 + 新建文件夹 + 刷新；文件表格（名称/大小/修改时间 + 下载/重命名/删除按钮）；底部进度条（`transfer` 时显示百分比）；整个文件区 `@dragover.prevent @drop.prevent="onDrop"` 支持拖拽上传。下载用 `@tauri-apps/plugin-dialog` 的 `save` 弹出保存路径，分块写本地文件（`fs` 用 `@tauri-apps/plugin-fs` 或 Node 侧 `Web File System Access`——**优先用 `@tauri-apps/plugin-fs` 的 `writeFile`/`create`**，需确认该插件在 `package.json`；若未安装则 `pnpm add @tauri-apps/plugin-fs` 并在 `lib.rs` 注册 `tauri_plugin_fs::init()`）。

`decode`/`btoa` 辅助函数与 Task 10 相同。

- [ ] **Step 3: 编译验证**

Run: `pnpm check` → 通过。

- [ ] **Step 4: 手动验证**

Run: `pnpm tauri dev` → 连接后浏览目录、进入/返回、上传拖拽、下载到本地、新建文件夹、重命名、删除。

- [ ] **Step 5: 提交**

```bash
git add src/views/SSHSftp.vue src/lib/api-ssh.js
git commit -m "feat(ssh): SFTP 双栏文件浏览器与拖拽传输"
```

---

### Task 12: 全量门禁与收尾

**Files:**
- Modify: 依检查结果修正。

**Interfaces:**
- Consumes: Task 1–11。

- [ ] **Step 1: 全量测试**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
pnpm check
cargo clippy --all-targets -- -D warnings
cargo fmt --check --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 2: 手动集成验证清单**

- [ ] 连接页：新建/编辑/删除/测试连接
- [ ] 终端：多标签打开、输入命令、窗口缩放、关闭、断线提示、host key 首连确认
- [ ] SFTP：浏览/上传（拖拽）/下载/新建文件夹/重命名/删除
- [ ] 侧边栏：图标轨点击 SSH 展开上下文面板；无上下文模块收起面板；子项高亮跟随路由

- [ ] **Step 3: 更新文档**

`README.md` 特性列表加 SSH；`docs/modules/` 新增 `14-ssh.md`（简述架构与命令清单）；`CHANGELOG.md` 记一条。

- [ ] **Step 4: 提交**

```bash
git add README.md docs CHANGELOG.md
git commit -m "docs: SSH 模块文档与变更日志"
```

---

## Self-Review 记录

- **Spec 覆盖**：spec 第 4 节（connections/session/terminal/sftp）→ Task 2–5；spec 第 5.1（nav-config + Sidebar 双栏）→ Task 7–8；spec 第 5.2（三个视图）→ Task 9–11；spec 第 6（测试）→ Task 1–3 的 Rust 单测 + Task 12 手动清单；spec 第 8（依赖）→ Task 2/7。`ssh_hostkey_accept/reject` 命令（spec 4.2）→ Task 3。
- **类型一致性**：`sessionId`/`session_id` 前端 snake_case、Rust JSON 用 snake_case payload，`api-ssh.js` 与后端命令参数名对齐（`connectionId`、`sessionId`、`sftpId`、`data`、`path`、`offset`、`length`、`cols`、`rows`）。Task 4 中 `ssh_terminal_open` 返回 terminal_id 同时复用连接 session——语义为"终端标签 id"，前端以此区分 tab。
- **占位符检查**：`with_sftp` 闭包在 Task 5 明确标注"内联实现、可删除"，非遗留占位。