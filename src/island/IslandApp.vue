<script setup>
// IslandApp.vue — 灵动岛悬浮胶囊（加载于 island.html 透明置顶窗口）
// 交互模型（参考苹果灵动岛，代码自研）：
//   click   → 胶囊变长变高展开（仍是胶囊，圆角 = 高度一半），再点收起
//   拖拽    → 移动窗口，位置持久化到 localStorage（主应用重启后恢复）
//   功能    → 展开态含 时钟/日期 + 媒体控制（MPRIS）+ 专注倒计时；
//             系统通知（微信/QQ 等）以横幅形式从胶囊弹出，数秒后自动收起
import { ref, computed, watch, onMounted, onBeforeUnmount } from "vue";
import { getCurrentWindow, cursorPosition } from "@tauri-apps/api/window";
import { PhysicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  LayoutDashboard,
  X,
  Play,
  Pause,
  SkipBack,
  SkipForward,
  RotateCcw,
  Minus,
  Plus,
  Timer,
  Music2,
  Wallet,
} from "@lucide/vue";
import deepseekIcon from "../assets/deepseek.png";

const win = getCurrentWindow();

const now = ref(new Date());
const dragging = ref(false);

// 两态：收起（小胶囊，只放时间/状态）/ 展开（大胶囊，显示媒体详情 + 控制）
// 触发：鼠标悬停展开、移出收起；点击切换；拖拽移动不受影响。
const expanded = ref(false);
let hoverTimer = null; // 悬停防抖：避免鼠标快速划过时误展开
let clickSuppressed = false; // 拖拽结束后抑制紧随其后的 click（防止拖动也算点击）

// 语言跟随主应用偏好（主应用切换语言后本窗口下次显示时生效）
const lang = localStorage.getItem("devnexus-lang") || "en";
const loc = lang === "zh" ? "zh-CN" : lang === "ru" ? "ru-RU" : "en-US";

const timeFull = computed(() =>
  new Intl.DateTimeFormat(loc, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(now.value)
);

const timeHM = computed(() =>
  new Intl.DateTimeFormat(loc, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(now.value)
);

const seconds = computed(() => now.value.getSeconds().toString().padStart(2, "0"));

const dateText = computed(() =>
  new Intl.DateTimeFormat(loc, {
    weekday: "long",
    month: "long",
    day: "numeric",
  }).format(now.value)
);

let intervalTimer = null;

function startClock() {
  const tick = () => {
    now.value = new Date();
  };
  tick();
  intervalTimer = setInterval(tick, 1000);
}

// ═══════════════ 媒体控制（MPRIS）═══════════════
const media = ref(null); // { player, title, artist, status, lengthMs }
// 媒体状态仅轮询刷新，不再自动切换模块：默认保持时间模块，
// 用户滚轮可随时切到媒体模块查看播放信息。

async function pollMedia() {
  try {
    media.value = await invoke("island_media_status");
  } catch {
    media.value = null;
  }
}

async function mediaAction(action) {
  try {
    await invoke("island_media_control", { action });
  } catch {
    // ignore
  }
  setTimeout(pollMedia, 300); // 等 MPRIS 状态更新
}

let mediaTimer = null;

function startMediaPoll() {
  pollMedia();
  mediaTimer = setInterval(pollMedia, 3000);
}

// ═══════════════ 专注倒计时 ═══════════════
// 单位：毫秒。25 分钟 = 25 × 60 × 1000 ms。
// 之前误写成 25*60（毫秒）→ 初始化只剩 1.5 秒，显示 "00:02"，是显示 bug 根因。
const totalMs = ref(25 * 60 * 1000); // 当前倒计时总长（毫秒）
const remainingMs = ref(25 * 60 * 1000); // 剩余（毫秒）
const timerRunning = ref(false);
let timerTick = null;

const timerText = computed(() => {
  const s = Math.max(0, Math.ceil(remainingMs.value / 1000));
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
});

function toggleTimer() {
  if (timerRunning.value) {
    timerRunning.value = false;
    if (timerTick) clearInterval(timerTick);
    timerTick = null;
  } else {
    if (remainingMs.value <= 0) remainingMs.value = totalMs.value;
    timerRunning.value = true;
    timerTick = setInterval(() => {
      remainingMs.value -= 1000;
      if (remainingMs.value <= 0) {
        remainingMs.value = 0;
        timerRunning.value = false;
        if (timerTick) clearInterval(timerTick);
        timerTick = null;
        notifyLocal("Timer", "Focus session finished");
      }
    }, 1000);
  }
}

function resetTimer() {
  remainingMs.value = totalMs.value;
  timerRunning.value = false;
  if (timerTick) clearInterval(timerTick);
  timerTick = null;
}

// 调整倒计时总时长：每次 ±5 分钟，封顶 60 分钟、下限 5 分钟
function adjustTimer(deltaMinutes) {
  const STEP = 5 * 60 * 1000;
  const MIN = 5 * 60 * 1000;
  const MAX = 60 * 60 * 1000;
  totalMs.value = Math.min(MAX, Math.max(MIN, totalMs.value + deltaMinutes * STEP));
  // 未运行时直接同步剩余时间；运行中若剩余超过新总长则截断
  if (!timerRunning.value) {
    remainingMs.value = totalMs.value;
  } else if (remainingMs.value > totalMs.value) {
    remainingMs.value = totalMs.value;
  }
}

// ═══════════════ DeepSeek 余额 ═══════════════
// API Key 由主应用设置页写入 localStorage（devnexus-deepseek-key），
// 本窗口查询时读取并透传给 Rust 命令，key 不在此持久化。
// 状态区分：null=未配置 / loading / error=查询失败 / 数据=成功
const balance = ref(null); // { isAvailable, balanceInfos[] }
const balanceLoading = ref(false);
const balanceError = ref("");

async function loadBalance() {
  // key 从 Rust 侧内存读取（设置页写入），不读 localStorage——
  // Tauri 多窗口 localStorage 按 origin 隔离，岛窗口读不到主窗口的 key
  let key = "";
  try {
    key = await invoke("deepseek_get_key");
  } catch {
    key = "";
  }
  if (!key || !key.trim()) {
    // 兜底：兼容旧版本残留的 localStorage key
    key = localStorage.getItem("devnexus-deepseek-key") || "";
  }
  if (!key || !key.trim()) {
    balance.value = null;
    balanceError.value = "";
    return;
  }
  balanceLoading.value = true;
  balanceError.value = "";
  try {
    // key 由 Rust 侧从 store 读取，前端不传参
    balance.value = await invoke("deepseek_get_balance");
  } catch (e) {
    balance.value = null;
    balanceError.value = String(e).slice(0, 120);
  } finally {
    balanceLoading.value = false;
  }
}

// 轮询检测 key 变化：设置页填 key 后，岛无需重启自动刷新余额
let lastBalanceKey = "";
let keyPollTimer = null;

function startKeyPoll() {
  lastBalanceKey = "";
  loadBalance();
  keyPollTimer = setInterval(async () => {
    let k = "";
    try {
      k = await invoke("deepseek_get_key");
    } catch {
      k = "";
    }
    if (k !== lastBalanceKey) {
      lastBalanceKey = k;
      loadBalance();
    }
  }, 2000);
}

/** 展示用的总余额（优先人民币） */
const balanceText = computed(() => {
  const b = balance.value;
  if (!b || !b.balanceInfos?.length) return null;
  const info = b.balanceInfos.find((i) => i.currency === "CNY") || b.balanceInfos[0];
  return { currency: info.currency, total: info.totalBalance };
});

/** 展示用标题：过长主动截断，避免长标题把胶囊撑满 */
const displayMediaTitle = computed(() => {
  const t = (media.value && media.value.title) || "";
  return t.length > 14 ? `${t.slice(0, 14)}…` : t;
});

/** 展示用播放器名：同样截断 */
const displayMediaArtist = computed(() => {
  const t =
    (media.value && (media.value.artist || media.value.player.replace("org.mpris.MediaPlayer2.", ""))) ||
    "";
  return t.length > 18 ? `${t.slice(0, 18)}…` : t;
});

// ═══════════════ 模块切换（滚轮循环）═══════════════
// 0 = 时间, 1 = DeepSeek 余额, 2 = 倒计时, 3 = 媒体控制；一次只显示一个
const activeModule = ref(0);

const modules = [
  { key: "clock", icon: Timer },
  { key: "balance", icon: Wallet },
  { key: "timer", icon: Timer },
  { key: "media", icon: Music2 },
];

function switchModule(i) {
  const n = modules.length;
  activeModule.value = ((i % n) + n) % n;
}

function nextModule() {
  switchModule(activeModule.value + 1);
}

function prevModule() {
  switchModule(activeModule.value - 1);
}

/** 滚轮切换模块：向上滚=上一个，向下滚=下一个 */
function onWheel(e) {
  if (e.deltaY > 0) nextModule();
  else if (e.deltaY < 0) prevModule();
}

// ═══════════════ 通知横幅（系统通知 / 本地事件）═══════════════
// 横幅优先级：系统通知 > 本地事件（倒计时完成等）
const banner = ref(null); // { app, title, body, kind }
let bannerTimer = null;
let bannerSeq = 0;

function showBanner(app, title, body, kind = "system") {
  bannerSeq += 1;
  const seq = bannerSeq;
  banner.value = { app, title, body, kind };
  // 通知嵌入胶囊：强制展开（窗口恒定，动画由胶囊 CSS spring 完成）
  expanded.value = true;
  if (bannerTimer) clearTimeout(bannerTimer);
  bannerTimer = setTimeout(() => {
    if (bannerSeq === seq) {
      banner.value = null;
      expanded.value = false; // 通知结束自动收起回小胶囊
    }
  }, 5000);
}

/** 本地通知（倒计时完成等）：无图标，走同一条横幅通道 */
function notifyLocal(title, body) {
  showBanner("DevNexus", title, body, "local");
}

// ---- 拖拽 vs 点击判定 ----
// 坐标统一使用物理像素（Tauri 的 Position 均为物理像素）：
//   - 窗口位置：outerPosition() 返回物理像素
//   - 光标位置：用 cursorPosition()（Rust 侧全局光标），不用 e.screenX——
//     WebKitGTK 在 X11/XWayland 下 screenX 不可靠（HiDPI 下只有上下能动、
//     左右无响应的根因之一），cursorPosition 始终返回真实屏幕坐标。
// 按下时记录窗口物理位置 + 光标物理位置，每次 move 用「窗口起点 + 光标位移」重定位。
// 原生 data-tauri-drag-region 会吞掉点击事件，故不用。
const DRAG_THRESHOLD = 4;

let dragStart = null; // { cx, cy, wx, wy } 均为物理像素
let movePending = false; // 防止高频 pointermove 并发 setPosition 乱序
let pressEl = null; // 按下时的元素，判定拖拽后再对其捕获指针

// 阻止 WebKit 原生文本/图片拖拽（HTML5 drag）：否则拖动岛时会把文本
// 当拖拽源拖出去（桌面生成 "Dragged Text-*.txt"），并抢占 pointer 事件
// 导致窗口拖不动。dragstart 在 document 上拦截，覆盖所有子元素。
document.addEventListener("dragstart", (e) => e.preventDefault());
document.addEventListener("drop", (e) => e.preventDefault());
document.addEventListener("dragover", (e) => e.preventDefault());

async function onPointerDown(e) {
  // 交互元素（按钮/横幅）不参与拖拽判定，click 事件交给按钮自身
  if (e.target.closest(".act-btn, .med-btn, .banner")) return;
  dragging.value = false;
  pressEl = e.target;
  const [pos, cur] = await Promise.all([win.outerPosition(), cursorPosition()]);
  dragStart = { cx: cur.x, cy: cur.y, wx: pos.x, wy: pos.y };
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
}

async function onPointerMove(e) {
  if (!dragStart || movePending) return;
  const cur = await cursorPosition();
  const dx = cur.x - dragStart.cx;
  const dy = cur.y - dragStart.cy;
  if (Math.abs(dx) < DRAG_THRESHOLD && Math.abs(dy) < DRAG_THRESHOLD) return;
  if (!dragging.value) {
    // 判定为拖拽后才捕获指针：保证按钮 click 不被吞掉
    dragging.value = true;
    try {
      pressEl?.setPointerCapture?.(e.pointerId);
    } catch {
      // 某些平台不支持捕获，忽略
    }
  }
  movePending = true;
  try {
    await win.setPosition(new PhysicalPosition(dragStart.wx + dx, dragStart.wy + dy));
  } finally {
    movePending = false;
  }
}

// ═══════════════ 跨工作区可见（延迟重试）═══════════════
// 发布版 webview 加载快，首次执行时 GTK 窗口可能尚未 realize，
// tao 的 setVisibleOnAllWorkspaces / Rust 侧 island_set_sticky 会静默失败。
// 这里做多次延迟重试，确保窗口跨工作区常驻。
async function ensureSticky() {
  const attempts = [300, 800, 1500, 3000, 5000];
  for (const delay of attempts) {
    await new Promise((r) => setTimeout(r, delay));
    try {
      await win.setVisibleOnAllWorkspaces(true);
      await invoke("island_set_sticky");
      return; // 成功即停
    } catch {
      // 窗口未就绪，继续下一轮重试
    }
  }
}

/** 位置持久化 key：按窗口 label 隔离（多显示器下每个岛实例各自保存位置，互不覆盖）。
 *  主实例 label="island" 沿用旧 key 兼容历史数据；其他实例 key 带 label 后缀。 */
function posKey(axis) {
  const label = win.label;
  return label === "island"
    ? `devnexus-island-${axis}-v2`
    : `devnexus-island-${axis}-v2-${label}`;
}

async function onPointerUp() {
  if (dragStart) {
    // 拖动结束 → 持久化位置；未达阈值 → 视为点击（无展开/收起切换）
    if (dragging.value) {
      const p = await win.outerPosition(); // 物理像素，恢复时也用 PhysicalPosition
      localStorage.setItem(posKey("x"), String(p.x));
      localStorage.setItem(posKey("y"), String(p.y));
      clickSuppressed = true; // 拖拽结束后抑制紧随其后的 click，防止"拖动也算点击"
    }
  }
  dragStart = null;
  pressEl = null;
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  dragging.value = false;
}

/** 展开视图按钮：唤出主窗口 */
async function openMainWindow() {
  try {
    const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
    const mainWin = await WebviewWindow.getByLabel("main");
    if (mainWin) {
      // 恢复任务栏/Alt-Tab 显示（后台运行时不显示，主动打开后恢复）
      await mainWin.setSkipTaskbar(false);
      await mainWin.unminimize();
      await mainWin.show();
      await mainWin.setFocus();
    }
  } catch {
    // 非 Tauri 环境忽略
  }
}

/** 隐藏岛上气（不改变启用状态，设置页可随时重新显示） */
async function hideIsland() {
  try {
    await win.hide();
  } catch {
    // ignore
  }
}

// ═══════════════ 两态交互（悬停展开 / 移出收起 / 点击切换）═══════════════
// 悬停：鼠标进入胶囊短暂延迟后展开（防误触），移出后收起。
// 点击：切换展开/收起（拖拽结束后 clickSuppressed 会抑制本次 click）。
// 窗口尺寸恒定，展开/收起完全由胶囊 CSS spring 动画过渡，无窗口 resize 抖动。
async function setExpanded(val) {
  if (expanded.value === val) return;
  expanded.value = val;
}

function onMouseEnter() {
  if (banner.value) return; // 横幅显示期间不响应悬停展开（通知已强制展开）
  clearTimeout(hoverTimer);
  hoverTimer = setTimeout(() => setExpanded(true), 120);
}

function onMouseLeave() {
  clearTimeout(hoverTimer);
  if (banner.value) return; // 横幅显示期间不收起：通知由自身计时器统一收起
  setExpanded(false);
}

async function onCapsuleClick() {
  if (clickSuppressed) {
    clickSuppressed = false;
    return;
  }
  if (banner.value) return;
  await setExpanded(!expanded.value);
}

/** 点击横幅：打开主窗口并关闭横幅（收起回小胶囊） */
async function onBannerClick() {
  banner.value = null;
  if (bannerTimer) clearTimeout(bannerTimer);
  bannerTimer = null;
  expanded.value = false; // 窗口恒定，收起由胶囊 CSS 动画完成
  await openMainWindow();
}

/** 关闭横幅（不打开主窗口） */
function closeBanner() {
  banner.value = null;
  if (bannerTimer) clearTimeout(bannerTimer);
  bannerTimer = null;
  expanded.value = false;
}

let unlistenNotify = null;

onMounted(async () => {
  startClock();
  startMediaPoll();
  startKeyPoll(); // 读取设置页保存的 DeepSeek Key 查询余额，并轮询 key 变化
  await resizeWindow(); // 先定尺寸再定位：避免与下方位置恢复竞态，保证默认落在顶部居中
  try {
    // 系统通知监听（Rust 侧 BecomeMonitor 转发）
    unlistenNotify = await listen("island-notify", (ev) => {
      const p = ev.payload;
      if (p && p.title) showBanner(p.app || "Notification", p.title, p.body || "", "system");
    });
  } catch {
    unlistenNotify = null;
  }
  try {
    await win.setAlwaysOnTop(true);
    // 所有工作区可见：灵动岛是全局悬浮窗，切到任意虚拟桌面/工作区都应保持显示。
    // 发布版 webview 加载快，onMounted 首次执行时 GTK 窗口可能尚未 realize，
    // setVisibleOnAllWorkspaces / island_set_sticky 会静默失败 → 用延迟重试兜底。
    ensureSticky();
    // 恢复上次位置；无记录时置于当前窗口所在显示器顶部居中
    // 注意：localStorage 存的是 outerPosition() 的物理像素，恢复必须用 PhysicalPosition
    const x = localStorage.getItem(posKey("x"));
    const y = localStorage.getItem(posKey("y"));
    if (x !== null && y !== null) {
      await win.setPosition(new PhysicalPosition(Number(x), Number(y)));
    } else {
      // 默认放当前窗口所在显示器（currentMonitor）顶部居中，而非主屏——
      // 多显示器下每个岛窗口实例各自定位到自己的显示器。
      const { currentMonitor } = await import("@tauri-apps/api/window");
      const monitor = await currentMonitor();
      if (monitor) {
        const size = await win.outerSize();
        await win.setPosition(
          new PhysicalPosition(
            Math.round(monitor.position.x + (monitor.size.width - size.width) / 2),
            Math.round(monitor.position.y + 12)
          )
        );
      }
    }
  } catch {
    // 非 Tauri 环境：保留窗口当前位置
  }
});

onBeforeUnmount(() => {
  if (intervalTimer) clearInterval(intervalTimer);
  if (mediaTimer) clearInterval(mediaTimer);
  if (timerTick) clearInterval(timerTick);
  if (keyPollTimer) clearInterval(keyPollTimer);
  if (bannerTimer) clearTimeout(bannerTimer);
  if (unlistenNotify) unlistenNotify();
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
});

// ── 调试自检：把实际渲染几何/颜色/缩放写入窗口标题，xwininfo 可直接读取 ──
function selfReport() {
  const el = document.querySelector(".capsule");
  if (!el) return;
  const r = el.getBoundingClientRect();
  const cs = getComputedStyle(el);
  document.title =
    `w=${Math.round(r.width)} h=${Math.round(r.height)} r=${cs.borderRadius}` +
    ` bg=${cs.backgroundColor} dpr=${window.devicePixelRatio}` +
    ` win=${window.innerWidth}x${window.innerHeight} mod=${activeModule.value}`;
}

// ═══════════════ 窗口尺寸 ═══════════════
// 窗口尺寸恒定（400×116）：收起/展开完全由胶囊 CSS 动画完成，
// 窗口不再随状态 resize——否则收起瞬间窗口缩小会把 384px 宽的大胶囊
// 裁剪成"长方形"，且 resize 与 CSS 动画竞争导致卡顿。
const WIN_ISLAND = { w: 400, h: 116 };

/** 设置窗口尺寸并保持顶部 y 不变、水平中心不变（仅创建/恢复时调用一次） */
async function resizeWindow() {
  try {
    const pos = await win.outerPosition();
    const size = await win.outerSize();
    await win.setSize(new LogicalSize(WIN_ISLAND.w, WIN_ISLAND.h));
    // resize 后保持顶部 y 不变、水平中心不变（窗口左右各扩/缩一半）
    const newSize = await win.outerSize();
    const centerX = pos.x + size.width / 2;
    await win.setPosition(
      new PhysicalPosition(Math.round(centerX - newSize.width / 2), pos.y)
    );
  } catch {
    // 非 Tauri 环境忽略
  }
}

onMounted(() => {
  setTimeout(selfReport, 250);
});
watch(activeModule, () => {
  setTimeout(selfReport, 300); // 切换模块后上报几何
});
</script>

<template>
  <div
    class="island"
    @pointerdown="onPointerDown"
    @wheel="onWheel"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
    @click="onCapsuleClick"
  >
    <!-- 胶囊层：紧凑药丸（收起态）或大胶囊（展开态），内容随模块切换 -->
    <div class="capsule" :class="{ expanded }">
      <!-- 通知横幅：嵌入胶囊内部（通知到来时胶囊膨胀展示，结束收起） -->
      <div v-if="banner" class="banner" @click.stop="onBannerClick">
        <div class="banner-icon">
          <Music2 v-if="banner.kind === 'system'" :size="18" />
          <Timer v-else :size="18" />
        </div>
        <div class="banner-text">
          <div class="banner-title">{{ banner.title }}</div>
          <div v-if="banner.body" class="banner-body">{{ banner.body }}</div>
        </div>
        <button class="banner-close" @click.stop="closeBanner">
          <X :size="14" />
        </button>
      </div>
      <!-- 模块内容（一次只显示一个；滚轮循环切换；事件发生时优先显示） -->
      <div v-else class="exp-body">
        <!-- 模块0：时间 -->
        <div v-if="activeModule === 0" class="module module-clock">
          <span class="clock-hm">{{ timeHM }}</span>
          <span class="clock-sec">{{ seconds }}</span>
        </div>

        <!-- 模块1：DeepSeek 余额 -->
        <div v-else-if="activeModule === 1" class="module module-balance">
          <template v-if="balanceText">
            <img :src="deepseekIcon" class="bal-icon" alt="DeepSeek" draggable="false" />
            <div class="bal-info">
              <div class="bal-total">{{ balanceText.total }} <span class="bal-cur">{{ balanceText.currency }}</span></div>
              <div class="bal-status" :class="balance?.isAvailable ? 'ok' : 'bad'">
                {{ balance?.isAvailable ? "可用" : "余额不足" }}
              </div>
            </div>
          </template>
          <div v-else-if="balanceLoading" class="bal-empty">
            <img :src="deepseekIcon" class="bal-empty-icon" alt="DeepSeek" draggable="false" />
            <span>查询中…</span>
          </div>
          <div v-else-if="balanceError" class="bal-empty">
            <img :src="deepseekIcon" class="bal-empty-icon" alt="DeepSeek" draggable="false" />
            <span class="bal-error-text">查询失败</span>
          </div>
          <div v-else class="bal-empty">
            <img :src="deepseekIcon" class="bal-empty-icon" alt="DeepSeek" draggable="false" />
            <span>未配置 DeepSeek Key</span>
          </div>
        </div>

        <!-- 模块2：专注倒计时（时间 + 时长调整 +/- + 开始/暂停/重置） -->
        <div v-else-if="activeModule === 2" class="module module-timer">
          <button class="med-btn" title="-5 min" @click.stop="adjustTimer(-1)">
            <Minus :size="14" />
          </button>
          <Timer :size="16" class="timer-icon" />
          <span class="timer-text" :class="{ done: remainingMs <= 0 }">{{ timerText }}</span>
          <button class="med-btn" :title="timerRunning ? 'Pause' : 'Start'" @click.stop="toggleTimer">
            <Pause v-if="timerRunning" :size="14" />
            <Play v-else :size="14" />
          </button>
          <button class="med-btn" title="Reset" @click.stop="resetTimer">
            <RotateCcw :size="14" />
          </button>
          <button class="med-btn" title="+5 min" @click.stop="adjustTimer(1)">
            <Plus :size="14" />
          </button>
        </div>

        <!-- 模块3：媒体控制（收起态横向一行；展开态上行标题、下行按钮+播放器名） -->
        <div v-else class="module module-media" :class="{ expanded }">
          <template v-if="media && (media.title || media.status)">
            <!-- 展开态：上下结构（上行标题、下行控制按钮 + 播放器名） -->
            <div v-if="expanded" class="media-expanded">
              <div class="med-title" :title="media.title">{{ displayMediaTitle }}</div>
              <div class="media-controls">
                <button class="med-btn" title="Previous" @click.stop="mediaAction('previous')">
                  <SkipBack :size="14" />
                </button>
                <button
                  class="med-btn"
                  :title="media.status === 'Playing' ? 'Pause' : 'Play'"
                  @click.stop="mediaAction('play_pause')"
                >
                  <Pause v-if="media.status === 'Playing'" :size="14" />
                  <Play v-else :size="14" />
                </button>
                <button class="med-btn" title="Next" @click.stop="mediaAction('next')">
                  <SkipForward :size="14" />
                </button>
                <span class="med-artist">{{ displayMediaArtist }}</span>
              </div>
            </div>
            <!-- 收起态：横向一行 -->
            <template v-else>
              <button class="med-btn" title="Previous" @click.stop="mediaAction('previous')">
                <SkipBack :size="14" />
              </button>
              <button
                class="med-btn"
                :title="media.status === 'Playing' ? 'Pause' : 'Play'"
                @click.stop="mediaAction('play_pause')"
              >
                <Pause v-if="media.status === 'Playing'" :size="14" />
                <Play v-else :size="14" />
              </button>
              <button class="med-btn" title="Next" @click.stop="mediaAction('next')">
                <SkipForward :size="14" />
              </button>
              <div class="med-info">
                <div class="med-title" :title="media.title">{{ displayMediaTitle }}</div>
                <div class="med-artist">{{ displayMediaArtist }}</div>
              </div>
            </template>
          </template>
          <div v-else class="med-empty">
            <Music2 :size="16" />
            <span>未检测到播放器</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
