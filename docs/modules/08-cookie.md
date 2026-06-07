# Cookie 提取器 — 模块设计文档

## 1. 功能概述

从浏览器导出 Cookie 为文本格式（Netscape Cookie File / cURL 格式），便于在其他命令行工具（如 `curl`）中使用 Cookie 进行认证请求。

**通信链路**:
```
CookieViewer.svelte ──→ invoke("list_cookies")      ──→ cookie_manager.rs
                   ──→ invoke("export_cookies")     ──→ cookie_manager.rs
                   ──→ invoke("list_browsers")      ──→ cookie_manager.rs
```

---

## 2. 数据结构

```rust
#[derive(Serialize, Debug, Clone)]
pub struct CookieInfo {
    pub domain: String,         // ".github.com"
    pub name: String,           // "session_token"
    pub value: String,          // 屏蔽显示为 "••••••"
    pub path: String,           // "/"
    pub expires: String,        // "2025-12-31T23:59:59Z"
    pub secure: bool,
    pub httponly: bool,
}

#[derive(Serialize, Debug)]
pub struct BrowserInfo {
    pub name: String,           // "Google Chrome"
    pub cookie_count: usize,    // 可读取的 cookie 数量
    pub status: String,         // "Available" / "Locked" / "Not Found"
}

/// 导出格式
#[derive(Serialize, Deserialize)]
pub enum CookieExportFormat {
    Netscape,  // Netscape cookie file 格式
    Curl,      // cURL 参数格式
    Json,      // JSON 格式
}
```

**前端对应** (`routes/CookieViewer.svelte`):

```javascript
let browsers = $state([]);
let selectedBrowser = $state(null);
let cookies = $state([]);
let searchQuery = $state("");
let exportFormat = $state("Netscape");
```

---

## 3. 核心实现

### 3.1 支持的浏览器

不同平台基于浏览器 Cookie 数据库的存储位置进行检测:

| 浏览器 | macOS | Linux | Windows |
|--------|-------|-------|---------|
| Chrome | `~/Library/Application Support/Google/Chrome/Default/Cookies` | `~/.config/google-chrome/Default/Cookies` | `%LOCALAPPDATA%\Google\Chrome\User Data\Default\Cookies` |
| Chromium | `~/Library/Application Support/Chromium/Default/Cookies` | `~/.config/chromium/Default/Cookies` | `%LOCALAPPDATA%\Chromium\User Data\Default\Cookies` |
| Edge | `~/Library/Application Support/Microsoft Edge/Default/Cookies` | `~/.config/microsoft-edge/Default/Cookies` | `%LOCALAPPDATA%\Microsoft\Edge\User Data\Default\Cookies` |
| Firefox | `~/Library/Application Support/Firefox/Profiles/*.default/cookies.sqlite` | `~/.mozilla/firefox/*.default/cookies.sqlite` | `%APPDATA%\Mozilla\Firefox\Profiles\*.default\cookies.sqlite` |
| Brave | `~/Library/Application Support/BraveSoftware/Brave-Browser/Default/Cookies` | `~/.config/BraveSoftware/Brave-Browser/Default/Cookies` | `%LOCALAPPDATA%\BraveSoftware\Brave-Browser\User Data\Default\Cookies` |
| Safari | `~/Library/Cookies/Cookies.binarycookies` | ❌ | ❌ |

### 3.2 浏览器检测

```rust
pub fn list_browsers() -> Vec<BrowserInfo> {
    let mut browsers = Vec::new();
    let profiles = detect_browser_profiles();

    for (name, path) in profiles {
        if path.exists() {
            // 尝试打开数据库获取 cookie 数量
            let count = get_cookie_count(&path);
            browsers.push(BrowserInfo {
                name: name.to_string(),
                cookie_count: count,
                status: if count > 0 { "Available".into() } else { "Empty".into() },
            });
        } else {
            browsers.push(BrowserInfo {
                name: name.to_string(),
                cookie_count: 0,
                status: "Not Found".into(),
            });
        }
    }

    browsers
}
```

### 3.3 Cookie 数据库读取

Chromium 系浏览器使用 SQLite 存储 cookie，Firefox 也是如此。Safari 使用二进制 plist 格式。

**Chromium/Chrome/Edge/Brave**:

```rust
fn read_chromium_cookies(path: &Path) -> Result<Vec<CookieInfo>, String> {
    // Chromium 的 cookie 数据库是 SQLite 文件
    let conn = Connection::open(path)
        .map_err(|e| format!("Failed to open cookie DB: {}", e))?;

    // 查询未过期的 cookie
    let mut stmt = conn.prepare(
        "SELECT host_key, name, value, path, expires_utc, is_secure, is_httponly
         FROM cookies
         WHERE expires_utc > ?1
         ORDER BY host_key, name"
    ).map_err(|e| format!("Query failed: {}", e))?;

    // Chromium 使用 WebKit 时间戳（1601-01-01 以来的微秒数）
    let now_webkit = (SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH).unwrap().as_micros() as i64)
        + 11644473600000000i64;  // 1601-01-01 → 1970-01-01 差值

    let cookies = stmt.query_map(params![now_webkit], |row| {
        // 解码加密的 value 字段
        let encrypted_value: Vec<u8> = row.get(2)?;
        let value = decrypt_chromium_value(&encrypted_value)
            .unwrap_or_else(|_| "[encrypted]".to_string());

        Ok(CookieInfo {
            domain: row.get(0)?,
            name: row.get(1)?,
            value,
            path: row.get(3)?,
            expires: webkit_to_rfc3339(row.get(4)?),
            secure: row.get(5)?,
            httponly: row.get(6)?,
        })
    });

    Ok(cookies.filter_map(|c| c.ok()).collect())
}
```

**Chromium cookie 值加密**:

Chromium 在 Linux 上使用 AES 加密存储 cookie 值（通过 libsecret），macOS 使用 Keychain，Windows 使用 DPAPI。

```rust
fn decrypt_chromium_value(encrypted: &[u8]) -> Result<String, String> {
    // Chrome 版本 >= 80 使用 AES-256-GCM
    // 加密格式: "v10" (2 bytes) + nonce (12 bytes) + ciphertext + tag (16 bytes)
    if encrypted.len() < 15 || &encrypted[0..3] != b"v10" {
        return Err("unsupported format".to_string());
    }

    let nonce = &encrypted[3..15];
    let ciphertext = &encrypted[15..];

    // 获取系统密钥（不同平台获取方式不同）
    let key = get_chromium_encryption_key()?;
    // ...
}
```

**获取系统密钥**:

```rust
fn get_chromium_encryption_key() -> Result<Vec<u8>, String> {
    // macOS: 从 Keychain 读取 "Chrome Safe Storage" 条目
    #[cfg(target_os = "macos")] {
        // Security framework 访问钥匙串
        let output = Command::new("security")
            .args(["find-generic-password", "-w", "-s", "Chrome"])
            .output()?;
        // ...
    }

    // Linux: 从密钥环服务读取
    #[cfg(target_os = "linux")] {
        // 读取 ~/.config/google-chrome/Local State 中的 encrypted_key
        let local_state_path = get_chrome_local_state()?;
        let local_state: serde_json::Value = serde_json::from_str(&content)?;
        let encrypted_key = local_state["os_crypt"]["encrypted_key"]
            .as_str().ok_or("no key")?;
        // base64 解码 → 去除 "DPAPI" 前缀 → 解密
        // ...
    }

    // Windows: DPAPI 解密
    #[cfg(target_os = "windows")] {
        // 使用 Win32 CryptUnprotectData
    }
}
```

**Firefox**:

```rust
fn read_firefox_cookies(path: &Path) -> Result<Vec<CookieInfo>, String> {
    // Firefox cookie 数据库也是 SQLite，且值不加密（未加密存储）
    let conn = Connection::open(path)
        .map_err(|e| format!("Failed to open Firefox cookie DB: {}", e))?;

    let mut stmt = conn.prepare(
        "SELECT host, name, value, path, expiry, isSecure, isHttpOnly
         FROM moz_cookies
         WHERE expiry > strftime('%s', 'now')
         ORDER BY host, name"
    ).map_err(|e| e.to_string())?;
}
```

**Safari**:

```rust
fn read_safari_cookies(path: &Path) -> Result<Vec<CookieInfo>, String> {
    // Safari 使用 binary cookies 格式（二进制 plist）
    // 需要解析 BinaryCookie 文件格式：
    // 页眉 (4 bytes) + 页面 (多个 page)
    // 每个 page 包含: 元数据 + cookie 记录（变长）
    let data = std::fs::read(path)?;
    // BinaryCookie 文件格式解析...
}
```

---

## 4. Cookie 导出

```rust
pub fn export_cookies(
    browser_key: String,
    format: CookieExportFormat,
    domain_filter: Option<String>,
) -> Result<String, String> {
    let cookies = read_cookies_from_browser(&browser_key)?;

    // 域名过滤
    let cookies: Vec<&CookieInfo> = if let Some(ref filter) = domain_filter {
        cookies.iter().filter(|c| c.domain.contains(filter)).collect()
    } else {
        cookies.iter().collect()
    };

    match format {
        CookieExportFormat::Netscape => {
            // Netscape cookie file 格式
            let mut output = String::from("# Netscape HTTP Cookie File\n");
            for cookie in &cookies {
                let include_sub = if cookie.domain.starts_with('.') { "TRUE" } else { "FALSE" };
                let secure = if cookie.secure { "TRUE" } else { "FALSE" };
                output.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    cookie.domain, include_sub, cookie.path,
                    secure, expiry_timestamp, cookie.name, cookie.value
                ));
            }
            Ok(output)
        }
        CookieExportFormat::Curl => {
            // cURL -H "Cookie: ..." 参数格式
            let header: Vec<String> = cookies.iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect();
            Ok(header.join("; "))
        }
        CookieExportFormat::Json => {
            serde_json::to_string_pretty(&cookies)
                .map_err(|e| e.to_string())
        }
    }
}
```

---

## 5. 前端实现

### 5.1 浏览器选择

```html
<div class="grid grid-cols-3 gap-4">
  {#each browsers as browser}
    <button onclick={() => selectBrowser(browser)}>
      <BrowserIcon name={browser.name} />
      <span>{browser.name}</span>
      <span class="text-nx-text-muted">{browser.cookie_count} cookies</span>
    </button>
  {/each}
</div>
```

### 5.2 Cookie 表格

```html
<table>
  <thead>
    <tr><th>Domain</th><th>Name</th><th>Value</th><th>Expires</th></tr>
  </thead>
  <tbody>
    {#each filteredCookies as cookie}
      <tr>
        <td class="font-mono text-xs">{cookie.domain}</td>
        <td>{cookie.name}</td>
        <td>
          <span class="obscured">{obscureValue(cookie.value)}</span>
          <button onclick={() => copyToClipboard(cookie.value)}>Copy</button>
        </td>
        <td>{cookie.expires}</td>
      </tr>
    {/each}
  </tbody>
</table>
```

### 5.3 导出操作

```javascript
async function handleExport() {
    const content = await invoke("export_cookies", {
        browserKey: selectedBrowser.name,
        format: exportFormat,
        domainFilter: domainFilter || null,
    });
    // 下载导出结果
    const blob = new Blob([content], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `cookies_${selectedBrowser.name}.txt`;
    a.click();
}
```

---

## 6. 日志和审计

```rust
fn log_cookie_export(browser: &str, count: usize) {
    // 记录导出事件到日志文件
    log::info!("Exported {} cookies from {}", count, browser);
}
```

---

## 7. 跨平台注意事项

| 浏览器 | macOS | Linux | Windows |
|--------|-------|-------|---------|
| Chrome 解密 | Keychain | libsecret/AES | DPAPI |
| Firefox 解密 | 无加密 | 无加密 | 无加密 |
| Safari | BinaryCookie | ❌ | ❌ |
| Edge | Keychain | libsecret | DPAPI |

Chrome 和 Chromium 系的 cookie 值加密是最大的跨平台挑战：每个平台使用不同的系统级密钥存储机制（macOS Keychain、Linux libsecret、Windows DPAPI），每个系统需要独立的解密实现。

---

## 8. 关键设计决策

1. **Firefox 不加密 cookie 值**: 因此优先推荐用户选择 Firefox 作为导出来源

2. **Netscape 格式优先**: 这是 `curl` 原生支持的 Cookie 文件格式，也是开发者最常用的场景

3. **域名筛选**: 用户可能只需要某个特定域名的 cookie（如 `api.github.com`），避免将所有 cookie 暴露在导出文件中

4. **解密失败处理**: 如果某个 cookie 解密失败，使用 `[encrypted]` 占位而非中止整个导出流程

5. **Safari 的有限支持**: Safari 仅 macOS 可用，且 BinaryCookie 格式解析较脆弱，导出前会显示警告
