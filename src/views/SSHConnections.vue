<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useRouter } from "vue-router";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import {
  listConnections,
  saveConnection,
  deleteConnection,
  testConnection,
  onHostkeyPrompt,
  acceptHostkey,
  rejectHostkey,
  importOpenSshConfig,
  exportOpenSshConfig,
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
import { Textarea } from "@/components/ui/textarea";
import { Spinner } from "@/components/ui/spinner";
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

const router = useRouter();

const conns = ref([]);
const loading = ref(true);
const testingId = ref(null);

// 新建/编辑对话框（null = 关闭）
const editing = ref(null);
const form = ref(emptyForm());

function emptyForm() {
  return {
    id: null,
    name: "",
    host: "",
    port: 22,
    username: "",
    auth_type: "password",
    secret: "",
    key_passphrase: "",
    group: "",
    tags: [],
    keepalive_secs: 30,
    jump_host_id: null,
  };
}

// 当前选中的分组 Tab（null = 全部）
const activeGroup = ref(null);
// 所有不重复的 group 列表
const allGroups = computed(() => {
  const gs = conns.value.map((c) => c.group).filter(Boolean);
  return [...new Set(gs)];
});

// 按 group 过滤后的连接列表
const filteredConns = computed(() => {
  if (!activeGroup.value) return conns.value;
  return conns.value.filter((c) => c.group === activeGroup.value);
});

// host key 首连确认对话框（null = 关闭）
const hostkeyPrompt = ref(null);
let unlistenHostkey = null;

async function refresh() {
  loading.value = true;
  try {
    conns.value = await listConnections();
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  await refresh();
  unlistenHostkey = await onHostkeyPrompt((p) => {
    hostkeyPrompt.value = p;
  });
});

onBeforeUnmount(() => {
  if (unlistenHostkey) unlistenHostkey();
});

function openCreate() {
  form.value = emptyForm();
  editing.value = { mode: "create" };
}

function openEdit(c) {
  form.value = {
    id: c.id,
    name: c.name,
    host: c.host,
    port: c.port,
    username: c.username,
    auth_type: c.auth_type,
    secret: "",
    key_passphrase: "",
    group: c.group || "",
    tags: c.tags || [],
    keepalive_secs: c.keepalive_secs || 30,
    jump_host_id: c.jump_host_id || null,
  };
  editing.value = { mode: "edit" };
}

function closeEdit() {
  editing.value = null;
}

async function onSave() {
  const f = form.value;
  if (!f.name || !f.host || !f.username || !f.secret) {
    showToast(t("ssh.secret_required"), "warning");
    return;
  }
  try {
    await saveConnection({
      id: f.id,
      name: f.name,
      host: f.host,
      port: Number(f.port) || 22,
      username: f.username,
      auth_type: f.auth_type,
      secret: f.secret,
      key_passphrase: f.auth_type === "private_key" && f.key_passphrase ? f.key_passphrase : null,
      group: f.group || null,
      tags: f.tags || [],
      keepalive_secs: Number(f.keepalive_secs) || 30,
      jump_host_id: f.jump_host_id || null,
    });
    showToast(t("ssh.save_success"), "success");
    closeEdit();
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onDelete(c) {
  if (!(await showConfirm(tFormat("ssh.delete_confirm", { name: c.name })))) return;
  try {
    await deleteConnection(c.id);
    await refresh();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function onTest(c) {
  testingId.value = c.id;
  try {
    await testConnection(c.id);
    showToast(t("ssh.test_success"), "success");
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    testingId.value = null;
  }
}

function openTerminal(c) {
  router.push({ path: "/ssh/sessions", query: { open: c.id } });
}

// 从 ~/.ssh/config 导入连接
async function onImportOpenSsh() {
  try {
    const hosts = await importOpenSshConfig();
    if (!hosts || hosts.length === 0) {
      showToast(t("ssh.openssh_empty"), "info");
      return;
    }
    // 打开导入确认对话框
    sshImportDialog.value = hosts;
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

// 确认导入选中的 SSH config 条目（secret 留空，后续编辑补充）
const sshImportDialog = ref(null);
const sshImportSelected = ref({});

async function onConfirmImport() {
  const selected = sshImportSelected.value;
  const selectedHosts = (sshImportDialog.value || []).filter((h) => selected[h.host]);
  if (selectedHosts.length === 0) {
    showToast(t("ssh.import_none_selected"), "warning");
    return;
  }
  sshImportDialog.value = null;
  let imported = 0;
  for (const h of selectedHosts) {
    try {
      await saveConnection({
        id: null,
        name: h.host,
        host: h.host_name || h.host,
        port: h.port || 22,
        username: h.user || "",
        auth_type: "password",
        secret: "",
        key_passphrase: null,
        group: null,
        tags: ["imported"],
        keepalive_secs: 30,
        jump_host_id: null,
      });
      imported++;
    } catch {
      // 忽略单条失败
    }
  }
  showToast(tFormat("ssh.import_done", { n: imported }), "success");
  await refresh();
}

// 导出选中连接为 ~/.ssh/config 格式
const exportDialog = ref(null);
const exportSelected = ref({});

function openExportDialog() {
  exportSelected.value = {};
  exportDialog.value = conns.value.map((c) => c.id);
}

async function onConfirmExport() {
  const ids = exportSelected.value;
  const selectedIds = (exportDialog.value || []).filter((id) => ids[id]);
  if (selectedIds.length === 0) {
    showToast(t("ssh.export_none_selected"), "warning");
    return;
  }
  try {
    const config = await exportOpenSshConfig(selectedIds);
    const { documentDir } = await import("@tauri-apps/api/path");
    const downloadsPath = await documentDir();
    const outPath = `${downloadsPath}/devnexus_ssh_config_${Date.now()}.txt`;
    await invoke("local_write_text", { path: outPath, content: config });
    showToast(tFormat("ssh.export_done", { path: outPath }), "success");
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    exportDialog.value = null;
  }
}

// 从本地文件导入私钥（~/.ssh/id_ed25519 之类）
async function importKeyFile() {
  let path;
  try {
    path = await open({
      multiple: false,
      directory: false,
      title: t("ssh.private_key"),
    });
  } catch {
    return; // 对话框失败/取消
  }
  if (!path) return;
  try {
    const content = await invoke("local_read_text", { path });
    if (!content.trim()) {
      showToast(t("ssh.key_file_empty"), "warning");
      return;
    }
    form.value.secret = content;
    const name = String(path).split("/").pop();
    showToast(`${name} ✓`, "success");
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
</script>

<template>
  <div class="page ssh-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("nav.ssh") }}</h1>
        <p class="page-desc">{{ t("ssh.connections") }}</p>
      </div>
      <div class="flex items-center gap-2">
        <Button size="sm" variant="outline" @click="openExportDialog">
          <AppIcon name="download" class="size-4" />
          {{ t("ssh.export") }}
        </Button>
        <Button size="sm" variant="outline" @click="onImportOpenSsh">
          <AppIcon name="upload" class="size-4" />
          {{ t("ssh.import_openssh") }}
        </Button>
        <Button @click="openCreate">
          <AppIcon name="plus" class="size-4" />
          {{ t("ssh.add") }}
        </Button>
      </div>
    </div>

    <!-- Group filter tabs -->
    <div v-if="allGroups.length > 0" class="group-tabs">
      <button
        class="group-tab"
        :class="{ active: activeGroup === null }"
        @click="activeGroup = null"
      >
        {{ t("ssh.all") }}
      </button>
      <button
        v-for="g in allGroups"
        :key="g"
        class="group-tab"
        :class="{ active: activeGroup === g }"
        @click="activeGroup = g"
      >
        {{ g }}
      </button>
    </div>

    <div v-if="loading" class="flex justify-center py-14">
      <Spinner />
    </div>

    <Empty v-else-if="filteredConns.length === 0" class="py-14">
      <EmptyMedia>
        <AppIcon name="server" class="size-10 text-muted-foreground/60" />
      </EmptyMedia>
      <EmptyContent>
        <EmptyDescription>
          <div>{{ t("ssh.empty") }}</div>
          <div class="empty-hint">{{ t("ssh.add_hint") }}</div>
        </EmptyDescription>
      </EmptyContent>
    </Empty>

    <div v-else class="conn-grid">
      <Card v-for="c in filteredConns" :key="c.id" class="shadow-sm">
        <CardContent class="conn-card">
          <div class="conn-title">
            <div class="conn-icon-wrap">
              <AppIcon name="server" class="size-5" />
            </div>
            <div class="min-w-0">
              <div class="conn-name">{{ c.name }}</div>
              <div class="conn-sub">{{ c.username }}@{{ c.host }}:{{ c.port }}</div>
              <div v-if="c.group || (c.tags && c.tags.length)" class="conn-meta">
                <span v-if="c.group" class="conn-group">{{ c.group }}</span>
                <span v-for="tag in (c.tags || [])" :key="tag" class="conn-tag">{{ tag }}</span>
                <span v-if="c.jump_host_id" class="conn-jump">
                  <AppIcon name="log-out" class="size-3" />
                  jump
                </span>
              </div>
            </div>
            <span class="conn-auth">
              {{ c.auth_type === "private_key" ? t("ssh.private_key") : t("ssh.password") }}
            </span>
          </div>
          <div class="conn-actions">
            <Button size="sm" @click="openTerminal(c)">
              <AppIcon name="terminal" class="size-4" />
              {{ t("ssh.open_terminal") }}
            </Button>
            <Button size="sm" variant="outline" :disabled="testingId === c.id" @click="onTest(c)">
              <Spinner v-if="testingId === c.id" class="size-4" />
              <AppIcon v-else name="check" class="size-4" />
              {{ testingId === c.id ? t("ssh.testing") : t("ssh.test") }}
            </Button>
            <Button size="sm" variant="outline" @click="openEdit(c)">
              <AppIcon name="edit" class="size-4" />
              {{ t("ssh.edit") }}
            </Button>
            <Button size="sm" variant="ghost" class="text-destructive" @click="onDelete(c)">
              <AppIcon name="delete" class="size-4" />
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>

    <!-- 新建/编辑连接 -->
    <Dialog :open="editing !== null" @update:open="(v) => !v && closeEdit()">
      <DialogContent class="max-h-[85vh] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ editing?.mode === "edit" ? t("ssh.edit") : t("ssh.add") }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <div>
            <Label for="ssh-name" class="mb-1.5 block">{{ t("ssh.name") }} *</Label>
            <Input id="ssh-name" v-model="form.name" placeholder="My Server" />
          </div>
          <div class="grid grid-cols-[1fr_100px] gap-3">
            <div>
              <Label for="ssh-host" class="mb-1.5 block">{{ t("ssh.host") }} *</Label>
              <Input id="ssh-host" v-model="form.host" placeholder="192.168.1.10" />
            </div>
            <div>
              <Label for="ssh-port" class="mb-1.5 block">{{ t("ssh.port") }}</Label>
              <Input id="ssh-port" v-model="form.port" type="number" min="1" max="65535" />
            </div>
          </div>
          <div>
            <Label for="ssh-username" class="mb-1.5 block">{{ t("ssh.username") }} *</Label>
            <Input id="ssh-username" v-model="form.username" placeholder="root" />
          </div>
          <div>
            <Label class="mb-1.5 block">{{ t("ssh.auth_type") }}</Label>
            <Select v-model="form.auth_type">
              <SelectTrigger class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="password">{{ t("ssh.password") }}</SelectItem>
                <SelectItem value="private_key">{{ t("ssh.private_key") }}</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <div class="mb-1.5 flex items-center justify-between">
              <Label for="ssh-secret">
                {{ form.auth_type === "private_key" ? t("ssh.private_key") : t("ssh.password") }} *
              </Label>
              <Button
                v-if="form.auth_type === 'private_key'"
                variant="ghost"
                size="sm"
                class="h-6 gap-1 px-2 text-xs"
                @click="importKeyFile"
              >
                <AppIcon name="folder-open" class="size-3.5" />
                {{ t("ssh.import_key") }}
              </Button>
            </div>
            <Textarea
              v-if="form.auth_type === 'private_key'"
              id="ssh-secret"
              v-model="form.secret"
              :rows="4"
              class="font-mono text-xs"
              placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
            />
            <Input
              v-else
              id="ssh-secret"
              v-model="form.secret"
              type="password"
              :placeholder="form.id ? t('ssh.secret_required') : ''"
            />
            <p v-if="form.id" class="mt-1 text-xs text-muted-foreground">
              {{ t("ssh.secret_required") }}
            </p>
          </div>
          <div v-if="form.auth_type === 'private_key'">
            <Label for="ssh-passphrase" class="mb-1.5 block">{{ t("ssh.key_passphrase") }}</Label>
            <Input id="ssh-passphrase" v-model="form.key_passphrase" type="password" />
          </div>
          <div>
            <Label for="ssh-group" class="mb-1.5 block">{{ t("ssh.group") }}</Label>
            <Input id="ssh-group" v-model="form.group" :placeholder="t('ssh.group_placeholder')" />
          </div>
          <div>
            <Label for="ssh-tags" class="mb-1.5 block">{{ t("ssh.tags") }}</Label>
            <Input id="ssh-tags" :model-value="(form.tags || []).join(', ')"
              @update:model-value="v => form.tags = v.split(',').map(s => s.trim()).filter(Boolean)"
              :placeholder="t('ssh.tags_placeholder')" />
            <p class="mt-1 text-xs text-muted-foreground">{{ t("ssh.tags_hint") }}</p>
          </div>
          <div>
            <Label for="ssh-keepalive" class="mb-1.5 block">{{ t("ssh.keepalive") }}</Label>
            <div class="flex items-center gap-2">
              <Input id="ssh-keepalive" v-model="form.keepalive_secs" type="number" min="0" max="300" class="w-24" />
              <span class="text-xs text-muted-foreground">{{ t("ssh.keepalive_unit") }}</span>
            </div>
          </div>
          <div>
            <Label class="mb-1.5 block">{{ t("ssh.jump_host") }}</Label>
            <Select v-model="form.jump_host_id">
              <SelectTrigger class="w-full">
                <SelectValue :placeholder="t('ssh.jump_host_none')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="null">{{ t("ssh.jump_host_none") }}</SelectItem>
                <SelectItem v-for="c in conns.filter(c => c.id !== form.id)" :key="c.id" :value="c.id">
                  {{ c.name }} ({{ c.username }}@{{ c.host }})
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="closeEdit">{{ t("common.cancel") }}</Button>
          <Button @click="onSave">{{ t("common.save") }}</Button>
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

    <!-- OpenSSH config 导入确认 -->
    <Dialog :open="sshImportDialog !== null" @update:open="(v) => !v && (sshImportDialog = null)">
      <DialogContent class="max-h-[80vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.import_openssh_title") }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-2">
          <p class="text-sm text-muted-foreground">{{ t("ssh.import_openssh_desc") }}</p>
          <div v-for="h in (sshImportDialog || [])" :key="h.host" class="flex items-center gap-3 p-2 rounded border">
            <input type="checkbox" :value="h.host" v-model="sshImportSelected[h.host]" class="accent-primary" />
            <div class="min-w-0">
              <div class="font-medium text-sm">{{ h.host }}</div>
              <div class="text-xs text-muted-foreground font-mono truncate">
                {{ h.user || '?' }}@{{ h.host_name || h.host }}{{ h.port && h.port !== 22 ? ':' + h.port : '' }}
              </div>
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="sshImportDialog = null">{{ t("common.cancel") }}</Button>
          <Button @click="onConfirmImport">{{ t("ssh.import_confirm") }} ({{ Object.values(sshImportSelected).filter(Boolean).length }})</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- 导出 OpenSSH config -->
    <Dialog :open="exportDialog !== null" @update:open="(v) => !v && (exportDialog = null)">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.export_title") }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-2">
          <p class="text-sm text-muted-foreground">{{ t("ssh.export_desc") }}</p>
          <div v-for="c in conns" :key="c.id" class="flex items-center gap-3 p-2 rounded border">
            <input type="checkbox" :value="c.id" v-model="exportSelected[c.id]" class="accent-primary" />
            <div class="min-w-0">
              <div class="font-medium text-sm">{{ c.name }}</div>
              <div class="text-xs text-muted-foreground font-mono truncate">{{ c.username }}@{{ c.host }}</div>
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="exportDialog = null">{{ t("common.cancel") }}</Button>
          <Button @click="onConfirmExport">{{ t("ssh.export_confirm") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<style scoped>
.conn-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 12px;
}

.conn-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
}

.conn-title {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.conn-icon-wrap {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  flex-shrink: 0;
  background-color: var(--color-muted);
  color: var(--color-primary);
}

.conn-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conn-sub {
  font-size: 12px;
  color: var(--color-muted-foreground);
  font-family: "JetBrains Mono", monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conn-auth {
  margin-left: auto;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 9999px;
  flex-shrink: 0;
  background-color: var(--color-muted);
  color: var(--color-muted-foreground);
}

.conn-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.conn-actions .text-destructive {
  margin-left: auto;
}

.empty-hint {
  margin-top: 4px;
  font-size: 12px;
  opacity: 0.7;
}

.group-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 0 12px;
  flex-wrap: wrap;
}

.group-tab {
  padding: 4px 12px;
  border-radius: 9999px;
  border: 1px solid var(--color-border);
  background: transparent;
  font-size: 12px;
  color: var(--color-muted-foreground);
  cursor: pointer;
  transition: all 0.15s;
}

.group-tab:hover {
  background: var(--color-sidebar-accent);
  color: var(--color-foreground);
}

.group-tab.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

.conn-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  flex-wrap: wrap;
}

.conn-group {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 9999px;
  background: var(--color-primary);
  color: #fff;
  font-weight: 500;
}

.conn-tag {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 9999px;
  background: var(--color-muted);
  color: var(--color-muted-foreground);
}

.conn-jump {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 9999px;
  background: var(--color-muted);
  color: var(--color-muted-foreground);
}
</style>
