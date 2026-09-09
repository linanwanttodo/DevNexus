<script setup>
import { ref, onMounted, nextTick } from "vue";
import { useRouter } from "vue-router";
import {
  aiListProviders,
  aiAddProvider,
  aiUpdateProvider,
  aiDeleteProvider,
  aiListModels,
  aiListTerminals,
  aiChat,
  aiExecute,
} from "../lib/api-ssh.js";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Spinner } from "@/components/ui/spinner";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

const router = useRouter();
const at = (k) => t(`ssh.ai.assistant.${k}`);

// ── 服务配置 ──
const providers = ref([]);
const editing = ref(null); // null = 新增，否则为原 name
const form = ref({ name: "", baseUrl: "", apiKey: "", protocol: "openai", models: "", enabled: true });
const formBusy = ref(false);

async function loadProviders() {
  try {
    providers.value = (await aiListProviders()) || [];
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

function startAdd() {
  editing.value = null;
  form.value = { name: "", baseUrl: "", apiKey: "", protocol: "openai", models: "", enabled: true };
}

function startEdit(p) {
  editing.value = p.name;
  form.value = {
    name: p.name,
    baseUrl: p.base_url,
    apiKey: p.api_key || "",
    protocol: p.protocol || "openai",
    models: (p.models || []).join(", "),
    enabled: p.enabled !== false,
  };
}

async function saveProvider() {
  const f = form.value;
  if (!f.name.trim() || !f.baseUrl.trim()) {
    showToast(at("required"), "error");
    return;
  }
  const payload = {
    name: f.name.trim(),
    baseUrl: f.baseUrl.trim(),
    apiKey: f.apiKey,
    protocol: f.protocol,
    models: f.models.split(/[,，\n]/).map((s) => s.trim()).filter(Boolean),
  };
  formBusy.value = true;
  try {
    if (editing.value === null) {
      await aiAddProvider(payload);
    } else {
      await aiUpdateProvider({ ...payload, enabled: f.enabled });
    }
    showToast(at("saved") + " ✓", "success");
    startAdd();
    await loadProviders();
    await loadModels();
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    formBusy.value = false;
  }
}

async function toggleEnabled(p) {
  try {
    await aiUpdateProvider({
      name: p.name,
      baseUrl: p.base_url,
      apiKey: p.api_key || "",
      protocol: p.protocol || "openai",
      models: p.models || [],
      enabled: !p.enabled,
    });
    await loadProviders();
    await loadModels();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function removeProvider(p) {
  if (!(await showConfirm(at("deleteConfirm")))) return;
  try {
    await aiDeleteProvider(p.name);
    if (editing.value === p.name) startAdd();
    showToast(at("deleted") + " ✓", "success");
    await loadProviders();
    await loadModels();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

// ── 对话 ──
const aiModels = ref([]);
const aiSelectedModel = ref("");
const terminals = ref([]);
const targetTerm = ref("");
const useContext = ref(true);
const aiMessages = ref([]);
const aiInput = ref("");
const aiBusy = ref(false);
const pendingDanger = ref(null);

async function loadModels() {
  try {
    aiModels.value = (await aiListModels()) || [];
    if (!aiSelectedModel.value && aiModels.value.length) {
      aiSelectedModel.value = aiModels.value[0].model;
    }
  } catch {
    // 无服务时静默，发送时提示
  }
}

async function loadTerminals() {
  try {
    terminals.value = (await aiListTerminals()) || [];
    if (targetTerm.value && !terminals.value.some((x) => x.term_id === targetTerm.value)) {
      targetTerm.value = "";
    }
  } catch {
    terminals.value = [];
  }
}

function scrollToBottom() {
  const box = document.querySelector(".ssh-ai-messages");
  if (box) box.scrollTop = box.scrollHeight;
}

async function sendMessage() {
  const text = aiInput.value.trim();
  if (!text || aiBusy.value) return;
  if (!aiModels.value.length) {
    showToast(t("ssh.ai.noProvider"), "error");
    return;
  }
  aiBusy.value = true;
  aiInput.value = "";
  aiMessages.value.push({ role: "user", content: text });
  await nextTick();
  scrollToBottom();

  const history = aiMessages.value
    .filter((m) => m.role === "user" || m.role === "assistant")
    .map((m) => ({ role: m.role, content: m.content }));

  try {
    const res = await aiChat({
      termId: useContext.value && targetTerm.value ? targetTerm.value : null,
      history,
      message: text,
      model: aiSelectedModel.value || null,
    });
    const reply = res.reply || "";
    const cmds = res.commands || [];
    const flags = res.dangerous_flags || cmds.map(() => !!res.dangerous);
    aiMessages.value.push({
      role: "assistant",
      content: reply,
      commands: cmds,
      dangerous_flags: flags,
      dangerous: !!res.dangerous,
      model: res.model,
      provider: res.provider,
    });
  } catch (err) {
    showToast(friendlyError(err), "error");
    aiMessages.value.push({ role: "assistant", content: `⚠️ ${friendlyError(err)}`, commands: [] });
  } finally {
    aiBusy.value = false;
    await nextTick();
    scrollToBottom();
  }
}

async function runCommand(cmd, dangerous) {
  const tid = targetTerm.value;
  if (!tid) {
    showToast(at("noTerminal"), "error");
    return;
  }
  if (dangerous) {
    pendingDanger.value = { command: cmd };
    return;
  }
  await execCommand(cmd, false);
}

async function execCommand(cmd, confirmed = false) {
  const tid = targetTerm.value;
  if (!tid) return;
  try {
    await aiExecute(tid, cmd, confirmed);
    showToast(t("ssh.ai.executed"));
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

function confirmDanger() {
  if (pendingDanger.value) {
    const cmd = pendingDanger.value.command;
    pendingDanger.value = null;
    execCommand(cmd, true);
  }
}
function cancelDanger() {
  pendingDanger.value = null;
}

onMounted(async () => {
  await loadProviders();
  await loadModels();
  await loadTerminals();
});
</script>

<template>
  <div class="page ssh-ai-page">
    <div class="breadcrumb">
      <Button variant="ghost" size="sm" class="back-btn" @click="router.push('/ssh')">
        <AppIcon name="left" class="size-4" />
        SSH
      </Button>
      <span class="crumb-sep">/</span>
      <span class="crumb-title">{{ t("nav.sshAssistant") }}</span>
    </div>

    <div class="ssh-ai-grid">
      <!-- 服务配置 -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ at("config") }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div v-if="!providers.length" class="empty-hint">{{ t("ssh.ai.emptyHint") }}</div>
          <div v-for="p in providers" :key="p.name" class="provider-row">
            <div class="provider-info">
              <div class="provider-name">{{ p.name }}</div>
              <div class="provider-meta">{{ p.base_url }} · {{ (p.models || []).length }} 模型</div>
            </div>
            <Switch :model-value="p.enabled !== false" @update:model-value="toggleEnabled(p)" />
            <Button size="sm" variant="ghost" @click="startEdit(p)">
              <AppIcon name="edit" class="size-3.5" />
            </Button>
            <Button size="sm" variant="ghost" @click="removeProvider(p)">
              <AppIcon name="delete" class="size-3.5" />
            </Button>
          </div>

          <div class="provider-form space-y-3">
            <div class="form-title">{{ editing === null ? at("addProvider") : at("editProvider") }}</div>
            <div>
              <Label>SSH AI · {{ t("ssh.name") }}</Label>
              <Input v-model="form.name" :disabled="editing !== null" :placeholder="at('namePh')" />
            </div>
            <div>
              <Label>{{ at("baseUrl") }}</Label>
              <Input v-model="form.baseUrl" :placeholder="at('baseUrlPh')" />
            </div>
            <div>
              <Label>{{ at("apiKey") }}</Label>
              <Input v-model="form.apiKey" type="password" placeholder="sk-..." />
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div>
                <Label>{{ at("protocol") }}</Label>
                <Select v-model="form.protocol">
                  <SelectTrigger class="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="openai">OpenAI 兼容</SelectItem>
                    <SelectItem value="anthropic">Anthropic</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div class="flex items-end gap-2 pb-1">
                <Switch v-model="form.enabled" />
                <span class="text-sm">{{ at("enabled") }}</span>
              </div>
            </div>
            <div>
              <Label>{{ at("models") }}</Label>
              <Textarea v-model="form.models" :placeholder="at('modelsPh')" rows="2" />
            </div>
            <div class="flex gap-2">
              <Button :disabled="formBusy" @click="saveProvider">
                <Spinner v-if="formBusy" class="size-3.5" />
                {{ t("common.confirm") }}
              </Button>
              <Button v-if="editing !== null" variant="outline" @click="startAdd">
                {{ t("common.cancel") }}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- 对话 -->
      <Card class="section-card shadow-sm chat-card">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ at("chat") }}</CardTitle>
        </CardHeader>
        <CardContent class="chat-body space-y-3">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <Label>{{ t("ssh.ai.pickModel") }}</Label>
              <Select v-model="aiSelectedModel">
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
            <div>
              <Label>{{ at("terminalTarget") }}</Label>
              <div class="flex gap-2">
                <Select v-model="targetTerm">
                  <SelectTrigger class="w-full">
                    <SelectValue :placeholder="at('noTarget')" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="">{{
                      at("noTarget")
                    }}</SelectItem>
                    <SelectItem v-for="x in terminals" :key="x.term_id" :value="x.term_id">
                      {{ x.connection_name }} · {{ x.term_id.slice(0, 8) }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <Button size="sm" variant="outline" @click="loadTerminals">
                  <AppIcon name="refresh" class="size-3.5" />
                </Button>
              </div>
            </div>
          </div>
          <label class="ctx-check">
            <input type="checkbox" v-model="useContext" :disabled="!targetTerm" />
            {{ at("useContext") }}
          </label>

          <div class="ssh-ai-messages">
            <div v-if="!aiMessages.length" class="ai-empty">{{ at("emptyHint") }}</div>
            <div v-for="(m, i) in aiMessages" :key="i" class="ai-msg" :class="m.role">
              <div class="ai-msg-role">{{ m.role === "user" ? t("ssh.ai.you") : t("ssh.ai.assistant") }}</div>
              <div class="ai-msg-text">{{ m.content }}</div>
              <div v-if="m.commands && m.commands.length" class="ai-cmds">
                <div
                  v-for="(cmd, ci) in m.commands"
                  :key="ci"
                  class="ai-cmd"
                  :class="{ danger: (m.dangerous_flags ? m.dangerous_flags[ci] : m.dangerous) }"
                >
                  <code>{{ cmd }}</code>
                  <Button size="sm" variant="outline" :disabled="!targetTerm" @click="runCommand(cmd, m.dangerous_flags ? m.dangerous_flags[ci] : m.dangerous)">
                    <AppIcon name="play" class="size-3.5" />
                    {{ t("ssh.ai.run") }}
                  </Button>
                </div>
              </div>
            </div>
          </div>

          <div class="flex gap-2 items-end">
            <Textarea
              v-model="aiInput"
              :placeholder="t('ssh.ai.inputPlaceholder')"
              rows="2"
              class="flex-1"
              @keydown.enter.exact.prevent="sendMessage"
            />
            <Button :disabled="aiBusy || !aiInput.trim()" @click="sendMessage">
              <Spinner v-if="aiBusy" class="size-3.5" />
              <AppIcon v-else name="send" class="size-4" />
              {{ t("ssh.ai.send") }}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>

    <!-- 危险命令二次确认 -->
    <Dialog :open="pendingDanger !== null" @update:open="(v) => !v && cancelDanger()">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.ai.dangerTitle") }}</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground break-all">
          <code class="danger-cmd">{{ pendingDanger?.command }}</code>
        </p>
        <p class="text-xs text-muted-foreground">{{ t("ssh.ai.dangerHint") }}</p>
        <DialogFooter>
          <Button variant="outline" @click="cancelDanger">{{ t("common.cancel") }}</Button>
          <Button variant="destructive" @click="confirmDanger">{{ t("ssh.ai.runAnyway") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<style scoped>
.ssh-ai-grid {
  display: grid;
  grid-template-columns: 340px 1fr;
  gap: 12px;
  align-items: start;
}
@media (max-width: 1100px) {
  .ssh-ai-grid {
    grid-template-columns: 1fr;
  }
}
.provider-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
}
.provider-info {
  flex: 1;
  min-width: 0;
}
.provider-name {
  font-size: 13px;
  font-weight: 600;
}
.provider-meta {
  font-size: 11px;
  color: var(--color-muted-foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.provider-form {
  border-top: 1px solid var(--color-border);
  padding-top: 12px;
}
.form-title {
  font-size: 13px;
  font-weight: 600;
}
.chat-body {
  display: flex;
  flex-direction: column;
}
.ctx-check {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--color-muted-foreground);
  cursor: pointer;
}
.ssh-ai-messages {
  min-height: 280px;
  max-height: 52vh;
  overflow-y: auto;
  padding: 12px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  scrollbar-width: thin;
}
.ai-empty {
  margin: auto;
  text-align: center;
  font-size: 12px;
  color: var(--color-muted-foreground);
  padding: 20px;
}
.ai-msg {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.ai-msg.user .ai-msg-role {
  color: var(--color-primary);
}
.ai-msg.assistant .ai-msg-role {
  color: var(--color-success);
}
.ai-msg-role {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.ai-msg-text {
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}
.ai-cmds {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 4px;
}
.ai-cmd {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background-color: var(--color-muted);
}
.ai-cmd.danger {
  border-color: var(--color-danger, #ef4444);
}
.ai-cmd code {
  flex: 1;
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  white-space: pre-wrap;
  word-break: break-all;
}
.danger-cmd {
  font-family: "JetBrains Mono", monospace;
  color: var(--color-danger, #ef4444);
  word-break: break-all;
}
</style>
