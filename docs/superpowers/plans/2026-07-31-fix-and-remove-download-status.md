# DevNexus 全面修复与移除下载功能 — 销号清单

> 对应计划：`docs/superpowers/plans/2026-07-31-fix-and-remove-download.md`
> 执行日期：2026-07-31 ｜ 基线：`382f97d`（WIP commit）→ `HEAD` 共 **41 个提交**
> 净代码变化：89 files, +4864 / -12955（约 -8091 行，主要来自下载模块移除）

---

## Phase 0：移除下载功能 ✅

| 任务 | 提交 | 状态 |
|---|---|---|
| 后端下载模块（download/ 8 文件 + download_manager.rs + lib.rs/Cargo.toml 清理） | `e19d087` | ✅ |
| 前端页面与死代码组件（DownloadManager.svelte + downloads/* + locales） | `f8fb8dd` | ✅ |
| 文档与 CHANGELOG 更新（README 三语言、docs/、CHANGELOG） | `55c8d34` | ✅ |
| 收尾验证（cargo check/test + svelte-check + 残留引用检查） | — | ✅ 无残留 |

## Phase 1：安全修复 S1–S10 ✅

| 编号 | 问题 | 提交 | 状态 |
|---|---|---|---|
| S1 | nvm 版本切换命令注入 | `f9e3c42` | ✅ 字符白名单校验 + 回归测试 |
| S2 | API Hub CORS 全开（Any） | `3c63a57` | ✅ 白名单化 + 恶意 Origin 测试 |
| S3 | Cookie 完整性哈希从不比对 | `f7256f7` | ✅ Result 化 + constant 比对 + 测试 |
| S4 | shell rc 注入（mirror/environment） | `2d9c994` | ✅ validate_rc_value 统一校验 |
| S5 | 软件安装路径穿越 | `6babd0c` | ✅ is_valid_version 校验 |
| S6 | 残留扫描关键词误删 | `9795466` | ✅ 边界匹配 + is_safe_to_delete 保护 |
| S7 | keyring 无平台后端（密码持久化失效） | `781780d` | ✅ 三平台 feature + 失败告警 |
| S8 | PBKDF2 迭代数无上限 + 密钥不清理 | `7665fb3` | ✅ 区间校验 + lock 清零 + unlock 恢复 |
| S9 | docker 命令无白名单/超时 | `52998e7` | ✅ 白名单 + 参数校验 + 120s 超时 |
| S10 | Cookie 临时文件 0644/固定名/残留 | `50a46e4` | ✅ 随机名 + 0600 + RAII 清理 |

## Phase 2：正确性修复 C1–C9 ✅

| 编号 | 问题 | 提交 | 状态 |
|---|---|---|---|
| C1 | usage 统计 INSERT/UPDATE 竞态 | `c3e4b33` | ✅ await 串行化 + e2e 断言 token 回填 |
| C2 | Provider 重名检查在 INSERT 后 | `dfc733b` | ✅ 前置检查 + DB 唯一索引 |
| C3 | 协议转换方向静默降级 | `14cbe56` | ✅ 显式 422 报错 |
| C4 | 流式重复 message_delta/message_stop | `979579a` | ✅ stop_sent 状态位去重 |
| C5 | 错误码 500 当 400 + 启动 expect panic | `584fd29` | ✅ 客户端错误 400/422 + unwrap_or_else 降级 |
| C6 | update_provider 空 key 覆盖 / 不存在 id 静默 | `aa908cd` | ✅ 空 key 保留原值 + not found 报错 |
| C7 | 系统/进程锁内跑子进程 + unwrap | `c5799a6` | ✅ 临界区缩小 + poison 防护 |
| C8 | join_path 无边界子串匹配 | `1775079` | ✅ 按路径段边界去重 |
| C9 | fetch_models 每次新建 HTTP client | `09ee447` | ✅ 复用全局连接池 |

## Phase 3：架构分层 A1–A5 ✅

| 编号 | 内容 | 提交 | 状态 |
|---|---|---|---|
| A1 | 统一命令执行器 pm_exec（utils/exec.rs） | `d7af286` | ✅ container/version_manager 迁移 |
| A2 | shell rc 编辑统一 rc_editor（utils/rc_editor.rs） | `a0c139c` | ✅ mirror/environment 迁移 |
| A3 | 残留路径表单一数据源（known_paths 严格超集） | `6d0c6a4` | ✅ software.rs 删 233 行副本 |
| A4 | DevNexusError 统一错误类型（utils/error.rs） | `13c6ca2` | ✅ 三处信号错误修复 |
| A5 | 巨型文件拆分 | `4d6495c` `9e27301` `84d5810` `ed3ed40` | ✅ mirror 1184→658、cookie 1351→794、software 2083→1324 |

## Phase 4：前端重构 F1–F6 ✅

| 编号 | 内容 | 提交 | 状态 |
|---|---|---|---|
| F1 | ApiHub 完整 i18n（86 键 × 3 语言） | `13e0733` | ✅ |
| F2 | 巨型组件拆分（ProviderForm/ModelList/ContainerDialog/VaultDialog） | `88b923f` | ✅ ApiHub 669→419 行 |
| F3 | 错误归一化层（errors.svelte.js，12 路由替换） | `9a2873c` | ✅ |
| F4 | searchQuery 私有化 + ErrorBoundary 响应式 | `679944c` | ✅ |
| F5 | ConfirmDialog 渲染标题 + 数组 $derived | `98fad46` | ✅ |
| F6 | 其余页面硬编码文案清理（15 键 × 3） | `65d9c08` | ✅ 删除 replace 抠数字 |

## Phase 5：工程与 CI E1–E5 ✅

| 编号 | 内容 | 提交 | 状态 |
|---|---|---|---|
| E1 | 统一 TLS 栈为 rustls | `83f7644` | ✅ native-tls 从依赖树移除 |
| E2 | gitignore 补全 + 删过期 src-tauri/Cargo.lock | `6f8284e` | ✅ |
| E3 | CI 密钥条件注入 + 缓存 + release 清理过滤 | `b4edc98` | ✅ YAML 校验通过 |
| E4 | withGlobalTauri 关闭 + shell open scope 收紧 | `05e37f6` | ✅ |
| E5 | packageManager/engines 锁定 + esbuild 构建批准 | `978fa88` | ✅ pnpm 9/11 均验证 |

---

## 最终验证（端到端）

- `cargo check --manifest-path src-tauri/Cargo.toml` ✅
- `cargo test --manifest-path src-tauri/Cargo.toml` ✅ **125 passed, 0 failed**（基线 162 → 移除下载 97 → 新增回归测试后 125）
- `./node_modules/.bin/svelte-check` ✅ **0 errors, 0 warnings**（基线 23 warnings 全部消除）
- 前端残留硬编码中文检查 ✅ 仅注释
- `.qoder/`、`.agent-cache/` 已入 gitignore ✅

## 显式延后项（已记录，非遗漏）

1. **api_key 明文存 SQLite**（C6 已完成前端打码，加密存储涉及新依赖，延后）
2. **PBKDF2 默认迭代数 10 万 → 60 万**（S8 已加区间上限保护，默认值提升为低优项，文件格式兼容）
3. **Linux arm64 CI runner**（需公共预览 runner，矩阵已覆盖三平台 x86_64 + macOS aarch64）
4. **workflow_dispatch 手动发布的 updater 签名**（当前按 event 条件注入密钥）
