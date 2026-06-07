# 架构总览

## 系统架构

DevNexus 采用 **Tauri 2.0** 标准架构：Rust 后端 + Svelte 前端，通过 IPC (指令调用) 通信。

```
┌─────────────────────────────────────────────────────┐
│                     前端 (Svelte 5)                   │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────────┐  │
│  │ Dashboard   │ │SoftwareCenter│ │ Environment  │  │
│  ├─────────────┤ ├──────────────┤ ├──────────────┤  │
│  │MirrorSetting│ │  PortManager │ │TaskScheduler │  │
│  ├─────────────┤ ├──────────────┤ ├──────────────┤  │
│  │PasswordMgr  │ │ CookieViewer │ │  Settings    │  │
│  └──────┬──────┘ └──────┬───────┘ └──────┬───────┘  │
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
│  │  mirror.rs  port_manager.rs  scheduler.rs     │   ││
│  │  password_manager.rs  cookie_manager.rs       │   ││
│  │  version_manager.rs                           │   ││
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
   ┌──────┬──────┬──────┬────┼────┬──────┬──────┬──────┐
   │      │      │      │    │    │      │      │      │
┌──▼──┐┌─▼───┐┌─▼──┐┌─▼──┐┌▼───┐┌▼──┐┌─▼───┐┌─▼──┐
│system││ env ││mirr││port││soft││pass││cookie││sched│
│      ││     ││ or ││    ││ware││word││      ││uler │
└─────┘└─────┘└────┘└────┘└──┬─┘└────┘└─────┘└─────┘
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
用户操作 → Svelte Event Handler
         → invoke("command_name", { args })
         → Tauri IPC (JSON 序列化)
         → Rust #[tauri::command] fn
         → 业务逻辑（系统调用、文件读写、加密解密）
         → Result<T, String>
         → JSON 反序列化
         → 前端响应式更新 ($state / $derived)
```

所有命令都是请求-响应模式。没有 WebSocket / 事件推送（除了前端的 `setInterval` 轮询）。

## 前端路由表

```javascript
// +page.svelte 中的路由
const routes = {
    '/':                    Dashboard,          // 系统仪表板
    '/software':            SoftwareCenter,      // 软件中心
    '/environment':         EnvironmentManager,  // 环境管理
    '/mirrors':             MirrorSettings,      // 镜像设置
    '/ports':               PortManager,         // 端口管理
    '/scheduler':           TaskScheduler,       // 任务调度
    '/passwords':           PasswordManager,     // 密码管理器
    '/cookies':             CookieViewer,        // Cookie 提取
    '/settings':            Settings,            // 设置
};
```

## 模块文档索引

| 编号 | 模块 | 文件 | 核心功能 |
|------|------|------|---------|
| 01 | 系统仪表板 | `commands/system.rs` | 硬件信息、CPU/内存/磁盘使用率 |
| 02 | 软件中心 | `commands/software.rs` | 37+ 工具管理、跨平台包管理器 |
| 03 | 环境管理 | `commands/environment.rs` | 运行时检测、PATH 编辑 |
| 04 | 镜像设置 | `commands/mirror.rs` | 12 种包源切换、延迟测试 |
| 05 | 端口管理 | `port_manager.rs` | 端口列表、进程查杀 |
| 06 | 任务调度 | `scheduler.rs` | Cron 定时、Shell/Python 执行 |
| 07 | 密码管理器 | `password_manager.rs` | AES-256-GCM 加密存储、密码生成 |
| 08 | Cookie 提取 | `cookie_manager.rs` | 浏览器 Cookie 读取与导出 |
| 09 | 深度卸载 | `residue_scanner.rs` | 残留扫描、跨平台路径数据库 |
| 10 | 版本管理 | `version_manager.rs` | 6 种语言版本切换、Shell 配置 |

## 安全边界

1. **IPC 接口保护**: 所有 Tauri 命令均为显式注册，无自动暴露
2. **本地数据存储**: 密码管理器使用 AES-256-GCM 加密，密钥仅在解锁状态存于内存
3. **Shell 执行**: 定时任务的脚本通过临时文件执行，避免 shell 注入
4. **权限提升**: 不同平台使用对应的提权机制（pkexec / sudo / UAC）
