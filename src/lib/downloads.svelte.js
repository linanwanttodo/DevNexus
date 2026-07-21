import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { showToast } from "./toast.svelte.js";

let downloads = $state([]);
let config = $state(null);
let monitorUnsub = $state(null);

export function getDownloads() {
  return downloads;
}

export function getConfig() {
  return config;
}

export async function loadDownloads() {
  try {
    const list = await invoke("get_downloads");
    downloads = list || [];
  } catch (err) {
    console.error("Failed to load downloads:", err);
  }
}

export async function loadConfig() {
  try {
    config = await invoke("get_download_config");
  } catch (err) {
    console.error("Failed to load config:", err);
  }
}

export async function saveConfig(newConfig) {
  try {
    await invoke("set_download_config", { config: newConfig });
    config = newConfig;
  } catch (err) {
    console.error("Failed to save config:", err);
    throw err;
  }
}

export async function addDownload(url, savePath = null) {
  try {
    const taskId = await invoke("create_download", { url, savePath });
    await invoke("start_download", { taskId });
    await loadDownloads();
    return taskId;
  } catch (err) {
    showToast(`Failed to create download: ${err}`);
    throw err;
  }
}

export async function startDownload(taskId) {
  try {
    await invoke("start_download", { taskId });
  } catch (err) {
    showToast(`Failed to start: ${err}`);
  }
}

export async function pauseDownload(taskId) {
  try {
    await invoke("pause_download", { taskId });
  } catch (err) {
    showToast(`Failed to pause: ${err}`);
  }
}

export async function resumeDownload(taskId) {
  try {
    await invoke("resume_download", { taskId });
  } catch (err) {
    showToast(`Failed to resume: ${err}`);
  }
}

export async function cancelDownload(taskId) {
  try {
    await invoke("cancel_download", { taskId });
    await loadDownloads();
  } catch (err) {
    showToast(`Failed to cancel: ${err}`);
  }
}

export async function deleteDownload(taskId, deleteFile = false) {
  try {
    await invoke("delete_download", { taskId, deleteFile });
    await loadDownloads();
  } catch (err) {
    showToast(`Failed to delete: ${err}`);
  }
}

export async function startMonitor() {
  if (monitorUnsub) return;
  try {
    const unsub = await listen("download-progress", (event) => {
      const progress = event.payload;
      const idx = downloads.findIndex((d) => d.id === progress.task_id);
      if (idx !== -1) {
        downloads[idx] = { ...downloads[idx], ...progress };
      }
    });
    monitorUnsub = unsub;
  } catch (err) {
    console.error("Failed to start monitor:", err);
  }
}

export async function stopMonitor() {
  if (monitorUnsub) {
    try {
      await monitorUnsub();
    } catch {}
    monitorUnsub = null;
  }
}

export function formatBytes(bytes) {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return (bytes / Math.pow(k, i)).toFixed(1) + " " + sizes[i];
}

export function formatSpeed(bytesPerSec) {
  return formatBytes(bytesPerSec) + "/s";
}

export function formatETA(seconds) {
  if (!seconds || seconds === 0) return "--";
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}m ${s}s`;
  }
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${h}h ${m}m`;
}

export async function getGithubMirrors() {
  try {
    return await invoke("get_github_mirrors");
  } catch {
    return [];
  }
}

export async function saveGithubMirrors(mirrors) {
  try {
    await invoke("save_github_mirrors", { mirrors });
  } catch (err) {
    console.error("Failed to save mirrors:", err);
  }
}

export function isGithubUrl(url) {
  return url.startsWith("https://github.com/") || url.startsWith("http://github.com/");
}

export async function readClipboard() {
  try {
    const text = await navigator.clipboard.readText();
    if (text && (text.startsWith("http://") || text.startsWith("https://"))) return text;
  } catch {}
  return "";
}

export function getStatusInfo(status) {
  switch (status) {
    case "Pending":
      return { label: "Pending", icon: "schedule", color: "text-nx-text-muted" };
    case "Downloading":
      return { label: "Downloading", icon: "download", color: "text-nx-accent" };
    case "Paused":
      return { label: "Paused", icon: "pause_circle", color: "text-nx-warning" };
    case "Completed":
      return { label: "Completed", icon: "check_circle", color: "text-nx-success" };
    case "Failed":
      return { label: "Failed", icon: "error", color: "text-nx-error" };
    case "Cancelled":
      return { label: "Cancelled", icon: "cancel", color: "text-nx-text-muted" };
    default:
      return { label: status, icon: "help", color: "text-nx-text-muted" };
  }
}