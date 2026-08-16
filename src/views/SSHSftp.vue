<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import {
  listConnections,
  openSftp,
  listSftpDir,
  readSftpFile,
  writeSftpFile,
  mkdirSftp,
  renameSftp,
  deleteSftp,
  onHostkeyPrompt,
  acceptHostkey,
  rejectHostkey,
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
      await writeFile(local, bytes, { append: offset > 0, create: true });
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
    <Card v-else class="shadow-sm drop-zone" @dragover.prevent @drop.prevent="onDrop">
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
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>

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
</style>
