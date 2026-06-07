# DevNexus

> 开发者一站式工具集 — 跨平台桌面工具应用

DevNexus 是一款面向开发者的系统工具集合，提供系统信息监控、软件包管理、环境管理、镜像源配置、端口管理、定时任务、密码管理和浏览器 Cookie 提取等功能的桌面应用。基于 Tauri 2.0 + Rust + Svelte 5 构建。

## 功能概览

| 功能            | 说明 | 文档 |
|---------------|------|------|
| **系统仪表板**     | 查看系统硬件信息、CPU/内存/磁盘实时使用率、系统运行时间 | [01-system.md](modules/01-system.md) |
| **软件中心**      | 浏览器/安装/卸载 37+ 款开发者工具，支持跨平台包管理器自动适配 | [02-software.md](modules/02-software.md) |
| **环境管理**      | 检测 Python/Node/Java/Go/Rust 等运行时环境，管理 PATH 配置 | [03-environment.md](modules/03-environment.md) |
| **镜像设置**      | 一键切换 12 种包管理器和语言运行时的镜像源，延迟测试与推荐 | [04-mirror.md](modules/04-mirror.md) |
| **端口管理**      | 列出监听端口及占用进程，一键释放冲突端口 | [05-port.md](modules/05-port.md) |
| **任务调度**      | Cron 定时任务，支持 Shell/Python 脚本执行和系统操作 | [06-scheduler.md](modules/06-scheduler.md) |
| **密码管理器**     | AES-256-GCM 加密本地密码存储，支持强密码生成 | [07-password.md](modules/07-password.md) |
| **Cookie 提取** | 从浏览器导出 Cookie 为 Netscape/cURL 格式 | [08-cookie.md](modules/08-cookie.md) |
| **深度卸载**    | 残留文件扫描，跨平台清理已知工具的配置文件和数据目录 | [09-uninstall.md](modules/09-uninstall.md) |
| **版本管理**    | 多语言版本检测与切换，支持 pyenv/fnm/rustup 等 6 种版本管理器 | [10-version.md](modules/10-version.md) |

## 跨平台支持

| 平台 | 支持 | 包管理器 | 架构 |
|------|------|---------|------|
| macOS 12+ | ✅ | Homebrew, MacPorts | x86_64, arm64 |
| Linux (glibc 2.28+) | ✅ | apt, dnf, pacman, zypper, apk, snap, flatpak | x86_64 |
| Windows 10+ | ✅ | winget, Chocolatey | x86_64 |

详细跨平台实现见 [cross-platform.md](modules/99-cross-platform.md)。

## 技术栈

**后端**: Rust + Tauri 2.0 + sysinfo + serde + rusqlite + aes-gcm + cron

**前端**: Svelte 5 + Tailwind CSS + Lucide Icons

**构建**: Vite + Tauri CLI

## 快速开始

```bash
# 先决条件: Rust 1.77+, Node.js 18+, pnpm 9+

# 安装依赖
pnpm install

# 启动开发模式
cargo tauri dev

# 构建发行版
cargo tauri build --release
```

详细开发指南见 [dev-guide.md](dev-guide.md)。

## 文档目录

```
docs/
├── architecture.md       # 架构总览与模块关系
├── dev-guide.md          # 开发环境搭建与编码规范
├── modules/
│   ├── 01-system.md      # 系统仪表板
│   ├── 02-software.md    # 软件中心
│   ├── 03-environment.md # 环境管理
│   ├── 04-mirror.md      # 镜像设置
│   ├── 05-port.md        # 端口管理
│   ├── 06-scheduler.md   # 任务调度
│   ├── 07-password.md    # 密码管理器
│   ├── 08-cookie.md      # Cookie 提取
│   ├── 09-uninstall.md   # 深度卸载与残留扫描
│   ├── 10-version.md     # 版本管理
│   └── 99-cross-platform.md  # 跨平台实现详解
└── README.md             # 本文件
```

## 许可证

MIT
