# 消息提示与反馈体系改造实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立结构化错误码协议与统一反馈门面，修复消息提示体系全部已核实缺陷，并以 container 模块完成首个错误迁移试点。

**Architecture:** 后端 `DevNexusError` 结构化对象（code/params/detail）作为命令错误类型；前端 `friendlyError` 三段式解析（code 对象 → 旧字符串模式 → 未知），新旧共存兼容；`feedback.js` 单一门面收口四通道（toast/confirm/sudo/loading），confirm/sudo 队列化根治并发 Promise 悬挂。

**Tech Stack:** Tauri 2 + serde（后端）；Vue 3 + vue-sonner + shadcn 风格组件（前端）；locales/{zh,en,ru}.json 三语言。

## Global Constraints

- 提交信息：中文、Conventional Commits、禁止 emoji 与 Unicode 修饰符号、不加其他协作者。
- 代码注释：禁止 emoji 与 Unicode 修饰符号。
- UI：禁止渐变色与 hover 效果；沿用现有 shadcn 语义色 token（destructive/muted 等）。
- 每个任务提交前必须通过：`cargo fmt --check`、`cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`、`pnpm check`（涉及前端的任务）。
- 仓库无前端单元测试框架：前端行为验证用 `pnpm check` + 计划末尾的手动验收清单。
- pre-commit 钩子会自动跑 fmt/clippy/vite build，若被钩子拦截先 `cargo fmt` 再重试。

---

### Task 1: 后端结构化错误对象 DevNexusError

**Files:**
- Modify: `src-tauri/src/utils/error.rs`（整文件重写）

**Interfaces:**
- Produces: `DevNexusError { code: String, params: HashMap<String,String>, detail: Option<String> }`，构造器 `new(code)` / `.param(k,v)` / `.detail(s)` / `invalid_input` / `not_found` / `permission` / `internal` / `cancelled`；`impl From<String>`（COMMON_INTERNAL）。后续任务（Task 11）依赖此类型作为命令错误类型。

- [ ] **Step 1: 重写 error.rs（实现 + 单测）**

```rust
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;

/// 统一结构化错误：code 为稳定错误码（前端按 errors.<code> 翻译），
/// params 为 i18n 插值参数，detail 为原始细节（仅日志与详情展示，不直接面向用户）。
#[derive(Debug, Clone, Serialize)]
pub struct DevNexusError {
    pub code: String,
    pub params: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl DevNexusError {
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into(), params: HashMap::new(), detail: None }
    }

    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail);
        self
    }

    pub fn invalid_input(detail: impl Into<String>) -> Self {
        Self::new("COMMON_INVALID_INPUT").detail(detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new("COMMON_NOT_FOUND").detail(detail)
    }

    pub fn permission(detail: impl Into<String>) -> Self {
        Self::new("COMMON_PERMISSION_DENIED").detail(detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new("COMMON_INTERNAL").detail(detail)
    }

    pub fn cancelled() -> Self {
        Self::new("COMMON_CANCELLED")
    }
}

impl fmt::Display for DevNexusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.detail, self.params.get("op")) {
            (Some(d), _) => write!(f, "{}: {}", self.code, d),
            (None, Some(op)) => write!(f, "{} ({})", self.code, op),
            _ => write!(f, "{}", self.code),
        }
    }
}

impl std::error::Error for DevNexusError {}

impl From<String> for DevNexusError {
    fn from(s: String) -> Self {
        DevNexusError::internal(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialized_shape() {
        let e = DevNexusError::new("DOCKER_NOT_RUNNING").detail("daemon down");
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert_eq!(v["code"], "DOCKER_NOT_RUNNING");
        assert_eq!(v["detail"], "daemon down");
        assert!(v["params"].is_object());
    }

    #[test]
    fn test_detail_skipped_when_none() {
        let e = DevNexusError::new("COMMON_CANCELLED");
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert!(v.get("detail").is_none());
        assert_eq!(v["code"], "COMMON_CANCELLED");
    }

    #[test]
    fn test_params_interpolation_input() {
        let e = DevNexusError::new("DOCKER_INVALID_ACTION").param("action", "rename");
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert_eq!(v["params"]["action"], "rename");
    }

    #[test]
    fn test_from_string_maps_to_internal() {
        let e: DevNexusError = String::from("boom").into();
        assert_eq!(e.code, "COMMON_INTERNAL");
        assert_eq!(e.detail.as_deref(), Some("boom"));
    }

    #[test]
    fn test_display_includes_detail() {
        let e = DevNexusError::new("X").detail("raw");
        assert_eq!(e.to_string(), "X: raw");
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml utils::error`
Expected: PASS（5 个新测试）

- [ ] **Step 3: 全量校验并提交**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`

```bash
git add src-tauri/src/utils/error.rs
git commit -m "feat(feedback): 后端结构化错误对象 DevNexusError（code/params/detail）"
```

---

### Task 2: 前端 errors.js 三段式解析

**Files:**
- Modify: `src/lib/errors.js`（整文件重写）

**Interfaces:**
- Produces: `friendlyError(err)`（签名不变，行为升级）、`isStructuredError(err)`、`interpolate(text, params)`、`LEGACY_PATTERNS`（导出供后续模块迁移扩充）。Task 6 门面依赖 `friendlyError` 与 `isStructuredError`。

- [ ] **Step 1: 重写 errors.js**

```js
// src/lib/errors.js — 后端错误 -> 本地化文案
// 三段式：结构化错误对象（{code, params, detail}）优先；其次旧字符串模式匹配；最后通用回退。
import { t } from "./i18n.js";

/** 旧版字符串错误模式（未迁移命令的过渡兼容，随模块迁移逐步缩减） */
export const LEGACY_PATTERNS = {
  "Provider already exists": "errors.provider_exists",
  "already exists": "errors.already_exists",
  "Invalid version string": "errors.invalid_version",
};

/** 判断 invoke 的 reject 值是否为结构化错误对象 */
export function isStructuredError(err) {
  return !!err && typeof err === "object" && typeof err.code === "string";
}

/** {name} 风格插值 */
export function interpolate(text, params) {
  if (!params) return text;
  return String(text).replace(/\{(\w+)\}/g, (m, k) =>
    Object.prototype.hasOwnProperty.call(params, k) ? String(params[k]) : m
  );
}

/** 把 invoke 抛出的错误（结构化对象或字符串）转为用户可读的本地化文案 */
export function friendlyError(err) {
  if (isStructuredError(err)) {
    const key = `errors.${err.code}`;
    const text = t(key);
    if (text && text !== key) return interpolate(text, err.params);
    if (err.detail) return err.detail;
    return err.code;
  }
  const msg = typeof err === "string" ? err : err?.message || String(err);
  for (const [pattern, key] of Object.entries(LEGACY_PATTERNS)) {
    if (msg.includes(pattern)) return t(key);
  }
  return msg;
}
```

- [ ] **Step 2: 校验并提交**

Run: `pnpm check`

```bash
git add src/lib/errors.js
git commit -m "feat(feedback): friendlyError 三段式解析，支持结构化错误码对象"
```

---

### Task 3: confirm.js 队列化

**Files:**
- Modify: `src/lib/confirm.js`（整文件重写）

**Interfaces:**
- Produces: `showConfirm(message, title?, opts?) => Promise<boolean>`（签名不变，并发排队）、`confirmResolve(result)`、`confirmState`（ref，结构不变：message/title/okText/cancelText/danger）。ConfirmDialog.vue 无需改动。

- [ ] **Step 1: 重写 confirm.js**

```js
// src/lib/confirm.js — 全局确认对话框（Promise API，基于 shadcn Dialog）
// 并发调用按序排队展示：每次 resolve 后自动推进下一个，先前的 Promise 不再被覆盖悬挂。
import { ref } from "vue";
import { t } from "./i18n.js";

export const confirmState = ref(null);

const confirmQueue = [];

function advanceConfirmQueue() {
  const next = confirmQueue.shift();
  if (next) {
    confirmState.value = next;
  } else {
    confirmState.value = null;
  }
}

/**
 * 弹出确认对话框，返回 Promise<boolean>
 * @param {string} message 提示文案
 * @param {string} [title] 标题（默认取 common.confirm）
 * @param {object} [opts] 额外选项：{ okText, cancelText, danger }
 */
export function showConfirm(message, title, opts = {}) {
  return new Promise((resolve) => {
    confirmQueue.push({
      message,
      title: title || t("common.confirm"),
      okText: opts.okText || t("common.confirm"),
      cancelText: opts.cancelText || t("common.cancel"),
      danger: !!opts.danger,
      resolve,
    });
    if (confirmState.value === null) advanceConfirmQueue();
  });
}

/** 由 ConfirmDialog 调用，结算当前确认框并推进队列 */
export function confirmResolve(result) {
  const state = confirmState.value;
  if (state) state.resolve(result);
  advanceConfirmQueue();
}
```

- [ ] **Step 2: 校验并提交**

Run: `pnpm check`

```bash
git add src/lib/confirm.js
git commit -m "fix(feedback): 确认框队列化，根治并发调用 Promise 悬挂"
```

---

### Task 4: sudo.js 队列化 + SudoDialog 空输入区分与密码清理

**Files:**
- Modify: `src/lib/sudo.js`（整文件重写）
- Modify: `src/components/SudoDialog.vue`（onOk/onCancel/watch 部分）

**Interfaces:**
- Produces: `promptSudo(message) => Promise<string|null>`（签名不变，并发排队；null 仅表示用户取消）、`sudoResolve(result)`、`sudoRejectCurrent(message)`（向当前对话框展示校验错误）、`sudoState`（新增 `error` 字段）。安全敏感路径：resolve 后清空输入引用。

- [ ] **Step 1: 重写 sudo.js**

```js
// src/lib/sudo.js — 全局 sudo 密码输入对话框（Promise API）
// 并发调用按序排队；null 仅表示用户主动取消；空输入由对话框内校验，不 resolve。
// 安全敏感路径：密码仅存活于输入框引用，resolve 后立即清空，不写日志与持久层。
import { ref } from "vue";

export const sudoState = ref(null);

const sudoQueue = [];

function advanceSudoQueue() {
  const next = sudoQueue.shift();
  if (next) {
    sudoState.value = next;
  } else {
    sudoState.value = null;
  }
}

/**
 * @param {string} message 展示给用户的说明文案
 * @returns {Promise<string|null>} 密码；用户取消返回 null
 */
export function promptSudo(message) {
  return new Promise((resolve) => {
    sudoQueue.push({ message, resolve, error: "" });
    if (sudoState.value === null) advanceSudoQueue();
  });
}

/** 向当前对话框展示校验错误（如后端验证失败），用户可重试或取消 */
export function sudoRejectCurrent(message) {
  const s = sudoState.value;
  if (s) s.error = message;
}

/** 结算当前对话框并推进队列；调用后由 SudoDialog 清空输入引用 */
export function sudoResolve(result) {
  const s = sudoState.value;
  if (s) s.resolve(result);
  advanceSudoQueue();
}
```

- [ ] **Step 2: 修改 SudoDialog.vue 脚本段**

将 `onOk`/`onCancel`/`watch` 替换为：

```js
function onOk() {
  const pw = input.value;
  if (!pw) {
    if (sudoState.value) sudoState.value.error = t("sudo.empty_password");
    return;
  }
  sudoResolve(pw);
  input.value = "";
}
function onCancel() {
  sudoResolve(null);
  input.value = "";
}
```

watch 中同步清空错误：`(v) => { input.value = ""; }` 保持，另在模板 `<Input>` 下方加入错误提示（放在现有 hint 的 `<p>` 之后）：

```html
<p v-if="sudoState?.error" class="text-[11px] text-destructive">{{ sudoState.error }}</p>
```

- [ ] **Step 3: 校验并提交**

Run: `pnpm check`

```bash
git add src/lib/sudo.js src/components/SudoDialog.vue
git commit -m "fix(feedback): sudo 框队列化、空输入校验、取消语义区分与密码即时清理"
```

---

### Task 5: toast.js 文案 i18n 化与复制绑定修复

**Files:**
- Modify: `src/lib/toast.js`

**Interfaces:**
- Produces: `showToast(message, type?, duration?)` 签名不变。依赖 Task 9 的 locale 键：`common.toast.copied` / `common.toast.click_to_copy` / `common.toast.copy`。

- [ ] **Step 1: 修改 toast.js**

1. 文件头补 `import { t } from "./i18n.js";`
2. 两处 `toast.success("已复制", ...)` 改为 `toast.success(t("common.toast.copied"), ...)`
3. `el.title = "点击复制报错"` 改为 `el.title = t("common.toast.click_to_copy")`
4. `description: "点击任意处复制报错 · 右侧“复制”亦可"` 改为 `description: t("common.toast.click_to_copy")`；`label: "复制"` 改为 `label: t("common.toast.copy")`
5. `attachClickCopy` 中删除"最后一个 toast 兜底"分支：

```js
    if (!el) {
      const all = document.querySelectorAll("[data-sonner-toast]");
      el = all.length ? all[all.length - 1] : null;
    }
```

改为

```js
    // 仅按 id 精确绑定：兜底"最后一个 toast"会把复制行为绑到别的消息上
    if (!el) return;
```

（同时删除其后的 `if (!el || el.dataset.copyBound === "1") return;` 中对 `!el` 的判断，保留 copyBound 判断，即改为 `if (el.dataset.copyBound === "1") return;`）

- [ ] **Step 2: 校验并提交**

Run: `pnpm check`

```bash
git add src/lib/toast.js
git commit -m "fix(feedback): toast 文案接入 i18n，复制绑定仅按 id 精确生效"
```

---

### Task 6: 统一门面 feedback.js

**Files:**
- Create: `src/lib/feedback.js`

**Interfaces:**
- Consumes: Task 2 `friendlyError/isStructuredError`、Task 3 `showConfirm`、Task 4 `promptSudo`、Task 5 `showToast`、`t/tFormat`（i18n.js）。
- Produces: `feedback.toast.success/info/warning/error`、`feedback.confirm.danger({titleKey, descKey, okKey, cancelKey})`、`feedback.sudo.prompt({reasonKey})`、`feedback.loading.wrap(fn)`、内部 `tf(keyOrText, params)`。Task 10 与后续视图接入依赖。

- [ ] **Step 1: 创建 feedback.js**

```js
// src/lib/feedback.js — 统一用户反馈门面
// 视图层反馈统一走这里：toast/confirm/sudo/loading。
// 文案约定：优先传 i18n 键（支持 {name} 插值）；键缺失时自动按原文案处理。
import { showToast } from "./toast.js";
import { showConfirm } from "./confirm.js";
import { promptSudo } from "./sudo.js";
import { friendlyError } from "./errors.js";
import { t } from "./i18n.js";

/** i18n 键优先的文案解析：t(key) 与键名相同视为缺键，回退原文案并做插值 */
function tf(keyOrText, params) {
  if (typeof keyOrText !== "string") return String(keyOrText ?? "");
  const translated = t(keyOrText);
  const text = translated && translated !== keyOrText ? translated : keyOrText;
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (m, k) =>
    Object.prototype.hasOwnProperty.call(params, k) ? String(params[k]) : m
  );
}

export const feedback = {
  toast: {
    success: (keyOrText, params) => showToast(tf(keyOrText, params), "success"),
    info: (keyOrText, params) => showToast(tf(keyOrText, params), "info"),
    warning: (keyOrText, params) => showToast(tf(keyOrText, params), "warning"),
    /** err 可为结构化错误对象（{code,...}）或字符串，内部走 friendlyError */
    error: (err) => showToast(friendlyError(err), "error"),
  },
  confirm: {
    /** 危险操作确认：{ titleKey, descKey, okKey?, cancelKey? } => Promise<boolean> */
    danger(opts) {
      return showConfirm(t(opts.descKey), t(opts.titleKey), {
        danger: true,
        okText: opts.okKey ? t(opts.okKey) : undefined,
        cancelText: opts.cancelKey ? t(opts.cancelKey) : undefined,
      });
    },
  },
  sudo: {
    /** { reasonKey } => Promise<string|null>，null 表示用户取消 */
    prompt(opts) {
      return promptSudo(t(opts.reasonKey));
    },
  },
  loading: {
    /** 统一长任务包装：失败自动弹错误 toast 后 rethrow，调用方可再捕获做局部处理 */
    async wrap(fn) {
      try {
        return await fn();
      } catch (err) {
        feedback.toast.error(err);
        throw err;
      }
    },
  },
};
```

- [ ] **Step 2: 校验并提交**

Run: `pnpm check`

```bash
git add src/lib/feedback.js
git commit -m "feat(feedback): 新增统一反馈门面 feedback.js（toast/confirm/sudo/loading）"
```

---

### Task 7: error.js + ErrorBoundary.vue 单一状态源与去重

**Files:**
- Modify: `src/lib/error.js`（整文件重写）
- Modify: `src/components/ErrorBoundary.vue`（脚本段）

**Interfaces:**
- Consumes: Task 6 `feedback.toast.error`、locale 键 `common.error_msg`（已存在："错误：{error}"）。
- Produces: `captureError(err, source?)` 成为唯一入口（存储 + 单条 toast + console），`getError()` 返回响应式 ref 本体。

- [ ] **Step 1: 重写 error.js**

```js
// src/lib/error.js — 全局错误状态（单一状态源）
// ErrorBoundary 直接使用本模块导出的 ref，禁止复制快照导致双份真相。
import { ref } from "vue";
import { tFormat } from "./i18n.js";
import { feedback } from "./feedback.js";

const errorInfo = ref(null);

export function getError() {
  return errorInfo;
}

export function clearError() {
  errorInfo.value = null;
}

/**
 * 唯一错误捕获入口：写入状态、弹一条错误 toast、输出 console。
 * @param {unknown} err 错误对象或任意值
 * @param {string|null} source 错误来源描述（组件栈 / 文件:行号）
 */
export function captureError(err, source = null) {
  const message = err instanceof Error ? err.message : String(err);
  errorInfo.value = {
    message,
    stack: source,
    timestamp: Date.now(),
  };
  feedback.toast.error(tFormat("common.error_msg", { error: message }));
  console.error("[ErrorBoundary] Caught error:", err, source);
}
```

- [ ] **Step 2: 修改 ErrorBoundary.vue 脚本段**

1. `const errorInfo = ref(getError().value);` 改为 `const errorInfo = getError();`（并从 vue 导入中移除不再使用的 `ref`，保留其他）。
2. `onErrorCaptured` 回调整体替换为：

```js
onErrorCaptured((err, _instance, info) => {
  captureError(err, info);
  return false; // 阻止继续冒泡，避免全局 handler 重复处理
});
```

3. `handleError` 整体替换为：

```js
const handleError = (event) => {
  const err = event.error || new Error(String(event.error));
  const loc = [event.filename, event.lineno, event.colno].filter(Boolean).join(":");
  captureError(err, loc || null);
  event.preventDefault();
};
```

4. `handleRejection` 整体替换为：

```js
const handleRejection = (event) => {
  const err = event.reason || new Error("Unhandled promise rejection");
  captureError(err, null);
  event.preventDefault();
};
```

- [ ] **Step 3: 校验并提交**

Run: `pnpm check`

```bash
git add src/lib/error.js src/components/ErrorBoundary.vue
git commit -m "fix(feedback): 错误捕获单一状态源，去除重复 toast 并修复来源定位"
```

---

### Task 8: App.vue Sonner 全局配置

**Files:**
- Modify: `src/App.vue:57`

**Interfaces:**
- Consumes: 无。Toast 展示位置避开 36px 标题栏。

- [ ] **Step 1: 修改 Sonner 行**

```html
    <Sonner :theme="theme" position="top-center" rich-colors expand :visible-toasts="5" :offset="{ top: 44 }" :toast-options="{ duration: 3500 }" />
```

（移除全局 `close-button`；toast.js 已按条设置 `closeButton: true`。）

- [ ] **Step 2: 校验并提交**

Run: `pnpm check`

```bash
git add src/App.vue
git commit -m "fix(feedback): toast 避让标题栏并限制同屏数量"
```

---

### Task 9: 三语言补齐反馈相关键

**Files:**
- Modify: `src/locales/zh.json` / `src/locales/en.json` / `src/locales/ru.json`（`errors` 段与 `common` 段）

**Interfaces:**
- Produces: `errors.unknown`、`errors.COMMON_*`（5 个）、`errors.DOCKER_*`（8 个）、`errors.COMPOSE_*`（2 个）；`common.toast.copied` / `common.toast.click_to_copy` / `common.toast.copy`；`sudo.empty_password`。Task 4/5/10/11 依赖。

- [ ] **Step 1: zh.json 的 `errors` 段补齐**

在既有三个键后追加：

```json
    "unknown": "发生未知错误",
    "COMMON_INVALID_INPUT": "输入无效",
    "COMMON_NOT_FOUND": "未找到目标资源",
    "COMMON_PERMISSION_DENIED": "权限不足：此操作需要授权",
    "COMMON_INTERNAL": "发生内部错误",
    "COMMON_CANCELLED": "操作已取消",
    "DOCKER_NOT_INSTALLED": "未检测到 Docker，请先安装 Docker",
    "DOCKER_NOT_RUNNING": "Docker 未运行，请启动 Docker 后重试",
    "DOCKER_TIMEOUT": "Docker 操作超时，已终止命令",
    "DOCKER_COMMAND_FAILED": "Docker 操作失败",
    "DOCKER_INVALID_ID": "无效的容器 ID：{id}",
    "DOCKER_INVALID_ACTION": "不支持的操作：{action}",
    "DOCKER_EXEC_EMPTY": "请输入要在容器内执行的命令",
    "DOCKER_EXEC_FORBIDDEN": "命令包含被禁止的字符",
    "COMPOSE_NOT_INSTALLED": "未检测到 Docker Compose 插件"
```

- [ ] **Step 2: zh.json 的 `common` 段补 `toast` 子对象、`sudo` 段补键**

```json
    "toast": {
      "copied": "已复制",
      "click_to_copy": "点击任意处复制报错，右侧按钮亦可",
      "copy": "复制"
    },
```

`sudo` 段追加：`"empty_password": "请输入密码"`。

- [ ] **Step 3: en.json 对应英文**

```json
    "unknown": "Unknown error occurred",
    "COMMON_INVALID_INPUT": "Invalid input",
    "COMMON_NOT_FOUND": "Target resource not found",
    "COMMON_PERMISSION_DENIED": "Permission denied: authorization required",
    "COMMON_INTERNAL": "Internal error occurred",
    "COMMON_CANCELLED": "Operation cancelled",
    "DOCKER_NOT_INSTALLED": "Docker not found. Please install Docker first",
    "DOCKER_NOT_RUNNING": "Docker is not running. Please start Docker and retry",
    "DOCKER_TIMEOUT": "Docker operation timed out and was terminated",
    "DOCKER_COMMAND_FAILED": "Docker operation failed",
    "DOCKER_INVALID_ID": "Invalid container ID: {id}",
    "DOCKER_INVALID_ACTION": "Unsupported action: {action}",
    "DOCKER_EXEC_EMPTY": "Enter a command to execute in the container",
    "DOCKER_EXEC_FORBIDDEN": "Command contains forbidden characters",
    "COMPOSE_NOT_INSTALLED": "Docker Compose plugin not found"
```

common.toast: `"copied": "Copied"`, `"click_to_copy": "Click anywhere to copy the error, or use the button"`, `"copy": "Copy"`；sudo.empty_password: `"Please enter a password"`。

- [ ] **Step 4: ru.json 对应俄文**

```json
    "unknown": "Произошла неизвестная ошибка",
    "COMMON_INVALID_INPUT": "Неверный ввод",
    "COMMON_NOT_FOUND": "Целевой ресурс не найден",
    "COMMON_PERMISSION_DENIED": "Недостаточно прав: требуется авторизация",
    "COMMON_INTERNAL": "Внутренняя ошибка",
    "COMMON_CANCELLED": "Операция отменена",
    "DOCKER_NOT_INSTALLED": "Docker не найден. Сначала установите Docker",
    "DOCKER_NOT_RUNNING": "Docker не запущен. Запустите Docker и повторите",
    "DOCKER_TIMEOUT": "Операция Docker прервана по таймауту",
    "DOCKER_COMMAND_FAILED": "Операция Docker завершилась ошибкой",
    "DOCKER_INVALID_ID": "Недопустимый ID контейнера: {id}",
    "DOCKER_INVALID_ACTION": "Действие не поддерживается: {action}",
    "DOCKER_EXEC_EMPTY": "Введите команду для выполнения в контейнере",
    "DOCKER_EXEC_FORBIDDEN": "Команда содержит запрещённые символы",
    "COMPOSE_NOT_INSTALLED": "Плагин Docker Compose не найден"
```

common.toast: `"copied": "Скопировано"`, `"click_to_copy": "Нажмите в любом месте, чтобы скопировать ошибку, или кнопку справа"`, `"copy": "Копировать"`；sudo.empty_password: `"Введите пароль"`。

- [ ] **Step 5: 校验并提交**

Run: `pnpm check`

```bash
git add src/locales/zh.json src/locales/en.json src/locales/ru.json
git commit -m "feat(feedback): 三语言补齐错误码与反馈文案键"
```

---

### Task 10: 静默失败修复与门面接入

**Files:**
- Modify: `src/views/EnvironmentManager.vue`（约 118、167、179、190、216 行附近的 5 处漏传 "error" 类型的 toast）
- Modify: `src/views/Settings.vue`（约 107 行 `downloadAndInstall` 失败仅 console.error）
- Modify: `src/views/MirrorSettings.vue`（约 84、130 行延迟测试失败仅 console）
- Modify: `src/views/SoftwareCenter.vue`（约 129 行辅助加载失败静默清空）

**Interfaces:**
- Consumes: Task 6 `feedback`。

- [ ] **Step 1: EnvironmentManager.vue**

5 处 `showToast(<...>, "info"|缺省)` 中语义为失败的调用改为 `feedback.toast.error(<err>)`；错误对象直接传 `err`（经 friendlyError 翻译），不再手工拼 `{error}` 文案。保留 92 行等真正的 info 调用不变。

- [ ] **Step 2: Settings.vue**

`downloadAndInstall` 的 catch 中，在 console.error 后追加 `feedback.toast.error(err)`，并把进度条状态复位（将 loading/进度标记置回初始值，沿用该文件现有的状态变量）。

- [ ] **Step 3: MirrorSettings.vue**

两处 catch 中 console 后追加 `feedback.toast.error(err)`。

- [ ] **Step 4: SoftwareCenter.vue**

辅助数据加载失败处：静默清空数组前追加 `feedback.toast.error(err)`（失败与空列表要可区分）。

- [ ] **Step 5: 校验并提交**

Run: `pnpm check`

```bash
git add src/views/EnvironmentManager.vue src/views/Settings.vue src/views/MirrorSettings.vue src/views/SoftwareCenter.vue
git commit -m "fix(feedback): 修复静默失败与错误类型误标，统一经 feedback 门面"
```

---

### Task 11: container.rs 试点迁移到结构化错误

**Files:**
- Modify: `src-tauri/src/commands/container.rs`

**Interfaces:**
- Consumes: Task 1 `DevNexusError`。
- Produces: 全部命令签名 `Result<T, DevNexusError>`（check_docker 仍返回 DockerStatus）；`run_docker` 返回 `Result<(String,String), DevNexusError>` 并分类：spawn 失败含 "No such file or directory" → `DOCKER_NOT_INSTALLED`；含 "timed out" → `DOCKER_TIMEOUT`；非零退出且 stderr/stdout 含 daemon 连接失败特征 → `DOCKER_NOT_RUNNING`；其余 → `DOCKER_COMMAND_FAILED`。校验函数返回 `Result<(), DevNexusError>`（现有测试的 `is_ok/is_err` 断言无需改动）。

- [ ] **Step 1: 修改 helpers**

文件头新增 `use crate::utils::error::DevNexusError;`。`validate_container_id` 改为：

```rust
fn validate_container_id(id: &str) -> Result<(), DevNexusError> {
    if id.is_empty() || id.len() > 128 {
        return Err(DevNexusError::new("DOCKER_INVALID_ID").param("id", truncate_for_param(id)));
    }
    if id.starts_with('-')
        || id
            .chars()
            .any(|c| c.is_whitespace() || ";|&$`\'\"\\".contains(c))
    {
        return Err(DevNexusError::new("DOCKER_INVALID_ID").param("id", truncate_for_param(id)));
    }
    Ok(())
}

/// 参数回显截断，避免异常超长输入进入错误对象
fn truncate_for_param(s: &str) -> String {
    s.chars().take(64).collect()
}
```

`validate_exec_command` 改为：

```rust
fn validate_exec_command(command: &str) -> Result<(), DevNexusError> {
    if command.is_empty() {
        return Err(DevNexusError::new("DOCKER_EXEC_EMPTY"));
    }
    let forbidden = ";|&$`\"\n\r";
    if command.chars().any(|c| forbidden.contains(c)) {
        return Err(DevNexusError::new("DOCKER_EXEC_FORBIDDEN"));
    }
    Ok(())
}
```

`run_docker` 改为：

```rust
/// 将 exec 层错误归类为稳定错误码
fn classify_spawn_error(err: &str) -> DevNexusError {
    if err.contains("No such file or directory") || err.contains("program not found") {
        DevNexusError::new("DOCKER_NOT_INSTALLED").detail(err)
    } else if err.contains("timed out") {
        DevNexusError::new("DOCKER_TIMEOUT").detail(err)
    } else {
        DevNexusError::new("DOCKER_COMMAND_FAILED").detail(err)
    }
}

/// 将 docker 命令非零退出的输出归类为稳定错误码
fn classify_exit_error(output: &str) -> DevNexusError {
    let s = output.trim();
    let lower = s.to_lowercase();
    if lower.contains("cannot connect to the docker daemon")
        || lower.contains("is the docker daemon running")
        || lower.contains("error during connect")
        || lower.contains("docker daemon is not running")
    {
        DevNexusError::new("DOCKER_NOT_RUNNING").detail(s.to_string())
    } else if lower.contains("compose") && lower.contains("no such command") {
        DevNexusError::new("COMPOSE_NOT_INSTALLED").detail(s.to_string())
    } else {
        DevNexusError::new("DOCKER_COMMAND_FAILED").detail(s.to_string())
    }
}

fn run_docker(args: &[&str]) -> Result<(String, String), DevNexusError> {
    let r = crate::utils::exec::run("docker", args, docker_timeout_for(args))
        .map_err(|e| classify_spawn_error(&e))?;
    if r.status != 0 {
        let msg = if r.stderr.trim().is_empty() {
            r.stdout.trim().to_string()
        } else {
            r.stderr.trim().to_string()
        };
        return Err(classify_exit_error(&msg));
    }
    Ok((r.stdout, r.stderr))
}
```

- [ ] **Step 2: 修改命令签名与错误构造**

- `container_action`：白名单拒绝改 `Err(DevNexusError::new("DOCKER_INVALID_ACTION").param("action", action.clone()))`；`validate_container_id(&name)?` 不变（错误类型已升级）。
- 其余 `Result<_, String>` 全部改为 `Result<_, DevNexusError>`（list_containers / get_container_logs / exec_in_container / list_images / pull_image / remove_image / build_image / tag_image / push_image / list_volumes / volume_action / list_networks / network_action / compose_up / compose_down / compose_ps / compose_logs），函数体不需要改动（`?` 与 `Ok` 均兼容）。
- `check_docker` 不变（返回 DockerStatus，前端以状态字段做降级视图）。

- [ ] **Step 3: 运行测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml container::` 
Expected: PASS（现有测试 is_ok/is_err 断言不受影响）

- [ ] **Step 4: 全量校验并提交**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings && cargo test --manifest-path src-tauri/Cargo.toml && pnpm check`

```bash
git add src-tauri/src/commands/container.rs
git commit -m "feat(docker): 容器命令迁移结构化错误码，区分未安装/未运行/超时/失败"
```

---

### Task 12: 码表文档、验收与安全扫描

**Files:**
- Create: `docs/modules/error-codes.md`
- Modify: `docs/modules/13-containers.md`（错误行为描述与实现同步）

- [ ] **Step 1: 编写 docs/modules/error-codes.md**

内容：协议说明（code/params/detail 序列化形态、前端三段式解析）、COMMON_/DOCKER_/COMPOSE_ 码表（码、含义、参数、三语言文案）、后续模块迁移指引（software/ssh/tune/env 前缀）。

- [ ] **Step 2: 手动验收清单执行**

1. `pnpm check`、`cargo test`、clippy 全绿。
2. 三语言切换检查错误 toast 文案。
3. 并发确认框：`showConfirm` 连续调用两次，两个 Promise 均正确 resolve。
4. sudo 框空输入显示"请输入密码"；取消返回 null。
5. 错误遮罩单条 toast；详情显示文件:行号。
6. toast 不遮挡标题栏（顶部偏移 44px）。

- [ ] **Step 3: Opsera 安全扫描与遥测**

调用 opsera security-scan 扫描本次改动范围，按规则报告遥测；发现 critical/high 需先修复再提交。

```bash
git add docs/modules/error-codes.md docs/modules/13-containers.md
git commit -m "docs(feedback): 结构化错误码表与容器模块错误行为文档"
```

---

## Self-Review 记录

- 规格覆盖：设计 2-8 节逐节对应 Task 1-11；设计 9 节验收对应 Task 12；设计 6 节静默失败对应 Task 10。
- 占位符：Task 10 为既有代码行级修改，计划给出模式与行号，实施时按现场代码微调（不引入新约定）。
- 类型一致性：`feedback.*`、`isStructuredError`、`sudoRejectCurrent`、`DevNexusError::new/.param/.detail` 在消费任务中名称一致。
