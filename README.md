<br>

# DevNexus 
> 一站式跨平台开发者工具栈管理器 — 用 GUI 掌控你的整个开发环境

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)]()
[![Tauri](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-stable-dea584?logo=rust)](https://www.rust-lang.org/)

<p align="center">
  <img src="src-tauri/icons/DevNexus.png" alt="DevNexus" width="180">
</p>
<div align="center">
  <strong>中文 | <a href="README.en.md">English</a> | <a href="README.ru.md">Русский</a></strong>
</div>

---

## 简介

DevNexus 是一个**跨平台桌面应用**，将开发者日常需要的环境管理操作整合到一个轻量级 GUI 中：

- **软件中心** — 可视化管理系统包（brew / apt / winget / choco / pip / npm）
- **环境管理器** — 编辑 PATH、环境变量、dotfile 配置
- **终端核心** — 嵌入式 PTY 终端，真实 shell 会话
- **镜像设置** — 一键配置 pip / npm / apt 镜像源
- **系统仪表板** — 实时查看 CPU、内存、磁盘、运行时版本
- **全局设置** — 应用偏好与主题管理

安装包仅 **~10MB**，内存占用约 **60MB**，告别 Electron 的臃肿。

---

## 开发文档

详细的模块设计、跨平台实现原理和开发指南请参阅 [`docs/`](docs/) 目录：

| 文档 | 说明 |
|------|------|
| [架构总览](docs/architecture.md) | 模块依赖关系、数据流、安全边界 |
| [开发指南](docs/dev-guide.md) | 环境搭建、编码规范、构建发布、调试技巧 |
| [系统仪表板](docs/modules/01-system.md) | sysinfo + OnceLock 磁盘缓存 |
| [软件中心](docs/modules/02-software.md) | 37 款工具、9 种包管理器、跨平台偏移 |
| [环境管理](docs/modules/03-environment.md) | 运行时检测、Unix/Windows PATH 编辑 |
| [镜像设置](docs/modules/04-mirror.md) | 12 种包源切换、延迟测试与推荐 |
| [端口管理](docs/modules/05-port.md) | lsof / procfs / netstat 三平台方案 |
| [任务调度](docs/modules/06-scheduler.md) | Cron 引擎、Shell/Python 执行、系统关机 |
| [密码管理器](docs/modules/07-password.md) | AES-256-GCM + PBKDF2 + SQLite |
| [Cookie 提取](docs/modules/08-cookie.md) | 5 种浏览器、3 种加密机制 |
| [深度卸载](docs/modules/09-uninstall.md) | 残留路径数据库 + 关键词扫描 |
| [版本管理](docs/modules/10-version.md) | pyenv/fnm/jenv/rustup/gvm 6 合一 |
| [跨平台详解](docs/modules/99-cross-platform.md) | 三层策略、路径映射、命令差异表 |

---

## 软件截图

| ![概览](docs/Picture/概览.png) |
|:--:|
| *系统概览 — 实时查看 CPU、内存、磁盘信息* |

| ![软件中心](docs/Picture/软件中心.png) | ![环境管理](docs/Picture/环境管理.png) |
|:--:|:--:|
| *软件中心 — 可视化管理系统包* | *环境管理 — 可视化编辑 PATH 与环境变量* |

| ![端口管理](docs/Picture/端口管理.png) |
|:--:|
| *端口管理 — 查看与管理本地端口占用* |

---


## 为什么需要 DevNexus？

开发者每天要面对这些碎片化工具：

| 任务 | 现有方案 | 问题 |
|---|---|---|
| 安装开发工具 | `brew install` / `apt install` / `winget` | 每个平台命令不同，无统一视图 |
| 管理 SDK 版本 | nvm / pyenv / asdf / sdkman | CLI 操作，Windows 支持差 |
| 切换环境变量 | 手动编辑 `.bashrc` / `.zshrc` | 容易出错，无可视化 |
| 配置镜像源 | 分别查文档改配置 | 繁琐，记不住 |
| 查看系统信息 | `htop` / `df` / `node -v` 到处跑 | 没有集中面板 |

**DevNexus 把这些全部整合到一个 GUI 里。** 不用记命令，不用在不同工具间切换。

---

## 竞品对比

| 特性 | **DevNexus** | [nvm-desktop](https://github.com/1111mp/nvm-desktop) ⭐1.3k | [VMR](https://github.com/gvcgo/version-manager) ⭐1.3k | [vfox](https://github.com/version-fox/vfox) ⭐3.8k | [DevTool Manager](https://github.com/dengyuwu/dev-tools) | [DevTools-X](https://github.com/fosslife/devtools-x) ⭐1.5k |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| **GUI 界面** | ✅ | ✅ | ❌ TUI | ❌ CLI | ✅ | ✅ |
| **安装包大小** | ~10MB | ~30MB | ~8MB | ~5MB | ~15MB | ~10MB |
| **系统包管理** (brew/apt/winget) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **多语言运行时管理** | ✅ | ❌ 仅 Node | ✅ 30+ SDK | ✅ 插件化 | ❌ | ❌ |
| **npm/cargo/pip 全局包** | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| **环境变量/PATH 编辑** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **嵌入式终端** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **镜像源配置** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| **系统信息仪表板** | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| **macOS** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Linux** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Windows** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **框架** | Tauri+Svelte+Rust | Tauri+React+Rust | Go | Go | Tauri+React+Rust | Tauri+React+Rust |

**核心差异：**

- **nvm-desktop** — 只管 Node.js 版本，功能单一
- **VMR / vfox** — 功能强大但纯 CLI/TUI，无可视化界面
- **DevTool Manager** — 只管 npm/cargo/pip 全局包，不涉及系统级环境和终端
- **DevTools-X** — 开发者小工具集合（JSON 格式化、JWT 解析等），不是环境管理器
- **DevNexus** — **唯一将系统包管理 + 多语言版本 + 环境变量 + 终端 + 镜像配置整合到一个 GUI 的项目**

---

## 技术架构

```
┌──────────────────────────────────────────────┐
│              Frontend (Svelte 5)              │
│  Tailwind CSS · xterm.js · svelte-spa-router  │
├──────────────────────────────────────────────┤
│            Tauri 2.0 IPC Bridge              │
│         invoke() / emit() / Channel          │
├──────────────────────────────────────────────┤
│              Backend (Rust)                   │
│  ┌─────────┬──────────┬──────────┬─────────┐  │
│  │ pkg_mgr │ env_mgr  │ terminal │ sysinfo │  │
│  │ brew/   │ PATH &   │ portable │ CPU/    │  │
│  │ apt/    │ dotfile  │ -pty     │ MEM/Disk│  │
│  │ winget  │ parser   │ tokio    │ which   │  │
│  └─────────┴──────────┴──────────┴─────────┘  │
└──────────────────────────────────────────────┘
```

### 技术栈

| 层级 | 技术 | 说明 |
|---|---|---|
| **桌面框架** | [Tauri 2.0](https://tauri.app/) | 系统原生 Webview，非 Electron |
| **前端** | [Svelte 5](https://svelte.dev/) | 编译时框架，运行时仅 ~2KB |
| **样式** | [Tailwind CSS](https://tailwindcss.com/) | 原子化 CSS，直接复用设计原型 |
| **终端** | [xterm.js](https://xtermjs.org/) | Web 终端渲染 |
| **后端语言** | [Rust](https://www.rust-lang.org/) | 系统调用、性能、内存安全 |
| **PTY** | [portable-pty](https://crates.io/crates/portable-pty) | 跨平台终端后端 |
| **异步运行时** | [tokio](https://crates.io/crates/tokio) | Rust 异步 I/O |
| **系统信息** | [sysinfo](https://crates.io/crates/sysinfo) | CPU/内存/磁盘/进程 |
| **可执行文件查找** | [which](https://crates.io/crates/which) | 跨平台 PATH 查找 |
| **序列化** | [serde](https://crates.io/crates/serde) | JSON/TOML 配置读写 |

### 为什么选这套技术？

- **Tauri 而非 Electron** — 安装包 10MB vs 150MB，内存 60MB vs 300MB，使用系统 Webview 而非内置 Chromium
- **Svelte 而非 React** — 编译时消除框架运行时，产物更小；HTML 原生语法，迁移设计原型零成本
- **Rust 而非 Node.js** — 原生系统调用能力，`portable-pty` 是最成熟的跨平台 PTY 方案，内存安全

---

## 项目结构

```
devnexus/
├── src/                          # Svelte 前端
│   ├── lib/
│   │   ├── stores.js             # 路由与搜索状态
│   │   └── i18n.js               # 多语言 (zh/en/ru)
│   ├── locales/                  # 翻译文件
│   │   ├── zh.json
│   │   ├── en.json
│   │   └── ru.json
│   ├── routes/                   # 页面路由
│   │   ├── Dashboard.svelte      # 系统仪表板
│   │   ├── EnvironmentManager.svelte
│   │   ├── SoftwareCenter.svelte
│   │   ├── MirrorSettings.svelte
│   │   ├── PortManager.svelte    # 端口管理
│   │   ├── TaskScheduler.svelte
│   │   ├── PasswordManager.svelte
│   │   ├── CookieExtractor.svelte
│   │   ├── AppUninstaller.svelte # 深度卸载
│   │   ├── VersionManager.svelte # 版本管理
│   │   └── Settings.svelte
│   ├── components/
│   │   ├── Sidebar.svelte
│   │   ├── TopBar.svelte
│   │   └── TitleBar.svelte
│   ├── app.svelte
│   └── main.js
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   └── commands/
│   │       ├── system.rs         # 系统信息
│   │       ├── environment.rs    # PATH/环境变量
│   │       ├── software.rs       # 软件包管理
│   │       ├── mirror.rs         # 镜像源
│   │       ├── port_manager.rs   # 端口管理
│   │       ├── scheduler.rs      # 任务调度
│   │       ├── password_manager.rs
│   │       ├── cookie_extractor.rs
│   │       ├── version_manager.rs # 版本管理（pyenv/fnm/jenv/gvm/rustup）
│   │       ├── updater.rs         # 自动更新
│   │       └── mod.rs
│   ├── icons/
│   │   └── DevNexus.png          # 应用图标源文件
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/
│   └── generate_icons.py         # 图标转换脚本
├── .github/workflows/
│   └── build.yml                 # CI 自动构建
├── package.json
└── README.md
```

---

## 开发指南

### 环境要求

- [Node.js](https://nodejs.org/) >= 20
- [Rust](https://rustup.rs/) >= 1.80
- 系统依赖（[Tauri 前置条件](https://v2.tauri.app/start/prerequisites/)）

### 安装依赖

```bash
pnpm install
```

### 开发模式

```bash
pnpm tauri dev
```

### 构建发布

```bash
pnpm tauri build
```

构建产物：
- **macOS**: `.dmg` / `.app`
- **Linux**: `.deb` / `.rpm` / AppImage
- **Windows**: `.msi` / `.exe`

---

## 路线图

### 已完成 ✅

- [x] 项目骨架搭建
- [x] 系统包管理器后端（brew / apt / winget）
- [x] 软件中心 UI 与后端对接
- [x] 环境变量读写与可视化编辑
- [x] 镜像源配置
- [x] 系统信息仪表板
- [x] 端口管理（lsof / procfs / netstat）
- [x] 进程管理器（实时进程列表 + 分组视图 + 杀进程）
- [x] 任务调度（Cron 引擎 + Shell/Python/关机）
- [x] 密码管理器（AES-256-GCM + SQLite）
- [x] Cookie 提取（5 种浏览器）
- [x] 深度卸载（残留扫描 + 注册表 + 快捷方式）
- [x] 版本管理（pyenv/fnm/jenv/gvm/rustup/gcc）
- [x] 主题与国际化（zh / en / ru）
- [x] 自动更新机制（GitHub Release + updater 插件）

### 进行中 / 计划中 🚧

- [ ] Docker / Podman 容器管理
- [ ] 云服务配置（AWS / GCP CLI 凭证管理）

---

## License

[MIT](LICENSE)
