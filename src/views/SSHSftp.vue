<script setup>
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import {
  listConnections,
  openSftp,
  listSftpDir,
  readSftpFile,
  writeSftpFile,
  mkdirSftp,
  renameSftp,
  deleteSftp,
  statSftp,
  chmodSftp,
  copyRecursiveSftp,
  rmRecursiveSftp,
  searchSftp,
  onHostkeyPrompt,
  acceptHostkey,
  rejectHostkey,
  aiSftp,
  aiSftpModels,
} from "../lib/api-ssh.js";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Progress } from "@/components/ui/progress";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

const conns = ref([]);
const connId = ref("");
const sftpId = ref(null);
const connecting = ref(false);
const loadingDir = ref(false);
const cwd = ref("/");
const entries = ref([]);

// 传输进度：{ kind: 'up' | 'down', name, done, total }
const transfer = ref(null);
const CHUNK = 256 * 1024;

// 输入对话框（新建文件夹 / 重命名）：{ title, value, onOk }
const prompt = ref(null);
const promptValue = ref("");

// host key 首连确认
const hostkeyPrompt = ref(null);
let unlistenHostkey = null;

// ── SFTP AI 助手 ──
const aiOpen = ref(true);
const aiModels = ref([]);
const aiModel = ref("");
const aiBusy = ref(false);
const aiMessages = ref([]); // { role, content, actions? }
const aiInput = ref("");

async function loadAiModels() {
  try {
    const models = await aiSftpModels();
    aiModels.value = models || [];
    if (!aiModel.value && aiModels.value.length) aiModel.value = aiModels.value[0].model;
  } catch {
    // 无 Provider 时静默，AI 面板发送时会提示
  }
}

async function sendAi() {
  const text = aiInput.value.trim();
  if (!text || aiBusy.value || !sftpId.value) return;
  if (!aiModels.value.length) {
    showToast(t("ssh.ai.noProvider"), "error");
    return;
  }
  aiBusy.value = true;
  aiInput.value = "";
  aiMessages.value.push({ role: "user", content: text });
  await nextTick();
  scrollAi();

  const history = aiMessages.value
    .filter((m) => m.role === "user" || m.role === "assistant")
    .map((m) => ({ role: m.role, content: m.content }));

  try {
    const res = await aiSftp({
      sftpId: sftpId.value,
      cwd: cwd.value,
      listing: entries.value,
      history,
      message: text,
      model: aiModel.value || null,
    });
    aiMessages.value.push({
      role: "assistant",
      content: res.reply,
      actions: res.actions || [],
    });
  } catch (err) {
    aiMessages.value.push({ role: "assistant", content: `⚠️ ${friendlyError(err)}`, actions: [] });
  } finally {
    aiBusy.value = false;
    await nextTick();
    scrollAi();
  }
}

function scrollAi() {
  const box = document.querySelector(".sftp-ai-messages");
  if (box) box.scrollTop = box.scrollHeight;
}

async function runAiAction(action) {
  if (!sftpId.value) return;
  try {
    if (action.action === "navigate" && action.path) {
      await cd(action.path);
    } else if (action.action === "rename" && action.from && action.to) {
      if (await showConfirm(tFormat("ssh.rename_confirm", { name: action.from }))) {
        await renameSftp(sftpId.value, action.from, action.to);
        await refresh();
      }
    } else if (action.action === "delete" && action.path) {
      const name = action.path.split("/").pop() || action.path;
      if (await showConfirm(tFormat("ssh.delete_confirm", { name }))) {
        await deleteSftp(sftpId.value, action.path, !!action.is_dir);
        await refresh();
      }
    } else if (action.action === "open" && action.path) {
      // 尝试下载该文件（复用下载逻辑）
      const name = action.path.split("/").pop() || action.path;
      const total = null;
      let local;
      try {
        local = await save({ defaultPath: name });
      } catch {
        return;
      }
      if (!local) return;
      let offset = 0;
      transfer.value = { kind: "down", name, done: 0, total: total || 0 };
      try {
        while (true) {
          const b64 = await readSftpFile(sftpId.value, action.path, offset, CHUNK);
          const bytes = b64ToBytes(b64);
          if (bytes.length === 0) break;
          await invoke("sftp_write_local_chunk", {
        path: local,
        dataB64: bytesToBase64(bytes),
        append: offset > 0,
      });
          offset += bytes.length;
          transfer.value.done = offset;
        }
        showToast(t("ssh.download") + " ✓ " + name, "success");
      } catch (err) {
        showToast(friendlyError(err), "error");
      } finally {
        transfer.value = null;
      }
    }
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

const busy = computed(() => transfer.value !== null);
const sortedEntries = computed(() =>
  [...entries.value].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
    return a.name.localeCompare(b.name);
  })
);
const crumbs = computed(() => {
  const parts = cwd.value.split("/").filter(Boolean);
  return [{ name: "/", path: "/" }].concat(
    parts.map((p, i) => ({ name: p, path: "/" + parts.slice(0, i + 1).join("/") }))
  );
});

// 二进制安全的 base64（勿用 TextEncoder 编码字节流）
function bytesToBase64(bytes) {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

function b64ToBytes(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

function join(base, name) {
  return (base === "/" ? "" : base) + "/" + name;
}

function parentOf(p) {
  if (p === "/") return "/";
  const idx = p.lastIndexOf("/");
  return idx <= 0 ? "/" : p.slice(0, idx);
}

function humanSize(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function fmtTime(mtime) {
  if (!mtime) return "-";
  return new Date(mtime * 1000).toLocaleString();
}

async function connect() {
  if (!connId.value || connecting.value) return;
  connecting.value = true;
  try {
    sftpId.value = await openSftp(connId.value);
    await cd("/");
  } catch (err) {
    sftpId.value = null;
    showToast(friendlyError(err), "error");
  } finally {
    connecting.value = false;
  }
}

async function disconnect() {
  // 后端暂无 ssh_sftp_close：SFTP 句柄随 SSH 会话生命周期回收，
  // 这里仅丢弃前端引用（不能调 ssh_close——会误杀同连接的终端会话）
  sftpId.value = null;
  entries.value = [];
  cwd.value = "/";
}

async function cd(p) {
  loadingDir.value = true;
  try {
    entries.value = await listSftpDir(sftpId.value, p);
    cwd.value = p;
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    loadingDir.value = false;
  }
}

function enter(entry) {
  if (entry.is_dir && !busy.value) cd(join(cwd.value, entry.name));
}

function goUp() {
  if (!busy.value) cd(parentOf(cwd.value));
}

async function refresh() {
  if (sftpId.value && !busy.value) await cd(cwd.value);
}

async function download(entry) {
  if (busy.value || entry.is_dir) return;
  const remote = join(cwd.value, entry.name);
  let local;
  try {
    local = await save({ defaultPath: entry.name });
  } catch {
    return; // 对话框失败/取消
  }
  if (!local) return;

  const total = entry.size;
  let offset = 0;
  transfer.value = { kind: "down", name: entry.name, done: 0, total };
  try {
    while (offset < total) {
      const b64 = await readSftpFile(sftpId.value, remote, offset, CHUNK);
      const bytes = b64ToBytes(b64);
      if (bytes.length === 0) break; // 提前 EOF（文件被截断）
      await invoke("sftp_write_local_chunk", {
        path: local,
        dataB64: bytesToBase64(bytes),
        append: offset > 0,
      });
      offset += bytes.length;
      transfer.value.done = offset;
    }
    showToast(t("ssh.download") + " ✓ " + entry.name, "success");
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    transfer.value = null;
  }
}

async function upload(file) {
  if (busy.value) return;
  const data = new Uint8Array(await file.arrayBuffer());
  const remote = join(cwd.value, file.name);
  const total = data.byteLength;
  transfer.value = { kind: "up", name: file.name, done: 0, total };
  try {
    let offset = 0;
    while (offset < total) {
      const chunk = data.subarray(offset, offset + CHUNK);
      await writeSftpFile(sftpId.value, remote, bytesToBase64(chunk), offset);
      offset += chunk.length;
      transfer.value.done = offset;
    }
    showToast(t("ssh.upload") + " ✓ " + file.name, "success");
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    transfer.value = null;
  }
}

function onDrop(e) {
  if (!sftpId.value || busy.value) return;
  const files = [...(e.dataTransfer?.files || [])];
  for (const f of files) upload(f);
}

function onPickFiles(e) {
  const files = [...(e.target.files || [])];
  for (const f of files) upload(f);
  e.target.value = "";
}

function openMkdir() {
  promptValue.value = "";
  prompt.value = {
    title: t("ssh.new_folder"),
    onOk: async () => {
      const name = promptValue.value.trim();
      if (!name) return false;
      await mkdirSftp(sftpId.value, join(cwd.value, name));
      await refresh();
      return true;
    },
  };
}

function openRename(entry) {
  promptValue.value = entry.name;
  prompt.value = {
    title: t("ssh.rename"),
    onOk: async () => {
      const name = promptValue.value.trim();
      if (!name || name === entry.name) return true;
      await renameSftp(sftpId.value, join(cwd.value, entry.name), join(cwd.value, name));
      await refresh();
      return true;
    },
  };
}

async function onDelete(entry) {
  if (!(await showConfirm(tFormat("ssh.delete_confirm", { name: entry.name })))) return;
  try {
    await deleteSftp(sftpId.value, join(cwd.value, entry.name), entry.is_dir);
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

// ── 权限编辑（chmod）──
const chmod = ref(null); // { path, modeStr } 对话框
function openChmod(entry) {
  chmod.value = {
    path: join(cwd.value, entry.name),
    name: entry.name,
    modeStr: (entry.mode & 0o7777).toString(8).padStart(3, "0"),
  };
}
async function onChmodOk() {
  if (!chmod.value) return;
  const mode = parseInt(chmod.value.modeStr, 8);
  if (Number.isNaN(mode)) {
    showToast(t("ssh.chmod_invalid"), "error");
    return;
  }
  try {
    await chmodSftp(sftpId.value, chmod.value.path, mode);
    chmod.value = null;
    showToast(t("ssh.chmod_done"), "success");
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

// ── 文件搜索（远端 find）──
const searchDialog = ref(null); // { pattern, maxDepth }
const searchResults = ref([]);
const searchBusy = ref(false);
function openSearch() {
  searchDialog.value = { pattern: "", maxDepth: null };
  searchResults.value = [];
}
async function onSearchOk() {
  if (!searchDialog.value) return;
  const pattern = searchDialog.value.pattern.trim();
  if (!pattern) {
    showToast(t("ssh.search_empty"), "error");
    return;
  }
  searchBusy.value = true;
  searchResults.value = [];
  try {
    const res = await searchSftp(sftpId.value, cwd.value, pattern, searchDialog.value.maxDepth || null);
    searchResults.value = res || [];
    if (searchResults.value.length === 0) showToast(t("ssh.search_no_result"), "info");
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    searchBusy.value = false;
  }
}
function gotoResult(p) {
  const parent = parentOf(p);
  searchDialog.value = null;
  cd(parent).then(() => {
    const name = p.split("/").pop();
    showToast(t("ssh.search_in") + " " + p, "success");
    void name;
  });
}

// ── 复制/移动 → 目标目录（dialog）──
const moveDialog = ref(null); // { entry, action: 'copy'|'move', dest }
function openCopy(entry, action) {
  moveDialog.value = {
    entry,
    action,
    src: join(cwd.value, entry.name),
    dest: cwd.value,
  };
}
async function onMoveOk() {
  const d = moveDialog.value;
  if (!d) return;
  try {
    const destDir = d.dest.trim() || "/";
    const target = join(destDir, d.entry.name);
    if (d.action === "copy") {
      await copyRecursiveSftp(sftpId.value, d.src, target, false);
      showToast(t("ssh.copy_done") + " ✓ " + d.entry.name, "success");
    } else {
      // 移动 = 复制 + 删除源
      await copyRecursiveSftp(sftpId.value, d.src, target, false);
      if (d.entry.is_dir) await rmRecursiveSftp(sftpId.value, d.src);
      else await deleteSftp(sftpId.value, d.src, false);
      showToast(t("ssh.move_done") + " ✓ " + d.entry.name, "success");
    }
    moveDialog.value = null;
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onPromptOk() {
  const p = prompt.value;
  if (!p) return;
  try {
    const ok = await p.onOk();
    if (ok !== false) prompt.value = null;
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onHostkeyAccept() {
  const p = hostkeyPrompt.value;
  hostkeyPrompt.value = null;
  try {
    await acceptHostkey(p.sessionId, p.host, p.fingerprint);
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onHostkeyReject() {
  const p = hostkeyPrompt.value;
  hostkeyPrompt.value = null;
  try {
    await rejectHostkey(p.sessionId);
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

const transferPercent = computed(() =>
  transfer.value && transfer.value.total > 0
    ? Math.round((transfer.value.done / transfer.value.total) * 100)
    : 0
);

onMounted(async () => {
  unlistenHostkey = await onHostkeyPrompt((p) => {
    hostkeyPrompt.value = p;
  });
  loadAiModels();
  try {
    conns.value = await listConnections();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
});

onBeforeUnmount(() => {
  if (unlistenHostkey) unlistenHostkey();
});
</script>

<template>
  <div class="page sftp-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("ssh.sftp") }}</h1>
        <p class="page-desc">{{ t("nav.ssh") }}</p>
      </div>
      <div class="flex items-center gap-2">
        <Select v-model="connId" :disabled="!!sftpId">
          <SelectTrigger class="w-[200px]">
            <SelectValue :placeholder="t('ssh.connections')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="c in conns" :key="c.id" :value="c.id">
              {{ c.name }}
            </SelectItem>
          </SelectContent>
        </Select>
        <Button v-if="!sftpId" :disabled="!connId || connecting" @click="connect">
          <Spinner v-if="connecting" class="size-4" />
          <AppIcon v-else name="play-arrow" class="size-4" />
          {{ t("ssh.connect") }}
        </Button>
        <Button v-else variant="outline" @click="disconnect">
          <AppIcon name="close-circle-fill" class="size-4" />
          {{ t("ssh.disconnect") }}
        </Button>
      </div>
    </div>

    <!-- 未连接 -->
    <Empty v-if="!sftpId" class="py-14">
      <EmptyMedia>
        <AppIcon name="folder" class="size-10 text-muted-foreground/60" />
      </EmptyMedia>
      <EmptyContent>
        <EmptyDescription>
          <div>{{ t("ssh.not_connected") }}</div>
          <div class="empty-hint">{{ t("ssh.sftp_hint") }}</div>
        </EmptyDescription>
      </EmptyContent>
    </Empty>

    <!-- 文件浏览器 -->
    <div v-else class="sftp-ai-layout">
      <Card class="shadow-sm drop-zone flex-1 min-w-0" @dragover.prevent @drop.prevent="onDrop">
      <CardContent class="p-0">
        <!-- 工具栏 -->
        <div class="sftp-toolbar">
          <div class="crumbs">
            <template v-for="(crumb, i) in crumbs" :key="crumb.path">
              <button type="button" class="crumb" @click="cd(crumb.path)">
                {{ crumb.name === "/" ? " / " : crumb.name }}
              </button>
              <span v-if="i < crumbs.length - 1" class="crumb-sep">/</span>
            </template>
          </div>
          <div class="flex items-center gap-1.5">
            <Button size="sm" variant="ghost" :title="t('ssh.up')" :disabled="busy" @click="goUp">
              <AppIcon name="arrow-up" class="size-4" />
            </Button>
            <Button size="sm" variant="ghost" :title="t('ssh.refresh')" :disabled="busy" @click="refresh">
              <AppIcon name="refresh" class="size-4" />
            </Button>
            <Button size="sm" variant="ghost" :title="t('ssh.search_files')" :disabled="busy" @click="openSearch">
              <AppIcon name="search" class="size-4" />
            </Button>
            <Button size="sm" variant="ghost" :title="t('ssh.new_folder')" :disabled="busy" @click="openMkdir">
              <AppIcon name="plus" class="size-4" />
            </Button>
            <label class="cursor-pointer">
              <span
                class="inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-sm font-medium transition-colors hover:bg-accent"
                :class="{ 'pointer-events-none opacity-50': busy }"
              >
                <AppIcon name="upload" class="size-4" />
              </span>
              <input type="file" multiple class="sr-only" :disabled="busy" @change="onPickFiles" />
            </label>
          </div>
        </div>

        <div v-if="loadingDir" class="flex justify-center py-10">
          <Spinner />
        </div>

        <Empty v-else-if="entries.length === 0" class="py-10">
          <EmptyContent>
            <EmptyDescription>{{ t("ssh.empty_dir") }}</EmptyDescription>
          </EmptyContent>
        </Empty>

        <Table v-else>
          <TableHeader>
            <TableRow>
              <TableHead>{{ t("ssh.name") }}</TableHead>
              <TableHead class="w-[110px]">{{ t("ssh.size") }}</TableHead>
              <TableHead class="w-[180px]">{{ t("ssh.modified") }}</TableHead>
              <TableHead class="w-[130px] text-right">{{ t("common.actions") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="e in sortedEntries"
              :key="e.name"
              class="entry-row"
              :class="{ dir: e.is_dir }"
              @dblclick="enter(e)"
            >
              <TableCell>
                <span class="entry-name" @click="enter(e)">
                  <AppIcon :name="e.is_dir ? 'folder' : 'file'" class="size-4 shrink-0" />
                  <span class="truncate">{{ e.name }}</span>
                </span>
              </TableCell>
              <TableCell class="text-muted-foreground">
                {{ e.is_dir ? "-" : humanSize(e.size) }}
              </TableCell>
              <TableCell class="text-muted-foreground">{{ fmtTime(e.mtime) }}</TableCell>
              <TableCell class="text-right">
                <div class="flex items-center justify-end gap-0.5">
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.chmod')"
                  :disabled="busy"
                  @click="openChmod(e)"
                >
                  <AppIcon name="shield" class="size-3.5" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.copy_to')"
                  :disabled="busy"
                  @click="openCopy(e, 'copy')"
                >
                  <AppIcon name="copy" class="size-3.5" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.move_to')"
                  :disabled="busy"
                  @click="openCopy(e, 'move')"
                >
                  <AppIcon name="move" class="size-3.5" />
                </Button>
                <Button
                  v-if="!e.is_dir"
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.download')"
                  :disabled="busy"
                  @click="download(e)"
                >
                  <AppIcon name="download" class="size-4" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  :title="t('ssh.rename')"
                  :disabled="busy"
                  @click="openRename(e)"
                >
                  <AppIcon name="edit" class="size-4" />
                </Button>
                <Button
                  size="icon-sm"
                  variant="ghost"
                  class="text-destructive"
                  :title="t('ssh.delete')"
                  :disabled="busy"
                  @click="onDelete(e)"
                >
                  <AppIcon name="delete" class="size-4" />
                </Button>
                </div>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

    <!-- SFTP AI 助手面板 -->
    <aside class="sftp-ai-panel" :class="{ collapsed: !aiOpen }">
      <div class="sftp-ai-head">
        <div class="sftp-ai-title">
          <AppIcon name="sparkles" class="size-4" />
          <span>{{ t("ssh.ai.title") }}</span>
        </div>
        <button class="sftp-ai-toggle" @click="aiOpen = !aiOpen">
          <AppIcon :name="aiOpen ? 'panel-right-close' : 'panel-right-open'" class="size-4" />
        </button>
      </div>
      <div v-if="aiOpen" class="sftp-ai-body">
        <div class="sftp-ai-models">
          <Select v-model="aiModel">
            <SelectTrigger class="w-full">
              <SelectValue :placeholder="t('ssh.ai.pickModel')" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="m in aiModels" :key="m.model + m.provider" :value="m.model">
                {{ m.model }} · {{ m.provider }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="sftp-ai-messages">
          <div v-if="!aiMessages.length" class="sftp-ai-empty">
            {{ t("ssh.ai.sftpHint") }}
          </div>
          <div v-for="(m, i) in aiMessages" :key="i" class="sftp-ai-msg" :class="m.role">
            <div class="sftp-ai-msg-role">{{ m.role === 'user' ? t('ssh.ai.you') : t('ssh.ai.assistant') }}</div>
            <div class="sftp-ai-msg-text">{{ m.content }}</div>
            <div v-if="m.actions && m.actions.length" class="sftp-ai-actions">
              <div v-for="(act, ai) in m.actions" :key="ai" class="sftp-ai-action">
                <code class="sftp-ai-action-code">{{ act.action }} {{ act.path || '' }}</code>
                <Button size="sm" variant="outline" @click="runAiAction(act)">
                  <AppIcon name="play" class="size-3.5" />
                  {{ t("ssh.ai.run") }}
                </Button>
              </div>
            </div>
          </div>
        </div>
        <div class="sftp-ai-input">
          <Input
            v-model="aiInput"
            :placeholder="t('ssh.ai.inputPlaceholder')"
            @keydown.enter.prevent="sendAi"
          />
          <Button :disabled="aiBusy || !aiInput.trim()" @click="sendAi">
            <Spinner v-if="aiBusy" class="size-3.5" />
            <AppIcon v-else name="send" class="size-4" />
            {{ t("ssh.ai.send") }}
          </Button>
        </div>
      </div>
    </aside>
  </div>

    <!-- 传输进度 -->
    <div v-if="transfer" class="transfer-bar">
      <AppIcon :name="transfer.kind === 'up' ? 'upload' : 'download'" class="size-4 shrink-0" />
      <span class="transfer-name">{{ transfer.name }}</span>
      <span class="transfer-kind">
        {{ transfer.kind === "up" ? t("ssh.uploading") : t("ssh.downloading") }}
      </span>
      <Progress :model-value="transferPercent" class="flex-1" />
      <span class="transfer-pct">{{ transferPercent }}%</span>
    </div>

    <!-- 新建文件夹 / 重命名 -->
    <Dialog :open="prompt !== null" @update:open="(v) => !v && (prompt = null)">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ prompt?.title }}</DialogTitle>
        </DialogHeader>
        <div>
          <Label for="sftp-prompt" class="mb-1.5 block">{{ t("ssh.name") }}</Label>
          <Input
            id="sftp-prompt"
            v-model="promptValue"
            @keydown.enter="onPromptOk"
          />
        </div>
        <DialogFooter>
          <Button variant="outline" @click="prompt = null">{{ t("common.cancel") }}</Button>
          <Button @click="onPromptOk">{{ t("common.confirm") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- host key 首连确认 -->
    <Dialog :open="hostkeyPrompt !== null" @update:open="(v) => !v && onHostkeyReject()">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.hostkey_title") }}</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground break-all">
          {{
            tFormat("ssh.hostkey_body", {
              host: hostkeyPrompt?.host || "",
              fingerprint: hostkeyPrompt?.fingerprint || "",
            })
          }}
        </p>
        <DialogFooter>
          <Button variant="outline" @click="onHostkeyReject">{{ t("ssh.reject") }}</Button>
          <Button @click="onHostkeyAccept">{{ t("ssh.accept") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 权限编辑（chmod） -->
    <Dialog :open="chmod !== null" @update:open="(v) => !v && (chmod = null)">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.chmod") }} — {{ chmod?.name }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <div>
            <Label for="ssh-chmod-mode" class="mb-1.5 block">{{ t("ssh.chmod_mode") }}</Label>
            <div class="flex items-center gap-2">
              <Input id="ssh-chmod-mode" v-model="chmod.modeStr" class="w-24 font-mono" placeholder="755" />
              <span class="text-xs text-muted-foreground">{{ t("ssh.chmod_octal") }}</span>
            </div>
          </div>
          <p class="text-xs text-muted-foreground">
            {{ t("ssh.chmod_hint") }}
          </p>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="chmod = null">{{ t("common.cancel") }}</Button>
          <Button @click="onChmodOk">{{ t("common.confirm") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 复制/移动 到目录 -->
    <Dialog :open="moveDialog !== null" @update:open="(v) => !v && (moveDialog = null)">
      <DialogContent class="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{{ moveDialog?.action === 'copy' ? t('ssh.copy_to') : t('ssh.move_to') }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <p class="text-sm">{{ moveDialog?.entry?.name }}</p>
          <div>
            <Label for="sftp-move-dest" class="mb-1.5 block">{{ t("ssh.dest_path") }}</Label>
            <Input id="sftp-move-dest" v-model="moveDialog.dest" placeholder="/" />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="moveDialog = null">{{ t("common.cancel") }}</Button>
          <Button @click="onMoveOk">{{ t("common.confirm") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 文件搜索 -->
    <Dialog :open="searchDialog !== null" @update:open="(v) => !v && (searchDialog = null)">
      <DialogContent class="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.search_files") }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <div class="flex items-center gap-2">
            <Input v-model="searchDialog.pattern" :placeholder="t('ssh.search_placeholder')" class="flex-1" @keydown.enter="onSearchOk" />
            <Button :disabled="searchBusy" @click="onSearchOk">
              <Spinner v-if="searchBusy" class="size-3.5" />
              <AppIcon v-else name="search" class="size-3.5" />
              {{ t("ssh.search") }}
            </Button>
          </div>
          <div class="flex items-center gap-2 text-xs text-muted-foreground">
            <Label for="sftp-search-depth" class="shrink-0">{{ t("ssh.search_maxdepth") }}</Label>
            <Input id="sftp-search-depth" v-model="searchDialog.maxDepth" type="number" min="1" max="20" class="w-16" placeholder="5" />
          </div>
          <div v-if="searchResults.length" class="sftp-search-results">
            <div v-for="(r, ri) in searchResults" :key="ri" class="sftp-search-result" @click="gotoResult(r)">
              <AppIcon name="file" class="size-3.5 shrink-0" />
              <span class="truncate font-mono text-xs">{{ r }}</span>
            </div>
          </div>
          <p v-if="!searchResults.length && !searchBusy" class="text-xs text-muted-foreground">
            {{ t("ssh.search_no_result") }}
          </p>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="searchDialog = null">{{ t("common.close") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<style scoped>
.sftp-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--color-border);
  flex-wrap: wrap;
}

.crumbs {
  display: flex;
  align-items: center;
  gap: 2px;
  font-size: 13px;
  min-width: 0;
  overflow-x: auto;
  white-space: nowrap;
}

.crumb {
  border: none;
  background: transparent;
  color: var(--color-muted-foreground);
  padding: 2px 4px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}

.crumb:hover {
  background-color: var(--color-accent);
  color: var(--color-foreground);
}

.crumb:last-child {
  color: var(--color-foreground);
  font-weight: 500;
}

.crumb-sep {
  color: var(--color-muted-foreground);
  opacity: 0.5;
}

.entry-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  cursor: pointer;
}

.entry-row.dir .entry-name:hover {
  color: var(--color-primary);
}

.sftp-search-results {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 260px;
  overflow-y: auto;
}
.sftp-search-result {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  cursor: pointer;
  color: var(--color-foreground);
  transition: background-color 0.12s ease;
}
.sftp-search-result:hover {
  background-color: var(--color-muted);
}

.drop-zone {
  transition: outline 0.12s ease;
}

.transfer-bar {
  position: sticky;
  bottom: 8px;
  margin-top: 12px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  background-color: var(--color-card);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

.transfer-name {
  font-size: 13px;
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.transfer-kind {
  font-size: 12px;
  color: var(--color-muted-foreground);
  flex-shrink: 0;
}

.transfer-pct {
  font-size: 12px;
  font-family: "JetBrains Mono", monospace;
  color: var(--color-muted-foreground);
  width: 42px;
  text-align: right;
  flex-shrink: 0;
}

.empty-hint {
  margin-top: 4px;
  font-size: 12px;
  opacity: 0.7;
}

/* ── SFTP AI 助手布局 ─────────────────────────────────────── */
.sftp-ai-layout {
  display: flex;
  gap: 12px;
  min-height: 0;
  flex: 1;
}

.sftp-ai-panel {
  width: 330px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-card);
  overflow: hidden;
}
.sftp-ai-panel.collapsed {
  width: 44px;
}
.sftp-ai-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
  background-color: var(--color-muted);
}
.sftp-ai-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-foreground);
}
.sftp-ai-toggle {
  border: none;
  background: transparent;
  color: var(--color-muted-foreground);
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
  display: inline-flex;
}
.sftp-ai-toggle:hover {
  background-color: var(--color-border);
  color: var(--color-foreground);
}
.sftp-ai-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}
.sftp-ai-models {
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
}
.sftp-ai-messages {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  scrollbar-width: thin;
}
.sftp-ai-empty {
  margin: auto;
  text-align: center;
  font-size: 12px;
  color: var(--color-muted-foreground);
  padding: 20px;
}
.sftp-ai-msg {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.sftp-ai-msg.user .sftp-ai-msg-role {
  color: var(--color-primary);
}
.sftp-ai-msg.assistant .sftp-ai-msg-role {
  color: var(--color-success);
}
.sftp-ai-msg-role {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.sftp-ai-msg-text {
  font-size: 12px;
  line-height: 1.55;
  color: var(--color-foreground);
  white-space: pre-wrap;
  word-break: break-word;
}
.sftp-ai-actions {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 4px;
}
.sftp-ai-action {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background-color: var(--color-muted);
}
.sftp-ai-action-code {
  flex: 1;
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-foreground);
  white-space: pre-wrap;
  word-break: break-all;
}
.sftp-ai-input {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--color-border);
}
</style>
