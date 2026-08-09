# 架构总览

## 系统架构

DevNexus 采用 **Tauri 2.0** 标准架构：Rust 后端 + Vue 3 前端，通过 IPC (指令调用) 通信。

```
┌─────────────────────────────────────────────────────┐
│                     前端 (Vue 3)                      │
│  ┌─────────────┐ ┌──────────────┐ ┌────────────────┐│
│  │ Dashboard   │ │              │ │SoftwareCenter  ││
│  ├─────────────┤ ├──────────────┤ ├────────────────┤│
│  │ ContainerMgr│ │   ApiHub     │ │ Environment    ││
│  ├─────────────┤ ├──────────────┤ ├────────────────┤│
│  │MirrorSetting│ │ ProcessMgr   │ │  Uninstaller ││
│  ├─────────────┤ ├──────────────┤ ├────────────────┤│
│  │PasswordMgr  │ │CookieExtractor│ │ Migration      ││
│  ├─────────────┤ ├──────────────┤ ├────────────────┤│
│  │AppUninstall │ │VersionMgr    │ │  Settings      ││
│  └──────┬──────┘ └──────┬───────┘ └──────┬─────────┘│
│         │               │                │          │
│         └───────────────┼────────────────┘          │
│                         │ invoke()                   │
│                 @tauri-apps/api                      │
└────────────────────┬────┘                            │
                     │ IPC                             │
┌────────────────────┴────────────────────────────────┐│
│                 后端 (Rust)                           ││
│  main.rs / lib.rs  (命令注册)                         ││
│  ┌──────────────────────────────────────────────┐   ││
│  │ commands/                                     │   ││
│  │  system.rs  environment.rs  software.rs       │   ││
│  │  container.rs  api_hub/                      │   ││
│  │  mirror.rs  process_ports.rs                │   ││
│  │  password_manager.rs  cookie_extractor.rs     │   ││
│  │  version_manager.rs  migration.rs  updater.rs │   ││
│  └──────────────────────────────────────────────┘   ││
│  ┌──────────────────────────────────────────────┐   ││
│  │ 工具模块                                       │   ││
│  │  residue_scanner.rs                          │   ││
│  │  utils.rs                                    │   ││
│  └──────────────────────────────────────────────┘   ││
└─────────────────────────────────────────────────────┘┘
```

## 模块依赖关系

```
                        ┌──────────┐
                        │  utils   │ ← 被所有模块使用: 路径、命令执行
                        └────┬─────┘
                             │
   ┌────┬────┬──────┬────┬───────┬────┬────┬────┬─────┬──────┐
   │    │    │      │    │       │    │    │    │     │      │
 ┌─▼──┐┌▼──┐┌▼───┐┌─▼──┐┌▼────┐┌▼───┐┌▼──┐┌▼──┐┌▼───┐┌▼────┐
 │down││sys ││env ││mir││soft ││cont││api ││port││pass ││cook │
 │load││tem ││    ││ror││ware ││ain ││hub ││/pro││word ││ie   │
 └────┘└───┘└────┘└───┘└──┬──┘└────┘└───┘└───┘└────┘└─────┘
                          │
                     ┌────▼──────┐
                     │residue    │
                     │scanner    │
                     └───────────┘
```

**依赖关系说明**:
- `utils.rs` — 被所有模块使用（PATH 查找、用户目录、shell 配置检测）
- `residue_scanner` — 仅被 `software.rs` 的深度卸载调用
- 其他模块间无直接调用关系（仅通过 Tauri 命令与前端 IPC）

## 数据流

```
用户操作 → Vue 组件事件处理 (script setup)
         → invoke("command_name", { args })
         → Tauri IPC (JSON 序列化)
         → Rust #[tauri::command] fn
         → 业务逻辑（系统调用、文件读写、加密解密）
         → Result<T, String>
         → JSON 反序列化
         → 前端响应式更新 (Vue ref / reactive + <script setup>)
```

所有命令都是请求-响应模式。没有 WebSocket / 事件推送（除了前端的 `setInterval` 轮询）。

## 前端路由表

```javascript
// src/router.js 中的路由（Vue Router，hash 模式，懒加载）
const routes = {
    '/':                Dashboard,          // 系统仪表板
    '/environments':    EnvironmentManager,  // 环境管理
    '/software':        SoftwareCenter,      // 软件中心
    '/mirrors':         MirrorSettings,      // 镜像设置
    '/processes':       ProcessManager,      // 进程/端口管理
    '/passwords':       PasswordManager,     // 密码管理器
    '/cookies':         CookieExtractor,     // Cookie 提取
    '/uninstall':       AppUninstaller,      // 应用卸载（深度清理）
    '/containers':      ContainerManager,    // 容器管理
    '/api-hub':         ApiHub,              // API Hub
    '/migration':       Migration,           // 环境迁移
    '/settings':        Settings,            // 设置
};
```

## 模块文档索引

| 编号 | 模块 | 文件 | 核心功能 |
|------|------|------|---------|
| 01 | 系统仪表板 | `commands/system.rs` | 硬件信息、CPU/内存/磁盘使用率 |
| 02 | 软件中心 | `commands/software.rs` | 工具管理、跨平台包管理器 |
| 03 | 环境管理 | `commands/environment.rs` | 运行时检测、PATH 编辑 |
| 04 | 镜像设置 | `commands/mirror.rs` | 包源切换、延迟测试 |
| 05 | 端口/进程管理 | `commands/process_ports.rs` | 端口列表、进程查杀 |
| 07 | 密码管理器 | `commands/password_manager.rs` | AES-256-GCM 加密存储、密码生成 |
| 08 | Cookie 提取 | `commands/cookie_extractor.rs` | 浏览器 Cookie 读取与导出 |
| 09 | 深度卸载 | `residue_scanner/` | 残留扫描、跨平台路径数据库 |
| 10 | 版本管理 | `commands/version_manager.rs` | 多语言运行时版本切换、Shell 配置 |
| 11 | API Hub | `api_hub/` | 本地 AI 网关、多协议格式转换、流式 |
| 13 | 容器管理 | `commands/container.rs` | Docker/Podman 容器、镜像、卷、Compose |
| 14 | 环境迁移 | `commands/migration.rs` | 配置文件导入/导出 |
| 15 | 自动更新 | `commands/updater.rs` | GitHub Release 检查 + 双语日志 |

## 安全边界

1. **IPC 接口保护**: 所有 Tauri 命令均为显式注册，无自动暴露
2. **本地数据存储**: 密码管理器使用 AES-256-GCM 加密，密钥仅在解锁状态存于内存
3. **Shell 执行**: 定时任务的脚本通过临时文件执行，避免 shell 注入
4. **权限提升**: 不同平台使用对应的提权机制（pkexec / sudo / UAC）
