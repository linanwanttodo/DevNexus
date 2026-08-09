<script setup>
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import BrandIcons from "../icons/BrandIcons.vue";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";
import {
  Alert,
  AlertTitle,
} from "@/components/ui/alert";
import { Spinner } from "@/components/ui/spinner";

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
    <Card class="section-card mb-3">
      <CardHeader class="flex-row items-center justify-between space-y-0">
        <CardTitle class="text-sm font-medium">
          {{ t("cookies.select_browser") }}
        </CardTitle>
        <span v-if="selectedBrowser" class="found-count">
          {{ selectedBrowserCount }} {{ t("cookies.found") }}
        </span>
      </CardHeader>
      <CardContent>
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

        <Alert v-if="browsers.length === 0" class="mt-4">
          <AlertTitle>{{ t("cookies.no_browsers") }}</AlertTitle>
        </Alert>
      </CardContent>
    </Card>

    <!-- Filter and Extract -->
    <Card class="section-card mb-3">
      <CardContent class="extract-row pt-6">
        <div class="filter-col">
          <label class="field-label">{{ t("cookies.domain_filter") }}</label>
          <Input
            v-model="domainFilter"
            :placeholder="t('cookies.filter_placeholder')"
          />
        </div>
        <Button
          size="lg"
          :disabled="!selectedBrowser || extracting"
          @click="extractCookies"
        >
          <Spinner v-if="extracting" />
          <AppIcon v-else name="download" />
          {{ extracting ? t("cookies.extracting") : t("cookies.extract") }}
        </Button>
      </CardContent>
    </Card>

    <!-- Results -->
    <template v-if="cookies.length > 0">
      <Alert
        v-if="cookies.length >= MAX_DISPLAY"
        class="mb-4"
      >
        <AlertTitle>
          {{ tFormat("cookies.display_limit", { count: MAX_DISPLAY }) }}
        </AlertTitle>
      </Alert>
      <Card class="section-card">
        <CardHeader class="flex-row items-center justify-between space-y-0">
          <CardTitle class="text-sm font-medium results-count">
            <span class="count-num">{{ cookies.length }}</span> {{ t("cookies.cookies_extracted") }}
          </CardTitle>
          <div class="toolbar-actions">
            <Button variant="outline" size="sm" @click="copyAllCookies">
              <AppIcon name="copy" />
              {{ t("cookies.copy_all") }}
            </Button>
            <Button variant="outline" size="sm" @click="exportJSON">
              <AppIcon name="export" />
              {{ t("cookies.export_json") }}
            </Button>
            <Button variant="outline" size="sm" @click="exportNetscape">
              <AppIcon name="export" />
              {{ t("cookies.export_netscape") }}
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          <Table class="max-h-[384px] overflow-y-auto">
            <TableHeader>
              <TableRow>
                <TableHead class="w-[200px]">{{ t("cookies.name") }}</TableHead>
                <TableHead>{{ t("cookies.value") }}</TableHead>
                <TableHead class="w-[160px]">{{ t("cookies.domain") }}</TableHead>
                <TableHead class="w-[120px]">{{ t("cookies.expires") }}</TableHead>
                <TableHead class="w-[70px] text-right">{{ t("cookies.actions") }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="record in cookies" :key="record.name + record.domain">
                <TableCell class="font-medium">
                  <span class="cookie-name" :title="record.name">{{ record.name }}</span>
                </TableCell>
                <TableCell>
                  <span class="cookie-value" :title="record.value">{{ record.value }}</span>
                </TableCell>
                <TableCell>
                  <span class="cookie-domain">{{ record.domain }}</span>
                </TableCell>
                <TableCell>
                  <span class="cookie-domain">{{ formatDate(record.expires) }}</span>
                </TableCell>
                <TableCell class="text-right">
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    :title="t('cookies.copy_cookie')"
                    @click="copyCookie(record)"
                  >
                    <AppIcon name="copy" />
                  </Button>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </template>

    <Card
      v-else-if="!extracting && selectedBrowser"
      class="empty-state"
    >
      <CardContent class="empty-state-inner">
        <Empty>
          <EmptyMedia>
            <AppIcon name="cookie" class="size-9 text-muted-foreground/60" />
          </EmptyMedia>
          <EmptyContent>
            <EmptyDescription class="empty-text">{{ t("cookies.no_cookies") }}</EmptyDescription>
            <p class="empty-hint">{{ t("cookies.extract_begin") }}</p>
            <p class="empty-warn">{{ t("cookies.extract_hint") }}</p>
          </EmptyContent>
        </Empty>
      </CardContent>
    </Card>
  </div>
</template>

<style scoped>
.section-card {
  border-radius: 10px;
}
.found-count {
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.browser-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
  margin-top: 16px;
}
.browser-card {
  border: 1px solid var(--color-border);
  border-radius: 10px;
  padding: 12px;
  text-align: left;
  cursor: pointer;
  background-color: var(--color-muted);
  transition: all 0.15s;
}
.browser-card:hover {
  border-color: color-mix(in srgb, var(--color-primary) 60%, transparent);
}
.browser-card.active {
  border-color: var(--color-primary);
  background-color: color-mix(in srgb, var(--color-primary) 8%, transparent);
}
.browser-inner {
  display: flex;
  align-items: center;
  gap: 12px;
}
.browser-icon {
  color: var(--color-muted-foreground);
}
.browser-card.active .browser-icon {
  color: var(--color-primary);
}
.browser-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-foreground);
}
.browser-count {
  font-size: 12px;
  color: var(--color-muted-foreground);
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
  color: var(--color-muted-foreground);
}
.results-count {
  font-size: 14px;
  color: var(--color-foreground);
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
  color: var(--color-foreground);
  display: block;
  max-width: 200px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cookie-value {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
  display: block;
  max-width: 300px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cookie-domain {
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.empty-state {
  border-radius: 10px;
}
.empty-state-inner {
  padding: 32px;
}
.empty-text {
  font-size: 14px;
}
.empty-hint {
  margin-top: 4px;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.empty-warn {
  margin-top: 12px;
  font-size: 12px;
  color: rgb(249 115 22);
}
</style>