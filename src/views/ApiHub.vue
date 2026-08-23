<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { useRoute } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat, getLang } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
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
import { Tabs, TabsContent } from "@/components/ui/tabs";
import ProviderForm from "../components/hub/ProviderForm.vue";

const route = useRoute();

// 子导航由路由驱动（侧边栏 /api-hub、/api-hub/providers、/api-hub/endpoints、/api-hub/logs）
const activeTab = computed(() => {
  const seg = route.path.split("/")[2];
  return seg === "providers" || seg === "endpoints" || seg === "logs" ? seg : "stats";
});
const providers = ref([]);
const logs = ref([]);
const stats = ref(null);
const status = ref({ running: false, port: 3456, auth_token: "" });
const loading = ref(false);
const error = ref(null);
const showForm = ref(false);
const editingId = ref(null);
// 日志分页与 token 脱敏
const LOG_PAGE_SIZE = 20;
const logOffset = ref(0);
const hasMoreLogs = ref(true);
const loadingMore = ref(false);
const showToken = ref(false);

// 单一协议选项：同时决定品牌、线协议、端点与认证方式
const protocolOptions = computed(() => [
  {
    id: "openai_chat",
    label: t("apiHub.protocol.openai_chat.label"),
    defaultUrl: "https://api.openai.com",
    endpoint: "/v1/chat/completions",
    desc: t("apiHub.protocol.openai_chat.desc"),
  },
  {
    id: "openai_responses",
    label: t("apiHub.protocol.openai_responses.label"),
    defaultUrl: "https://api.openai.com",
    endpoint: "/v1/responses",
    desc: t("apiHub.protocol.openai_responses.desc"),
  },
  {
    id: "anthropic",
    label: t("apiHub.protocol.anthropic.label"),
    defaultUrl: "https://api.anthropic.com",
    endpoint: "/v1/messages",
    desc: t("apiHub.protocol.anthropic.desc"),
  },
  {
    id: "gemini",
    label: t("apiHub.protocol.gemini.label"),
    defaultUrl: "https://generativelanguage.googleapis.com",
    endpoint: "/v1beta/models/{model}:generateContent",
    desc: t("apiHub.protocol.gemini.desc"),
  },
  {
    id: "ollama",
    label: t("apiHub.protocol.ollama.label"),
    defaultUrl: "http://localhost:11434",
    endpoint: "/api/chat",
    desc: t("apiHub.protocol.ollama.desc"),
  },
]);

let pollTimer = null;

onMounted(() => {
  loadData();
  // 轮询以 visibility 感知为主，间隔 20s，隐藏时暂停
  pollTimer = setInterval(() => {
    if (document.hidden) return;
    loadStats();
  }, 20000);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) loadStats();
  });
});
onBeforeUnmount(() => {
  if (pollTimer) clearInterval(pollTimer);
});

async function loadData() {
  loading.value = true;
  error.value = null;
  try {
    const [p, l, s, st] = await Promise.all([
      invoke("api_hub_list_providers"),
      invoke("api_hub_get_logs", { limit: LOG_PAGE_SIZE, offset: 0 }),
      invoke("api_hub_get_usage_stats"),
      invoke("api_hub_status"),
    ]);
    providers.value = p;
    logs.value = l;
    logOffset.value = l.length;
    hasMoreLogs.value = l.length === LOG_PAGE_SIZE;
    stats.value = s;
    status.value = st;
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
  }
}

async function loadStats() {
  if (document.hidden) return;
  try {
    // stats 与 logs 分离，logs 分页为 20 条，避免一次性拉 50 条阻塞渲染
    const s = await invoke("api_hub_get_usage_stats");
    stats.value = s;
  } catch {}
}

async function refreshLogs() {
  try {
    const l = await invoke("api_hub_get_logs", { limit: LOG_PAGE_SIZE, offset: 0 });
    logs.value = l;
    logOffset.value = l.length;
    hasMoreLogs.value = l.length === LOG_PAGE_SIZE;
  } catch {}
}

async function loadMoreLogs() {
  if (loadingMore.value || !hasMoreLogs.value) return;
  loadingMore.value = true;
  try {
    const more = await invoke("api_hub_get_logs", { limit: LOG_PAGE_SIZE, offset: logOffset.value });
    logs.value = [...logs.value, ...more];
    logOffset.value += more.length;
    hasMoreLogs.value = more.length === LOG_PAGE_SIZE;
  } catch {}
  finally { loadingMore.value = false; }
}

function beginAdd() {
  editingId.value = null;
  showForm.value = true;
}
function beginEdit(p) {
  editingId.value = p.id;
  showForm.value = true;
}
function cancelForm() {
  showForm.value = false;
  editingId.value = null;
}

async function saveProvider(data, isEdit) {
  try {
    if (isEdit) {
      await invoke("api_hub_update_provider", { id: data.id, provider: data });
      showToast(t("apiHub.toast.updated"));
    } else {
      await invoke("api_hub_add_provider", { provider: data });
      showToast(t("apiHub.toast.added"));
    }
    showForm.value = false;
    editingId.value = null;
    providers.value = await invoke("api_hub_list_providers");
  } catch (err) {
    showToast(tFormat("apiHub.toast.error", { error: friendlyError(err) }), "error");
  }
}

async function deleteProvider(id) {
  const p = providers.value.find((x) => x.id === id);
  const ok = await showConfirm(
    tFormat("apiHub.confirmDelete", { name: p?.name || id }),
    t("apiHub.deleteProvider")
  );
  if (!ok) return;
  try {
    await invoke("api_hub_delete_provider", { id });
    showToast(t("apiHub.toast.deleted"));
    providers.value = await invoke("api_hub_list_providers");
  } catch (err) {
    showToast(tFormat("apiHub.toast.deleteFailed", { error: friendlyError(err) }), "error");
  }
}

function protocolName(id) {
  return protocolOptions.value.find((p) => p.id === id)?.label || id;
}

function fmtTokens(n) {
  if (!n) return "0";
  return new Intl.NumberFormat(getLang().value, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(n);
}

const LOCALE_TAGS = { zh: "zh-CN", en: "en-US", ru: "ru-RU" };
function localeTag() {
  return LOCALE_TAGS[getLang().value] || "en-US";
}
function fmtTime(ts) {
  return ts ? new Date(ts * 1000).toLocaleTimeString(localeTag()) : "-";
}
function fmtLatency(ms) {
  return !ms ? "-" : ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}
function statusColor(c) {
  return c >= 200 && c < 300 ? "ok" : c >= 400 ? "bad" : "warn";
}

// ── GitHub 风格贡献热力图：53 周 × 7 天 ───────────────
const HEATMAP_WEEKS = 53;

function dayMs(date) {
  const d = date instanceof Date ? date : new Date(date);
  return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate());
}
function heatmapDayKey(ms) {
  // 与后端 (timestamp / 86400) * 86400 对齐（UTC 日界）
  return Math.floor(ms / 1000 / 86400) * 86400;
}

// 从本周日起往前铺满 53 周，生成 [ms, count] 网格（周日→周六 = 列内 7 行）
function heatmapCells() {
  const dayMap = stats.value?.by_day || {};
  const todayMs = dayMs(Date.now());
  const start = todayMs - (HEATMAP_WEEKS * 7 - 1) * 86400000;
  const firstSunday = start - new Date(start).getUTCDay() * 86400000;
  const cells = [];
  for (let ms = firstSunday; ms <= todayMs; ms += 86400000) {
    cells.push({
      ms,
      count: dayMap[heatmapDayKey(ms)]?.requests || 0,
    });
  }
  return cells;
}

// 每列（周）的周三代表该周，月份变化时在顶部标注。
// 数量与列数一一对应（与 heatmap-months 的 gridTemplateColumns 对齐），
// 超出列数的留空，避免错位。
function heatmapMonths(cells, cols) {
  const labels = Array(cols).fill("");
  let prev = "";
  for (let w = 0; w < cols; w++) {
    const idx = w * 7 + 3;
    if (idx >= cells.length) break;
    const wed = new Date(cells[idx].ms);
    const name = wed.toLocaleDateString(localeTag(), { month: "short" });
    if (name !== prev) {
      labels[w] = name;
      prev = name;
    }
  }
  return labels;
}

function heatmapMax(cells) {
  return cells.length ? Math.max(...cells.map((c) => c.count), 1) : 1;
}

// GitHub 式 5 档配色：0 → 底色（浅色模式下也可见），其余按量分 4 级。
// 用 color-mix 在 primary 与 accent 间取色；无数据格子退化为 accent（略深于卡片背景）。
function heatmapColor(count, max) {
  if (!count) return "var(--color-accent)";
  const pct = count / Math.max(max, 1);
  const lvl = pct <= 0.25 ? 0.3 : pct <= 0.5 ? 0.45 : pct <= 0.75 ? 0.6 : 0.85;
  return `color-mix(in srgb, var(--color-primary) ${(lvl * 100).toFixed(1)}%, var(--color-accent))`;
}
function heatmapTooltip(cell) {
  const date = new Date(cell.ms).toISOString().slice(0, 10);
  return t("apiHub.heatmap.requestTitle")
    .replace("{date}", date)
    .replace("{count}", cell.count);
}
// 一次计算整张热力图：格子、月份标注、最大值
const heatmap = computed(() => {
  const cells = heatmapCells();
  const cols = Math.ceil(cells.length / 7);
  return {
    cells,
    months: heatmapMonths(cells, cols),
    max: heatmapMax(cells),
    cols,
  };
});

// 过去一年请求总数（复用 heatmap 计算，避免重复全量遍历）
const heatmapTotal = computed(() =>
  heatmap.value.cells.reduce((sum, c) => sum + c.count, 0)
);

// 模型排行：按 by_model 引用变化才重算，避免每次渲染排序
const modelEntries = computed(() =>
  stats.value?.by_model
    ? Object.entries(stats.value.by_model).sort(
        (a, b) => Number(b[1]?.requests) - Number(a[1]?.requests)
      )
    : []
);
const modelMax = computed(() =>
  modelEntries.value.length ? modelEntries.value[0][1].requests : 1
);

function getModelEntries() {
  return modelEntries.value;
}
function getAlias(p, id) {
  return p.model_aliases?.[id] || id;
}

const endpoints = computed(() =>
  status.value
    ? [
        `http://localhost:${status.value.port}/v1/chat/completions`,
        `http://localhost:${status.value.port}/v1/responses`,
        `http://localhost:${status.value.port}/v1/messages`,
        `http://localhost:${status.value.port}/v1beta/models/{model}:generateContent`,
        `http://localhost:${status.value.port}/api/chat`,
      ]
    : []
);

// Token 脱敏：status.auth_token 已为掩码，按需通过 api_hub_get_token 拉取明文
const realToken = ref("");
const maskedToken = computed(() => status.value?.auth_token || status.value?.auth_token_masked || "");
async function toggleToken() {
  if (!showToken.value) {
    if (!realToken.value) {
      try { realToken.value = await invoke("api_hub_get_token"); } catch { realToken.value = ""; }
    }
  }
  showToken.value = !showToken.value;
}

async function copyEndpoint(url) {
  try {
    await navigator.clipboard.writeText(url);
    showToast(t("apiHub.gateway.copied"));
  } catch {
    showToast(t("apiHub.gateway.copyFailed"), "error");
  }
}

async function copyToken() {
  try {
    const tok = realToken.value || (await invoke("api_hub_get_token"));
    if (!realToken.value) realToken.value = tok;
    await navigator.clipboard.writeText(tok || "");
    showToast(t("apiHub.gateway.copied"));
  } catch {
    showToast(t("apiHub.gateway.copyFailed"), "error");
  }
}

const metricCards = computed(() =>
  stats.value
    ? [
        {
          icon: "fire",
          label: t("apiHub.metrics.tokens"),
          value: fmtTokens(stats.value.total_input_tokens + stats.value.total_output_tokens),
        },
        {
          icon: "chat",
          label: t("apiHub.metrics.requests"),
          value: fmtTokens(stats.value.total_requests),
        },
        {
          icon: "check",
          label: t("apiHub.metrics.successRate"),
          value: stats.value.total_requests
            ? `${(100 * (1 - stats.value.total_errors / stats.value.total_requests)).toFixed(1)}%`
            : "——",
        },
        {
          icon: "clock",
          label: t("apiHub.metrics.avgLatency"),
          value: stats.value.total_requests ? fmtLatency(stats.value.avg_latency_ms) : "——",
        },
      ]
    : []
);

const logColumns = computed(() => [
  { title: t("apiHub.logs.time"), slotName: "time", width: 90 },
  { title: t("apiHub.logs.model"), slotName: "model" },
  { title: t("apiHub.logs.provider"), slotName: "provider", width: 140 },
  { title: t("apiHub.logs.tokens"), slotName: "tokens", align: "right", width: 140 },
  { title: t("apiHub.logs.latency"), slotName: "latency", align: "right", width: 90 },
  { title: t("apiHub.logs.status"), slotName: "status", align: "center", width: 80 },
]);
</script>

<template>
  <div class="page apihub-page">
    <!-- ════ Header ════ -->
    <div class="page-header">
      <div>
        <h1 class="page-title">API Hub</h1>
      </div>
    </div>

    <!-- ════ 内容区：子导航由侧边栏路由驱动 ════ -->
    <Tabs :model-value="activeTab" class="apihub-tabs">
      <!-- ════ STATS ════ -->
      <TabsContent value="stats">
          <Alert v-if="error" variant="destructive" class="mb-4">
            <AppIcon name="close-circle-fill" class="size-4" />
            <AlertTitle>{{ t("error.title") }}</AlertTitle>
            <AlertDescription>{{ error }}</AlertDescription>
          </Alert>

          <div v-if="loading && !providers.length" class="loading-block">
            <div class="space-y-3 py-2">
              <Skeleton v-for="i in 6" :key="i" class="h-4 w-full" />
            </div>
          </div>

          <template v-else-if="stats">
          <!-- Metric cards -->
          <div class="mb-3 grid grid-cols-2 gap-4 lg:grid-cols-4">
            <Card v-for="card in metricCards" :key="card.label" class="metric-card">
              <CardContent class="pt-4">
                <div class="metric-head">
                  <AppIcon v-if="card.icon === 'fire'" name="fire" class="metric-icon size-4" />
                  <AppIcon v-else-if="card.icon === 'chat'" name="chat-line" class="metric-icon size-4" />
                  <AppIcon v-else-if="card.icon === 'check'" name="check-circle" class="metric-icon size-4" />
                  <AppIcon v-else name="clock-circle" class="metric-icon size-4" />
                  <span class="metric-label">{{ card.label }}</span>
                </div>
                <div class="metric-value">{{ card.value }}</div>
              </CardContent>
            </Card>
          </div>

          <!-- Heatmap: GitHub 风格贡献图 -->
          <Card class="mb-4">
            <CardHeader class="flex-row items-center justify-between space-y-0">
              <CardTitle class="text-sm font-medium">{{ t("apiHub.heatmap.title") }}</CardTitle>
              <div class="legend">
                <span>{{ t("apiHub.heatmap.less") }}</span>
                <span
                  v-for="lv in [0, 0.3, 0.55, 0.8, 1]"
                  :key="lv"
                  class="legend-cell"
                  :style="{ background: heatmapColor(lv, 1) }"
                ></span>
                <span>{{ t("apiHub.heatmap.more") }}</span>
              </div>
            </CardHeader>
            <CardContent>
              <template v-if="stats.total_requests > 0">
                <p class="heatmap-summary">
                  {{ tFormat("apiHub.heatmap.summary", { count: heatmapTotal }) }}
                </p>
              </template>
              <div class="heatmap-wrap">
                <!-- 左列：星期标签 -->
                <div class="heatmap-days">
                  <span class="heatmap-day-label"></span>
                  <span class="heatmap-day-label">Mon</span>
                  <span class="heatmap-day-label"></span>
                  <span class="heatmap-day-label">Wed</span>
                  <span class="heatmap-day-label"></span>
                  <span class="heatmap-day-label">Fri</span>
                  <span class="heatmap-day-label"></span>
                </div>
                <div class="heatmap-body">
                  <div
                    class="heatmap-months"
                    :style="{ gridTemplateColumns: `repeat(${heatmap.cols}, 12px)` }"
                  >
                    <span v-for="(m, i) in heatmap.months" :key="i" class="heatmap-month">
                      {{ m }}
                    </span>
                  </div>
                  <div class="heatmap-grid">
                    <div
                      v-for="cell in heatmap.cells"
                      :key="cell.ms"
                      class="heatmap-cell"
                      :style="{ background: heatmapColor(cell.count, heatmap.max) }"
                      :title="heatmapTooltip(cell)"
                    ></div>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>

          <!-- Model Usage -->
          <Card>
            <CardHeader>
              <CardTitle class="text-sm font-medium">{{ t("apiHub.models.usageRanking") }}</CardTitle>
            </CardHeader>
            <CardContent>
              <div v-if="getModelEntries().length > 0" class="model-rank">
                <div
                  v-for="([model, md], i) in getModelEntries().slice(0, 15)"
                  :key="model"
                  class="rank-row"
                >
                  <span class="rank-index">{{ i + 1 }}</span>
                  <div class="rank-model" :title="model">{{ model }}</div>
                  <div class="rank-bar-track">
                    <div
                      class="rank-bar"
                      :style="{ width: `${(md.requests / modelMax) * 100}%` }"
                    ></div>
                  </div>
                  <span class="rank-tokens">{{ fmtTokens(md.input_tokens + md.output_tokens) }} {{ t("apiHub.models.tokens") }}</span>
                  <span class="rank-requests">{{ md.requests }} {{ t("apiHub.models.requestsSuffix") }}</span>
                </div>
                <div v-if="getModelEntries().length > 15" class="rank-more">
                  {{ tFormat("apiHub.models.onlyTop15", { count: getModelEntries().length }) }}
                </div>
              </div>
              <Empty v-else class="py-4">
                <EmptyContent>
                  <EmptyDescription>{{ t("apiHub.empty.noData") }}</EmptyDescription>
                </EmptyContent>
              </Empty>
            </CardContent>
          </Card>
        </template>

        <Card v-else class="empty-state">
          <AppIcon name="bar-chart" class="empty-state-icon size-7" />
          <div class="empty-state-text">{{ t("apiHub.empty.waiting") }}</div>
        </Card>
      </TabsContent>

      <!-- ════ PROVIDERS ════ -->
      <TabsContent value="providers">
        <div class="toolbar">
          <span class="toolbar-count">
            {{ tFormat("apiHub.providerCount", { count: providers.length }) }}
          </span>
          <Button v-if="!showForm" size="sm" @click="beginAdd">
            <AppIcon name="plus" class="size-4" />
            {{ t("apiHub.addProvider") }}
          </Button>
        </div>

        <!-- Add/Edit form -->
        <ProviderForm
          v-if="showForm"
          :mode="editingId ? 'edit' : 'add'"
          :title="editingId ? t('apiHub.editProvider') : t('apiHub.addProvider')"
          :subtitle="providers.find((p) => p.id === editingId)?.name || ''"
          :initial="providers.find((p) => p.id === editingId) || null"
          :protocol-options="protocolOptions"
          :on-save="saveProvider"
          :on-cancel="cancelForm"
        />

        <!-- Provider list -->
        <div v-for="p in providers" :key="p.id" class="provider-card">
          <ProviderForm
            v-if="showForm && editingId === p.id"
            mode="edit"
            :title="t('apiHub.editProvider')"
            :subtitle="p.name"
            :initial="p"
            :protocol-options="protocolOptions"
            :on-save="saveProvider"
            :on-cancel="cancelForm"
          />
          <div v-else class="provider-row">
            <div class="provider-info">
              <div class="provider-avatar">
                <AppIcon name="relation" class="provider-avatar-icon size-4" />
              </div>
              <div class="provider-main">
                <div class="provider-title-row">
                  <span class="provider-name">{{ p.name }}</span>
                  <Badge class="bg-primary/10 text-primary">{{ protocolName(p.protocol) }}</Badge>
                  <span class="provider-status" :class="p.enabled ? 'on' : 'off'">
                    <span class="mini-dot" :class="p.enabled ? 'on' : 'off'"></span>
                    {{ p.enabled ? t("apiHub.status.active") : t("apiHub.status.disabled") }}
                  </span>
                </div>
                <div class="provider-url">{{ p.base_url }}</div>
                <div class="provider-models">
                  <Badge
                    v-for="m in p.models.slice(0, 8)"
                    :key="m"
                    variant="secondary"
                    class="model-tag"
                  >
                    {{ getAlias(p, m) }}
                  </Badge>
                  <span v-if="p.models.length > 8" class="model-more">+{{ p.models.length - 8 }}</span>
                </div>
              </div>
            </div>
            <div class="provider-actions">
              <Button
                variant="ghost"
                size="icon"
                class="h-7 w-7"
                :title="t('apiHub.edit')"
                @click="beginEdit(p)"
              >
                <AppIcon name="edit" class="size-4" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                class="h-7 w-7 text-destructive hover:text-destructive"
                :title="t('apiHub.delete')"
                @click="deleteProvider(p.id)"
              >
                <AppIcon name="delete" class="size-4" />
              </Button>
            </div>
          </div>
        </div>

        <!-- Empty state -->
        <Card v-if="providers.length === 0 && !showForm" class="empty-state">
          <AppIcon name="relation" class="empty-state-icon size-7" />
          <div class="empty-state-text">{{ t("apiHub.empty.noProviders") }}</div>
          <p class="empty-state-hint">{{ t("apiHub.empty.addHint") }}</p>
          <Button size="sm" class="mt-3" @click="beginAdd">
            <AppIcon name="plus" class="size-4" />
            {{ t("apiHub.addFirstProvider") }}
          </Button>
        </Card>
      </TabsContent>

      <!-- ════ ENDPOINTS ════ -->
      <TabsContent value="endpoints">
        <Card class="gateway-card">
          <CardContent>
            <div class="gateway-head">
              <div class="gateway-title-row">
                <span class="status-dot" :class="status?.running ? 'on' : 'off'"></span>
                <h2 class="gateway-title">{{ t("apiHub.gateway.title") }}</h2>
                <Badge class="bg-primary/10 text-primary">localhost:{{ status?.port }}</Badge>
                <Badge
                  v-if="status?.running && status?.auth_token"
                  class="bg-success/10 text-success dark:text-success"
                >
                  {{ t("apiHub.gateway.authEnabled") }}
                </Badge>
              </div>
              <p class="gateway-desc">{{ t("apiHub.gateway.desc") }}</p>
            </div>

            <div class="endpoint-grid">
              <button
                v-for="ep in endpoints"
                :key="ep"
                type="button"
                class="endpoint-btn"
                :title="t('apiHub.gateway.copyTooltip')"
                @click="copyEndpoint(ep)"
              >
                <AppIcon name="copy" class="endpoint-icon size-4" />
                <span class="endpoint-text">{{ ep }}</span>
              </button>
            </div>

            <!-- Auth token display -->
            <div v-if="status?.running && status?.auth_token" class="token-row">
              <div class="token-info" style="cursor: pointer" @click="toggleToken">
                <AppIcon name="lock" class="token-icon size-4" />
                <span class="token-label">X-DevNexus-Token</span>
                <code class="token-value">{{ showToken ? realToken : maskedToken }}</code>
                <AppIcon :name="showToken ? 'eye-close' : 'eye'" class="size-3.5 opacity-50" />
              </div>
              <Button size="sm" variant="outline" @click="copyToken">
                <AppIcon name="copy" class="size-3.5" />
                {{ t("apiHub.gateway.copyTooltip") }}
              </Button>
            </div>
            <p v-if="status?.key_encrypted === false" class="mt-2 text-xs text-warning">
              {{ t("apiHub.gateway.notEncrypted") || "Key storage unavailable, API keys stored in plaintext" }}
            </p>
          </CardContent>
        </Card>
      </TabsContent>

      <!-- ════ LOGS ════ -->
      <TabsContent value="logs">
        <Card>
          <CardContent class="p-0">
            <div class="max-h-[500px] overflow-y-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead
                      v-for="col in logColumns"
                      :key="col.slotName"
                      :class="[
                        col.align === 'right' && 'text-right',
                        col.align === 'center' && 'text-center',
                      ]"
                      :style="col.width ? { width: col.width + 'px' } : {}"
                    >
                      {{ col.title }}
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-for="(record, i) in logs" :key="i">
                    <TableCell>
                      <span class="log-mono log-muted">{{ fmtTime(record.timestamp) }}</span>
                    </TableCell>
                    <TableCell>
                      <span class="log-model">
                        {{ record.model }}
                        <AppIcon
                          v-if="record.is_streaming"
                          name="water-drop"
                          class="log-stream size-3"
                          :title="t('apiHub.logs.streaming')"
                        />
                      </span>
                    </TableCell>
                    <TableCell>
                      <span class="log-muted">{{ record.provider_name }}</span>
                    </TableCell>
                    <TableCell class="text-right">
                      <span class="log-tokens">
                        <span class="log-token-val">↑{{ fmtTokens(record.input_tokens) }}</span>
                        <span class="log-token-sep">/</span>
                        <span class="log-token-val">↓{{ fmtTokens(record.output_tokens) }}</span>
                      </span>
                    </TableCell>
                    <TableCell class="text-right">
                      <span class="log-muted">{{ fmtLatency(record.latency_ms) }}</span>
                    </TableCell>
                    <TableCell class="text-center">
                      <span
                        class="log-status"
                        :class="statusColor(record.status_code)"
                        :title="record.error_message || ''"
                      >
                        <span class="status-mini-dot"></span>
                        {{ record.status_code || "—" }}
                      </span>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>

              <Empty v-if="logs.length === 0" class="py-4">
                <EmptyMedia>
                  <AppIcon name="file" class="logs-empty-icon size-5" />
                </EmptyMedia>
                <EmptyContent>
                  <EmptyDescription>{{ t("apiHub.logs.empty") }}</EmptyDescription>
                </EmptyContent>
              </Empty>
              <div v-else-if="hasMoreLogs" class="flex justify-center py-3">
                <Button size="sm" variant="outline" :disabled="loadingMore" @click="loadMoreLogs">
                  {{ loadingMore ? t("loading") : t("apiHub.logs.loadMore") }}
                </Button>
              </div>
              <div v-else-if="logs.length >= LOG_PAGE_SIZE" class="py-2 text-center text-xs text-muted-foreground">
                {{ t("apiHub.logs.allLoaded") }}
              </div>
            </div>
            <div class="flex justify-end px-4 py-2">
              <Button size="sm" variant="ghost" @click="refreshLogs">{{ t("refresh") }}</Button>
            </div>
          </CardContent>
        </Card>
      </TabsContent>
    </Tabs>
  </div>
</template>

<style scoped>
.gateway-card {
  margin-bottom: 16px;
}
.gateway-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}
.gateway-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
  flex-shrink: 0;
}
.status-dot.on {
  background-color: var(--color-success);
}
.status-dot.off {
  background-color: var(--color-muted-foreground);
}
.gateway-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-foreground);
  margin: 0;
}
.gateway-desc {
  font-size: 11px;
  color: var(--color-muted-foreground);
  margin: 0;
}
.endpoint-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}
.endpoint-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-muted);
  cursor: pointer;
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-muted-foreground);
  text-align: left;
  transition: all 0.15s;
  overflow: hidden;
}
.endpoint-btn:hover {
  border-color: var(--color-primary);
  color: var(--color-foreground);
}
.endpoint-icon {
  opacity: 0.5;
  flex-shrink: 0;
}
.endpoint-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.token-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 12px;
  padding: 10px 12px;
  background-color: var(--color-muted);
  border: 1px dashed var(--color-border);
  border-radius: 8px;
}
.token-info {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.token-icon {
  color: var(--color-success);
  flex-shrink: 0;
}
.token-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--color-muted-foreground);
  flex-shrink: 0;
}
.token-value {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-muted-foreground);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.apihub-tabs {
  width: 100%;
}
.loading-block {
  padding: 8px 0;
}
.metric-card {
  border-radius: 10px;
}
.metric-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
}
.metric-icon {
  opacity: 0.6;
  color: var(--color-muted-foreground);
}
.metric-label {
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.metric-value {
  font-size: 20px;
  font-weight: 600;
  color: var(--color-foreground);
  letter-spacing: -0.02em;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.card-head-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}
.legend {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
  color: var(--color-muted-foreground);
}
.legend-cell {
  width: 12px;
  height: 12px;
  border-radius: 3px;
}
.heatmap-summary {
  margin: 0 0 12px;
  font-size: 12px;
  color: var(--color-muted-foreground);
  font-variant-numeric: tabular-nums;
}
.heatmap-wrap {
  display: flex;
  gap: 6px;
  overflow-x: auto;
  padding-bottom: 4px;
}
.heatmap-days {
  display: grid;
  grid-template-rows: repeat(7, 12px);
  gap: 3px;
  font-size: 10px;
  color: var(--color-muted-foreground);
  flex-shrink: 0;
}
.heatmap-day-label {
  height: 12px;
  line-height: 12px;
}
.heatmap-body {
  min-width: 0;
}
.heatmap-months {
  display: grid;
  gap: 3px;
  height: 18px;
  margin-bottom: 3px;
}
.heatmap-month {
  font-size: 10px;
  color: var(--color-muted-foreground);
  white-space: nowrap;
  overflow: hidden;
}
.heatmap-grid {
  display: grid;
  grid-template-rows: repeat(7, 12px);
  grid-auto-flow: column;
  grid-auto-columns: 12px;
  gap: 3px;
}
.heatmap-cell {
  width: 12px;
  height: 12px;
  border-radius: 3px;
  transition: filter 0.15s;
  cursor: pointer;
}
.heatmap-cell:hover {
  filter: brightness(1.2);
}
.model-rank {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.rank-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 5px 0;
}
.rank-index {
  width: 20px;
  text-align: right;
  font-size: 11px;
  color: var(--color-muted-foreground);
  font-variant-numeric: tabular-nums;
}
.rank-model {
  width: 140px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 12px;
  color: var(--color-muted-foreground);
  font-family: "JetBrains Mono", monospace;
}
.rank-bar-track {
  flex: 1;
  height: 10px;
  border-radius: 5px;
  background-color: var(--color-accent);
  overflow: hidden;
}
.rank-bar {
  height: 100%;
  border-radius: 5px;
  background-color: var(--color-primary);
  transition: width 1s ease-out;
}
.rank-tokens {
  width: 110px;
  text-align: right;
  font-size: 11px;
  color: var(--color-muted-foreground);
  font-variant-numeric: tabular-nums;
}
.rank-requests {
  width: 80px;
  text-align: right;
  font-size: 11px;
  color: var(--color-muted-foreground);
  font-variant-numeric: tabular-nums;
}
.rank-more {
  padding-top: 4px;
  text-align: center;
  font-size: 11px;
  color: var(--color-muted-foreground);
}
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}
.toolbar-count {
  font-size: 12px;
  color: var(--color-muted-foreground);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.provider-card {
  margin-bottom: 12px;
}
.provider-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: 16px;
  background-color: var(--color-card);
  border: 1px solid var(--color-border);
  border-radius: 10px;
}
.provider-info {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  min-width: 0;
  flex: 1;
}
.provider-avatar {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background-color: color-mix(in srgb, var(--color-primary) 12%, transparent);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.provider-avatar-icon {
  color: var(--color-primary);
}
.provider-main {
  min-width: 0;
  flex: 1;
}
.provider-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.provider-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-foreground);
}
.provider-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 10px;
}
.provider-status.on {
  color: var(--color-success);
}
.provider-status.off {
  color: var(--color-muted-foreground);
}
.mini-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
}
.mini-dot.on {
  background-color: var(--color-success);
}
.mini-dot.off {
  background-color: var(--color-muted-foreground);
}
.provider-url {
  margin-top: 4px;
  font-size: 11px;
  color: var(--color-muted-foreground);
  font-family: "JetBrains Mono", monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.provider-models {
  margin-top: 8px;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.model-tag {
  font-size: 10px;
}
.model-more {
  font-size: 10px;
  color: var(--color-muted-foreground);
  align-self: center;
}
.provider-actions {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}
.empty-state {
  border-radius: 10px;
  padding: 24px;
  text-align: center;
}
.empty-state-icon {
  color: var(--color-muted-foreground);
}
.empty-state-text {
  margin-top: 8px;
  font-size: 14px;
  color: var(--color-muted-foreground);
}
.empty-state-hint {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.log-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
}
.log-muted {
  color: var(--color-muted-foreground);
}
.log-model {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  font-weight: 500;
  color: var(--color-foreground);
}
.log-stream {
  color: var(--color-muted-foreground);
  margin-left: 2px;
  vertical-align: middle;
}
.log-tokens {
  font-size: 12px;
  color: var(--color-muted-foreground);
  font-variant-numeric: tabular-nums;
}
.log-token-val {
  color: var(--color-foreground);
}
.log-token-sep {
  opacity: 0.3;
  margin: 0 2px;
}
.log-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
}
.log-status.ok {
  color: var(--color-success);
}
.log-status.bad {
  color: var(--color-danger);
}
.log-status.warn {
  color: var(--color-warning);
}
.status-mini-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: currentColor;
  display: inline-block;
}
.logs-empty-icon {
  color: var(--color-muted-foreground);
}
</style>