<script setup>
import { ref, watch, onMounted, onBeforeUnmount, nextTick } from "vue";
import { useRoute } from "vue-router";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import "@xterm/xterm/css/xterm.css";
import {
  listConnections,
  openTerminal,
  sendTerminalInput,
  resizeTerminal,
  closeTerminal,
  touchConnection,
  onTerminalOutput,
  onTerminalClosed,
  onHostkeyPrompt,
  acceptHostkey,
  rejectHostkey,
  toBase64,
  fromBase64,
  aiListModels,
  aiChat,
  aiExecute,
  aiGetBuffer,
  forwardLocal,
  listForwards,
  closeForward,
  forwardAgent,
  startSocksProxy,
  closeSocks,
  listSocks,
} from "../lib/api-ssh.js";
import { Input } from "@/components/ui/input";
import { showToast } from "../lib/toast.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
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

// ── AI 助手状态 ──
const aiOpen = ref(true);
const aiModels = ref([]);
const aiSelectedModel = ref("");
const aiMessages = ref([]); // { role: 'user'|'assistant', content, commands?, dangerous? }
const aiInput = ref("");
const aiBusy = ref(false);
const aiTermContext = ref(true); // 是否把终端最近输出作为上下文
const pendingDanger = ref(null); // { command, reply } 待确认的危险命令

// 快捷命令 chips：点按直接在活动终端执行
const QUICK_COMMANDS = ["top", "htop", "df -h", "free -m", "ls -lah", "ps aux | head -20", "uptime", "ip addr", "pwd"];

function quickExec(cmd) {
  execCommand(cmd);
}

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

function activeTermId() {
  const tab = tabs.value.find((tb) => tb.key === activeKey.value);
  return tab?.sessionId || null;
}

async function loadAiModels() {
  try {
    const models = await aiListModels();
    aiModels.value = models || [];
    if (!aiSelectedModel.value && aiModels.value.length) {
      aiSelectedModel.value = aiModels.value[0].model;
    }
  } catch (err) {
    // 不阻塞终端使用；仅在 AI 面板提示
    showToast(friendlyError(err), "error");
  }
}

async function sendAiMessage() {
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
  scrollAiToBottom();

  const history = aiMessages.value
    .filter((m) => m.role === "user" || m.role === "assistant")
    .map((m) => ({ role: m.role, content: m.content }));

  try {
    const res = await aiChat({
      termId: aiTermContext.value ? activeTermId() : null,
      history,
      message: text,
      model: aiSelectedModel.value || null,
    });
    const reply = res.reply || "";
    const cmds = res.commands || [];
    const flags = res.dangerous_flags || cmds.map(() => !!res.dangerous);
    const assistantMsg = {
      role: "assistant",
      content: "",
      commands: cmds,
      dangerous_flags: flags,
      dangerous: !!res.dangerous,
      model: res.model,
      provider: res.provider,
    };
    aiMessages.value.push(assistantMsg);
    // 流式打字：字符逐段填充，模拟流式输出
    const words = reply.match(/\S+\s*/g) || [reply];
    for (let si = 0; si < words.length; si++) {
      assistantMsg.content += words[si];
      if (si % 3 === 0 || si === words.length - 1) {
        await nextTick();
        scrollAiToBottom();
        await new Promise((r) => setTimeout(r, 12));
      }
    }
    await nextTick();
    scrollAiToBottom();
  } catch (err) {
    showToast(friendlyError(err), "error");
    aiMessages.value.push({
      role: "assistant",
      content: `⚠️ ${friendlyError(err)}`,
      commands: [],
      dangerous: false,
    });
  } finally {
    aiBusy.value = false;
    await nextTick();
    scrollAiToBottom();
  }
}

function scrollAiToBottom() {
  const box = document.querySelector(".ai-messages");
  if (box) box.scrollTop = box.scrollHeight;
}

async function runAiCommand(cmd, dangerous) {
  const tid = activeTermId();
  if (!tid) {
    showToast(t("ssh.ai.noTerminal"), "error");
    return;
  }
  if (dangerous) {
    pendingDanger.value = { command: cmd, reply: null };
    return;
  }
  await execCommand(cmd, false);
}

async function execCommand(cmd, confirmed = false) {
  const tid = activeTermId();
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

  const tab = { key: ++seq, sessionId: null, connectionId, name: c.name, term: null, fit: null, search: null, status: "connecting", autoReconnect: true };
  tabs.value.push(tab);
  activeKey.value = tab.key;
  await nextTick();

  const el = els.get(tab.key);
  if (!el) return;

  const term = new Terminal({
    cursorBlink: true,
    fontFamily: '"JetBrains Mono", Menlo, monospace',
    fontSize: 13,
    lineHeight: 1.2,
    theme: {
      background: "#0d1117",
      foreground: "#e6edf3",
      cursor: "#58a6ff",
      selectionBackground: "#264f78",
      black: "#484f58",
      red: "#ff7b72",
      green: "#3fb950",
      yellow: "#d29922",
      blue: "#58a6ff",
      magenta: "#bc8cff",
      cyan: "#39c5cf",
      white: "#b1bac4",
      brightBlack: "#6e7681",
      brightRed: "#ffa198",
      brightGreen: "#56d364",
      brightYellow: "#e3b341",
      brightBlue: "#79c0ff",
      brightMagenta: "#d2a8ff",
      brightCyan: "#56d4dd",
      brightWhite: "#f0f6fc",
    },
  });
  const fit = new FitAddon();
  const search = new SearchAddon();
  term.loadAddon(fit);
  term.loadAddon(search);
  term.open(el);
  try {
    fit.fit();
  } catch {
    // 首帧可能尚未布局完成，稍后由 watch(activeKey) 兜底
  }

  // Ctrl+Shift+C 复制选中文本；无选中则发送系统中断（与常规终端一致）
  term.attachCustomKeyEventHandler((ev) => {
    if (ev.type !== "keydown") return true;
    if ((ev.ctrlKey || ev.metaKey) && ev.shiftKey && (ev.code === "KeyC" || ev.code === "KeyV")) {
      if (ev.code === "KeyC" && term.hasSelection()) {
        onCopySelection(tab);
      }
      return false; // 由我们不发送到远端
    }
    if (ev.ctrlKey || ev.metaKey) {
      if (ev.key.toLowerCase() === "f") {
        openSearch(tab);
        return false;
      }
    }
    return true;
  });

  try {
    // 先按当前容器尺寸打开 PTY，避免 80x24 之后再resize抖动
    const sessionId = await openTerminal(connectionId, term.cols, term.rows);
    tab.sessionId = sessionId;
    tab.term = term;
    tab.fit = fit;
    tab.search = search;
    tab.status = "live";
    touchConnection(connectionId).catch(() => {}); // 记录最近使用时间
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
  const name = tab.name;
  const wasAuto = tab.autoReconnect;
  // 保留历史：copy 旧 xterm 缓冲到新实例前先记录 scrollback
  const history = tab.term ? tab.term.buffer.active ? scrollbackText(tab.term) : "" : "";
  closeTab(tab);
  const key = openTabRestore(connectionId, history);
  // 恢复自动重连开关
  const nt = tabs.value.find((t) => t.key === key);
  if (nt) nt.autoReconnect = wasAuto;
  void name;
}

/** 读取 xterm scrollback 为纯文本（分块遍历行），用于会话历史持久化。
 *  只用公共 API（IBufferLine.translateToString），不碰 _line 私有字段，
 *  避免 xterm 升级后 silently 损坏历史保留。 */
function scrollbackText(term) {
  if (!term || !term.buffer) return "";
  const buf = term.buffer.active;
  const rows = buf.length;
  const lines = [];
  const max = Math.min(rows, 4000); // 限制内存
  for (let i = Math.max(0, rows - max); i < rows; i++) {
    const line = buf.getLine(i);
    if (!line) continue;
    lines.push(line.translateToString(false));
  }
  return lines.join("\n");
}

/**
 * 以「保留历史」的方式打开新终端：openTab 的变体。
 * 返回新 tab 的 key。历史上段在 PTY 建立后写入终端并换行。
 */
function openTabRestore(connectionId, history) {
  const c = conns.value.find((x) => x.id === connectionId);
  if (!c) return null;
  const existing = tabs.value.find(
    (tb) => tb.connectionId === connectionId && tb.status !== "disconnected"
  );
  if (existing) {
    activeKey.value = existing.key;
    existing.term?.focus();
    return existing.key;
  }
  const tab = { key: ++seq, sessionId: null, connectionId, name: c.name, term: null, fit: null, search: null, status: "connecting", autoReconnect: true };
  tabs.value.push(tab);
  activeKey.value = tab.key;
  nextTick(async () => {
    const el = els.get(tab.key);
    if (!el) return;
    const term = new Terminal({
      cursorBlink: true,
      fontFamily: '"JetBrains Mono", Menlo, monospace',
      fontSize: 13,
      lineHeight: 1.2,
      theme: {
        background: "#0d1117", foreground: "#e6edf3", cursor: "#58a6ff",
        selectionBackground: "#264f78", black: "#484f58", red: "#ff7b72",
        green: "#3fb950", yellow: "#d29922", blue: "#58a6ff", magenta: "#bc8cff",
        cyan: "#39c5cf", white: "#b1bac4", brightBlack: "#6e7681",
        brightRed: "#ffa198", brightGreen: "#56d364", brightYellow: "#e3b341",
        brightBlue: "#79c0ff", brightMagenta: "#d2a8ff", brightCyan: "#56d4dd",
        brightWhite: "#f0f6fc",
      },
    });
    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);
    term.open(el);
    try { fit.fit(); } catch {}
    term.attachCustomKeyEventHandler((ev) => {
      if (ev.type !== "keydown") return true;
      if ((ev.ctrlKey || ev.metaKey) && ev.shiftKey && (ev.code === "KeyC" || ev.code === "KeyV")) {
        if (ev.code === "KeyC" && term.hasSelection()) onCopySelection(tab);
        return false;
      }
      if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "f") { openSearch(tab); return false; }
      return true;
    });
    try {
      const sessionId = await openTerminal(connectionId, term.cols, term.rows);
      tab.sessionId = sessionId;
      tab.term = term;
      tab.fit = fit;
      tab.search = search;
      tab.status = "live";
      term.onData((d) => sendTerminalInput(sessionId, toBase64(d)).catch(() => {}));
      term.onResize(({ cols, rows }) => resizeTerminal(sessionId, cols, rows).catch(() => {}));
      if (history) {
        term.write("\x1b[90m── 历史会话（已断开重连） ──\x1b[0m\r\n" + history + "\r\n");
      }
      term.focus();
    } catch (err) {
      tab.term = term;
      tab.status = "disconnected";
      term.writeln(`\r\n\x1b[31m${friendlyError(err)}\x1b[0m`);
      showToast(friendlyError(err), "error");
    }
  });
  return tab.key;
}

/** Ctrl+C 复制：用 navigator.clipboard 写入 xterm 选中文本 */
function onCopySelection(tab) {
  if (!tab?.term) return;
  const sel = tab.term.getSelection();
  if (!sel) return;
  try {
    navigator.clipboard
      .writeText(sel)
      .then(() => showToast(t("ssh.copied"), "success"))
      .catch(() => {});
  } catch {
    // clipboard API 不可用时忽略
  }
}

// ── 终端内搜索（Ctrl+F）──
const searchOpen = ref(false);
const searchQuery = ref("");
const searchTabKey = ref(null);
const searchInputRef = ref(null);

function openSearch(tab) {
  searchTabKey.value = tab?.key ?? null;
  searchOpen.value = true;
  searchQuery.value = "";
  tab?.term?.focus();
  nextTick(() => searchInputRef.value?.focus());
}

function doSearch(dir) {
  const tab = tabs.value.find((tb) => tb.key === searchTabKey.value);
  if (!tab?.search || !searchQuery.value) return;
  try {
    if (dir === "prev") tab.search.findPrevious(searchQuery.value, { caseSensitive: false, wholeWord: false });
    else tab.search.findNext(searchQuery.value, { caseSensitive: false, wholeWord: false });
  } catch {
    // 空输入等边界忽略
  }
}

function closeSearch() {
  const tab = tabs.value.find((tb) => tb.key === searchTabKey.value);
  tab?.search?.clearDecorations();
  searchOpen.value = false;
  searchQuery.value = "";
  tab?.term?.focus();
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
        // 自动重连（默认开启，最多重试 3 次，间隔 3s）
        if (tab.autoReconnect) {
          const connId = tab.connectionId;
          const known = tabs.value.includes(tab);
          let attempts = 0;
          const timer = setInterval(() => {
            const still = tabs.value.find((x) => x.connectionId === connId && x.status === "disconnected");
            attempts += 1;
            if (!known || !still || attempts >= 3) {
              clearInterval(timer);
              return;
            }
            reconnect(still);
            clearInterval(timer);
          }, 3000);
        }
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

  loadAiModels();

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

// ── 端口转发（-L）面板 ──
const forwardOpen = ref(false);
const forwardList = ref([]);
const fwd = ref({ bindHost: "127.0.0.1", bindPort: null, destHost: "", destPort: null });
const fwdBusy = ref(false);

// ── 动态 SOCKS5 代理（-D）面板 ──
const socksList = ref([]);
const socks = ref({ bindHost: "127.0.0.1", bindPort: 1080 });
const socksBusy = ref(false);

async function refreshForwards() {
  const sid = activeTermId();
  if (!sid) return;
  try {
    forwardList.value = await listForwards(sid);
  } catch (err) {
    forwardList.value = [];
  }
  try {
    socksList.value = await listSocks(sid);
  } catch (err) {
    socksList.value = [];
  }
}

async function openForwardDialog() {
  fwd.value = { bindHost: "127.0.0.1", bindPort: null, destHost: "", destPort: null };
  forwardOpen.value = true;
  await refreshForwards();
}

async function addForward() {
  const sid = activeTermId();
  if (!sid) return;
  const { bindHost, bindPort, destHost, destPort } = fwd.value;
  if (!bindPort || !destHost || !destPort) {
    showToast(t("ssh.forward.fillRequired"), "error");
    return;
  }
  fwdBusy.value = true;
  try {
    await forwardLocal(sid, bindHost, Number(bindPort), destHost, Number(destPort));
    showToast(t("ssh.forward.added"), "success");
    await refreshForwards();
    fwd.value = { bindHost: "127.0.0.1", bindPort: null, destHost: "", destPort: null };
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    fwdBusy.value = false;
  }
}

async function removeForward(id) {
  const sid = activeTermId();
  if (!sid) return;
  try {
    await closeForward(sid, id);
    await refreshForwards();
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function enableAgentForward() {
  const sid = activeTermId();
  if (!sid) return;
  try {
    const sock = await forwardAgent(sid);
    showToast(t("ssh.forward.agentOn") + " " + sock, "success");
  } catch (err) {
    showToast(friendlyError(err), "error");
  }
}

async function addSocks() {
  const sid = activeTermId();
  if (!sid) return;
  const { bindHost, bindPort } = socks.value;
  if (!bindPort) {
    showToast(t("ssh.forward.socksFillRequired"), "error");
    return;
  }
  socksBusy.value = true;
  try {
    await startSocksProxy(sid, bindHost, Number(bindPort));
    showToast(t("ssh.forward.socksAdded"), "success");
    await refreshForwards();
    socks.value = { bindHost: "127.0.0.1", bindPort: 1080 };
  } catch (err) {
    showToast(friendlyError(err), "error");
  } finally {
    socksBusy.value = false;
  }
}

async function removeSocks(id) {
  const sid = activeTermId();
  if (!sid) return;
  try {
    await closeSocks(sid, id);
    await refreshForwards();
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
        <Button variant="outline" :disabled="!activeTermId()" @click="openForwardDialog">
          <AppIcon name="network" class="size-4" />
          {{ t("ssh.forward.title") }}
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

    <!-- 终端 + AI 助手：左右布局 -->
    <div class="term-layout">
      <div class="term-col">
        <!-- 终端内搜索栏（Ctrl+F） -->
        <div v-if="searchOpen" class="search-bar" @keydown.escape="closeSearch">
          <div class="search-input-wrap">
            <AppIcon name="search" class="size-3.5" />
            <input
              ref="searchInputRef"
              v-model="searchQuery"
              :placeholder="t('ssh.search_placeholder')"
              @keydown.enter.prevent="doSearch('next')"
              @keydown.shift.enter.prevent="doSearch('prev')"
            />
            <span class="search-hint">{{ t("ssh.search_hint") }}</span>
          </div>
          <button class="search-btn" @click="doSearch('prev')" :title="t('ssh.search_prev')">
            <AppIcon name="chevron-up" class="size-3.5" />
          </button>
          <button class="search-btn" @click="doSearch('next')" :title="t('ssh.search_next')">
            <AppIcon name="chevron-down" class="size-3.5" />
          </button>
          <button class="search-btn" @click="closeSearch" :title="t('common.close')">
            <AppIcon name="close" class="size-3.5" />
          </button>
        </div>
        <!-- 终端区：每个标签一个常驻容器，v-show 切换保住 xterm 布局 -->
        <div class="term-holder" v-if="tabs.length">
          <div
            v-for="tab in tabs"
            :key="tab.key"
            class="term-container"
            :class="{ visible: tab.key === activeKey }"
            :ref="(el) => setEl(tab, el)"
          ></div>
        </div>

      </div>

      <!-- AI 助手面板 -->
      <aside class="ai-panel" :class="{ collapsed: !aiOpen }">
        <div class="ai-head">
          <div class="ai-title">
            <AppIcon name="sparkles" class="size-4" />
            <span>{{ t("ssh.ai.title") }}</span>
          </div>
          <button class="ai-toggle" :title="aiOpen ? t('ssh.ai.collapse') : t('ssh.ai.expand')" @click="aiOpen = !aiOpen">
            <AppIcon :name="aiOpen ? 'panel-right-close' : 'panel-right-open'" class="size-4" />
          </button>
        </div>

        <div v-if="aiOpen" class="ai-body">
          <div class="ai-models">
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
            <label class="ai-ctx">
              <input type="checkbox" v-model="aiTermContext" />
              {{ t("ssh.ai.useContext") }}
            </label>
          </div>

          <div class="ai-messages" ref="aiMsgBox">
            <div v-if="!aiMessages.length" class="ai-empty">
              {{ t("ssh.ai.emptyHint") }}
            </div>
            <div
              v-for="(m, i) in aiMessages"
              :key="i"
              class="ai-msg"
              :class="m.role"
            >
              <div class="ai-msg-role">{{ m.role === 'user' ? t('ssh.ai.you') : t('ssh.ai.assistant') }}</div>
              <div class="ai-msg-text">{{ m.content }}</div>
              <div v-if="m.commands && m.commands.length" class="ai-cmds">
                <div
                  v-for="(cmd, ci) in m.commands"
                  :key="ci"
                  class="ai-cmd"
                  :class="{ danger: (m.dangerous_flags ? m.dangerous_flags[ci] : m.dangerous) }"
                >
                  <code>{{ cmd }}</code>
                  <Button size="sm" variant="outline" @click="runAiCommand(cmd, m.dangerous_flags ? m.dangerous_flags[ci] : m.dangerous)">
                    <AppIcon name="play" class="size-3.5" />
                    {{ t("ssh.ai.run") }}
                  </Button>
                </div>
              </div>
            </div>
          </div>

          <div class="ai-input">
            <div class="ai-chips">
              <button
                v-for="chip in QUICK_COMMANDS"
                :key="chip"
                type="button"
                class="ai-chip"
                :disabled="aiBusy"
                @click="quickExec(chip)"
              >
                {{ chip }}
              </button>
            </div>
            <Textarea
              v-model="aiInput"
              :placeholder="t('ssh.ai.inputPlaceholder')"
              rows="3"
              @keydown.enter.exact.prevent="sendAiMessage"
            />
            <Button :disabled="aiBusy || !aiInput.trim()" @click="sendAiMessage">
              <Spinner v-if="aiBusy" class="size-3.5" />
              <AppIcon v-else name="send" class="size-4" />
              {{ t("ssh.ai.send") }}
            </Button>
          </div>
        </div>
      </aside>
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

    <!-- 端口转发（-L）与 Agent 转发 -->
    <Dialog :open="forwardOpen" @update:open="(v) => !v && (forwardOpen = false)">
      <DialogContent class="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{{ t("ssh.forward.title") }}</DialogTitle>
        </DialogHeader>

        <div class="fwd-form grid grid-cols-2 gap-2">
          <label class="text-xs text-muted-foreground">
            {{ t("ssh.forward.bindHost") }}
            <Input v-model="fwd.bindHost" placeholder="127.0.0.1" />
          </label>
          <label class="text-xs text-muted-foreground">
            {{ t("ssh.forward.bindPort") }}
            <Input v-model="fwd.bindPort" type="number" placeholder="8080" />
          </label>
          <label class="text-xs text-muted-foreground">
            {{ t("ssh.forward.destHost") }}
            <Input v-model="fwd.destHost" placeholder="localhost" />
          </label>
          <label class="text-xs text-muted-foreground">
            {{ t("ssh.forward.destPort") }}
            <Input v-model="fwd.destPort" type="number" placeholder="80" />
          </label>
        </div>
        <div class="flex items-center gap-2 mt-2">
          <Button :disabled="fwdBusy" @click="addForward">
            <Spinner v-if="fwdBusy" class="size-3.5" />
            <AppIcon v-else name="forward" class="size-3.5" />
            {{ t("ssh.forward.add") }}
          </Button>
          <Button variant="outline" @click="enableAgentForward">
            <AppIcon name="key" class="size-3.5" />
            {{ t("ssh.forward.agent") }}
          </Button>
        </div>

        <div class="fwd-list mt-3 space-y-1">
          <div v-if="!forwardList.length" class="text-xs text-muted-foreground">
            {{ t("ssh.forward.empty") }}
          </div>
          <div
            v-for="f in forwardList"
            :key="f.id"
            class="flex items-center justify-between rounded border px-2 py-1 text-sm"
            :class="{ 'opacity-50': !f.active }"
          >
            <span class="font-mono">{{ f.bindHost }}:{{ f.bindPort }} → {{ f.destHost }}:{{ f.destPort }}</span>
            <Button v-if="f.active" size="sm" variant="ghost" @click="removeForward(f.id)">
              <AppIcon name="close" class="size-3.5" />
              {{ t("ssh.forward.close") }}
            </Button>
          </div>
        </div>

        <div class="fwd-divider" />

        <!-- 动态 SOCKS5 代理（-D） -->
        <div class="fwd-form grid grid-cols-2 gap-2">
          <label class="text-xs text-muted-foreground">
            {{ t("ssh.forward.socksBindHost") }}
            <Input v-model="socks.bindHost" placeholder="127.0.0.1" />
          </label>
          <label class="text-xs text-muted-foreground">
            {{ t("ssh.forward.socksBindPort") }}
            <Input v-model="socks.bindPort" type="number" placeholder="1080" />
          </label>
        </div>
        <div class="flex items-center gap-2 mt-2">
          <Button :disabled="socksBusy" @click="addSocks">
            <Spinner v-if="socksBusy" class="size-3.5" />
            <AppIcon v-else name="network" class="size-3.5" />
            {{ t("ssh.forward.socksAdd") }}
          </Button>
        </div>
        <p class="text-xs text-muted-foreground mt-2">
          {{
            tFormat("ssh.forward.socksHint", {
              host: socks.bindHost,
              port: socks.bindPort,
            })
          }}
        </p>
        <div class="fwd-list mt-3 space-y-1">
          <div v-if="!socksList.length" class="text-xs text-muted-foreground">
            {{ t("ssh.forward.socksEmpty") }}
          </div>
          <div
            v-for="s in socksList"
            :key="s.id"
            class="flex items-center justify-between rounded border px-2 py-1 text-sm"
            :class="{ 'opacity-50': !s.active }"
          >
            <span class="font-mono">SOCKS5 {{ s.bindHost }}:{{ s.bindPort }}</span>
            <Button v-if="s.active" size="sm" variant="ghost" @click="removeSocks(s.id)">
              <AppIcon name="close" class="size-3.5" />
              {{ t("ssh.forward.close") }}
            </Button>
          </div>
        </div>
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
  border: 1px solid rgba(48, 54, 61, 0.8);
  background-color: #0d1117;
}

/* ── 终端内搜索栏 ── */
.search-bar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  margin-bottom: 4px;
  border-radius: 6px;
  background-color: var(--color-muted);
}
.search-input-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
  padding: 4px 8px;
  border-radius: 4px;
  background-color: var(--color-card);
  color: var(--color-muted-foreground);
}
.search-input-wrap input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--color-foreground);
  font-size: 12px;
}
.search-hint {
  font-size: 10px;
  color: var(--color-muted-foreground);
  white-space: nowrap;
}
.search-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--color-muted-foreground);
  cursor: pointer;
}
.search-btn:hover {
  background-color: var(--color-border);
  color: var(--color-foreground);
}

.term-container {
  height: 100%;
  padding: 4px 2px 4px 6px;
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

/* ── AI 助手面板 ─────────────────────────────────────────────── */
.term-layout {
  display: flex;
  gap: 12px;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.term-col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.term-col .term-holder,
.term-col .term-empty {
  flex: 1;
  min-height: 0;
}

.ai-panel {
  width: 360px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-card);
  overflow: hidden;
}
.ai-panel.collapsed {
  width: 44px;
}
.ai-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
  background-color: var(--color-muted);
}
.ai-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: var(--color-foreground);
}
.ai-toggle {
  border: none;
  background: transparent;
  color: var(--color-muted-foreground);
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
  display: inline-flex;
}
.ai-toggle:hover {
  background-color: var(--color-border);
  color: var(--color-foreground);
}
.ai-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}
.ai-models {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--color-border);
}
.ai-ctx {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--color-muted-foreground);
  cursor: pointer;
}
.ai-ctx input {
  accent-color: var(--color-primary);
}
.ai-messages {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 12px;
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
  color: var(--color-foreground);
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
  color: var(--color-foreground);
  white-space: pre-wrap;
  word-break: break-all;
}
.ai-input {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--color-border);
  align-items: flex-end;
}
.ai-input :deep(textarea) {
  resize: none;
  font-size: 12px;
}
.ai-chips {
  position: absolute;
  bottom: calc(100% + 4px);
  left: 12px;
  right: 12px;
  display: flex;
  gap: 4px;
  overflow-x: auto;
  padding-bottom: 2px;
  scrollbar-width: thin;
}
.ai-chip {
  flex-shrink: 0;
  padding: 2px 8px;
  font-size: 11px;
  font-family: "JetBrains Mono", monospace;
  border: 1px solid var(--color-border);
  border-radius: 9999px;
  background: var(--color-card);
  color: var(--color-muted-foreground);
  cursor: pointer;
  transition: all 0.12s ease;
}
.ai-chip:hover {
  background-color: var(--color-sidebar-accent);
  color: var(--color-foreground);
}
.ai-chip:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.danger-cmd {
  font-family: "JetBrains Mono", monospace;
  color: var(--color-danger, #ef4444);
  word-break: break-all;
}

.fwd-divider {
  height: 1px;
  background-color: var(--color-border);
  margin: 16px 0 12px;
}
</style>
