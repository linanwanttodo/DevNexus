<script setup>
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { t, initI18n, getLang } from "../lib/i18n.js";
import { setTheme } from "../lib/stores.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { Progress } from "@/components/ui/progress";
import { Spinner } from "@/components/ui/spinner";

const router = useRouter();

const themePref = ref("dark");
const lang = ref(getLang().value);
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

const themeOptions = [
  { value: "light", labelKey: "settings.light" },
  { value: "dark", labelKey: "settings.dark" },
  { value: "system", labelKey: "settings.system" },
];

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
      <Button variant="ghost" size="sm" class="back-btn" @click="router.push('/dashboard')">
        <AppIcon name="left" class="size-4" />
        {{ t("nav.dashboard") }}
      </Button>
      <span class="crumb-sep">/</span>
      <span class="crumb-title">{{ t("settings.title") }}</span>
    </div>

    <!-- Content -->
    <div class="settings-content space-y-3">
      <!-- Appearance -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ t("settings.appearance") }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.theme") }}</div>
              <div class="setting-desc">{{ t("settings.theme_desc") }}</div>
            </div>
            <RadioGroup
              v-model="themePref"
              @update:model-value="(v) => setThemePref(v)"
              class="flex flex-row gap-4"
            >
              <div v-for="opt in themeOptions" :key="opt.value" class="flex items-center gap-2">
                <RadioGroupItem :id="`theme-${opt.value}`" :value="opt.value" />
                <Label :for="`theme-${opt.value}`" class="cursor-pointer text-sm">
                  {{ t(opt.labelKey) }}
                </Label>
              </div>
            </RadioGroup>
          </div>
        </CardContent>
      </Card>

      <!-- Language -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ t("settings.language") }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.language_desc") }}</div>
            </div>
            <Select v-model="lang" @update:model-value="setLang">
              <SelectTrigger class="w-[160px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="en">English</SelectItem>
                <SelectItem value="zh">中文</SelectItem>
                <SelectItem value="ru">Русский</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>

      <!-- Notifications -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ t("settings.notifications") }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.build_alerts") }}</div>
              <div class="setting-desc">{{ t("settings.build_alerts_desc") }}</div>
            </div>
            <Switch v-model="buildAlerts" />
          </div>
          <Separator />
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.security_notices") }}</div>
              <div class="setting-desc">{{ t("settings.security_notices_desc") }}</div>
            </div>
            <Switch v-model="securityNotices" />
          </div>
        </CardContent>
      </Card>

      <!-- Network Proxy -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ t("settings.network_proxy") }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.enable_proxy") }}</div>
              <div class="setting-desc">{{ t("settings.enable_proxy_desc") }}</div>
            </div>
            <Switch v-model="proxyEnabled" />
          </div>

          <div v-if="proxyEnabled" class="grid grid-cols-12 gap-3">
            <div class="col-span-12 sm:col-span-8">
              <label class="input-label" for="proxy-address">{{ t("settings.proxy_address") }}</label>
              <Input id="proxy-address" v-model="proxyAddress" placeholder="127.0.0.1" />
            </div>
            <div class="col-span-12 sm:col-span-4">
              <label class="input-label" for="proxy-port">{{ t("settings.port") }}</label>
              <Input id="proxy-port" v-model="proxyPort" placeholder="7890" />
            </div>
          </div>
        </CardContent>
      </Card>

      <!-- Updates -->
      <Card class="section-card shadow-sm">
        <CardHeader>
          <CardTitle class="text-base font-medium">{{ t("settings.updates") }}</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div class="setting-row">
            <div>
              <div class="setting-label">{{ t("settings.current_version") }}</div>
              <div class="version-mono">v{{ appVersion || "—" }}</div>
            </div>
            <Button
              size="sm"
              :disabled="isState('checking') || isState('downloading')"
              @click="checkForUpdates"
            >
              <Spinner v-if="isState('checking')" class="size-3.5" />
              {{ t("settings.check_updates") }}
            </Button>
          </div>

          <div class="update-status">
            <div v-if="isState('up_to_date')" class="inline-success">
              <AppIcon name="check-circle-fill" class="size-4 text-success" />
              <span>{{ t("settings.up_to_date") }}</span>
            </div>

            <div v-else-if="isState('available')" class="update-card">
              <div class="update-head">
                <AppIcon name="sync" class="update-icon size-4" />
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
              <Button class="w-full" @click="downloadAndInstall">
                <AppIcon name="download" class="size-4" />
                {{ t("settings.download_update") }}
              </Button>
            </div>

            <div v-else-if="isState('downloading')" class="download-block">
              <div class="download-row">
                <Spinner class="size-3.5" />
                <span>{{ t("settings.downloading") }}...</span>
                <span v-if="downloadProgress > 0" class="download-pct">{{ downloadProgress }}%</span>
              </div>
              <Progress :model-value="downloadProgress" class="h-2" />
            </div>

            <div v-else-if="isState('installed')" class="inline-success">
              <AppIcon name="check-circle-fill" class="size-4 text-success" />
              <span>{{ t("settings.update_installed") }}</span>
              <Button variant="ghost" size="sm" @click="restartApp">
                {{ t("settings.restart_now") }}
              </Button>
            </div>

            <div v-else-if="isState('error')" class="inline-error">
              <AppIcon name="close-circle-fill" class="size-4 text-danger" />
              <span>{{ t("settings.update_error") }}: {{ updateError }}</span>
            </div>

            <div v-else-if="isState('opened')" class="inline-success">
              <AppIcon name="check-circle-fill" class="size-4 text-success" />
              <span>{{ t("settings.download_opened") }}</span>
            </div>

            <div v-else class="inline-idle">
              <AppIcon name="info-circle" class="idle-icon size-4" />
              <span>{{ t("settings.click_to_check") }}</span>
            </div>
          </div>
        </CardContent>
      </Card>
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
  color: var(--color-muted-foreground);
}
.crumb-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-foreground);
}
.settings-content {
  max-width: 100%;
}
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.setting-label {
  font-size: 14px;
  color: var(--color-foreground);
}
.setting-desc {
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-top: 2px;
}
.input-label {
  display: block;
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-bottom: 6px;
}
.version-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-top: 2px;
}
.update-status {
  margin-top: 16px;
}
.update-card {
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-muted);
  padding: 12px;
}
.update-head {
  display: flex;
  gap: 10px;
  margin-bottom: 12px;
}
.update-icon {
  color: var(--color-primary);
  margin-top: 2px;
}
.update-info {
  flex: 1;
  min-width: 0;
}
.update-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-foreground);
}
.changelog {
  margin-top: 6px;
}
.changelog-lang {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-foreground);
  margin-top: 6px;
}
.changelog-body {
  max-height: 100px;
  overflow-y: auto;
  margin: 4px 0 0;
  white-space: pre-wrap;
  font-family: inherit;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.update-date {
  margin-top: 6px;
  font-size: 12px;
  color: var(--color-muted-foreground);
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
  color: var(--color-muted-foreground);
}
.download-pct {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-muted-foreground);
}
.inline-success,
.inline-error,
.inline-idle {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.idle-icon {
  color: var(--color-muted-foreground);
}
</style>