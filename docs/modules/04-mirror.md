# 镜像设置 — 模块设计文档

## 1. 功能概述

镜像设置（Mirror Settings）为 12 种包管理器/语言运行时提供一键切换国内/全球镜像源的功能。支持延迟测试、自动推荐最快源、按国家筛选。

**通信链路**:
```
MirrorSettings.svelte ──→ invoke("list_mirrors")        ──→ mirror.rs
                     ──→ invoke("test_mirror_latency")  ──→ mirror.rs
                     ──→ invoke("switch_mirror")        ──→ mirror.rs
```

---

## 2. 数据结构

```rust
// 单个镜像源
#[derive(Serialize)]
pub struct MirrorSource {
    pub name: String,           // "阿里云 npm"
    pub url: String,            // "https://registry.npmmirror.com"
    pub country: String,        // "CN" / "US" / "EU" / "JP" / "AU"
    pub latency_ms: i64,        // 延迟毫秒数（0 = 未测试）
    pub description: String,    // "阿里云提供的 npm 镜像服务"
}

// 一组镜像源（同一类型）
#[derive(Serialize)]
pub struct MirrorGroup {
    pub id: String,             // "npm" / "pip" / "cargo"
    pub name: String,           // "npm Registry"
    pub mirrors: Vec<MirrorSource>,
    pub current_url: Option<String>, // 当前用户配置的镜像地址
}
```

**前端对应** (`routes/MirrorSettings.svelte`):

```javascript
let groups = $state([]);
let selectedCountry = $state("all");

let filteredGroups = $derived(
    groups.map(g => ({
        ...g,
        mirrors: g.mirrors.filter(m =>
            selectedCountry === "all" || m.country === selectedCountry
        ),
    }))
);
```

---

## 3. 核心实现

### 3.1 预定义的镜像数据

`list_mirrors()` 内置了每种类型最常用的镜像源，覆盖中国、美国、欧洲、日本、俄罗斯、澳大利亚等地区的知名镜像站。

以 npm 为例：

```rust
// 在 list_mirrors 函数中
MirrorGroup {
    id: "npm".into(),
    name: "npm Registry".into(),
    current_url: get_npm_registry(),
    mirrors: vec![
        MirrorSource {
            name: "npm Official".into(),
            url: "https://registry.npmjs.org".into(),
            country: "US".into(),
            latency_ms: 0,
            description: "npm official registry".into(),
        },
        MirrorSource {
            name: "阿里云 npm".into(),
            url: "https://registry.npmmirror.com".into(),
            country: "CN".into(),
            latency_ms: 0,
            description: "阿里云提供的 npm 镜像".into(),
        },
        MirrorSource {
            name: "腾讯云 npm".into(),
            url: "https://mirrors.cloud.tencent.com/npm/".into(),
            country: "CN".into(),
            latency_ms: 0,
            description: "腾讯云提供的 npm 镜像".into(),
        },
        // ... 更多
    ],
}
```

### 3.2 当前配置检测

每个镜像类型有独立的检测函数，读取对应的配置文件来获取当前用户配置的镜像地址。

```rust
// npm → 读取 ~/.npmrc
fn get_npm_registry() -> Option<String> {
    let home = user_home();
    let npmrc = PathBuf::from(&home).join(".npmrc");
    if let Ok(content) = fs::read_to_string(&npmrc) {
        for line in content.lines() {
            if line.starts_with("registry=") {
                return Some(line.trim_start_matches("registry=").to_string());
            }
        }
    }
    None
}

// pip → 读取 ~/.pip/pip.conf 或 ~/.config/pip/pip.conf
fn get_pypi_index() -> Option<String> {
    // 查找 index-url 配置项
    for config_file in &[".pip/pip.conf", ".config/pip/pip.conf"] {
        // ...
    }
    None
}

// Docker → 读取 /etc/docker/daemon.json
fn get_docker_mirror() -> Option<String> {
    // 解析 JSON 格式 daemon.json，提取 registry-mirrors 数组
}
```

**支持的 12 种镜像类型**:

| 类型 | 配置文件路径 | 配置键 |
|------|-------------|--------|
| npm | `~/.npmrc` | `registry=` |
| pip | `~/.pip/pip.conf` / `~/.config/pip/pip.conf` | `index-url=` |
| cargo | `~/.cargo/config.toml` | `[source.crates-io]` + `replace-with` |
| brew | Shell profile (`~/.zshrc` 等) | `export HOMEBREW_API_DOMAIN=` |
| docker | `/etc/docker/daemon.json` | `registry-mirrors: [...]` |
| pypi | `~/.pip/pip.conf` | `index-url=` |
| go | Shell profile | `export GOPROXY=` |
| ruby gems | `~/.gemrc` | `:sources: [url]` |
| php composer | 全局配置 | `composer config -g repos` |
| maven | `~/.m2/settings.xml` | `<mirror><url>` |
| conda | `~/.condarc` | `channel_alias:` |
| nuget | `~/.nuget/NuGet/NuGet.Config` | XML `<add key="..." value="..." />` |
| dart/pub | Shell profile | `export PUB_HOSTED_URL=` |

### 3.3 镜像源切换

每个镜像类型的切换函数根据目标 URL 修改对应的配置文件。

```rust
pub fn switch_mirror(mirror_id: String, url: String) -> Result<String, String> {
    match mirror_id.as_str() {
        "npm"       => set_npm_mirror(&url),
        "pip"       => set_pypi_mirror(&url),
        "cargo"     => set_cargo_mirror(&url),
        "brew"      => set_brew_mirror(&url),
        // ...
    }
}
```

**示例 — npm 切换**:

```rust
fn set_npm_mirror(url: &str) -> Result<String, String> {
    let home = user_home();
    let npmrc = PathBuf::from(&home).join(".npmrc");
    let mut content = if npmrc.exists() {
        fs::read_to_string(&npmrc).map_err(|e| e.to_string())?
    } else {
        String::new()
    };

    // 替换或追加 registry 配置
    if content.contains("registry=") {
        content = content.lines()
            .map(|line| if line.starts_with("registry=") {
                format!("registry={}", url)
            } else {
                line.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
    } else {
        content.push_str(&format!("\nregistry={}\n", url));
    }

    fs::write(&npmrc, content).map_err(|e| e.to_string())?;
    Ok(format!("npm registry switched to {}", url))
}
```

**示例 — Cargo 切换**（配置格式最复杂）:

```rust
fn set_cargo_mirror(url: &str) -> Result<String, String> {
    // Cargo 使用 TOML 格式的 mirror 配置
    let config_content = format!(r#"
# Added by DevNexus
[source.crates-io]
replace-with = "devnexus-mirror"

[source."devnexus-mirror"]
registry = "{url}"
"#);
    // 写入 ~/.cargo/config.toml
}
```

### 3.4 延迟测试

```rust
pub async fn test_mirror_latency(url: String) -> i64 {
    // 使用全局延迟缓存，避免重复测试
    {
        let cache = LATENCY_CACHE.lock().unwrap();
        if let Some((latency, timestamp)) = cache.get(&url) {
            if timestamp.elapsed() < Duration::from_secs(300) { // 5 分钟缓存
                return *latency;
            }
        }
    }

    // 发起 HTTP HEAD 请求，超时 10 秒
    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build().unwrap();

    let result = client.head(&url).send().await;
    let latency = match result {
        Ok(_) => start.elapsed().as_millis() as i64,
        Err(_) => -1,  // 超时或不可达
    };

    // 写入缓存
    let mut cache = LATENCY_CACHE.lock().unwrap();
    cache.insert(url, (latency, Instant::now()));

    latency
}
```

**延迟缓存**: 使用 `LazyLock<Mutex<HashMap<String, (i64, Instant)>>>` 缓存测试结果 5 分钟，避免频繁点击重复请求，也防止用户高频测试造成对方服务器压力。

---

## 4. 前端实现

### 4.1 分组展示

每个镜像类型显示为一个卡片：

```html
<div class="border border-nx-border bg-nx-surface">
  <div class="border-b border-nx-border px-4 py-3">
    <div class="flex items-center justify-between">
      <h3>npm Registry</h3>
      <span class="text-xs text-nx-text-muted">npm</span>
    </div>
    <!-- 当前激活的镜像显示 -->
    {#if group.current_url}
      <span class="mt-1 text-xs text-nx-success">Active: ...</span>
    {/if}
  </div>
  <div class="divide-y divide-nx-border">
    {#each filteredMirrors as mirror}
      <div class="flex items-center justify-between px-4 py-3">
        <div>
          <span>{mirror.name} {getCountryFlag(mirror.country)}</span>
        </div>
        <div class="flex items-center gap-2">
          <button onclick={() => testMirror(group.id, mirror.url)}>
            {mirror.latency_ms > 0 ? `${mirror.latency_ms}ms` : 'test'}
          </button>
          {#if mirror.is_active}
            <span>Active</span>
          {:else}
            <button onclick={() => switchMirror(group.id, mirror.url)}>Use</button>
          {/if}
        </div>
      </div>
    {/each}
  </div>
  <div class="px-3 py-2">
    <button onclick={() => testAllMirrors(group)}>Test All</button>
  </div>
</div>
```

### 4.2 一键测试全部 + 自动推荐

```javascript
async function testAllMirrors(group) {
    const results = await Promise.all(
        group.mirrors.map(async (m) => {
            const latency = await invoke("test_mirror_latency", { url: m.url });
            m.latency_ms = latency;
            return { mirror: m, latency };
        })
    );
    // 找到延迟最低的镜像（排除超时）
    const fastest = results.reduce((best, cur) =>
        cur.latency > 0 && cur.latency < best.latency ? cur : best
    );
    if (fastest.mirror) {
        fastest.mirror.recommended = true;  // 标记推荐
    }
}
```

使用 `Promise.all` 并行测试组内所有镜像，单位为毫秒。

### 4.3 国家筛选

```javascript
const countries = [
    { id: "all", label: "All Regions" },
    { id: "CN",  label: "🇨🇳 China" },
    { id: "RU",  label: "🇷🇺 Russia" },
    { id: "US",  label: "🇺🇸 United States" },
    { id: "EU",  label: "🇪🇺 Europe" },
    { id: "JP",  label: "🇯🇵 Japan" },
    { id: "AU",  label: "🇦🇺 Australia" },
];
```

---

## 5. 跨平台注意事项

| 镜像类型 | macOS | Linux | Windows |
|---------|-------|-------|---------|
| npm | `~/.npmrc` ✅ | `~/.npmrc` ✅ | `~/.npmrc` ✅ |
| pip | `~/.pip/pip.conf` ✅ | `~/.config/pip/pip.conf` ✅ | `~/pip/pip.ini` ✅ |
| cargo | `~/.cargo/config.toml` ✅ | `~/.cargo/config.toml` ✅ | `~/.cargo/config.toml` ✅ |
| brew | `~/.zshrc` ✅ | ❌ 无 Homebrew | ❌ 无 Homebrew |
| docker | `/etc/daemon.json` ✅ | `/etc/docker/daemon.json` ✅ | `%PROGRAMDATA%\Docker\config\daemon.json` ✅ |
| nuget | `~/.nuget/NuGet/NuGet.Config` ✅ | `~/.nuget/NuGet/NuGet.Config` ✅ | `%APPDATA%\NuGet\NuGet.Config` ✅ |

Brew（Homebrew）仅在 macOS 上可用，其他平台不显示该镜像组。Caddy 和 Docker 在 Windows 上路径不同，由各自的 `get_*` 函数内部通过条件编译处理。

---

## 6. 关键设计决策

1. **配置文件直接修改 vs 工具命令**: 直接读写配置文件比调用包管理器自己的命令更可靠，避免依赖特定版本的工具命令接口

2. **缓存延迟结果**: 5 分钟 TTL 避免因用户频繁测试请求而触发镜像站的反爬

3. **TOML 配置的特殊处理**: Cargo 的 config.toml 不兼容简单的键值替换，使用了 TOML 的 mirror 替代源语法，需要整体写入 `[source]` 块

4. **冗余安全性**: 切换前读取当前配置备份到日志，理论上用户可以手动恢复
