use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct MirrorSource {
    pub name: String,
    pub url: String,
    pub country: String,
    pub latency_ms: i64, // -1=未测, 0=超时/错误, >0=实际延迟(ms)
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MirrorGroup {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub current_url: Option<String>,
    pub mirrors: Vec<MirrorSource>,
}

/// 静态镜像源数据表（不含当前生效 URL，current_url 由 mirror::list_mirrors 从配置读取填充）
/// 2026-08-16 全量实测清理：移除 DNS 已失效/服务停运/占位符条目；
/// cargo 组改为 sparse+ 协议 URL（现代 cargo 的标准形态，测速时会剥掉前缀探测）。
pub fn list_mirror_groups() -> Vec<MirrorGroup> {
    vec![
        MirrorGroup {
            id: "npm".into(),
            label: "NPM Registry".into(),
            icon: "npm".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "npmmirror (China)".into(),
                    url: "https://registry.npmmirror.com".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Huawei (China)".into(),
                    url: "https://mirrors.huaweicloud.com/repository/npm/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://registry.npmjs.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "pypi".into(),
            label: "PyPI".into(),
            icon: "python".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://pypi.tuna.tsinghua.edu.cn/simple".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/pypi/simple/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirrors.cloud.tencent.com/pypi/simple/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://pypi.org/simple/".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "docker".into(),
            label: "Docker Hub".into(),
            icon: "docker".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "DaoCloud (China)".into(),
                    url: "https://docker.m.daocloud.io".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://registry.cn-hangzhou.aliyuncs.com".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "NJU (China)".into(),
                    url: "https://docker.nju.edu.cn/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "AWS (Global)".into(),
                    url: "https://public.ecr.aws".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "cargo".into(),
            label: "Cargo (Rust)".into(),
            icon: "crate".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "RsProxy (China)".into(),
                    url: "sparse+https://rsproxy.cn/index/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tuna (China)".into(),
                    url: "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "sparse+https://mirrors.ustc.edu.cn/crates.io-index/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "sparse+https://mirrors.aliyun.com/crates.io-index/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "sparse+https://index.crates.io/".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        #[cfg(target_os = "macos")]
        MirrorGroup {
            id: "brew".into(),
            label: "Homebrew".into(),
            icon: "brew".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/homebrew/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/homebrew/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://github.com/Homebrew/brew".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "composer".into(),
            label: "Composer (PHP)".into(),
            icon: "php".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/composer/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirrors.cloud.tencent.com/composer/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://repo.packagist.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "go".into(),
            label: "Go Modules".into(),
            icon: "go".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Qiniu (goproxy.cn)".into(),
                    url: "https://goproxy.cn".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "goproxy.io (Global)".into(),
                    url: "https://goproxy.io".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "gems".into(),
            label: "RubyGems".into(),
            icon: "ruby".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/rubygems/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/rubygems/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://mirrors.aliyun.com/rubygems/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://rubygems.org".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "maven".into(),
            label: "Maven (Java)".into(),
            icon: "java".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Aliyun (China)".into(),
                    url: "https://maven.aliyun.com/repository/public/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Tencent (China)".into(),
                    url: "https://mirrors.cloud.tencent.com/maven/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Huawei (China)".into(),
                    url: "https://mirrors.huaweicloud.com/repository/maven/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://repo.maven.apache.org/maven2".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Google Cloud (Global)".into(),
                    url: "https://storage-download.googleapis.com/maven-central".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "conda".into(),
            label: "Conda (Anaconda)".into(),
            icon: "python".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/anaconda/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "USTC (China)".into(),
                    url: "https://mirrors.ustc.edu.cn/anaconda/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://repo.anaconda.com".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
        MirrorGroup {
            id: "nuget".into(),
            label: "NuGet (.NET)".into(),
            icon: "nuget".into(),
            current_url: None,
            mirrors: vec![MirrorSource {
                name: "Official (US)".into(),
                url: "https://api.nuget.org/v3/index.json".into(),
                country: "US".into(),
                latency_ms: -1,
                is_active: false,
            }],
        },
        MirrorGroup {
            id: "pub".into(),
            label: "Flutter (Pub)".into(),
            icon: "flutter".into(),
            current_url: None,
            mirrors: vec![
                MirrorSource {
                    name: "Tsinghua (China)".into(),
                    url: "https://mirrors.tuna.tsinghua.edu.cn/dart-pub/".into(),
                    country: "CN".into(),
                    latency_ms: -1,
                    is_active: false,
                },
                MirrorSource {
                    name: "Official (US)".into(),
                    url: "https://pub.dev".into(),
                    country: "US".into(),
                    latency_ms: -1,
                    is_active: false,
                },
            ],
        },
    ]
}
