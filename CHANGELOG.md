# Changelog

All notable changes to the DevNexus project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

- Cloud service credential management (AWS / GCP CLI) (planned)

---

## [1.3.12] - 2026-08-16

### Added (English)
- SSH module: encrypted connection store, multi-tab xterm terminal, SFTP browser with chunked transfer
- SSH ProxyJump (jump host), local port forwarding (-L), dynamic SOCKS5 proxy (-D), SSH agent forwarding
- SSH AI assistant for terminal and SFTP, reusing API Hub LLM providers
- OpenSSH config import/export; connection groups, tags, keepalive; key file import
- Generic sidebar context-panel navigation (icon rail + sub-items)

### 新增（中文）
- SSH 模块：加密连接存储、多标签 xterm 终端、SFTP 分块传输文件浏览器
- SSH 跳板机（ProxyJump）、本地端口转发（-L）、动态 SOCKS5 代理（-D）、Agent 转发
- 终端/SFTP AI 助手（复用 API Hub 的 LLM Provider 配置）
- OpenSSH config 导入/导出；连接分组、标签、Keepalive；私钥文件导入
- 侧边栏通用导航上下文机制（图标轨 + 子面板）

---

## [1.3.10] - 2026-08-13

### Added (English)
- Dynamic Island optimizations: HUD display, system status, auto-collapse, notification isolation
- Uninstaller list deduplication (duplicate entries removed)

### Fixed (English)
- Added cfg gates for Linux-only software-list helpers, fixing clippy failures on Windows/macOS CI
- Removed the Linux gate on `is_snap_runtime_package`, fixing non-Linux compile errors

### 新增（中文）
- 灵动岛优化：HUD 显示、系统状态、自动收起、通知隔离
- 卸载列表去重（重复条目自动合并）

### 修复（中文）
- 为 Linux-only 软件列表辅助函数补 cfg 门控，修复 Windows/macOS CI 的 clippy 失败
- 移除 `is_snap_runtime_package` 的 Linux 门控，修复非 Linux 平台编译错误

---

## [1.3.9] - 2026-08-12

### Changed (English)
- AppIcon now imports lucide icons on demand: chunk size reduced from ~672 KB to ~50 KB
- Uninstaller category icons migrated to the new icon system
- Gateway "Auth Token enabled" badge now uses i18n
- Fixed a large batch of frontend TypeScript errors (typed container / API Hub props, UI wrappers, router, event handlers)

### Fixed (English)
- Dynamic island banner close button now only closes the banner (no longer opens the main window)
- Opening the main window from the island restores taskbar / Alt-Tab visibility
- Fixed clippy / rustfmt warnings

### 变更（中文）
- AppIcon 按需导入 lucide 图标：打包体积从约 672KB 降至约 50KB
- 卸载器分类图标迁移到新图标系统
- 网关 "Auth Token 已启用" 徽标文案接入 i18n
- 修复一批前端 TypeScript 类型错误（容器 / API Hub 组件 props 类型标注、UI 包装组件、路由、事件处理）

### 修复（中文）
- 灵动岛横幅关闭按钮只关闭横幅（不再顺带打开主窗口）
- 从灵动岛打开主窗口时恢复任务栏 / Alt-Tab 显示
- 修复 clippy / rustfmt 告警

---

## [1.3.8] - 2026-08-10

### Added (English)
- Tray menu DeepSeek balance auto-refresh: queries on startup and every 5 minutes (no click needed)
- Sidebar island toggle icon: click to turn the Dynamic Island on/off (highlighted when on)
- Tray menu island item is now a checkbox toggle (check = on, uncheck = off)

### Fixed (English)
- Tray balance item showed a placeholder dash until clicked; now auto-fills

### 新增（中文）
- 托盘菜单 DeepSeek 余额自动刷新：启动即查询、每 5 分钟更新（无需点击）
- 侧边栏新增灵动岛开关图标：点击直接开启/关闭灵动岛（开启时高亮）
- 托盘菜单"灵动岛"改为勾选开关（勾选=开，取消=关）

### 修复（中文）
- 修复托盘余额项需点击才显示（现在自动查询填充）

---

## [1.3.7] - 2026-08-10

### Added (English)
- Dynamic Island stays visible on every workspace by following the current workspace (mutter/Wayland does not honor STICKY for XWayland windows)

### Fixed (English)
- Dynamic Island auto-switched to the media player module when audio started; now keeps showing the clock by default (scroll to media module manually)
- Workspace following: island window moved to the current desktop on workspace switch (reliable on GNOME Wayland + XWayland)

### 新增（中文）
- 灵动岛跨工作区常驻：改为跟随当前工作区（mutter/Wayland 下 XWayland 窗口不认 STICKY）

### 修复（中文）
- 修复媒体播放时灵动岛自动切到播放器模块；现在默认保持时间显示（可滚轮手动切到媒体）
- 修复切换工作区后岛窗口不跟随（工作区切换时把岛窗口移动到当前桌面）

---

## [1.3.6] - 2026-08-10

### Added (English)
- Autostart at login toggle in Settings (Linux XDG / macOS LaunchAgent / Windows registry)
- Silent start option: launch to tray + island without showing the main window
- Tray menu now follows the app language (Show DevNexus / Dynamic Island Settings / Check for Updates / Quit)
- DeepSeek balance shown in the tray menu (click to refresh)
- Tray menu navigation: Dynamic Island Settings and Check for Updates open the matching page
- Cross-workspace visibility hardened with delayed retry (fixes packaged builds where the island was only on one workspace)

### Fixed (English)
- Dynamic Island not visible on all workspaces in packaged builds (GTK window not realized yet when sticky was applied — retry until ready)
- Tray menu always in English regardless of app language

### 新增（中文）
- 设置页新增"开机自启"开关（Linux XDG / macOS LaunchAgent / Windows 注册表）
- 新增"静默启动"子选项：开机自启时不显示主窗口，后台常驻托盘与灵动岛
- 系统托盘菜单跟随应用语言（显示 DevNexus / 灵动岛设置 / 检查更新 / 退出）
- 托盘菜单显示 DeepSeek 余额（点击刷新）
- 托盘菜单支持直接跳转"灵动岛设置"与"检查更新"页面
- 跨工作区常驻增强：延迟重试，修复打包版灵动岛只在单个工作区显示的问题

### 修复（中文）
- 修复打包版灵动岛跨工作区不可见（窗口未就绪时已应用 sticky，改为重试直至生效）
- 修复托盘菜单不跟随语言、始终英文

---

## [1.3.5] - 2026-08-10

### Added (English)
- Dynamic Island two-state design: collapsed capsule / expanded capsule with spring animation (hover or click to expand)
- Dynamic Island now visible on all virtual desktops/workspaces (X11 `_NET_WM_DESKTOP=0xFFFFFFFF` via GDK)
- Per-monitor Dynamic Island instances on multi-display setups, each positioned and persisted independently
- Media control buttons (previous / play-pause / next) in the media module
- System notifications now expand the Dynamic Island inline like the native iPhone/Mac behavior, then auto-collapse

### Fixed (English)
- Dynamic Island toggle always showed "off" (Switch bound to `checked` instead of `modelValue`)
- Media player never detected (MPRIS `ListNames` reply parsed as tuple instead of array)
- UI froze during media polling (sync command with blocking D-Bus calls ran on the main thread)
- Island window disappeared when switching workspaces; disappeared on non-primary monitors
- Island lost drag/click responsiveness and stuck state when default-open
- Expand animation pivoted from the top-left instead of symmetric horizontal spread from center
- Collapse animation clipped the capsule into a rectangle (window resized before CSS transition finished)

### 新增（中文）
- 灵动岛两态设计：收起小胶囊 / 展开大胶囊，spring 弹性动画（悬停或点击展开）
- 灵动岛在所有虚拟桌面/工作区可见（GDK 写入 X11 `_NET_WM_DESKTOP=0xFFFFFFFF`）
- 多显示器每屏一个灵动岛实例，位置独立持久化
- 媒体模块补上控制按钮（上一首 / 播放暂停 / 下一首）
- 系统通知融入灵动岛：胶囊膨胀内联展示通知，结束后自动收起（iPhone/Mac 原生风格）

### 修复（中文）
- 修复灵动岛开关永远显示"关闭"（Switch 误绑 `checked` 而非 `modelValue`）
- 修复播放器永远识别不到（MPRIS `ListNames` 返回值按元组解析而非数组）
- 修复媒体轮询卡死界面（同步命令 + 阻塞 D-Bus 调用跑在主线程）
- 修复切换工作区、多显示器时岛窗口消失
- 修复岛窗口失去拖拽/点击响应、默认开启状态不一致
- 修复展开动画以左上角为基准偏移（改为从中心左右对称展开）
- 修复收起动画胶囊被裁剪成矩形（窗口在 CSS 过渡完成前就缩小）

---

## [1.3.1] - 2026-08-01

### Fixed
- System tray menu text invisible on Linux (menu object lifetime kept alive via app state)
- Startup flash of raw i18n keys in sidebar (language fallback chain + empty-safe t())
- Skeleton loading invisible on dark theme (theme-aware skeleton colors)
- Icon font blocked by CSP (restored Google Fonts for Material Symbols / Inter)
- Text contrast below WCAG AA in both themes (muted/secondary raised to ≥4.5:1)
- CI build failures caused by unformatted code (cargo fmt applied, fmt/clippy gates pass)

### 修复（中文）
- 修复 Linux 托盘菜单文字不显示（菜单对象生命周期保活）
- 修复启动时侧边栏闪现 i18n 键名（语言回退链 + t() 空安全）
- 修复暗色主题下骨架屏不可见（主题感知骨架色）
- 修复 CSP 拦截图标字体（恢复 Google Fonts 许可）
- 修复双主题文字对比度不达标（muted/secondary 提升至 ≥4.5:1）
- 修复 CI 构建失败（代码格式化，fmt/clippy 门禁通过）

---

## [1.3.0] - 2026-08-01

### Removed (English)
- Built-in download manager (use mature download tools instead)

### Security (English)
- Fixed command injection in nvm version switching (S1)
- API Hub CORS whitelisted to block cross-site key theft (S2)
- Cookie decryption now verifies SHA-256 integrity instead of silently trusting data (S3)
- Shell rc injection protection for mirror/path writes (S4)
- Software install path-traversal protection (S5)
- Residue scanner uses boundary keyword matching + safe-delete guards (S6)
- Keyring enabled with real platform backends (password persistence no longer fails) (S7)
- PBKDF2 iteration bounds + encryption key zeroing on lock (S8)
- Docker command whitelist, arg validation and 120s timeout (S9)
- Cookie temp files use random names, 0600 perms and RAII cleanup (S10)
- Provider API keys encrypted at rest with AES-256-GCM (OS keyring-backed key)

### Fixed (English)
- API Hub usage stats race: streaming token backfill no longer lost (C1)
- Provider duplicate-name check moved before DB insert + unique index (C2)
- Unsupported cross-protocol streaming now returns explicit 422 (C3)
- Duplicate message_delta/message_stop in stream conversion removed (C4)
- Client-input errors return 400/422 instead of 500; no startup panic (C5)
- Provider update preserves key on empty/masked input; missing id errors (C6)
- Shrunk system/process lock critical sections, removed unwrap panics (C7)
- Upstream URL join dedups by path segment (C8)
- fetch_models reuses global HTTP client (C9)
- Removed dead code, unified command executor, rc editor and error types (A1-A5)
- PBKDF2 default iterations raised to 600k (OWASP)

### Changed (English)
- Frontend fully i18n-ized (ApiHub + remaining pages), components split, keyed lists, unified loading indicator (F1-F9)
- TLS stack unified to rustls; CSP tightened (removed unused Google Fonts)
- Release profile: LTO + strip; CI key injection & release cleanup guards
- Dependencies upgraded (tauri 2.11.5, tokio 1.53.1); quick-xml 0.41.0 fixes high-severity DoS
- API Hub benchmark tooling added (`pnpm bench`, 3571 RPS baseline)

### 移除（中文）
- 内置下载管理器（改用成熟的下载工具）

### 安全修复（中文）
- 修复 nvm 版本切换命令注入（S1）
- API Hub CORS 白名单化，阻止跨站盗用密钥（S2）
- Cookie 解密增加 SHA-256 完整性校验（S3）
- shell rc 注入防护（镜像源/环境变量写入）（S4）
- 软件安装路径穿越防护（S5）
- 残留扫描改为边界关键词匹配 + 安全删除保护（S6）
- keyring 启用真实平台后端，密码持久化不再静默失败（S7）
- PBKDF2 迭代数上限 + 锁定时密钥内存清零（S8）
- docker 命令白名单、参数校验与超时（S9）
- Cookie 临时文件随机名、0600 权限与自动清理（S10）
- Provider API Key 落库加密（AES-256-GCM，密钥存 OS keyring）

### 修复（中文）
- API Hub 用量统计竞态，流式 token 回填不再丢失（C1）
- Provider 重名检查前置 + 数据库唯一索引（C2）
- 不支持的跨协议流式转换显式返回 422（C3）
- 流式转换重复停止事件去重（C4）
- 客户端输入错误返回 400/422，启动不再 panic（C5）
- Provider 更新空 key 保留原值、不存在 id 报错（C6）
- 缩小系统/进程锁临界区并消除 unwrap panic（C7）
- 上游 URL 拼接按路径段边界去重（C8）
- fetch_models 复用全局 HTTP 连接池（C9）
- 消除重复实现：统一命令执行器、rc 编辑器、错误类型（A1-A5）
- PBKDF2 默认迭代数提升至 60 万（OWASP 推荐）

### 优化（中文）
- 前端全面 i18n 化、组件拆分、keyed 列表、统一加载指示器（F1-F9）
- TLS 栈统一为 rustls；收紧 CSP（移除未使用的 Google Fonts）
- 发布构建 LTO + strip 优化；CI 密钥注入与 release 清理守卫
- 依赖整体升级（tauri 2.11.5、tokio 1.53.1）；quick-xml 0.41.0 修复高危 DoS
- 新增 API Hub 压测工具（`pnpm bench`，基线 3571 RPS）

---

## [1.2.2] - 2026-07-21

### Added (English)
- IDM-style segmented progress bar with per-chunk status colors
- Real-time speed/progress reporting via streaming download + global atomic counter
- Work queue download engine with fixed worker threads and dynamic load balancing
- GitHub URL auto-detection with configurable mirror support
- Bilingual changelog display in update dialog
- Clipboard auto-paste when opening the Add Download dialog
- Cookie string support for authenticated downloads
- Browser environment emulation (Sec-Fetch- headers, native-tls, complete Accept header)
- Exponential backoff retry for failed chunks
- Mirror management UI with strip_host mode for Xget-style proxies

### Fixed
- Download speed always showing 0 B/s (multiple root causes eliminated)
- HTTP 403 due to missing User-Agent and browser headers
- Content-Encoding decoding failure with no_gzip / no_brotli config
- Sequential chunk waiting replaced by FuturesUnordered for immediate progress
- Confirm dialog excessive width and redundant title bar
- All dialog widths reduced across the application

### Added (中文)
- IDM 风格分段进度条，根据不同分块状态显示不同颜色
- 基于流式下载 + 全局原子计数器的实时速度与进度推送
- 工作队列下载引擎，固定线程数并发 + 动态负载均衡
- GitHub 链接自动检测与可配置镜像加速
- 更新弹窗双语更新日志显示
- 打开添加下载弹窗时自动读取剪贴板
- Cookie 字符串支持，用于需要登录态的文件下载
- 浏览器环境模拟（Sec-Fetch 头、native-tls、完整 Accept 头）
- 分块失败指数退避重试
- 镜像管理界面，支持 strip_host 模式适配 Xget 风格代理

### Fixed (中文)
- 下载速度始终显示 0 B/s（多个根因全部修复）
- 缺少 User-Agent 和浏览器头导致的 HTTP 403
- 关闭 gzip/brotli 后 Content-Encoding 解码失败
- 顺序等待分块改为 FuturesUnordered，立即推送进度
- 确认弹窗过宽及多余标题栏
- 全局弹窗宽度缩小

---

## [1.2.0] - 2026-07-18

### Added
- API Hub gateway — unified API management and proxy interface
- Environment migration system — export and import environment profiles
- Comprehensive API Hub e2e test suite and migration parse checks

### Changed
- UI redesign with ZCode-inspired polish across all modules
- Upstream request handling: reduced timeout, auto-creation of data directory
- Deduplication of overlapping path segments in upstream URLs

### Fixed
- 12 missing i18n keys across error details, mirrors, software, and residue categories
- ApiProtocol `serde` rename_all snake_case for provider addition
- Type error in Migration import path (svelte-check)
- Various clippy warnings across API Hub and command modules

### Performance
- Cache sysinfo `System` instance across calls to reduce I/O

---

## [1.1.1] - 2026-07-09

### Added
- Network Acceleration module — optimized network connectivity and mirror latency

### Removed
- Task Scheduler module (Cron engine, Shell/Python execution, system shutdown)

### Fixed
- MirrorSettings race condition during concurrent latency tests
- EnvironmentManager and TaskScheduler cleanup after module removal
- ProcessManager, ContainerManager, PasswordManager bug fixes
- AppUninstaller table column width distribution

---

## [1.0.10] - 2026-07-05

### Added
- Docker / Podman container manager integration
- Process manager and port manager merge into unified view

### Changed
- Major UI redesign inspired by ZCode design language
- Layout and visual refresh across all pages

### Fixed
- macOS architecture detection — use artifact directory name instead of signature filename
- macOS app bundle paths for updater signatures
- NSIS signature mapping and `.app.tar.gz` URL references on macOS
- CI signing key and workflow env configuration
- Multiple release workflow issues (`.sig` collection, base64 encoding, YAML parsing)

---

## [1.0.9] - 2026-06-25

### Added
- Rewrote updater system based on DBX approach for reliable auto-updates

### Fixed
- Updater download URLs and CI workflow configuration
- Sidebar flicker during navigation
- Cargo formatting in updater module
- Signing key regeneration and secret management

---

## [1.0.8] - 2026-06-24

### Fixed
- Miscellaneous bug fixes across the application

---

## [1.0.7] - 2026-06-19

### Changed
- Version bump and miscellaneous fixes

---

## [1.0.6] - 2026-06-19

### Added
- Process manager — real-time process list with grouping and kill support
- Environment migration export functionality
- Improved Java version detection and switching

### Fixed
- Cargo fmt and clippy warnings

---

## [1.0.5] - 2026-06-15

### Added
- Development documentation links and detailed module explanations in README

### Fixed
- Bug fixes and performance optimizations
- Unused import warnings on Windows

---

## [1.0.4] - 2026-06-06

### Added
- Comprehensive Java version switching — full `jenv` integration

### Fixed
- Cross-platform dependency gating (`dbus` Linux-only, `sha2` conditional imports)
- CI workflow simplification

---

## [1.0.3] - 2026-06-06

### Fixed
- Chrome cookie decryption on Windows
- Windows-specific clippy warnings (dead code, formatting, replacements)
- Cargo fmt and formatting check consistency

---

## [1.0.2] - 2026-06-03

### Fixed
- Unused import on Windows after cfg-gating Chrome key functions

---

## [1.0.0] - 2026-05-19

### Added
- Initial public release of DevNexus

### Core Features
- **Software Center** — visual management of system packages (brew / apt / winget / choco / pip / npm)
- **Environment Manager** — PATH and environment variable editing with dotfile support
- **Mirror Settings** — one-click configuration for pip / npm / apt / Go / RubyGems / Maven / Conda / NuGet / Flutter / Docker / cargo mirrors with batch latency testing
- **System Dashboard** — real-time CPU, memory, disk, and runtime version monitoring
- **Port Manager** — port usage inspection via lsof / procfs / netstat
- **Process Manager** — real-time process listing, grouping, and termination
- **Task Scheduler** — Cron engine with Shell and Python script execution, system shutdown
- **Password Manager** — AES-256-GCM + PBKDF2 encrypted vault backed by SQLite
- **Cookie Extractor** — supports 5 major browsers with 3 encryption mechanisms (macOS Keychain, Linux libsecret, Windows DPAPI)
- **App Uninstaller** — deep scanning for residual files, registry entries, and shortcuts
- **Version Manager** — unified SDK version management via pyenv / fnm / jenv / gvm / rustup / gcc
- **Auto-Updater** — GitHub Release based update mechanism with signature verification

### Platform Support
- Windows (MSI / NSIS installer)
- macOS (DMG / .app bundle)
- Linux (deb / rpm / AppImage)

### Infrastructure
- Svelte 5 frontend with Tailwind CSS
- Rust backend with Tauri 2.0
- Trilingual i18n support (Chinese / English / Russian)
- CI/CD pipeline with automated cross-platform builds and releases
- Accessibility improvements (ARIA attributes)

---

[1.2.0]: https://github.com/lin/DevNexus/releases/tag/v1.2.0
[1.1.1]: https://github.com/lin/DevNexus/releases/tag/v1.1.1
[1.0.10]: https://github.com/lin/DevNexus/releases/tag/v1.0.10
[1.0.9]: https://github.com/lin/DevNexus/releases/tag/v1.0.9
[1.0.8]: https://github.com/lin/DevNexus/releases/tag/v1.0.8
[1.0.7]: https://github.com/lin/DevNexus/releases/tag/v1.0.7
[1.0.6]: https://github.com/lin/DevNexus/releases/tag/v1.0.6
[1.0.5]: https://github.com/lin/DevNexus/releases/tag/v1.0.5
[1.0.4]: https://github.com/lin/DevNexus/releases/tag/v1.0.4
[1.0.3]: https://github.com/lin/DevNexus/releases/tag/v1.0.3
[1.0.2]: https://github.com/lin/DevNexus/releases/tag/v1.0.2
[1.0.0]: https://github.com/lin/DevNexus/releases/tag/v1.0.0
