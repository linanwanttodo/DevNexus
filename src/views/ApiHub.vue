<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat, getLang } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import ProviderForm from "../components/hub/ProviderForm.vue";

const activeTab = ref("stats");
const providers = ref([]);
const logs = ref([]);
const stats = ref(null);
const status = ref({ running: false, port: 3456, auth_token: "" });
const loading = ref(false);
const error = ref(null);
const showForm = ref(false);
const editingId = ref(null);

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
]);

let pollTimer = null;

onMounted(() => {
  loadData();
  pollTimer = setInterval(loadStats, 15000);
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
      invoke("api_hub_get_logs", { limit: 100, offset: 0 }),
      invoke("api_hub_get_usage_stats"),
      invoke("api_hub_status"),
    ]);
    providers.value = p;
    logs.value = l;
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
    const [s, l] = await Promise.all([
      invoke("api_hub_get_usage_stats"),
      invoke("api_hub_get_logs", { limit: 100, offset: 0 }),
    ]);
    stats.value = s;
    logs.value = l;
  } catch {}
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
function fmtDate(ts) {
  return ts ? new Date(ts * 1000).toLocaleDateString(localeTag()) : "-";
}
function fmtLatency(ms) {
  return !ms ? "-" : ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
}
function statusColor(c) {
  return c >= 200 && c < 300 ? "ok" : c >= 400 ? "bad" : "warn";
}

function getChartHours() {
  return stats.value?.by_hour
    ? Object.entries(stats.value.by_hour).sort((a, b) => Number(a[0]) - Number(b[0]))
    : [];
}
function getModelEntries() {
  return stats.value?.by_model
    ? Object.entries(stats.value.by_model).sort(
        (a, b) => Number(b[1]?.requests) - Number(a[1]?.requests)
      )
    : [];
}
function heatmapColor(requests, max) {
  const pct = Math.min(requests / Math.max(max, 1), 1);
  const alpha = (0.12 + pct * 0.88).toFixed(3);
  return `rgb(var(--primary-6) / ${alpha})`;
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
      ]
    : []
);

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
    await navigator.clipboard.writeText(status.value.auth_token || "");
    showToast(t("apiHub.gateway.copied"));
  } catch {
    showToast(t("apiHub.gateway.copyFailed"), "error");
  }
}

const tabs = computed(() => [
  { id: "stats", label: t("apiHub.tabs.stats") },
  { id: "providers", label: t("apiHub.tabs.providers") },
  { id: "logs", label: t("apiHub.tabs.logs") },
]);

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

const hourMax = computed(() => {
  const hours = getChartHours();
  return hours.length ? Math.max(...hours.map((h) => h[1].requests), 1) : 1;
});
const modelMax = computed(() => {
  const models = getModelEntries();
  return models.length ? models[0][1].requests : 1;
});
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

    <!-- ════ 聚合网关 (Gateway) ════ -->
    <a-card :bordered="true" class="gateway-card">
      <div class="gateway-head">
        <div class="gateway-title-row">
          <span class="status-dot" :class="status?.running ? 'on' : 'off'"></span>
          <h2 class="gateway-title">{{ t("apiHub.gateway.title") }}</h2>
          <a-tag color="arcoblue" size="small">localhost:{{ status?.port }}</a-tag>
          <a-tag v-if="status?.running && status?.auth_token" color="green" size="small">
            Auth Token 已启用
          </a-tag>
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
          <icon-copy class="endpoint-icon" />
          <span class="endpoint-text">{{ ep }}</span>
        </button>
      </div>

      <!-- Auth token display -->
      <div v-if="status?.running && status?.auth_token" class="token-row">
        <div class="token-info">
          <icon-lock class="token-icon" />
          <span class="token-label">X-DevNexus-Token</span>
          <code class="token-value">{{ status.auth_token }}</code>
        </div>
        <a-button size="mini" @click="copyToken">
          <template #icon><icon-copy /></template>
          {{ t("apiHub.gateway.copyTooltip") }}
        </a-button>
      </div>
    </a-card>

    <!-- ════ Tabs ════ -->
    <a-tabs v-model:active-key="activeTab" class="apihub-tabs">
      <a-tab-pane
        v-for="tab in tabs"
        :key="tab.id"
        :title="tab.label"
      >
        <!-- ════ STATS ════ -->
        <template v-if="tab.id === 'stats'">
          <a-alert v-if="error" type="error" class="mb-4" :message="error" />
          <div v-if="loading && !providers.length" class="loading-block">
            <a-skeleton :animation="true">
              <a-skeleton-line :rows="6" />
            </a-skeleton>
          </div>
          <template v-else-if="stats">
            <!-- Metric cards -->
            <a-row :gutter="16" class="mb-5">
              <a-col v-for="card in metricCards" :key="card.label" :span="6">
                <a-card :bordered="true" class="metric-card">
                  <div class="metric-head">
                    <icon-fire v-if="card.icon === 'fire'" class="metric-icon" />
                    <icon-chat-line v-else-if="card.icon === 'chat'" class="metric-icon" />
                    <icon-check-circle v-else-if="card.icon === 'check'" class="metric-icon" />
                    <icon-clock-circle v-else class="metric-icon" />
                    <span class="metric-label">{{ card.label }}</span>
                  </div>
                  <div class="metric-value">{{ card.value }}</div>
                </a-card>
              </a-col>
            </a-row>

            <!-- Heatmap -->
            <a-card :bordered="true" class="mb-4">
              <template #title>
                <div class="card-head-row">
                  <span>{{ t("apiHub.heatmap.title") }}</span>
                  <div class="legend">
                    <span>{{ t("apiHub.heatmap.less") }}</span>
                    <span
                      v-for="hv in [0.15, 0.35, 0.55, 0.75, 0.95]"
                      :key="hv"
                      class="legend-cell"
                      :style="{ background: heatmapColor(hv, 1) }"
                    ></span>
                    <span>{{ t("apiHub.heatmap.more") }}</span>
                  </div>
                </div>
              </template>
              <div v-if="getChartHours().length > 0" class="heatmap-grid">
                <div
                  v-for="[ts, hd] in getChartHours()"
                  :key="ts"
                  class="heatmap-cell"
                  :style="{ background: heatmapColor(hd.requests, hourMax) }"
                  :title="t('apiHub.heatmap.requestTitle').replace('{date}', fmtDate(Number(ts))).replace('{count}', hd.requests)"
                ></div>
              </div>
              <a-empty v-else :description="t('apiHub.empty.noData')" />
            </a-card>

            <!-- Model Usage -->
            <a-card :bordered="true">
              <template #title>{{ t("apiHub.models.usageRanking") }}</template>
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
              <a-empty v-else :description="t('apiHub.empty.noData')" />
            </a-card>
          </template>
          <a-card v-else :bordered="true" class="empty-state">
            <icon-bar-chart class="empty-state-icon" />
            <div class="empty-state-text">{{ t("apiHub.empty.waiting") }}</div>
          </a-card>
        </template>

        <!-- ════ PROVIDERS ════ -->
        <template v-else-if="tab.id === 'providers'">
          <div class="toolbar">
            <span class="toolbar-count">
              {{ tFormat("apiHub.providerCount", { count: providers.length }) }}
            </span>
            <a-button v-if="!showForm" type="primary" size="small" @click="beginAdd">
              <template #icon><icon-plus /></template>
              {{ t("apiHub.addProvider") }}
            </a-button>
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
                  <icon-relation class="provider-avatar-icon" />
                </div>
                <div class="provider-main">
                  <div class="provider-title-row">
                    <span class="provider-name">{{ p.name }}</span>
                    <a-tag color="arcoblue" size="small">{{ protocolName(p.protocol) }}</a-tag>
                    <span class="provider-status" :class="p.enabled ? 'on' : 'off'">
                      <span class="mini-dot" :class="p.enabled ? 'on' : 'off'"></span>
                      {{ p.enabled ? t("apiHub.status.active") : t("apiHub.status.disabled") }}
                    </span>
                  </div>
                  <div class="provider-url">{{ p.base_url }}</div>
                  <div class="provider-models">
                    <a-tag v-for="m in p.models.slice(0, 8)" :key="m" size="mini" class="model-tag">
                      {{ getAlias(p, m) }}
                    </a-tag>
                    <span v-if="p.models.length > 8" class="model-more">+{{ p.models.length - 8 }}</span>
                  </div>
                </div>
              </div>
              <div class="provider-actions">
                <a-button type="text" size="small" @click="beginEdit(p)" :title="t('apiHub.edit')">
                  <template #icon><icon-edit /></template>
                </a-button>
                <a-button type="text" size="small" status="danger" @click="deleteProvider(p.id)" :title="t('apiHub.delete')">
                  <template #icon><icon-delete /></template>
                </a-button>
              </div>
            </div>
          </div>

          <!-- Empty state -->
          <a-card v-if="providers.length === 0 && !showForm" :bordered="true" class="empty-state">
            <icon-relation class="empty-state-icon" />
            <div class="empty-state-text">{{ t("apiHub.empty.noProviders") }}</div>
            <p class="empty-state-hint">{{ t("apiHub.empty.addHint") }}</p>
            <a-button type="primary" size="small" class="mt-3" @click="beginAdd">
              <template #icon><icon-plus /></template>
              {{ t("apiHub.addFirstProvider") }}
            </a-button>
          </a-card>
        </template>

        <!-- ════ LOGS ════ -->
        <template v-else>
          <a-card :bordered="true">
            <a-table
              :data="logs"
              :columns="logColumns"
              :pagination="false"
              :bordered="{ wrapper: false, cell: false }"
              size="small"
              :scroll="{ y: 500 }"
            >
              <template #time="{ record }">
                <span class="log-mono log-muted">{{ fmtTime(record.timestamp) }}</span>
              </template>
              <template #model="{ record }">
                <span class="log-model">
                  {{ record.model }}
                  <icon-water-drop
                    v-if="record.is_streaming"
                    class="log-stream"
                    :title="t('apiHub.logs.streaming')"
                  />
                </span>
              </template>
              <template #provider="{ record }">
                <span class="log-muted">{{ record.provider_name }}</span>
              </template>
              <template #tokens="{ record }">
                <span class="log-tokens">
                  <span class="log-token-val">↑{{ fmtTokens(record.input_tokens) }}</span>
                  <span class="log-token-sep">/</span>
                  <span class="log-token-val">↓{{ fmtTokens(record.output_tokens) }}</span>
                </span>
              </template>
              <template #latency="{ record }">
                <span class="log-muted">{{ fmtLatency(record.latency_ms) }}</span>
              </template>
              <template #status="{ record }">
                <span
                  class="log-status"
                  :class="statusColor(record.status_code)"
                  :title="record.error_message || ''"
                >
                  <span class="status-mini-dot"></span>
                  {{ record.status_code || "—" }}
                </span>
              </template>
              <template #empty>
                <div class="logs-empty">
                  <icon-file class="logs-empty-icon" />
                  <div class="logs-empty-text">{{ t("apiHub.logs.empty") }}</div>
                </div>
              </template>
            </a-table>
          </a-card>
        </template>
      </a-tab-pane>
    </a-tabs>
  </div>
</template>

<style scoped>
.gateway-card {
  border-radius: 10px;
  margin-bottom: 24px;
}
.gateway-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.gateway-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
}
.status-dot.on {
  background-color: rgb(var(--green-6));
}
.status-dot.off {
  background-color: var(--color-text-4);
}
.gateway-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
  margin: 0;
}
.gateway-desc {
  font-size: 11px;
  color: var(--color-text-4);
  margin: 0;
}
.endpoint-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin-top: 12px;
}
.endpoint-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid var(--color-border-2);
  border-radius: 8px;
  background-color: var(--color-fill-1);
  cursor: pointer;
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-text-2);
  text-align: left;
  transition: all 0.15s;
  overflow: hidden;
}
.endpoint-btn:hover {
  border-color: rgb(var(--primary-6));
  color: var(--color-text-1);
}
.endpoint-icon {
  font-size: 14px;
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
  background-color: var(--color-fill-1);
  border: 1px dashed var(--color-border-3);
  border-radius: 8px;
}
.token-info {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.token-icon {
  color: rgb(var(--green-6));
  flex-shrink: 0;
}
.token-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--color-text-2);
  flex-shrink: 0;
}
.token-value {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-text-2);
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
  font-size: 14px;
  opacity: 0.6;
  color: var(--color-text-3);
}
.metric-label {
  font-size: 12px;
  color: var(--color-text-3);
}
.metric-value {
  font-size: 20px;
  font-weight: 600;
  color: var(--color-text-1);
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
  color: var(--color-text-4);
}
.legend-cell {
  width: 12px;
  height: 12px;
  border-radius: 3px;
}
.heatmap-grid {
  display: grid;
  grid-template-columns: repeat(12, 1fr);
  gap: 6px;
}
.heatmap-cell {
  height: 16px;
  border-radius: 3px;
  transition: filter 0.15s;
  cursor: pointer;
}
.heatmap-cell:hover {
  filter: brightness(1.15);
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
  color: var(--color-text-4);
  font-variant-numeric: tabular-nums;
}
.rank-model {
  width: 140px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 12px;
  color: var(--color-text-2);
  font-family: "JetBrains Mono", monospace;
}
.rank-bar-track {
  flex: 1;
  height: 10px;
  border-radius: 5px;
  background-color: var(--color-fill-2);
  overflow: hidden;
}
.rank-bar {
  height: 100%;
  border-radius: 5px;
  background-color: rgb(var(--primary-6));
  transition: width 1s ease-out;
}
.rank-tokens {
  width: 110px;
  text-align: right;
  font-size: 11px;
  color: var(--color-text-3);
  font-variant-numeric: tabular-nums;
}
.rank-requests {
  width: 80px;
  text-align: right;
  font-size: 11px;
  color: var(--color-text-3);
  font-variant-numeric: tabular-nums;
}
.rank-more {
  padding-top: 4px;
  text-align: center;
  font-size: 11px;
  color: var(--color-text-4);
}
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
}
.toolbar-count {
  font-size: 12px;
  color: var(--color-text-3);
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
  background-color: var(--color-bg-2);
  border: 1px solid var(--color-border-2);
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
  background-color: rgb(var(--primary-6), 0.12);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.provider-avatar-icon {
  color: rgb(var(--primary-6));
  font-size: 18px;
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
  color: var(--color-text-1);
}
.provider-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 10px;
}
.provider-status.on {
  color: rgb(var(--green-6));
}
.provider-status.off {
  color: var(--color-text-4);
}
.mini-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  display: inline-block;
}
.mini-dot.on {
  background-color: rgb(var(--green-6));
}
.mini-dot.off {
  background-color: var(--color-text-4);
}
.provider-url {
  margin-top: 4px;
  font-size: 11px;
  color: var(--color-text-4);
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
  color: var(--color-text-4);
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
  font-size: 28px;
  color: var(--color-text-4);
}
.empty-state-text {
  margin-top: 8px;
  font-size: 14px;
  color: var(--color-text-3);
}
.empty-state-hint {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--color-text-4);
}
.log-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
}
.log-muted {
  color: var(--color-text-3);
}
.log-model {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-1);
}
.log-stream {
  font-size: 12px;
  color: var(--color-text-4);
  margin-left: 2px;
  vertical-align: middle;
}
.log-tokens {
  font-size: 12px;
  color: var(--color-text-3);
  font-variant-numeric: tabular-nums;
}
.log-token-val {
  color: var(--color-text-2);
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
  color: rgb(var(--green-6));
}
.log-status.bad {
  color: rgb(var(--red-6));
}
.log-status.warn {
  color: rgb(var(--orange-6));
}
.status-mini-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background-color: currentColor;
  display: inline-block;
}
.logs-empty {
  padding: 40px 0;
  text-align: center;
}
.logs-empty-icon {
  font-size: 22px;
  color: var(--color-text-4);
}
.logs-empty-text {
  font-size: 12px;
  color: var(--color-text-4);
  margin-top: 4px;
}
</style>
