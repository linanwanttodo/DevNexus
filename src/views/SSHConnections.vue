<script setup>
import { ref, onMounted, onBeforeUnmount } from "vue";
import { useRouter } from "vue-router";
import {
  listConnections,
  saveConnection,
  deleteConnection,
  testConnection,
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
  };
}

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
      <Button @click="openCreate">
        <AppIcon name="plus" class="size-4" />
        {{ t("ssh.add") }}
      </Button>
    </div>

    <div v-if="loading" class="flex justify-center py-14">
      <Spinner />
    </div>

    <Empty v-else-if="conns.length === 0" class="py-14">
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
      <Card v-for="c in conns" :key="c.id" class="shadow-sm">
        <CardContent class="conn-card">
          <div class="conn-title">
            <div class="conn-icon-wrap">
              <AppIcon name="server" class="size-5" />
            </div>
            <div class="min-w-0">
              <div class="conn-name">{{ c.name }}</div>
              <div class="conn-sub">{{ c.username }}@{{ c.host }}:{{ c.port }}</div>
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
            <Label for="ssh-secret" class="mb-1.5 block">
              {{ form.auth_type === "private_key" ? t("ssh.private_key") : t("ssh.password") }} *
            </Label>
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
</style>
