// src/lib/api-ssh.js — SSH 后端命令与事件封装（19 个命令 + 3 个事件）
// 约定：明文凭据只在 Rust 侧解密；终端输入输出与文件内容均以 base64 传输。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ---- 连接管理 ----
export const listConnections = () => invoke("ssh_list_connections");
export const saveConnection = (connection) =>
  invoke("ssh_save_connection", { connection });
export const deleteConnection = (id) => invoke("ssh_delete_connection", { id });
export const touchConnection = (id) => invoke("ssh_touch_connection", { id });
export const importOpenSshConfig = () => invoke("ssh_import_open_ssh_config");
export const exportOpenSshConfig = (connIds) =>
  invoke("ssh_export_openssh_config", { connIds });

// ---- 会话 / host key 首连确认 ----
export const testConnection = (connectionId) =>
  invoke("ssh_test_connection", { connectionId });
export const closeSession = (sessionId) => invoke("ssh_close", { sessionId });
export const acceptHostkey = (sessionId, host, fingerprint) =>
  invoke("ssh_hostkey_accept", { sessionId, host, fingerprint });
export const rejectHostkey = (sessionId) =>
  invoke("ssh_hostkey_reject", { sessionId });

// ---- 终端（PTY）----
export const openTerminal = (connectionId, cols, rows) =>
  invoke("ssh_terminal_open", { connectionId, cols, rows });
export const sendTerminalInput = (sessionId, data) =>
  invoke("ssh_terminal_input", { sessionId, data });
export const resizeTerminal = (sessionId, cols, rows) =>
  invoke("ssh_terminal_resize", { sessionId, cols, rows });
export const closeTerminal = (sessionId) =>
  invoke("ssh_terminal_close", { sessionId });

// ---- SFTP ----
export const openSftp = (connectionId) =>
  invoke("ssh_sftp_open", { connectionId });
export const listSftpDir = (sftpId, path) =>
  invoke("ssh_sftp_list_dir", { sftpId, path });
export const readSftpFile = (sftpId, path, offset, length) =>
  invoke("ssh_sftp_read_file", { sftpId, path, offset, length });
export const writeSftpFile = (sftpId, path, data, offset) =>
  invoke("ssh_sftp_write_file", { sftpId, path, data, offset });
export const mkdirSftp = (sftpId, path) =>
  invoke("ssh_sftp_mkdir", { sftpId, path });
export const renameSftp = (sftpId, from, to) =>
  invoke("ssh_sftp_rename", { sftpId, from, to });
export const deleteSftp = (sftpId, path, isDir) =>
  invoke("ssh_sftp_delete", { sftpId, path, isDir });
export const statSftp = (sftpId, path) =>
  invoke("ssh_sftp_stat", { sftpId, path });
export const chmodSftp = (sftpId, path, mode) =>
  invoke("ssh_sftp_chmod", { sftpId, path, mode });
export const copyRecursiveSftp = (sftpId, from, to, overwrite) =>
  invoke("ssh_sftp_copy_recursive", { sftpId, from, to, overwrite });
export const rmRecursiveSftp = (sftpId, path) =>
  invoke("ssh_sftp_rm_recursive", { sftpId, path });
export const searchSftp = (sftpId, root, pattern, maxDepth) =>
  invoke("ssh_sftp_search", { sftpId, root, pattern, maxDepth });

// ---- SFTP AI 助手（复用 API Hub 的 LLM Provider 配置）----
// 传入当前目录上下文，返回 { reply, actions, model, provider }
export const aiSftp = (params) =>
  invoke("ssh_ai_sftp", params);
export const aiSftpModels = () =>
  invoke("ssh_ai_list_models");

// ---- 端口转发与隧道 ----
export const forwardLocal = (sessionId, bindHost, bindPort, destHost, destPort) =>
  invoke("ssh_forward_local", { sessionId, bindHost, bindPort, destHost, destPort });
export const closeForward = (sessionId, forwardId) =>
  invoke("ssh_close_forward", { sessionId, forwardId });
export const listForwards = (sessionId) =>
  invoke("ssh_list_forwards", { sessionId });
export const forwardAgent = (sessionId) =>
  invoke("ssh_forward_agent", { sessionId });

// ---- 动态 SOCKS5 代理（-D）----
export const startSocksProxy = (sessionId, bindHost, bindPort) =>
  invoke("ssh_socks_proxy", { sessionId, bindHost, bindPort });
export const closeSocks = (sessionId, socksId) =>
  invoke("ssh_close_socks", { sessionId, socksId });
export const listSocks = (sessionId) =>
  invoke("ssh_list_socks", { sessionId });

// ---- AI 助手（复用 API Hub 的 LLM Provider 配置）----
// 列出可用模型（来自 API Hub 启用的 Provider）
export const aiListModels = () => invoke("ssh_ai_list_models");
// 发送一条消息，返回 { reply, commands, dangerous, model, provider }
export const aiChat = (params) =>
  invoke("ssh_ai_chat", params);
// 在指定终端执行一条命令（confirmed=true 时放行危险命令的后端二次校验）
export const aiExecute = (termId, command, confirmed = false) =>
  invoke("ssh_ai_execute", { termId, command, confirmed });
// 读取终端最近输出（调试/上下文查看）
export const aiGetBuffer = (termId, lines) =>
  invoke("ssh_ai_get_buffer", { termId, lines });

// ---- 事件（Rust 侧 payload 为 snake_case，这里归一为 camelCase）----
export function onTerminalOutput(cb) {
  return listen("ssh-terminal-output", (ev) =>
    cb({ sessionId: ev.payload.session_id, data: ev.payload.data })
  );
}

export function onTerminalClosed(cb) {
  return listen("ssh-terminal-closed", (ev) =>
    cb({ sessionId: ev.payload.session_id, reason: ev.payload.reason })
  );
}

export function onHostkeyPrompt(cb) {
  return listen("ssh-hostkey-prompt", (ev) =>
    cb({
      sessionId: ev.payload.session_id,
      host: ev.payload.host,
      fingerprint: ev.payload.fingerprint,
    })
  );
}

// ---- base64 工具（UTF-8 安全）----
export function toBase64(str) {
  const bytes = new TextEncoder().encode(str);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

export function fromBase64(b64) {
  const bin = atob(b64);
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}
