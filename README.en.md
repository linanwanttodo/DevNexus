<br>

# DevNexus 
> One-stop cross-platform developer toolchain manager — Control your entire dev environment with GUI

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)]()
[![Tauri](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-stable-dea584?logo=rust)](https://www.rust-lang.org/)

<p align="center">
  <img src="src-tauri/icons/DevNexus.png" alt="DevNexus" width="180">
</p>
<div align="center">
  <strong><a href="README.md">中文</a> | English | <a href="README.ru.md">Русский</a></strong>
</div>

---

## Introduction

DevNexus is a **cross-platform desktop application** that integrates everyday developer environment management tasks into a lightweight GUI:

- **Software Center** — Visual management of system packages (brew / apt / winget / choco / pip / npm)
- **Environment Manager** — Edit PATH, environment variables, dotfile configurations
- **Terminal Core** — Embedded PTY terminal with real shell sessions
- **Mirror Settings** — One-click configuration for pip / npm / apt mirror sources
- **System Dashboard** — Real-time CPU, memory, disk, and runtime version monitoring
- **Global Settings** — App preferences and theme management

Only **~10MB** installation, **~60MB** memory usage — say goodbye to Electron bloat.

---

## Screenshots

| ![Overview](docs/Picture/概览.png) |
|:--:|
| *Dashboard — Real-time CPU, memory, and disk info* |

| ![Software Center](docs/Picture/软件中心.png) | ![Environment Manager](docs/Picture/环境管理.png) |
|:--:|:--:|
| *Software Center — Visual system package management* | *Environment Manager — Visual PATH & env var editor* |

| ![Port Manager](docs/Picture/端口管理.png) |
|:--:|
| *Port Manager — View and manage local port usage* |

---

## Why DevNexus?

Developers face these fragmented tools every day:

| Task | Current Solutions | Problems |
|---|---|---|
| Install dev tools | `brew install` / `apt install` / `winget` | Different commands per platform, no unified view |
| Manage SDK versions | nvm / pyenv / asdf / sdkman | CLI-only, poor Windows support |
| Switch env variables | Manually edit `.bashrc` / `.zshrc` | Error-prone, no visualization |
| Configure mirrors | Look up docs separately | Tedious, hard to remember |
| View system info | `htop` / `df` / `node -v` everywhere | No centralized panel |

**DevNexus integrates all of this into a single GUI.** No need to memorize commands or switch between tools.

---

## Comparison

| Feature | **DevNexus** | [nvm-desktop](https://github.com/1111mp/nvm-desktop) ⭐1.3k | [VMR](https://github.com/gvcgo/version-manager) ⭐1.3k | [vfox](https://github.com/version-fox/vfox) ⭐3.8k | [DevTool Manager](https://github.com/dengyuwu/dev-tools) | [DevTools-X](https://github.com/fosslife/devtools-x) ⭐1.5k |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| **GUI** | ✅ | ✅ | ❌ TUI | ❌ CLI | ✅ | ✅ |
| **Install Size** | ~10MB | ~30MB | ~8MB | ~5MB | ~15MB | ~10MB |
| **System Pkg Mgr** (brew/apt/winget) | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Multi-runtime Mgr** | ✅ | ❌ Node only | ✅ 30+ SDKs | ✅ Plugin-based | ❌ | ❌ |
| **npm/cargo/pip globals** | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| **Env Var / PATH Editor** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Embedded Terminal** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Mirror Config** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| **System Dashboard** | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ |
| **macOS** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Linux** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Windows** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Stack** | Tauri+Svelte+Rust | Tauri+React+Rust | Go | Go | Tauri+React+Rust | Tauri+React+Rust |

**Key Differences:**

- **nvm-desktop** — Only manages Node.js versions, limited scope
- **VMR / vfox** — Powerful but pure CLI/TUI, no visual interface
- **DevTool Manager** — Only manages npm/cargo/pip global packages, no system-level env or terminal
- **DevTools-X** — Developer utility collection (JSON formatter, JWT parser, etc.), not an environment manager
- **DevNexus** — **The only project that integrates system package management + multi-runtime versions + env variables + terminal + mirror config into one GUI**

---

## Architecture

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

### Tech Stack

| Layer | Technology | Description |
|---|---|---|
| **Desktop Framework** | [Tauri 2.0](https://tauri.app/) | Native system Webview, not Electron |
| **Frontend** | [Svelte 5](https://svelte.dev/) | Compile-time framework, only ~2KB runtime |
| **Styling** | [Tailwind CSS](https://tailwindcss.com/) | Utility-first CSS |
| **Terminal** | [xterm.js](https://xtermjs.org/) | Web terminal rendering |
| **Backend Language** | [Rust](https://www.rust-lang.org/) | System calls, performance, memory safety |
| **PTY** | [portable-pty](https://crates.io/crates/portable-pty) | Cross-platform terminal backend |
| **Async Runtime** | [tokio](https://crates.io/crates/tokio) | Rust async I/O |
| **System Info** | [sysinfo](https://crates.io/crates/sysinfo) | CPU/Memory/Disk/Process |
| **Executable Lookup** | [which](https://crates.io/crates/which) | Cross-platform PATH lookup |
| **Serialization** | [serde](https://crates.io/crates/serde) | JSON/TOML config read/write |

### Why This Stack?

- **Tauri over Electron** — 10MB vs 150MB install, 60MB vs 300MB memory, uses system Webview instead of bundled Chromium
- **Svelte over React** — Compile-time elimination of framework runtime, smaller output; native HTML syntax, zero-cost migration from design prototypes
- **Rust over Node.js** — Native system call capabilities, `portable-pty` is the most mature cross-platform PTY solution, memory safe

---

## Project Structure

```
devnexus/
├── src/                          # Svelte Frontend
│   ├── lib/
│   │   ├── stores.js             # Router & search state
│   │   └── i18n.js               # i18n (zh/en/ru)
│   ├── locales/                  # Translation files
│   │   ├── zh.json
│   │   ├── en.json
│   │   └── ru.json
│   ├── routes/                   # Page routes
│   │   ├── Dashboard.svelte      # System dashboard
│   │   ├── EnvironmentManager.svelte
│   │   ├── SoftwareCenter.svelte
│   │   ├── MirrorSettings.svelte
│   │   ├── PortManager.svelte    # Port management
│   │   ├── TaskScheduler.svelte
│   │   ├── PasswordManager.svelte
│   │   ├── CookieExtractor.svelte
│   │   └── Settings.svelte
│   ├── components/
│   │   ├── Sidebar.svelte
│   │   ├── TopBar.svelte
│   │   └── TitleBar.svelte
│   ├── app.svelte
│   └── main.js
├── src-tauri/                    # Rust Backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   └── commands/
│   │       ├── system.rs         # System info
│   │       ├── environment.rs    # PATH/env variables
│   │       ├── software.rs       # Package management
│   │       ├── mirror.rs         # Mirror sources
│   │       ├── port_manager.rs   # Port management
│   │       ├── scheduler.rs      # Task scheduling
│   │       ├── password_manager.rs
│   │       ├── cookie_extractor.rs
│   │       ├── terminal.rs       # PTY terminal
│   │       └── mod.rs
│   ├── icons/
│   │   └── DevNexus.png          # App icon source
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/
│   └── generate_icons.py         # Icon conversion script
├── .github/workflows/
│   └── build.yml                 # CI auto build
├── package.json
└── README.md
```

---

## Development Guide

### Prerequisites

- [Node.js](https://nodejs.org/) >= 20
- [Rust](https://rustup.rs/) >= 1.80
- System dependencies ([Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

### Install Dependencies

```bash
pnpm install
```

### Development Mode

```bash
pnpm tauri dev
```

### Build for Production

```bash
pnpm tauri build
```

Build artifacts:
- **macOS**: `.dmg` / `.app`
- **Linux**: `.deb` / `.rpm` / AppImage
- **Windows**: `.msi` / `.exe`

---

## Roadmap

- [ ] Project skeleton setup
- [ ] System package manager backend (brew / apt / winget)
- [ ] Software Center UI & backend integration
- [ ] Environment variable read/write & visual editor
- [ ] Mirror source configuration
- [ ] System info dashboard
- [ ] Auto-update mechanism
- [ ] Theme & internationalization

---

## License

[MIT](LICENSE)
