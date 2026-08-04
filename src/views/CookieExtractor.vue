<script setup>
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import BrandIcons from "../icons/BrandIcons.vue";

const MAX_DISPLAY = 500;
const browsers = ref([]);
const selectedBrowser = ref(null);
const domainFilter = ref("");
const cookies = ref([]);
const extracting = ref(false);

async function loadBrowsers() {
  try {
    browsers.value = await invoke("get_supported_browsers");
    if (browsers.value.length > 0) {
      selectedBrowser.value = browsers.value[0].name;
    }
  } catch (err) {
    console.error("Failed to load browsers:", err);
    showToast(t("cookies.no_browsers"));
  }
}

async function extractCookies() {
  if (!selectedBrowser.value) {
    showToast(t("cookies.select_browser_first"));
    return;
  }
  const confirmed = await showConfirm(
    t("cookies.security_warning").replace("{browser}", selectedBrowser.value),
    t("cookies.security_warning_title")
  );
  if (!confirmed) return;

  extracting.value = true;
  try {
    const filter = domainFilter.value.trim() || null;
    cookies.value = await invoke("extract_cookies", {
      browserName: selectedBrowser.value,
      domainFilter: filter,
      maxResults: null,
    });
  } catch (err) {
    showToast(t("cookies.extract_failed").replace("{error}", friendlyError(err)));
    cookies.value = [];
  } finally {
    extracting.value = false;
  }
}

async function exportNetscape() {
  try {
    const filter = domainFilter.value.trim() || null;
    const content = await invoke("export_as_netscape", {
      browserName: selectedBrowser.value,
      domainFilter: filter,
    });
    downloadFile(content, `cookies_${selectedBrowser.value.toLowerCase()}.txt`, "text/plain");
    showToast(t("cookies.export_netscape_ok"));
  } catch (err) {
    showToast(t("cookies.export_failed").replace("{error}", friendlyError(err)));
  }
}

async function exportJSON() {
  try {
    const filter = domainFilter.value.trim() || null;
    const content = await invoke("export_as_json", {
      browserName: selectedBrowser.value,
      domainFilter: filter,
    });
    downloadFile(content, `cookies_${selectedBrowser.value.toLowerCase()}.json`, "application/json");
    showToast(t("cookies.export_json_ok"));
  } catch (err) {
    showToast(t("cookies.export_failed").replace("{error}", friendlyError(err)));
  }
}

function downloadFile(content, filename, mimeType) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function copyCookie(cookie) {
  const cookieString = `${cookie.name}=${cookie.value}`;
  navigator.clipboard
    .writeText(cookieString)
    .then(() => showToast(t("cookies.copy_single_ok")))
    .catch(() => showToast(t("common.copy_failed"), "error"));
}

function copyAllCookies() {
  const cookieStrings = cookies.value.map((c) => `${c.name}=${c.value}`).join("; ");
  navigator.clipboard
    .writeText(cookieStrings)
    .then(() => showToast(t("cookies.copy_all_ok")))
    .catch(() => showToast(t("common.copy_failed"), "error"));
}

function formatDate(timestamp) {
  if (timestamp === 0 || timestamp === null) return t("cookies.session");
  const date = new Date(timestamp * 1000);
  return date.toLocaleDateString();
}

const selectedBrowserCount = computed(() => {
  const b = browsers.value.find((b) => b.name === selectedBrowser.value);
  return b?.cookie_count || 0;
});

const columns = computed(() => [
  { title: t("cookies.name"), slotName: "name" },
  { title: t("cookies.value"), slotName: "value" },
  { title: t("cookies.domain"), slotName: "domain", width: 160 },
  { title: t("cookies.expires"), slotName: "expires", width: 120 },
  { title: t("cookies.actions"), slotName: "actions", align: "right", width: 70 },
]);

onMounted(() => {
  loadBrowsers();
});
</script>

<template>
  <div class="page cookie-page">
    <!-- Header -->
    <div class="page-header">
      <h1 class="page-title">{{ t("cookies.title") }}</h1>
      <p class="page-desc">{{ t("cookies.desc") }}</p>
    </div>

    <!-- Browser Selection -->
    <a-card :bordered="true" class="section-card mb-5">
      <div class="card-head-row">
        <h2 class="card-title">{{ t("cookies.select_browser") }}</h2>
        <span v-if="selectedBrowser" class="found-count">
          {{ selectedBrowserCount }} {{ t("cookies.found") }}
        </span>
      </div>

      <div class="browser-grid">
        <button
          v-for="browser in browsers"
          :key="browser.name"
          class="browser-card"
          :class="{ active: selectedBrowser === browser.name }"
          @click="selectedBrowser = browser.name"
        >
          <div class="browser-inner">
            <BrandIcons :name="browser.name.toLowerCase()" :size="28" class="browser-icon" />
            <div>
              <div class="browser-name">{{ browser.name }}</div>
              <div class="browser-count">{{ browser.cookie_count }} {{ t("cookies.cookies_label") }}</div>
            </div>
          </div>
        </button>
      </div>

      <a-alert v-if="browsers.length === 0" type="warning" class="mt-4">
        {{ t("cookies.no_browsers") }}
      </a-alert>
    </a-card>

    <!-- Filter and Extract -->
    <a-card :bordered="true" class="section-card mb-5">
      <div class="extract-row">
        <div class="filter-col">
          <label class="field-label">{{ t("cookies.domain_filter") }}</label>
          <a-input
            v-model="domainFilter"
            :placeholder="t('cookies.filter_placeholder')"
          />
        </div>
        <a-button
          type="primary"
          size="large"
          :loading="extracting"
          :disabled="!selectedBrowser"
          @click="extractCookies"
        >
          <template #icon><icon-download /></template>
          {{ extracting ? t("cookies.extracting") : t("cookies.extract") }}
        </a-button>
      </div>
    </a-card>

    <!-- Results -->
    <template v-if="cookies.length > 0">
      <a-alert
        v-if="cookies.length >= MAX_DISPLAY"
        type="warning"
        class="mb-4"
        :message="tFormat('cookies.display_limit', { count: MAX_DISPLAY })"
      />
      <a-card :bordered="true" class="section-card">
        <template #title>
          <div class="card-head-row">
            <span class="results-count">
              <span class="count-num">{{ cookies.length }}</span> {{ t("cookies.cookies_extracted") }}
            </span>
            <div class="toolbar-actions">
              <a-button size="small" @click="copyAllCookies">
                <template #icon><icon-copy /></template>
                {{ t("cookies.copy_all") }}
              </a-button>
              <a-button size="small" @click="exportJSON">
                <template #icon><icon-export /></template>
                {{ t("cookies.export_json") }}
              </a-button>
              <a-button size="small" @click="exportNetscape">
                <template #icon><icon-export /></template>
                {{ t("cookies.export_netscape") }}
              </a-button>
            </div>
          </div>
        </template>
        <a-table
          :data="cookies"
          :columns="columns"
          :pagination="false"
          :bordered="{ wrapper: false, cell: false }"
          size="small"
          :scroll="{ y: 384 }"
        >
          <template #name="{ record }">
            <span class="cookie-name" :title="record.name">{{ record.name }}</span>
          </template>
          <template #value="{ record }">
            <span class="cookie-value" :title="record.value">{{ record.value }}</span>
          </template>
          <template #domain="{ record }">
            <span class="cookie-domain">{{ record.domain }}</span>
          </template>
          <template #expires="{ record }">
            <span class="cookie-domain">{{ formatDate(record.expires) }}</span>
          </template>
          <template #actions="{ record }">
            <a-button type="text" size="mini" @click="copyCookie(record)" :title="t('cookies.copy_cookie')">
              <template #icon><icon-copy /></template>
            </a-button>
          </template>
        </a-table>
      </a-card>
    </template>

    <a-card
      v-else-if="!extracting && selectedBrowser"
      :bordered="true"
      class="empty-state"
    >
      <icon-cookie class="empty-icon" />
      <div class="empty-text">{{ t("cookies.no_cookies") }}</div>
      <div class="empty-hint">{{ t("cookies.extract_begin") }}</div>
      <div class="empty-warn">{{ t("cookies.extract_hint") }}</div>
    </a-card>
  </div>
</template>

<style scoped>
.section-card {
  border-radius: 10px;
}
.card-head-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}
.card-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
  margin: 0;
}
.found-count {
  font-size: 12px;
  color: var(--color-text-3);
}
.browser-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
  margin-top: 16px;
}
.browser-card {
  border: 1px solid var(--color-border-2);
  border-radius: 10px;
  padding: 12px;
  text-align: left;
  cursor: pointer;
  background-color: var(--color-fill-1);
  transition: all 0.15s;
}
.browser-card:hover {
  border-color: rgb(var(--primary-6), 0.6);
}
.browser-card.active {
  border-color: rgb(var(--primary-6));
  background-color: rgb(var(--primary-6), 0.08);
}
.browser-inner {
  display: flex;
  align-items: center;
  gap: 12px;
}
.browser-icon {
  color: var(--color-text-2);
}
.browser-card.active .browser-icon {
  color: rgb(var(--primary-6));
}
.browser-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
}
.browser-count {
  font-size: 12px;
  color: var(--color-text-3);
}
.extract-row {
  display: flex;
  align-items: flex-end;
  gap: 12px;
}
.filter-col {
  flex: 1;
}
.field-label {
  display: block;
  margin-bottom: 6px;
  font-size: 12px;
  color: var(--color-text-3);
}
.results-count {
  font-size: 14px;
  color: var(--color-text-1);
}
.count-num {
  font-weight: 600;
}
.toolbar-actions {
  display: flex;
  gap: 8px;
}
.cookie-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-1);
  display: block;
  max-width: 200px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cookie-value {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-2);
  display: block;
  max-width: 300px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cookie-domain {
  font-size: 12px;
  color: var(--color-text-3);
}
.empty-state {
  border-radius: 10px;
  padding: 32px;
  text-align: center;
}
.empty-icon {
  font-size: 36px;
  color: var(--color-text-4);
}
.empty-text {
  margin-top: 12px;
  font-size: 14px;
  color: var(--color-text-3);
}
.empty-hint {
  margin-top: 4px;
  font-size: 12px;
  color: var(--color-text-4);
}
.empty-warn {
  margin-top: 12px;
  font-size: 12px;
  color: rgb(var(--orange-6));
}
</style>
