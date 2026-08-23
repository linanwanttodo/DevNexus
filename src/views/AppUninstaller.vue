<script setup>
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Alert,
  AlertDescription,
} from "@/components/ui/alert";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyMedia,
} from "@/components/ui/empty";

const apps = ref([]);
const loading = ref(true);
const error = ref(null);
const search = ref("");
const sourceFilter = ref("all");
const uninstalling = ref(null);
const scanning = ref(null);
const residueScans = ref({});
const selectedResidues = ref({});
const cleaningResidues = ref(null);
const scanErrors = ref({});

/** 应用图标加载失败时隐藏占位（e.target 为 EventTarget，需收窄为元素） */
function onIconError(e) {
  const el = /** @type {HTMLImageElement} */ (e.currentTarget);
  if (el) el.style.display = "none";
}

async function loadApps() {
  try {
    loading.value = true;
    error.value = null;
    apps.value = await invoke("list_installed_apps");
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
  }
}

async function handleUninstall(app) {
  if (
    !(await showConfirm(
      tFormat("uninstall_mgr.confirm", { name: app.name }) || `Uninstall ${app.name}?`
    ))
  )
    return;

  uninstalling.value = app.name;
  try {
    // 优先带 source 定向到对应包管理器（如 flatpak/snap），避免全量轮询误配
    let result;
    try {
      result = await invoke("uninstall_software_deep_with_source", {
        packageName: app.name,
        appName: app.name,
        source: app.source,
      });
    } catch {
      // 兼容旧后端：回退到定向卸载或通用深度卸载
      try {
        result = await invoke("uninstall_installed_app", {
          packageName: app.name,
          source: app.source,
        });
      } catch {
        result = await invoke("uninstall_software_deep", {
          packageName: app.name,
          appName: app.name,
        });
      }
    }
    showToast(result);
    await scanResidues(app, true);
    await loadApps();
  } catch (err) {
    showToast(tFormat("uninstall_mgr.failed", { error: friendlyError(err) }));
    await scanResidues(app, true);
  } finally {
    uninstalling.value = null;
  }
}

async function handleForceUninstall(app) {
  if (
    !(await showConfirm(
      tFormat("uninstall_mgr.force_confirm", { name: app.name }) ||
        `Force uninstall ${app.name}? This will kill all related processes and remove ALL residue files.`
    ))
  )
    return;

  uninstalling.value = app.name;
  try {
    const result = await invoke("force_uninstall_software", {
      packageName: app.name,
      appName: app.name,
    });
    showToast(result);
    const name = String(app.name);
    delete residueScans.value[name];
    delete selectedResidues.value[name];
    await loadApps();
  } catch (err) {
    showToast(tFormat("uninstall_mgr.force_failed", { error: friendlyError(err) }));
  } finally {
    uninstalling.value = null;
  }
}

async function scanResidues(app, auto = false) {
  scanning.value = app.name;
  scanErrors.value[app.name] = null;
  try {
    const scan = await invoke("scan_app_residues", {
      appName: app.name,
      packageName: app.name,
    });
    residueScans.value[app.name] = scan;
    // 默认不自动勾选任何残留项。用户需手动勾选后再清理
    selectedResidues.value[app.name] = {};
  } catch (err) {
    scanErrors.value[app.name] = friendlyError(err);
    if (!auto) {
      showToast(tFormat("uninstall_mgr.scan_failed", { error: friendlyError(err) }));
    }
  } finally {
    scanning.value = null;
  }
}

function getAllItems(scan) {
  if (!scan) return [];
  let items = [
    ...(scan.directories || []),
    ...(scan.files || []),
    ...(scan.shortcuts || []),
    ...(scan.services || []),
  ];
  if (scan.registry_keys) {
    items = items.concat(scan.registry_keys);
  }
  return items;
}

function getCategoryIcon(cat) {
  const map = {
    config: "settings",
    cache: "history",
    log: "file",
    temp: "delete",
    data: "folder",
    shortcut: "shortcut",
    service: "tool",
    registry: "database",
  };
  return map[cat] || "file";
}

function getCategoryLabel(cat) {
  const map = {
    config: t("residue.type_config"),
    cache: t("residue.type_cache"),
    log: t("residue.type_log"),
    temp: t("residue.type_temp"),
    data: t("residue.type_data"),
    shortcut: t("residue.type_shortcut"),
    service: t("residue.type_service"),
    registry: t("residue.type_registry"),
  };
  return map[cat] || cat;
}

function formatSize(bytes) {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let size = bytes;
  while (size >= 1024 && i < units.length - 1) {
    size /= 1024;
    i++;
  }
  return `${size.toFixed(1)} ${units[i]}`;
}

async function cleanSelected(appName) {
  const appScan = residueScans.value[appName];
  if (!appScan) return;

  const sel = selectedResidues.value[appName] || {};
  const paths = Object.keys(sel).filter((p) => sel[p]);
  if (paths.length === 0) {
    showToast(t("uninstall_mgr.nothing_selected"));
    return;
  }

  if (
    !(await showConfirm(
      tFormat("uninstall_mgr.confirm_clean", { count: paths.length }) ||
        `Clean ${paths.length} selected residue item(s)?`
    ))
  )
    return;

  cleaningResidues.value = appName;
  try {
    const result = await invoke("clean_specific_residues", { items: paths });
    showToast(result);
    await scanResidues({ name: appName });
  } catch (err) {
    showToast(tFormat("uninstall_mgr.clean_failed", { error: friendlyError(err) }));
  } finally {
    cleaningResidues.value = null;
  }
}

function toggleScan(app) {
  if (residueScans.value[app.name]) {
    delete residueScans.value[app.name];
    delete selectedResidues.value[app.name];
  } else {
    scanResidues(app);
  }
}

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase();
  let list = apps.value;
  if (sourceFilter.value !== "all") {
    list = list.filter((a) => a.source === sourceFilter.value);
  }
  if (q) {
    list = list.filter(
      (a) =>
        a.name.toLowerCase().includes(q) ||
        a.source.toLowerCase().includes(q) ||
        a.version.toLowerCase().includes(q)
    );
  }
  return list;
});

const sources = computed(() => [...new Set(apps.value.map((a) => a.source))].sort());

const residueKeys = (scan) => [
  "directories",
  "files",
  "shortcuts",
  "services",
  ...(scan.registry_keys ? ["registry_keys"] : []),
];

function toggleAllInKey(appName, key, items) {
  const sel = { ...(selectedResidues.value[appName] || {}) };
  const allSelected = items.every((item) => sel[item.path]);
  for (const item of items) {
    if (item.is_safe_to_delete) {
      if (allSelected) delete sel[item.path];
      else sel[item.path] = true;
    }
  }
  selectedResidues.value[appName] = sel;
}

function toggleItem(appName, path, checked) {
  const sel = { ...(selectedResidues.value[appName] || {}) };
  if (checked) sel[path] = true;
  else delete sel[path];
  selectedResidues.value[appName] = sel;
}

const isSelected = (appName, path) => !!selectedResidues.value[appName]?.[path];

onMounted(() => {
  loadApps();
});
</script>

<template>
  <div class="page uninstall-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("uninstall_mgr.title") }}</h1>
        <p class="page-desc">{{ t("uninstall_mgr.desc") }}</p>
      </div>
      <Button variant="outline" @click="loadApps">
        <AppIcon name="refresh" class="size-4" />
        {{ t("common.refresh") }}
      </Button>
    </div>

    <!-- Search & Filter -->
    <div class="mb-4 flex items-center gap-3">
      <div class="relative flex-1">
        <AppIcon
          name="search"
          class="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          v-model="search"
          :placeholder="t('uninstall_mgr.search')"
          class="pl-9"
        />
      </div>
      <Select v-model="sourceFilter">
        <SelectTrigger class="w-[180px]">
          <SelectValue :placeholder="t('uninstall_mgr.all_sources')" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">{{ t("uninstall_mgr.all_sources") }}</SelectItem>
          <SelectItem v-for="src in sources" :key="src" :value="src">{{ src }}</SelectItem>
        </SelectContent>
      </Select>
    </div>

    <!-- App List -->
    <Card class="shadow-sm">
      <!-- Loading -->
      <CardContent v-if="loading" class="space-y-3 py-4">
        <Skeleton class="h-10 w-full" />
        <Skeleton class="h-10 w-full" />
        <Skeleton class="h-10 w-full" />
      </CardContent>

      <!-- Error -->
      <CardContent v-else-if="error" class="py-4">
        <Alert variant="destructive">
          <AppIcon name="close-circle-fill" class="size-4" />
          <AlertDescription>{{ error }}</AlertDescription>
        </Alert>
        <Button variant="default" class="mt-3" @click="loadApps">
          {{ t("common.retry") }}
        </Button>
      </CardContent>

      <!-- Empty -->
      <CardContent v-else-if="apps.length === 0" class="py-4">
        <Empty class="py-5">
          <EmptyMedia>
            <AppIcon name="delete" class="size-10 text-muted-foreground/60" />
          </EmptyMedia>
          <EmptyContent>
            <EmptyDescription>
              {{ t("uninstall_mgr.no_apps") }}
            </EmptyDescription>
          </EmptyContent>
        </Empty>
      </CardContent>

      <template v-else>
        <!-- Column headers -->
        <div class="flex items-center border-b border-border px-4 py-2 text-xs font-medium text-muted-foreground">
          <div class="col-name">{{ t("uninstall_mgr.app_name") }}</div>
          <div class="col-version">{{ t("uninstall_mgr.version") }}</div>
          <div class="col-source">{{ t("uninstall_mgr.source") }}</div>
          <div class="col-actions">{{ t("common.actions") }}</div>
        </div>

        <!-- Rows -->
        <div
          v-for="app in filtered"
          :key="`${app.name}-${app.source}`"
          class="border-b border-border last:border-b-0"
        >
          <!-- Main row -->
          <div class="flex items-center px-4 py-3 transition-colors hover:bg-muted">
            <!-- Name -->
            <div class="col-name">
              <img
                v-if="app.icon"
                :src="app.icon"
                alt=""
                class="size-7 shrink-0 rounded-md object-contain"
                loading="lazy"
                @error="onIconError"
              />
              <span v-else class="flex size-7 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
                <AppIcon name="apps" class="size-4" />
              </span>
              <span class="min-w-0 truncate text-sm font-medium text-foreground">{{ app.name }}</span>
              <Badge
                v-if="residueScans[app.name]"
                variant="outline"
                class="shrink-0 text-warning"
              >
                {{ tFormat("uninstall_mgr.residues_found", { count: residueScans[app.name].total_items }) }}
              </Badge>
            </div>

            <!-- Version -->
            <div class="col-version">
              <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                {{ app.version }}
              </code>
            </div>

            <!-- Source -->
            <div class="col-source">
              <Badge variant="secondary">{{ app.source }}</Badge>
            </div>

            <!-- Actions -->
            <div class="col-actions">
              <Button
                variant="outline"
                size="sm"
                :disabled="cleaningResidues === app.name"
                @click="toggleScan(app)"
              >
                <Spinner v-if="scanning === app.name" class="size-3.5" />
                <AppIcon v-else name="search" class="size-3.5" />
                {{ residueScans[app.name] ? t("uninstall_mgr.close_scan") : t("uninstall_mgr.residue_scan") }}
              </Button>
              <Button
                variant="destructive"
                size="sm"
                :disabled="uninstalling !== null || scanning === app.name"
                @click="handleUninstall(app)"
              >
                <Spinner v-if="uninstalling === app.name" class="size-3.5" />
                {{ t("uninstall_mgr.uninstall") }}
              </Button>
            </div>
          </div>

          <!-- Residue scan panel (expandable) -->
          <div v-if="residueScans[app.name]" class="border-t border-border bg-muted px-4 py-3">
            <Alert v-if="scanErrors[app.name]" variant="destructive" class="mb-3">
              <AlertDescription>{{ scanErrors[app.name] }}</AlertDescription>
            </Alert>
            <template v-else>
              <!-- Summary bar -->
              <div class="mb-3 flex items-center justify-between">
                <div class="flex items-center gap-4 text-xs text-muted-foreground">
                  <span>
                    <span class="font-semibold text-foreground">{{ residueScans[app.name].total_items }}</span>
                    {{ t("uninstall_mgr.residues_count") }}
                  </span>
                  <span>
                    {{ t("uninstall_mgr.total_size") }}
                    <span class="font-semibold text-foreground">{{ formatSize(residueScans[app.name].total_size) }}</span>
                  </span>
                </div>
                <div class="flex gap-2">
                  <Button
                    variant="default"
                    size="sm"
                    :disabled="cleaningResidues !== null || scanning !== null"
                    @click="cleanSelected(app.name)"
                  >
                    <Spinner v-if="cleaningResidues === app.name" class="size-3.5" />
                    <AppIcon v-else name="clean" class="size-3.5" />
                    {{ t("uninstall_mgr.clean_selected") }}
                  </Button>
                  <Button
                    variant="destructive"
                    size="sm"
                    :disabled="uninstalling !== null || cleaningResidues !== null"
                    @click="handleForceUninstall(app)"
                  >
                    <Spinner v-if="uninstalling === app.name" class="size-3.5" />
                    <AppIcon v-else name="delete" class="size-3.5" />
                    {{ t("uninstall_mgr.force_uninstall") }}
                  </Button>
                </div>
              </div>

              <!-- No residues -->
              <Empty
                v-if="getAllItems(residueScans[app.name]).length === 0"
                class="py-4"
              >
                <EmptyMedia>
                  <AppIcon name="check-circle" class="size-8 text-success" />
                </EmptyMedia>
                <EmptyContent>
                  <EmptyDescription>
                    {{ t("uninstall_mgr.no_residues") }}
                  </EmptyDescription>
                </EmptyContent>
              </Empty>

              <!-- Residue items grouped by category -->
              <div
                v-for="key in residueKeys(residueScans[app.name])"
                v-else
                :key="key"
                class="mb-2"
              >
                <template v-if="residueScans[app.name][key] && residueScans[app.name][key].length > 0">
                  <button
                    type="button"
                    class="mb-1 flex cursor-pointer items-center gap-1.5 border-0 bg-transparent p-0 text-xs font-medium text-foreground hover:text-primary"
                    @click="toggleAllInKey(app.name, key, residueScans[app.name][key])"
                  >
                    <AppIcon v-if="key === 'directories'" name="folder" class="size-3.5" />
                    <AppIcon v-else-if="key === 'files'" name="file" class="size-3.5" />
                    <AppIcon v-else-if="key === 'shortcuts'" name="shortcut" class="size-3.5" />
                    <AppIcon v-else-if="key === 'services'" name="tool" class="size-3.5" />
                    <AppIcon v-else name="database" class="size-3.5" />
                    {{
                      t("residue.category_" + (key === "registry_keys" ? "registry" : key))
                    }}
                    <span class="font-normal text-muted-foreground">({{ residueScans[app.name][key].length }})</span>
                  </button>
                  <div
                    v-for="item in residueScans[app.name][key]"
                    :key="item.path"
                    class="flex items-center gap-2 rounded-md px-2 py-1 text-xs hover:bg-accent"
                  >
                    <Checkbox
                      :model-value="isSelected(app.name, item.path)"
                      :disabled="!item.is_safe_to_delete"
                      @update:model-value="(c) => toggleItem(app.name, item.path, c)"
                    />
                    <AppIcon v-if="item.category === 'config'" name="settings" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else-if="item.category === 'cache'" name="history" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else-if="item.category === 'log'" name="file" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else-if="item.category === 'temp'" name="delete" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else-if="item.category === 'data'" name="folder" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else-if="item.category === 'shortcut'" name="shortcut" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else-if="item.category === 'service'" name="tool" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else-if="item.category === 'registry'" name="database" class="size-4 shrink-0 text-muted-foreground" />
                    <AppIcon v-else name="file" class="size-4 shrink-0 text-muted-foreground" />
                    <span class="min-w-0 flex-1 truncate text-muted-foreground" :title="item.path">{{ item.path }}</span>
                    <span class="shrink-0 font-mono text-muted-foreground">{{ item.size > 0 ? formatSize(item.size) : "" }}</span>
                    <Badge v-if="!item.is_safe_to_delete" variant="outline" class="text-warning">
                      {{ t("uninstall_mgr.caution") }}
                    </Badge>
                  </div>
                </template>
              </div>
            </template>
          </div>
        </div>

        <!-- Footer count -->
        <div class="flex items-center justify-end border-t border-border px-4 py-2 text-xs text-muted-foreground">
          <span>
            {{ filtered.length }} / {{ apps.length }} {{ t("uninstall_mgr.apps_count") }}
          </span>
        </div>
      </template>
    </Card>
  </div>
</template>

<style scoped>
.col-name {
  width: 40%;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 10px;
}
.col-version {
  width: 18%;
  text-align: right;
}
.col-source {
  width: 16%;
  text-align: right;
}
.col-actions {
  width: 26%;
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}
</style>