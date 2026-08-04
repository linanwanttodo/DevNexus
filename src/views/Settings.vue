<script setup>
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { t, initI18n, getLang } from "../lib/i18n.js";
import { setTheme } from "../lib/stores.js";
import { friendlyError } from "../lib/errors.js";

const router = useRouter();

const themePref = ref("dark");
const lang = ref(getLang().value);
const compactMode = ref(false);
const buildAlerts = ref(true);
const securityNotices = ref(true);
const proxyEnabled = ref(false);
const proxyAddress = ref("");
const proxyPort = ref("");
const appVersion = ref("");

const updateState = ref("idle");
const updateInfo = ref(null);
const updateError = ref("");
const changelogEn = ref("");
const changelogZh = ref("");
const downloadProgress = ref(0);

function isState(v) {
  return updateState.value === v;
}

function setThemePref(t) {
  themePref.value = t;
  setTheme(t);
}

async function setLang(l) {
  lang.value = l;
  await initI18n(l);
}

async function checkForUpdates() {
  updateState.value = "checking";
  updateError.value = "";
  updateInfo.value = null;
  changelogEn.value = "";
  changelogZh.value = "";
  try {
    const result = await invoke("check_for_updates_github");
    if (result.has_update) {
      updateInfo.value = result;
      try {
        const cl = await invoke("get_changelog", { version: null });
        if (cl) {
          changelogEn.value = cl.en;
          changelogZh.value = cl.zh;
        }
      } catch (err) {
        console.error("Failed to load changelog:", err);
      }
      updateState.value = "available";
    } else {
      updateState.value = "up_to_date";
    }
  } catch (err) {
    updateError.value = friendlyError(err);
    updateState.value = "error";
  }
}

async function downloadAndInstall() {
  updateState.value = "downloading";
  downloadProgress.value = 0;
  updateError.value = "";
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (update) {
      let totalBytes = 0;
      let downloadedBytes = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength)
          totalBytes = event.data.contentLength;
        else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          downloadProgress.value =
            totalBytes > 0 ? Math.round((downloadedBytes / totalBytes) * 100) : 0;
        } else if (event.event === "Finished") downloadProgress.value = 100;
      });
      updateState.value = "installed";
      return;
    }
  } catch (err) {
    console.error("Updater downloadAndInstall failed:", err);
  }

  try {
    const url = await invoke("get_download_url", {
      version: updateInfo.value?.latest_version || "",
    });
    const { open } = await import("@tauri-apps/plugin-shell");
    await open(url);
    updateState.value = "opened";
  } catch (fallbackErr) {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(
        updateInfo.value?.html_url ||
          "https://github.com/linanwanttodo/DevNexus/releases/latest"
      );
      updateState.value = "opened";
    } catch (e) {
      updateError.value = friendlyError(e);
      updateState.value = "error";
    }
  }
}

async function restartApp() {
  try {
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch (e) {
    updateError.value = friendlyError(e);
  }
}

onMounted(() => {
  const saved = localStorage.getItem("devnexus-theme") || "dark";
  themePref.value =
    saved === "light" || saved === "dark" ? saved : "system";
  setTheme(themePref.value);
  invoke("get_app_version")
    .then((v) => (appVersion.value = v))
    .catch(() => (appVersion.value = "1.1.1"));
});
</script>

<template>
  <div class="page settings-page">
    <!-- Header with back button -->
    <div class="breadcrumb">
      <a-button type="text" size="small" class="back-btn" @click="router.push('/dashboard')">
        <template #icon><icon-left /></template>
        {{ t("nav.dashboard") }}
      </a-button>
      <span class="crumb-sep">/</span>
      <span class="crumb-title">{{ t("settings.title") }}</span>
    </div>

    <!-- Content -->
    <div class="settings-content">
      <!-- Appearance -->
      <a-card :bordered="true" class="section-card" :title="t('settings.appearance')">
        <a-space direction="vertical" :size="16" fill>
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.theme") }}</div>
              <div class="setting-desc">{{ t("settings.theme_desc") }}</div>
            </div>
            <a-radio-group v-model="themePref" type="button" @change="(v) => setThemePref(v)">
              <a-radio value="light">{{ t("settings.light") }}</a-radio>
              <a-radio value="dark">{{ t("settings.dark") }}</a-radio>
              <a-radio value="system">{{ t("settings.system") }}</a-radio>
            </a-radio-group>
          </div>

          <a-divider class="setting-divider" />

          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.compact_mode") }}</div>
              <div class="setting-desc">{{ t("settings.compact_mode_desc") }}</div>
            </div>
            <a-switch v-model="compactMode" />
          </div>
        </a-space>
      </a-card>

      <!-- Language -->
      <a-card :bordered="true" class="section-card" :title="t('settings.language')">
        <div class="setting-row">
          <div>
            <div class="setting-label">{{ t("settings.language_desc") }}</div>
          </div>
          <a-select v-model="lang" style="width: 160px" @change="setLang">
            <a-option value="en">English</a-option>
            <a-option value="zh">中文</a-option>
            <a-option value="ru">Русский</a-option>
          </a-select>
        </div>
      </a-card>

      <!-- Notifications -->
      <a-card :bordered="true" class="section-card" :title="t('settings.notifications')">
        <a-space direction="vertical" :size="16" fill>
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.build_alerts") }}</div>
              <div class="setting-desc">{{ t("settings.build_alerts_desc") }}</div>
            </div>
            <a-switch v-model="buildAlerts" />
          </div>
          <a-divider class="setting-divider" />
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.security_notices") }}</div>
              <div class="setting-desc">{{ t("settings.security_notices_desc") }}</div>
            </div>
            <a-switch v-model="securityNotices" />
          </div>
        </a-space>
      </a-card>

      <!-- Network Proxy -->
      <a-card :bordered="true" class="section-card" :title="t('settings.network_proxy')">
        <a-space direction="vertical" :size="16" fill>
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.enable_proxy") }}</div>
              <div class="setting-desc">{{ t("settings.enable_proxy_desc") }}</div>
            </div>
            <a-switch v-model="proxyEnabled" />
          </div>

          <a-row v-if="proxyEnabled" :gutter="12">
            <a-col :span="16">
              <label class="input-label" for="proxy-address">{{ t("settings.proxy_address") }}</label>
              <a-input id="proxy-address" v-model="proxyAddress" placeholder="127.0.0.1" />
            </a-col>
            <a-col :span="8">
              <label class="input-label" for="proxy-port">{{ t("settings.port") }}</label>
              <a-input id="proxy-port" v-model="proxyPort" placeholder="7890" />
            </a-col>
          </a-row>
        </a-space>
      </a-card>

      <!-- Updates -->
      <a-card :bordered="true" class="section-card" :title="t('settings.updates')">
        <div class="setting-row">
          <div>
            <div class="setting-label">{{ t("settings.current_version") }}</div>
            <div class="version-mono">v{{ appVersion || "—" }}</div>
          </div>
          <a-button
            size="small"
            @click="checkForUpdates"
            :loading="isState('checking')"
            :disabled="isState('downloading')"
          >
            {{ t("settings.check_updates") }}
          </a-button>
        </div>

        <div class="update-status">
          <a-spin v-if="isState('checking')" :size="14" style="margin-right: 8px" />
          <a-result v-if="isState('up_to_date')" :status="'success'" class="inline-result">
            <template #title>{{ t("settings.up_to_date") }}</template>
          </a-result>

          <div v-else-if="isState('available')" class="update-card">
            <div class="update-head">
              <icon-sync class="update-icon" />
              <div class="update-info">
                <div class="update-title">
                  {{ t("settings.update_available") }} {{ updateInfo?.latest_version }}
                </div>
                <div v-if="changelogEn || changelogZh" class="changelog">
                  <template v-if="changelogEn">
                    <div class="changelog-lang">English</div>
                    <pre class="changelog-body">{{ changelogEn }}</pre>
                  </template>
                  <template v-if="changelogZh">
                    <div class="changelog-lang">中文</div>
                    <pre class="changelog-body">{{ changelogZh }}</pre>
                  </template>
                </div>
                <div v-else-if="updateInfo?.release_notes" class="changelog">
                  <pre class="changelog-body">{{ updateInfo.release_notes }}</pre>
                </div>
                <div v-if="updateInfo?.published_at" class="update-date">
                  {{ t("settings.released") }}: {{ new Date(updateInfo.published_at).toLocaleDateString() }}
                </div>
              </div>
            </div>
            <a-button type="primary" long @click="downloadAndInstall">
              <template #icon><icon-download /></template>
              {{ t("settings.download_update") }}
            </a-button>
          </div>

          <div v-else-if="isState('downloading')" class="download-block">
            <div class="download-row">
              <a-spin :size="14" />
              <span>{{ t("settings.downloading") }}...</span>
              <span v-if="downloadProgress > 0" class="download-pct">{{ downloadProgress }}%</span>
            </div>
            <a-progress :percent="downloadProgress" :show-text="false" />
          </div>

          <div v-else-if="isState('installed')" class="inline-success">
            <icon-check-circle-fill class="success-icon" />
            <span>{{ t("settings.update_installed") }}</span>
            <a-button type="text" size="small" @click="restartApp">
              {{ t("settings.restart_now") }}
            </a-button>
          </div>

          <div v-else-if="isState('error')" class="inline-error">
            <icon-close-circle-fill class="error-icon" />
            <span>{{ t("settings.update_error") }}: {{ updateError }}</span>
          </div>

          <div v-else-if="isState('opened')" class="inline-success">
            <icon-check-circle-fill class="success-icon" />
            <span>{{ t("settings.download_opened") }}</span>
          </div>

          <div v-else class="inline-idle">
            <icon-info-circle class="idle-icon" />
            <span>{{ t("settings.click_to_check") }}</span>
          </div>
        </div>
      </a-card>
    </div>
  </div>
</template>

<style scoped>
.breadcrumb {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 14px;
  margin-bottom: 16px;
  border-bottom: 1px solid var(--color-border);
}
.crumb-sep {
  font-size: 12px;
  color: var(--color-text-3);
}
.crumb-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
}
.settings-content {
  max-width: 640px;
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.setting-label {
  font-size: 14px;
  color: var(--color-text-1);
}
.setting-desc {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 2px;
}
.setting-divider {
  margin: 2px 0;
}
.input-label {
  display: block;
  font-size: 12px;
  color: var(--color-text-3);
  margin-bottom: 6px;
}
.version-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-2);
  margin-top: 2px;
}
.update-status {
  margin-top: 16px;
}
.update-card {
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-fill-1);
  padding: 12px;
}
.update-head {
  display: flex;
  gap: 10px;
  margin-bottom: 12px;
}
.update-icon {
  font-size: 16px;
  color: var(--color-primary-6);
  margin-top: 2px;
}
.update-info {
  flex: 1;
  min-width: 0;
}
.update-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
}
.changelog {
  margin-top: 6px;
}
.changelog-lang {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-top: 6px;
}
.changelog-body {
  max-height: 100px;
  overflow-y: auto;
  margin: 4px 0 0;
  white-space: pre-wrap;
  font-family: inherit;
  font-size: 12px;
  color: var(--color-text-2);
}
.update-date {
  margin-top: 6px;
  font-size: 12px;
  color: var(--color-text-3);
}
.download-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.download-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--color-text-2);
}
.download-pct {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-text-3);
}
.inline-success,
.inline-error,
.inline-idle {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.success-icon {
  color: rgb(var(--green-6));
}
.error-icon {
  color: rgb(var(--red-6));
}
.idle-icon {
  color: var(--color-text-3);
}
</style>
