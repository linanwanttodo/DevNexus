# 结构化错误码表

## 协议

后端命令错误类型统一为 `DevNexusError`（`src-tauri/src/utils/error.rs`）：

```rust
pub struct DevNexusError {
    pub code: String,                     // 稳定错误码，如 "DOCKER_NOT_RUNNING"
    pub params: HashMap<String, String>,  // i18n 插值参数（对应文案中的 {name} 占位符）
    pub detail: Option<String>,           // 原始细节（stderr 等），仅日志与详情展示
}
```

命令签名 `Result<T, DevNexusError>`，Tauri 2 将错误对象直接序列化为前端 reject 值。

前端解析（`src/lib/errors.js` 的 `friendlyError`，三段式）：

1. 错误为带 `code` 的对象：查 i18n 键 `errors.<code>` 并用 `params` 插值；键缺失回退 `detail`，再回退 code。
2. 错误为字符串：旧模式匹配表（未迁移命令的过渡兼容）。
3. 其余：回退原消息。

视图层调用约定：一律经 `src/lib/feedback.js` 门面（`feedback.toast.error(err)` 等），
错误对象原样传入，禁止在前端手工拼接错误文案。

## 码表

### 通用（COMMON_）

| 码 | 含义 | 参数 | 备注 |
|---|---|---|---|
| COMMON_INVALID_INPUT | 输入无效 | - | detail 携带校验细节 |
| COMMON_NOT_FOUND | 未找到目标资源 | - | |
| COMMON_PERMISSION_DENIED | 权限不足，需要授权 | - | |
| COMMON_INTERNAL | 内部错误 | - | `From<String>` 的默认落点 |
| COMMON_CANCELLED | 操作已取消 | - | |

### Docker（DOCKER_ / COMPOSE_）

| 码 | 含义 | 参数 | 触发点（container.rs） |
|---|---|---|---|
| DOCKER_NOT_INSTALLED | 未安装 Docker | - | spawn 失败且为二进制缺失 |
| DOCKER_NOT_RUNNING | Docker 未运行 | - | 非零退出且输出含 daemon 连接失败特征 |
| DOCKER_TIMEOUT | 操作超时已终止 | - | exec 层超时 |
| DOCKER_COMMAND_FAILED | Docker 操作失败 | - | 其余非零退出，detail 为原始输出 |
| DOCKER_INVALID_ID | 无效容器 ID | id | `validate_container_id` |
| DOCKER_INVALID_ACTION | 不支持的操作 | action | `container_action` 白名单 |
| DOCKER_EXEC_EMPTY | 未输入执行命令 | - | `validate_exec_command` |
| DOCKER_EXEC_FORBIDDEN | 命令含禁止字符 | - | `validate_exec_command` |
| COMPOSE_NOT_INSTALLED | 未安装 Compose 插件 | - | 非零退出且输出含 compose 命令缺失特征 |

## 后续模块迁移指引

- 前缀约定：`UNINSTALL_`（软件卸载）、`SSH_`、`TUNE_`（系统调优）、`ENV_`（环境管理）、`MIRROR_`、`MIGRATION_`。
- 每迁移一个模块：命令签名改为 `Result<T, DevNexusError>`；校验类错误用带参数的码；执行类错误经统一分类函数归类；三语言 locales 同步补齐 `errors.<code>`；在本文档登记码。
- 未迁移模块保持 `Result<T, String>` 不受影响，前端走旧模式匹配。
