// src-tauri/src/utils/timeouts.rs — 统一超时常量
//! 收敛分散在各模块中的魔法超时数字，便于审计与调参。

use std::time::Duration;

/// 子进程默认超时（utils::exec::run 的兜底）
pub const EXEC_DEFAULT: Duration = Duration::from_secs(60);
/// docker 长操作（pull/build/push/load/save）
pub const DOCKER_LONG: Duration = Duration::from_secs(900);

/// HTTP 客户端
pub const HTTP_CONNECT: Duration = Duration::from_secs(10);
pub const HTTP_TOTAL: Duration = Duration::from_secs(60);
pub const HTTP_MODEL_FETCH: Duration = Duration::from_secs(8);
pub const HTTP_MIRROR_TEST: Duration = Duration::from_secs(5);
pub const HTTP_VERSION_FETCH: Duration = Duration::from_secs(10);
pub const HTTP_DOWNLOAD_CONNECT: Duration = Duration::from_secs(15);
pub const HTTP_DOWNLOAD_TOTAL: Duration = Duration::from_secs(600);
pub const HTTP_DEEPSEEK: Duration = Duration::from_secs(10);

/// SSH / AI
pub const SSH_TCP_CONNECT: Duration = Duration::from_secs(10);
pub const SSH_LLM: Duration = Duration::from_secs(60);
pub const SSH_KEEPALIVE: Duration = Duration::from_secs(30);

/// 终端 / 系统
pub const VERSION_CHECK: Duration = Duration::from_secs(3);
pub const COOKIE_SECRET_TOOL: Duration = Duration::from_secs(3);
pub const DBUS: Duration = Duration::from_secs(2);
pub const TRAY_DEBOUNCE: Duration = Duration::from_secs(300);
pub const IDLE_SHUTDOWN: Duration = Duration::from_secs(30 * 60);
