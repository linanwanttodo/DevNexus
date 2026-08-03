# DevNexus UI 重构 + 安全修复设计文档

日期：2026-08-03
状态：已批准（用户确认：迁移 Vue 3 + Arco Design Vue；全部修复审查问题）

## 背景

DevNexus 是 Svelte 5 + Tauri 2 的开发者工具箱桌面应用（12 个路由页，前端约 5,800 行）。
用户反馈：当前手写 UI 不够美观（tailwind 配置将全部圆角设为 0，按钮观感生硬），
希望使用 Arco Design（https://arco.design/vue）官方组件重构。

**关键约束**：Arco Design 官方只有 Vue / React 版本，没有 Svelte 版本。
经用户确认，前端整体迁移到 **Vue 3 + @arco-design/web-vue**。

同时，全面审查发现 2 个高危 + 3 个中危 + 3 个低危问题，用户确认**全部修复**。

## 一、安全/逻辑修复（Rust 后端）

| 编号 | 问题 | 修复方案 | 文件 |
|------|------|----------|------|
| H1 | API Hub 本地服务（127.0.0.1:3456）无认证，恶意网页/本机进程可盗用 API Key 消耗额度 | 启动生成 48 位随机 token 持久化到 `data_dir/api_hub_token`（0600）；axum 中间件校验 `X-DevNexus-Token` 或 `Authorization: Bearer`，`/health` 匿名放行；`api_hub_status` 返回 token 供前端展示 | types.rs / mod.rs / server.rs / commands.rs |
| H2 | 密码管理器密钥在 keyring 不可用时既不落盘又删旧 key → 数据永久丢失 | 回退链：keyring → `data_dir/password_key.bin`（0600）→ 明确告警；仅当新密钥成功持久化后才清理旧 key.bin | password_manager.rs |
| M1/L3 | exec.rs 超时后子进程与线程继续存活（僵尸进程累积） | Unix 改用 `wait-timeout` + `kill()` 精确终止；Windows 保留旧行为 | exec.rs / Cargo.toml |
| M2 | install_software_from_url 无超时/大小限制/内存爆炸风险 | reqwest client 15s 连接 + 600s 总超时；流式写入文件并限制 2GiB | software.rs |
| M3 | force_uninstall 关键词杀进程过杀（"idea" 命中无关进程） | 收紧为：精确匹配 / 关键词+版本数字后缀 / 长关键词（≥4）完整单词匹配 | software.rs |
| L1 | 包管理器循环中 `?` 中断后续管理器尝试 | 改为记录 last_error 并 continue | software_pm.rs |
| L2 | list_installed_apps 单图标 base64 上限 2MB 过大 | 收紧到 512KB | software.rs |

## 二、前端迁移（Svelte 5 → Vue 3 + Arco Design）

### 技术栈

- Vue 3（Composition API + `<script setup>`）
- vue-router 4（hash 模式，保持现有 hash 导航习惯）
- @arco-design/web-vue（含 Arco Icons）
- 移除 Svelte / Tailwind / postcss（Arco 自带设计体系，避免双体系冲突）

### 文件结构

```
src/
  main.js            # Vue 入口：Arco + 主题 + i18n 就绪后挂载
  App.vue            # 应用壳：TitleBar + Sidebar + RouterView + Toast/Confirm 宿主
  router.js          # hash 路由表（12 个视图）
  styles/app.css     # Arco 主题变量覆盖（暗色为主 + 亮色） + 少量自定义样式
  lib/
    i18n.js          # 语言加载回退链（zh→en）+ t/tFormat
    toast.js         # 通知队列（Arco Message 之上封装，保留原 API）
    confirm.js       # 确认对话框 Promise 封装（Arco Modal.confirm）
    error.js         # 错误捕获 + toast 通知
    errors.js        # 后端错误文案 → i18n key 映射
    stores.js        # 主题等轻量全局状态
  components/
    TitleBar.vue     # 自定义标题栏（拖拽区域 + 窗口控制 + 主题/语言切换）
    Sidebar.vue      # Arco Menu 主导航 + CPU/MEM 状态 + 版本
    ToastHost.vue / ConfirmHost.vue / ErrorBoundary.vue
    VaultDialog.vue  # 密码库解锁对话框
    skeleton/*.vue   # Arco Skeleton 变体
    containers/*.vue # 容器管理 Tab 组件
    hub/*.vue        # Provider 表单 / 模型列表
  views/             # 12 个路由视图（逐一迁移）
  locales/zh.json en.json ru.json   # 复用现有语言包
```

### 设计原则

1. **Arco 原生组件优先**：按钮/表格/弹窗/表单/菜单/标签/进度条全部用 Arco 组件，
   不再手写类名体系（废弃 nx-* / tailwind）。
2. **暗色为主**：`body[arco-theme="dark"]`，跟随现有 `devnexus-theme` 偏好（默认 dark），
   Settings 中可切换。
3. **品牌色**：主色映射 Arco `--primary-6` 为现有 accent `#4099FF`（暗色）/ `#0B7FFF`（亮色）。
4. **Tauri 命令面不变**：`invoke()` 调用与后端命令一一对应，后端零改动。
5. **i18n 不变**：复用现有 locales JSON 与回退链，仅将 Svelte 响应式改为 Vue `ref`/`reactive`。
6. **深色标题栏 / 无边框窗口**：保留 `decorations: false`，TitleBar 实现拖拽与窗口控制。

### 迁移顺序

1. 脚手架与配置（package.json / vite / index.html / main.js / App.vue / router / lib）
2. 共享组件（TitleBar / Sidebar / Toast / Confirm / ErrorBoundary / VaultDialog）
3. 视图（按复杂度递增：Dashboard → Settings → MirrorSettings → SoftwareCenter →
   EnvironmentManager → ProcessManager → Migration → PasswordManager → CookieExtractor →
   AppUninstaller → ApiHub → ContainerManager）

### 测试策略

- 后端：`cargo test`（159 用例）+ `cargo clippy` + `cargo fmt`
- 前端：`pnpm build`（vite build 成功即通过）+ `pnpm install` 更新 lockfile
- 人工验证：`pnpm tauri dev` 检查各页渲染与 Tauri invoke

## 三、范围外（本次不做）

- Windows/macOS 图标解析（resolve_app_icon 仅 Linux）
- API Hub 增加 SSE 并发限制（已由 10MB body limit 覆盖）
- 单元测试逐视图补齐（后端已有，前端按需）
