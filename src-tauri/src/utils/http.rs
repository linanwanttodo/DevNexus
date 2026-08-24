// src-tauri/src/utils/http.rs — 共享 reqwest 客户端构建
//! 统一连接超时、总超时、pool 配置，避免各模块各自 new Client 造成连接池碎片。

use std::time::Duration;

/// 构建带默认超时的 reqwest 客户端
pub fn build_client(
    connect_timeout: Duration,
    total_timeout: Duration,
) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(total_timeout)
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("HTTP client build failed: {e}"))
}

/// 共享的版本抓取客户端（10s 总超时，复用连接池）.
/// 调用方应复用返回值而非每次新建。
pub fn version_fetch_client() -> Result<reqwest::Client, String> {
    build_client(
        crate::utils::timeouts::HTTP_CONNECT,
        crate::utils::timeouts::HTTP_VERSION_FETCH,
    )
}
