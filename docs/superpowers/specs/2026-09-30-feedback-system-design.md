# 消息提示与反馈体系改造设计

日期：2026-09-30
状态：已确认（方案 A 渐进式 + 兼容层；完整反馈层范围；全栈结构化错误码）
范围：本设计为四个子项目中的第一个（消息提示）。后续子项目依次为 Docker、软件删除、系统调优，各自独立出设计与计划。

## 1. 背景与问题

当前反馈体系由四条并行通道组成：toast（vue-sonner）、确认框（confirm.js + ConfirmDialog）、sudo 密码框（sudo.js + SudoDialog）、错误兜底（error.js + ErrorBoundary）。存在以下已核实的问题：

- ErrorBoundary.vue 三处路径每个错误弹出两条重复 toast；window error 事件取 `event.loc`（不存在，应为 filename/lineno/colno）导致详情永远缺失。
- confirm.js 与 sudo.js 为单槽位设计：并发调用时前者 Promise 永不 resolve，调用方按钮永久卡死。
- toast.js、confirm.js、error.js 共约 10 处文案硬编码中文/英文，绕过 i18n。
- 后端全部命令返回 `Result<T, String>` 英文裸字符串；前端 errors.js 仅 3 条模式映射，绝大多数错误英文原样透出。`DevNexusError` 枚举（utils/error.rs）已定义但全仓零使用。
- toast 未设置 offset，压在 36px 自定义标题栏上；无同屏数量上限；"最后一个 toast 兜底"的复制绑定可能把 A 消息的复制行为绑到 B 上。
- EnvironmentManager.vue 5 处错误漏传 "error" 类型（显示为灰色 info）；Settings.vue 更新失败、MirrorSettings.vue 延迟测试失败仅 console 静默；SoftwareCenter.vue 辅助加载失败静默清空。
- 长耗时操作（卸载、拉镜像、SSH 传输）无进行中反馈通道。

## 2. 错误码协议（后端）

`src-tauri/src/utils/error.rs` 中 `DevNexusError` 由 5 变体枚举改为结构化对象：

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DevNexusError {
    pub code: String,                     // 稳定错误码，如 "DOCKER_NOT_RUNNING"
    pub params: HashMap<String, String>,  // i18n 插值参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,           // 原始细节（如 docker stderr），供日志与详情展示
}
```

- 构造器：`DevNexusError::new(code)`、`.param(k, v)`、`.detail(s)`，以及常用快捷构造 `invalid_input` / `not_found` / `permission` / `internal` / `cancelled`（映射 COMMON_ 前缀码）。
- 实现 `From<String>`（映射为 COMMON_INTERNAL + detail），使既有 `Result<_, String>` 内部逻辑迁移时 `?` 可用。
- 命令签名从 `Result<T, String>` 改为 `Result<T, DevNexusError>`，Tauri 2 将错误对象直接序列化给前端 reject。
- 错误码按域前缀划分：COMMON_ / DOCKER_ / UNINSTALL_ / SSH_ / TUNE_ / ENV_。码表集中记录于 `docs/modules/error-codes.md`。

## 3. 前端解析与兼容层（src/lib/errors.js）

`friendlyError` 升级为三段式：

1. 错误为带 `code` 字段的对象（结构化错误）：查 i18n 键 `errors.<code>`，用 `params` 插值（`{name}` 语法）；键缺失则回退 `detail`，再回退 code 本身。
2. 错误为字符串（未迁移命令）：走既有模式匹配表（随模块迁移逐步扩充）。
3. 其余：回退 `errors.unknown` 或原始消息。

新增导出 `isStructuredError(err)` 供门面判断。i18n 的 `t()` 缺键时返回键名本身，以此判定翻译缺失。

## 4. 统一门面（src/lib/feedback.js）

视图层只允许经门面调用反馈，四通道收口：

```js
feedback.toast.success(keyOrText, params)  // i18n 键优先，键缺失时按原文案处理
feedback.toast.error(err)                  // 接受结构化错误对象或字符串，内部走 friendlyError
feedback.toast.info / warning
feedback.confirm.danger({ titleKey, descKey, okKey })  // Promise<boolean>
feedback.sudo.prompt({ reasonKey })        // Promise<string|null>
feedback.loading.wrap(fn)                  // 统一 try/catch，失败自动 toast 后 rethrow
```

- 文案辅助 `tf(keyOrText, params)`：`t(key)` 结果与键名相同视为缺键，回退原文案并插值。
- confirm.js 与 sudo.js 改为队列化：并发请求按序展示，每次 resolve 后自动推进队列，根治 Promise 悬挂。ConfirmDialog / SudoDialog 作为薄消费层无需改动队列逻辑。
- sudo 空输入与取消区分：SudoDialog 空密码提交时显示错误提示而不 resolve；取消按钮与 Esc 返回 null。resolve 后立即清空输入框引用中的密码（安全敏感路径：密码驻留内存最小化）。

## 5. 错误分级策略

- 致命（Vue 渲染错误、未捕获异常）：ErrorBoundary 全屏遮罩（保留现状），每次错误仅一条 toast。
- 操作失败（命令返回错误码）：红色 error toast（经 friendlyError 翻译）。
- 局部失败（表单校验、辅助数据加载）：页内 Alert / 空态，不弹遮罩；但失败必须有可见反馈，禁止静默 console。
- `COMMON_PERMISSION_DENIED` 类错误统一显示"需要授权"语义文案。

## 6. 全局修复清单

- App.vue 的 Sonner 增加 `offset`（顶部 44px，避开 36px 标题栏）与 `:visible-toasts="5"`；移除全局 `close-button`（toast.js 已按条设置）。
- ErrorBoundary.vue：单一状态源（直接使用 error.js 导出的 ref，不再复制快照）；三处重复 toast 去除（统一由 captureError 弹一次）；`event.loc` 改为 `filename:lineno:colno`。
- toast.js：硬编码文案改 i18n 键；移除"最后一个 toast"兜底绑定（仅当按 id 定位到元素时才绑复制）。
- 静默失败修复：Settings.vue 更新失败、MirrorSettings.vue 两处、SoftwareCenter.vue 辅助加载失败改为 error toast；Dashboard.vue 的有意静默保留。

## 7. 进度与长任务反馈（本设计内的最小实现）

- 本阶段先落地 `feedback.loading.wrap`（按钮级 loading + 失败自动 toast）。
- 结构化进度事件（Tauri Channel 推阶段进度）属于卸载与 Docker 子项目范围：届时为 `force_uninstall_software`（已有 9 阶段）、compose up/down、docker pull/build 接入，本设计不展开。

## 8. 试点迁移：container.rs

以 container 模块（18 个命令）作为结构化错误的首个迁移对象，验证"后端 code -> 前端翻译"全链路：

- 校验类：DOCKER_INVALID_ID / DOCKER_INVALID_NAME / DOCKER_INVALID_ACTION / DOCKER_EXEC_FORBIDDEN / DOCKER_INVALID_IMAGE / DOCKER_INVALID_TAG / DOCKER_INVALID_PATH / COMPOSE_FILE_INVALID / COMPOSE_INVALID_PROJECT。
- 执行类：DOCKER_NOT_INSTALLED / DOCKER_NOT_RUNNING / DOCKER_COMMAND_FAILED（params: op，detail: stderr）。
- check_docker 区分"未安装"与"未运行"两种码。
- 前端 locales 三语言补齐 `errors.<code>` 文案。
- 容器模块自身的行为缺陷（同步阻塞主线程、compose 超时、校验不对等）不属于本设计，在 Docker 子项目中处理。

## 9. 测试与验收

- Rust：error.rs 新增单元测试（序列化形态、params、detail 跳过、From<String>）；`cargo test --manifest-path src-tauri/Cargo.toml` 全绿；`cargo clippy --all-targets -- -D warnings` 全绿。
- 前端无单元测试框架（package.json 无 vitest），验证方式：`pnpm check`（vite build）通过 + 手动验收清单：
  1. zh/en/ru 三语言下弹出错误 toast 文案正确，无硬编码混排。
  2. 并发触发两个确认框：先弹 A 后弹 B，两者都能正确返回。
  3. sudo 框空输入提交显示提示而非静默关闭；取消返回 null。
  4. 错误遮罩只出现一条 toast；详情区显示 filename:lineno。
  5. toast 不遮挡标题栏；连续报错同屏不超过 5 条。
  6. Docker 未启动时容器页错误提示为本地化文案（试点迁移验证）。

## 10. 实施阶段

1. 基础设施：error.rs 结构化对象 + errors.js 三段式 + confirm/sudo 队列化 + toast.js 修复 + feedback.js 门面 + ErrorBoundary/App.vue 全局修复 + i18n 键。
2. 静默失败与类型修正：EnvironmentManager、Settings、MirrorSettings、SoftwareCenter 接入门面。
3. 试点迁移：container.rs 全量迁移 + 码表文档。
4. 收尾：docs/modules 文档更新、安全扫描（Opsera）与遥测报告。

## 11. 安全与合规

- SudoDialog 密码处理为安全敏感路径：resolve 后立即清空输入引用，密码不写入任何持久层，不进日志。
- 错误 detail 可能包含主机路径等敏感信息：仅用于详情展开与日志，不在默认文案中展示。
- 提交前运行 Opsera 安全扫描并报告遥测。
