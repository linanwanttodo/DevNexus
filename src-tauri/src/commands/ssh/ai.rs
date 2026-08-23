use crate::api_hub::types::{ApiProtocol, AppState, Provider};
use crate::commands::ssh::session::SshSessionManager;
use base64::{engine::general_purpose, Engine as _};
use serde_json::Value;
use std::time::Duration;

/// SSH AI 助手引擎。
///
/// 设计要点：
/// - 直接复用 API Hub 已配置的 LLM Provider（base_url / api_key / protocol / models），
///   用户无需为 SSH 再填一遍凭据（符合用户选择：复用 api_hub 配置）。
/// - 凭据解密在 api_hub 内存态已是明文（api_key 字段在 AppState 中为明文，
///   前端列表脱敏只是展示层），这里直接读取即可。
/// - LLM 请求使用阻塞式 reqwest 包在 spawn_blocking 中执行，与 usage.rs 的 SQLite 写法一致，
///   不引入新的异步流式复杂度。
use tauri::State;

/// 危险命令前缀/关键字：命中需用户确认后才执行。
/// 参考 DevSecOps 规范：破坏性/提权/不可逆操作需二次确认。
const DANGEROUS_PATTERNS: &[&str] = &[
    "rm ",
    "rm -",
    "mkfs",
    "dd ",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "kill ",
    "killall",
    "pkill",
    "sudo",
    "su ",
    "chmod 777",
    "chown",
    "mv ",
    ":(){",
    "chroot",
    ">",
    ">|",
    "curl ",
    "wget ",
    "nohup",
    "systemctl",
    "service ",
    "apt ",
    "apt-get",
    "yum ",
    "dnf ",
    "pacman",
    "brew ",
    "git reset --hard",
    "git push --force",
    "git checkout --",
    "truncate",
    "dropdb",
    "drop table",
];

/// 判断一条命令是否危险（需确认）。
pub fn is_dangerous(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    // 直接以危险前缀开头
    for p in DANGEROUS_PATTERNS {
        if trimmed.starts_with(p) {
            return true;
        }
    }
    // 或包含危险子串（如管道后的 rm）
    if trimmed.contains(" | rm ")
        || trimmed.contains("&& rm ")
        || trimmed.contains("; rm ")
        || trimmed.contains(" | sudo")
        || trimmed.contains("&& sudo")
    {
        return true;
    }
    false
}

/// 从 API Hub 的 Provider 列表中挑选一个可用 Provider 作为 AI 后端。
/// 优先返回首个启用的、且含模型的 Provider；若指定 model 则尽量匹配对应 Provider。
fn pick_provider(state: &AppState, preferred_model: &Option<String>) -> Result<Provider, String> {
    let providers = state.providers.blocking_read();
    let enabled: Vec<&Provider> = providers.iter().filter(|p| p.enabled).collect();
    if enabled.is_empty() {
        return Err(
            "未在 API Hub 配置任何启用的 Provider。请先在 API Hub 添加一个 LLM Provider。".into(),
        );
    }
    if let Some(m) = preferred_model {
        // 优先：该模型在某 Provider 的模型列表里
        if let Some(p) = enabled
            .iter()
            .find(|p| p.models.iter().any(|x| x.eq_ignore_ascii_case(m)))
        {
            return Ok((*p).clone());
        }
    }
    // 否则返回第一个启用的 Provider
    Ok(enabled[0].clone())
}

/// 去除 ANSI 转义序列，便于 AI 读取纯文本屏幕内容。
fn strip_ansi(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // 跳过直到字母终结符
            i += 2;
            while i < bytes.len() && !(bytes[i] as char).is_ascii_alphabetic() && bytes[i] != b'~' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // 跳过终结符
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// 构造 system prompt：让模型以"SSH 终端助手"身份工作，输出可执行命令。
fn build_system_prompt(platform_hint: &str) -> String {
    format!(
        "你是一个运行在远程 Linux/macOS 服务器的 SSH 终端 AI 助手。\
用户会用自然语言描述想执行的任务（如'找出占用内存最高的进程并杀掉'、'查看 nginx 错误日志'）。\
你的职责：\
1) 用简洁中文解释你要做什么；\
2) 给出一条或多条可在远程 shell 直接执行的命令（bash/sh 兼容）；\
3) 若需要多条命令，用分号或 && 串联，或逐条列出；\
4) 不要解释无关内容，不要输出 markdown 代码块以外的废话。\
远程系统提示：{platform_hint}。\
当命令可能产生不可逆/危险后果（rm -rf、kill、sudo、修改系统配置等）时，照常给出命令，但额外在回复开头加一行以 [DANGER] 开头说明风险。\
如果用户只问问题而不需要执行命令，正常用中文回答即可，不要强行给命令。"
    )
}

/// 调用 LLM 补全（阻塞式，应在 spawn_blocking 中调用）。
fn call_llm(
    provider: &Provider,
    model: &str,
    messages: &[Value],
    timeout_secs: u64,
) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| format!("HTTP client build failed: {e}"))?;

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "temperature": 0.2,
        "max_tokens": 1024,
    });

    let url = match provider.protocol {
        ApiProtocol::Anthropic => {
            format!("{}/v1/messages", provider.base_url.trim_end_matches('/'))
        }
        ApiProtocol::Gemini => {
            // Gemini 用 generateContent；但为简化，这里统一走 OpenAI 兼容路径
            // （多数自建/网关 Gemini 也提供 OpenAI 兼容端点）。
            format!(
                "{}/v1/chat/completions",
                provider.base_url.trim_end_matches('/')
            )
        }
        _ => format!(
            "{}/v1/chat/completions",
            provider.base_url.trim_end_matches('/')
        ),
    };

    let mut req = client.post(&url).json(&body);
    req = match provider.protocol {
        ApiProtocol::Anthropic => req
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", "2023-06-01"),
        _ => {
            if !provider.api_key.is_empty() {
                req.header("Authorization", format!("Bearer {}", provider.api_key))
            } else {
                req
            }
        }
    };

    let resp = req.send().map_err(|e| format!("LLM request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let txt = resp.text().unwrap_or_default();
        return Err(format!("LLM upstream error ({status}): {txt}"));
    }
    let json: Value = resp
        .json()
        .map_err(|e| format!("LLM response parse failed: {e}"))?;

    // 解析两种格式
    let content = if let Some(choices) = json.get("choices") {
        // OpenAI 兼容
        choices
            .get(0)
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string()
    } else if let Some(content_arr) = json.get("content") {
        // Anthropic
        content_arr
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    } else {
        return Err("无法解析 LLM 响应格式".into());
    };

    Ok(content)
}

/// 从回复文本中提取可执行命令：优先提取 ```bash/```shell 代码块，
/// 否则按行提取以 `$`/普通命令开头的行。
fn extract_commands(reply: &str) -> Vec<String> {
    let mut cmds = Vec::new();
    // 1) 代码块
    let re_block = regex_lazy_extract(reply);
    if !re_block.is_empty() {
        return re_block;
    }
    // 2) 逐行，去 `$`/`- ` 前缀
    for line in reply.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("//") {
            continue;
        }
        let stripped = l
            .strip_prefix("$")
            .or_else(|| l.strip_prefix(">"))
            .or_else(|| l.strip_prefix("- "))
            .unwrap_or(l)
            .trim();
        if stripped.contains(' ') && !stripped.contains('[') && !stripped.contains("DANGER") {
            cmds.push(stripped.to_string());
        }
    }
    cmds
}

/// 极简代码块提取（不引入 regex 依赖，手搓扫描）。
fn regex_lazy_extract(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let marker = b"```";
    let mut i = 0;
    while i + 3 <= bytes.len() {
        if &bytes[i..i + 3] == marker {
            // 跳到行尾，读取语言标识（忽略）
            let mut j = i + 3;
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            // 收集直到下一个 ```
            let mut k = j + 1;
            let mut block = String::new();
            while k + 3 <= bytes.len() {
                if &bytes[k..k + 3] == marker {
                    break;
                }
                block.push(bytes[k] as char);
                k += 1;
            }
            let block = block.trim().to_string();
            if !block.is_empty() {
                out.push(block);
            }
            i = k + 3;
            continue;
        }
        i += 1;
    }
    out
}

// ── Tauri 命令 ────────────────────────────────────────────────

/// 列出可用于 SSH AI 的模型（来自 API Hub 启用的 Provider 模型列表）。
#[tauri::command]
pub async fn ssh_ai_list_models(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let providers = state.providers.read().await;
    let mut out = Vec::new();
    for p in providers.iter().filter(|p| p.enabled) {
        for m in &p.models {
            out.push(serde_json::json!({
                "model": m,
                "provider": p.name,
                "protocol": p.protocol.as_str(),
            }));
        }
    }
    Ok(out)
}

/// 发送一条消息给 AI，返回回复文本 + 提取出的命令 + 危险标记。
/// 若提供了 term_id，会把终端最近输出作为上下文附给模型。
#[tauri::command]
pub async fn ssh_ai_chat(
    state: State<'_, AppState>,
    session_mgr: State<'_, SshSessionManager>,
    term_id: Option<String>,
    history: Vec<serde_json::Value>,
    message: String,
    model: Option<String>,
) -> Result<serde_json::Value, String> {
    let provider = pick_provider(&state, &model)?;
    let chosen_model = model
        .clone()
        .or_else(|| provider.models.first().cloned())
        .ok_or("所选 Provider 没有可用模型，请先在 API Hub 添加模型")?;

    // 收集终端上下文
    let mut context_lines = String::new();
    if let Some(tid) = &term_id {
        if let Some(term) = session_mgr.find_terminal(tid).await {
            let buf = term.output_buffer.lock().await;
            context_lines = strip_ansi(&buf.recent(200));
        }
    }

    let mut messages: Vec<Value> = vec![serde_json::json!({
        "role": "system",
        "content": build_system_prompt(
            if context_lines.contains("Darwin") { "macOS" } else { "Linux" }
        )
    })];

    // 历史（取最近 12 条）
    for h in history.iter().rev().take(12).rev() {
        if let (Some(role), Some(content)) = (
            h.get("role").and_then(|r| r.as_str()),
            h.get("content").and_then(|c| c.as_str()),
        ) {
            messages.push(serde_json::json!({ "role": role, "content": content }));
        }
    }

    // 当前用户消息（带终端上下文）
    let user_msg = if context_lines.is_empty() {
        message.clone()
    } else {
        format!(
            "{}\n\n[当前终端最近输出上下文]\n{}\n[/上下文]",
            message, context_lines
        )
    };
    messages.push(serde_json::json!({ "role": "user", "content": user_msg }));

    let provider_clone = provider.clone();
    let model_clone = chosen_model.clone();
    let reply =
        tokio::task::spawn_blocking(move || call_llm(&provider_clone, &model_clone, &messages, 60))
            .await
            .map_err(|e| format!("AI task join error: {e}"))??;

    let cmds = extract_commands(&reply);
    let danger = reply
        .lines()
        .any(|l| l.trim_start().starts_with("[DANGER]"))
        || cmds.iter().any(|c| is_dangerous(c));

    Ok(serde_json::json!({
        "reply": reply,
        "commands": cmds,
        "dangerous": danger,
        "model": chosen_model,
        "provider": provider.name,
    }))
}

/// 在指定终端执行一条命令（把命令作为输入发送给远端 PTY）。
/// 危险命令由前端确认后调用，这里不再二次判断。
#[tauri::command]
pub async fn ssh_ai_execute(
    session_mgr: State<'_, SshSessionManager>,
    term_id: String,
    command: String,
) -> Result<(), String> {
    let term = session_mgr
        .find_terminal(&term_id)
        .await
        .ok_or_else(|| format!("NO_TERMINAL: {term_id}"))?;

    // 命令以换行结尾，模拟用户在终端敲回车
    let payload = format!("{command}\n");
    let bytes = payload.as_bytes().to_vec();
    let write = term.write.lock().await;
    write
        .data_bytes(bytes)
        .await
        .map_err(|e| format!("EXEC_FAILED: {e}"))
}

/// SFTP AI 助手：基于当前目录的 SFTP 上下文回答自然语言问题，返回可执行动作。
/// 复用 API Hub 的 LLM Provider（与终端 AI 相同），无需额外配置。
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn ssh_ai_sftp(
    state: State<'_, AppState>,
    session_mgr: State<'_, SshSessionManager>,
    sftp_id: String,
    cwd: String,
    listing: serde_json::Value, // 当前目录文件列表（前端传入）
    history: Vec<serde_json::Value>,
    message: String,
    model: Option<String>,
) -> Result<serde_json::Value, String> {
    let provider = pick_provider(&state, &model)?;
    let chosen_model = model
        .clone()
        .or_else(|| provider.models.first().cloned())
        .ok_or("所选 Provider 没有可用模型，请先在 API Hub 添加模型")?;

    // 验证 SFTP 会话存在，确保上下文来自真实连接
    if session_mgr.find_sftp(&sftp_id).await.is_none() {
        return Err(format!("NO_SFTP: {sftp_id}"));
    }

    let listing_str = serde_json::to_string(&listing).unwrap_or_default();
    let prompt = format!(
        "你是运行在远程服务器上的 SFTP 文件管理器 AI 助手。\n\
当前目录：{cwd}\n\
当前目录内容（JSON：name/is_dir/size/mode/mtime）：\n{listing_str}\n\n\
用户会用自然语言描述文件操作意图（如'最大的文件是哪个'、'帮我整理这里的日志'、'这个目录有多大'）。\n\
你的职责：\n\
1) 用简洁中文解释你的判断；\n\
2) 若建议具体操作，只给出这些受支持的动作（JSON 数组，逐条）：\n\
   - {{\"action\":\"navigate\", \"path\":\"<目录绝对路径>\"}}  # 进入某个目录\n\
   - {{\"action\":\"rename\", \"from\":\"<旧路径>\", \"to\":\"<新路径>\"}}\n\
   - {{\"action\":\"delete\", \"path\":\"<路径>\", \"is_dir\":true|false}}\n\
   - {{\"action\":\"open\", \"path\":\"<文件绝对路径>\"}}  # 前端尝试下载/查看\n\
3) 不要输出任何除此之外的命令代码；没有可执行动作时返回空数组。\n\
请先用一段 markdown 说明，然后紧跟一行以 [ACTIONS] 开头的 JSON 数组。"
    );

    let mut messages: Vec<Value> = vec![serde_json::json!({ "role": "system", "content": prompt })];
    for h in history.iter().rev().take(10).rev() {
        if let (Some(role), Some(content)) = (
            h.get("role").and_then(|r| r.as_str()),
            h.get("content").and_then(|c| c.as_str()),
        ) {
            messages.push(serde_json::json!({ "role": role, "content": content }));
        }
    }
    messages.push(serde_json::json!({ "role": "user", "content": message }));

    let provider_clone = provider.clone();
    let model_clone = chosen_model.clone();
    let reply =
        tokio::task::spawn_blocking(move || call_llm(&provider_clone, &model_clone, &messages, 60))
            .await
            .map_err(|e| format!("AI task join error: {e}"))??;

    // 解析 [ACTIONS] 行后的 JSON
    let mut actions: Vec<Value> = Vec::new();
    for line in reply.lines() {
        if let Some(idx) = line.find("[ACTIONS]") {
            let rest = line[idx + "[ACTIONS]".len()..].trim();
            if let Ok(v) = serde_json::from_str::<Value>(rest) {
                if let Some(arr) = v.as_array() {
                    actions = arr.clone();
                }
            }
            break;
        }
    }

    Ok(serde_json::json!({
        "reply": reply,
        "actions": actions,
        "model": chosen_model,
        "provider": provider.name,
    }))
}

/// 读取终端最近输出（供前端"查看 AI 上下文"或调试）。
#[tauri::command]
pub async fn ssh_ai_get_buffer(
    session_mgr: State<'_, SshSessionManager>,
    term_id: String,
    lines: Option<usize>,
) -> Result<String, String> {
    let term = session_mgr
        .find_terminal(&term_id)
        .await
        .ok_or_else(|| format!("NO_TERMINAL: {term_id}"))?;
    let buf = term.output_buffer.lock().await;
    let n = lines.unwrap_or(200).min(buf.lines.len());
    Ok(strip_ansi(&buf.recent(n)))
}

// 抑制未使用告警：base64 在调试路径可能用到
#[allow(dead_code)]
fn _b64_keep(_: &[u8]) -> String {
    general_purpose::STANDARD.encode(b"")
}
