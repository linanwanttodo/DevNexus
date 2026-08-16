<script setup>
import { ref, watch, onMounted, onBeforeUnmount, nextTick } from "vue";
import { useRoute } from "vue-router";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  listConnections,
  openTerminal,
  sendTerminalInput,
  resizeTerminal,
  closeTerminal,
  onTerminalOutput,
  onTerminalClosed,
  onHostkeyPrompt,
  acceptHostkey,
  rejectHostkey,
  toBase64,
  fromBase64,
} from "../lib/api-ssh.js";
import { showToast } from "../lib/toast.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
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

const route = useRoute();

// 标签：{ key, sessionId, connectionId, name, term, fit, status }
// status: connecting | live | disconnected
const conns = ref([]);
const tabs = ref([]);
const activeKey = ref(null);
const newConnId = ref("");
const hostkeyPrompt = ref(null);

const els = new Map(); // key -> DOM 容器（v-for 的 ref 回调维护）
let unlisteners = [];
let seq = 0;

function setEl(tab, el) {
  if (el) els.set(tab.key, el);
  else els.delete(tab.key);
}

function findTabBySession(sessionId) {
  return tabs.value.find((tb) => tb.sessionId === sessionId);
}

async function fitActive() {
  const tab = tabs.value.find((tb) => tb.key === activeKey.value);
  if (tab?.fit && tab.sessionId) {
    try {
      tab.fit.fit();
    } catch {
      // 容器不可见时 fit 可能失败，忽略
    }
  }
}

async function openTab(connectionId) {
  const c = conns.value.find((x) => x.id === connectionId);
  if (!c) return;

  // 已有同连接的活跃标签：直接切换
  const existing = tabs.value.find(
    (tb) => tb.connectionId === connectionId && tb.status !== "disconnected"
  );
  if (existing) {
    activeKey.value = existing.key;
    existing.term?.focus();
    return;
  }

  const tab = { key: ++seq, sessionId: null, connectionId, name: c.name, term: null, fit: null, status: "connecting" };
  tabs.value.push(tab);
  activeKey.value = tab.key;
  await nextTick();

  const el = els.get(tab.key);
  if (!el) return;

  const term = new Terminal({
    cursorBlink: true,
    fontFamily: '"JetBrains Mono", Menlo, monospace',
    fontSize: 13,
    theme: { background: "#1e1e2e" },
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(el);
  try {
    fit.fit();
  } catch {
    // 首帧可能尚未布局完成，稍后由 watch(activeKey) 兜底
  }

  try {
    // 先按当前容器尺寸打开 PTY，避免 80x24 之后再resize抖动
    const sessionId = await openTerminal(connectionId, term.cols, term.rows);
    tab.sessionId = sessionId;
    tab.term = term;
    tab.fit = fit;
    tab.status = "live";
    term.onData((d) => {
      sendTerminalInput(sessionId, toBase64(d)).catch(() => {});
    });
    term.onResize(({ cols, rows }) => {
      resizeTerminal(sessionId, cols, rows).catch(() => {});
    });
    term.focus();
  } catch (err) {
    tab.term = term;
    tab.status = "disconnected";
    term.writeln(`\r\n\x1b[31m${friendlyError(err)}\x1b[0m`);
    showToast(friendlyError(err), "error");
  }
}

function closeTab(tab) {
  if (tab.sessionId) closeTerminal(tab.sessionId).catch(() => {});
  els.delete(tab.key);
  const idx = tabs.value.indexOf(tab);
  if (idx >= 0) tabs.value.splice(idx, 1);
  tab.term?.dispose();
  if (activeKey.value === tab.key) {
    activeKey.value = tabs.value.length ? tabs.value[tabs.value.length - 1].key : null;
  }
}

function reconnect(tab) {
  const connectionId = tab.connectionId;
  closeTab(tab);
  openTab(connectionId);
}

onMounted(async () => {
  unlisteners = [
    await onTerminalOutput(({ sessionId, data }) => {
      findTabBySession(sessionId)?.term?.write(fromBase64(data));
    }),
    await onTerminalClosed(({ sessionId }) => {
      const tab = findTabBySession(sessionId);
      if (tab) {
        tab.status = "disconnected";
        tab.term?.writeln(`\r\n\x1b[31m[${t("ssh.disconnected")}]\x1b[0m`);
      }
    }),
    await onHostkeyPrompt((p) => {
      hostkeyPrompt.value = p;
    }),
  ];

  try {
    conns.value = await listConnections();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }

  // 连接页「打开终端」跳转：/ssh/sessions?open=<id>
  const toOpen = route.query.open;
  if (toOpen) await openTab(String(toOpen));
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", onWindowResize);
  for (const fn of unlisteners) fn();
  for (const tab of tabs.value) {
    if (tab.sessionId) closeTerminal(tab.sessionId).catch(() => {});
    tab.term?.dispose();
  }
});

// 标签切换后 v-show 恢复布局，需要重新 fit
watch(activeKey, async () => {
  await nextTick();
  fitActive();
  const tab = tabs.value.find((tb) => tb.key === activeKey.value);
  tab?.term?.focus();
});

function onWindowResize() {
  fitActive();
}

window.addEventListener("resize", onWindowResize);

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
  <div class="page page-terminal">
    <!-- 页头：随时可以从右上角开新终端 -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("ssh.sessions") }}</h1>
        <p class="page-desc">{{ t("nav.ssh") }}</p>
      </div>
      <div class="flex items-center gap-2">
        <Select v-model="newConnId">
          <SelectTrigger class="w-[180px]">
            <SelectValue :placeholder="t('ssh.connections')" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="c in conns" :key="c.id" :value="c.id">
              {{ c.name }}
            </SelectItem>
          </SelectContent>
        </Select>
        <Button :disabled="!newConnId" @click="openTab(newConnId)">
          <AppIcon name="plus" class="size-4" />
          {{ t("ssh.open_terminal") }}
        </Button>
      </div>
    </div>

    <!-- 标签栏 -->
    <div v-if="tabs.length" class="tabbar">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        type="button"
        class="tab"
        :class="{ active: tab.key === activeKey, dead: tab.status === 'disconnected' }"
        @click="activeKey = tab.key"
      >
        <span v-if="tab.status === 'connecting'" class="tab-spinner">
          <Spinner class="size-3" />
        </span>
        <AppIcon v-else name="terminal" class="tab-icon" />
        <span class="tab-name">{{ tab.name }}</span>
        <span v-if="tab.status === 'disconnected'" class="tab-status">
          {{ t("ssh.disconnected") }}
        </span>
        <span
          v-if="tab.status === 'disconnected'"
          class="tab-reconnect"
          :title="t('ssh.reconnect')"
          @click.stop="reconnect(tab)"
        >
          <AppIcon name="refresh" class="size-3" />
        </span>
        <span class="tab-close" :title="t('common.close')" @click.stop="closeTab(tab)">
          <AppIcon name="close" class="size-3" />
        </span>
      </button>
    </div>

    <!-- 终端区：每个标签一个常驻容器，v-show 切换保住 xterm 布局 -->
    <div class="term-holder">
      <div
        v-for="tab in tabs"
        :key="tab.key"
        class="term-container"
        :class="{ visible: tab.key === activeKey }"
        :ref="(el) => setEl(tab, el)"
      ></div>
    </div>

    <!-- 无标签：快捷打开连接 -->
    <div v-if="!tabs.length" class="term-empty">
      <Empty class="py-10">
        <EmptyMedia>
          <AppIcon name="terminal" class="size-10 text-muted-foreground/60" />
        </EmptyMedia>
        <EmptyContent>
          <EmptyDescription>
            <div>{{ t("ssh.term_empty_hint") }}</div>
          </EmptyDescription>
        </EmptyContent>
      </Empty>
      <div v-if="conns.length" class="quick-list">
        <Button
          v-for="c in conns"
          :key="c.id"
          variant="outline"
          size="sm"
          @click="openTab(c.id)"
        >
          <AppIcon name="server" class="size-4" />
          {{ c.name }}
          <span class="quick-host">{{ c.username }}@{{ c.host }}</span>
        </Button>
      </div>
    </div>

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
.page-terminal {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.tabbar {
  display: flex;
  align-items: center;
  gap: 4px;
  overflow-x: auto;
  flex-shrink: 0;
  padding-bottom: 8px;
  scrollbar-width: thin;
}

.tab {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  background-color: transparent;
  color: var(--color-muted-foreground);
  font-size: 12px;
  padding: 6px 8px 6px 10px;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background-color 0.12s ease,
    color 0.12s ease,
    border-color 0.12s ease;
}

.tab::before {
  content: "";
  position: absolute;
  top: -1px;
  left: 8px;
  right: 8px;
  height: 2px;
  border-radius: 9999px;
  background-color: transparent;
  transition: background-color 0.12s ease;
}

.tab:hover {
  background-color: var(--color-muted);
  color: var(--color-foreground);
}

.tab.active {
  background-color: var(--color-muted);
  border-color: var(--color-border);
  color: var(--color-foreground);
  font-weight: 500;
}

.tab.active::before {
  background-color: var(--color-primary);
}

.tab.dead {
  opacity: 0.75;
}

.tab-spinner {
  display: inline-flex;
  align-items: center;
  width: 13px;
  height: 13px;
  flex-shrink: 0;
}

.tab-icon {
  width: 13px;
  height: 13px;
  flex-shrink: 0;
}

.tab-name {
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tab-status {
  font-size: 10px;
  opacity: 0.8;
}

.tab-reconnect,
.tab-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  flex-shrink: 0;
  opacity: 0;
  transition: opacity 0.12s ease;
}

.tab:hover .tab-close,
.tab:hover .tab-reconnect,
.tab.active .tab-close {
  opacity: 0.7;
}

.tab-reconnect:hover,
.tab-close:hover {
  opacity: 1;
  background-color: rgba(127, 127, 127, 0.25);
}

.term-holder {
  flex: 1;
  min-height: 0;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--color-border);
  box-shadow: inset 0 1px 4px rgba(0, 0, 0, 0.25);
  background-color: #1e1e2e;
}

.term-container {
  height: 100%;
  padding: 6px 8px;
  display: none;
}

.term-container.visible {
  display: block;
}

.term-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
}

.quick-list {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 8px;
  max-width: 640px;
}

.quick-host {
  font-size: 11px;
  opacity: 0.6;
  font-family: "JetBrains Mono", monospace;
}
</style>
