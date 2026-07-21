# Changelog

All notable changes to the DevNexus project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

- Docker / Podman container management (planned)
- Cloud service credential management (AWS / GCP CLI) (planned)

---

## [1.2.1] - 2026-07-21

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
