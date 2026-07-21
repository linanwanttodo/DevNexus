use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub version: String,
    pub en: String,
    pub zh: String,
}

pub fn get_changelog(version: &str) -> Option<ChangelogEntry> {
    all_changelogs().into_iter().find(|e| e.version == version)
}

pub fn get_latest_changelog() -> Option<ChangelogEntry> {
    all_changelogs().into_iter().next()
}

fn all_changelogs() -> Vec<ChangelogEntry> {
    vec![ChangelogEntry {
        version: "1.2.1".to_string(),
        en: "IDM-style segmented progress bar with per-chunk status colors\n\
                 Real-time speed/progress reporting via streaming download\n\
                 Work queue download engine with fixed worker threads\n\
                 GitHub URL auto-detection with configurable mirror support\n\
                 Bilingual changelog display in update dialog\n\
                 Clipboard auto-paste when opening Add Download dialog\n\
                 Cookie string support for authenticated downloads\n\
                 Browser environment emulation (Sec-Fetch- headers, native-tls)\n\
                 Exponential backoff retry for failed chunks\n\
                 Mirror management UI with strip_host mode\n\
                 \n\
                 Fixed: download speed always showing 0 B/s\n\
                 Fixed: HTTP 403 due to missing browser headers\n\
                 Fixed: Content-Encoding decoding failures\n\
                 Fixed: confirm dialog excessive width"
            .to_string(),
        zh: "IDM 风格分段进度条，按分块状态显示不同颜色\n\
                 基于流式下载的实时速度与进度推送\n\
                 工作队列下载引擎，固定线程数并发\n\
                 GitHub 链接自动检测与可配置镜像加速\n\
                 更新弹窗双语更新日志显示\n\
                 添加下载弹窗自动读取剪贴板\n\
                 Cookie 字符串支持登录态下载\n\
                 浏览器环境模拟（Sec-Fetch- 头、native-tls）\n\
                 分块失败指数退避重试\n\
                 镜像管理界面，支持 strip_host 模式\n\
                 \n\
                 修复：下载速度始终显示 0 B/s\n\
                 修复：缺少浏览器头导致的 HTTP 403\n\
                 修复：Content-Encoding 解码失败\n\
                 修复：确认弹窗过宽"
            .to_string(),
    }]
}
