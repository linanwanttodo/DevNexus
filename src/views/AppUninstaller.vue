<script setup>
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";

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
    const result = await invoke("uninstall_software_deep", {
      packageName: app.name,
      appName: app.name,
    });
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
    config: "icon-settings",
    cache: "icon-history",
    log: "icon-file",
    temp: "icon-delete",
    data: "icon-folder",
    shortcut: "icon-shortcut",
    service: "icon-tool",
    registry: "icon-database",
  };
  return map[cat] || "icon-file";
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
      <a-button @click="loadApps">
        <template #icon><icon-refresh /></template>
        {{ t("common.refresh") }}
      </a-button>
    </div>

    <!-- Search & Filter -->
    <div class="toolbar-row">
      <a-input
        v-model="search"
        allow-clear
        :placeholder="t('uninstall_mgr.search')"
        class="search-input"
      >
        <template #prefix><icon-search /></template>
      </a-input>
      <a-select v-model="sourceFilter" class="source-select">
        <a-option value="all">{{ t("uninstall_mgr.all_sources") }}</a-option>
        <a-option v-for="src in sources" :key="src" :value="src">{{ src }}</a-option>
      </a-select>
    </div>

    <!-- App List -->
    <a-card :bordered="true" class="app-list-card">
      <a-spin :loading="loading" class="w-full">
        <!-- Error -->
        <a-result
          v-if="error"
          status="error"
          :title="error"
        >
          <template #extra>
            <a-button type="primary" @click="loadApps">{{ t("common.retry") }}</a-button>
          </template>
        </a-result>

        <!-- Empty -->
        <a-empty v-else-if="apps.length === 0" :description="t('uninstall_mgr.no_apps')">
          <template #image>
            <icon-delete class="empty-icon-img" />
          </template>
        </a-empty>

        <template v-else>
          <!-- Column headers -->
          <div class="list-header">
            <div class="col-name">{{ t("uninstall_mgr.app_name") }}</div>
            <div class="col-version">{{ t("uninstall_mgr.version") }}</div>
            <div class="col-source">{{ t("uninstall_mgr.source") }}</div>
            <div class="col-actions">{{ t("common.actions") }}</div>
          </div>

          <!-- Rows -->
          <div v-for="app in filtered" :key="`${app.name}-${app.source}`" class="app-item">
            <!-- Main row -->
            <div class="app-row">
              <!-- Name -->
              <div class="col-name">
                <img
                  v-if="app.icon"
                  :src="app.icon"
                  alt=""
                  class="app-icon"
                  loading="lazy"
                  @error="(e) => (e.target.style.display = 'none')"
                />
                <span v-else class="app-icon-fallback">
                  <icon-apps />
                </span>
                <span class="app-name">{{ app.name }}</span>
                <a-tag
                  v-if="residueScans[app.name]"
                  color="orange"
                  size="small"
                  class="residue-badge"
                >
                  {{ tFormat("uninstall_mgr.residues_found", { count: residueScans[app.name].total_items }) }}
                </a-tag>
              </div>

              <!-- Version -->
              <div class="col-version">
                <a-typography-text code>{{ app.version }}</a-typography-text>
              </div>

              <!-- Source -->
              <div class="col-source">
                <a-tag color="arcoblue" size="small">{{ app.source }}</a-tag>
              </div>

              <!-- Actions -->
              <div class="col-actions">
                <a-button
                  size="mini"
                  :loading="scanning === app.name"
                  :disabled="cleaningResidues === app.name"
                  @click="toggleScan(app)"
                >
                  <template #icon><icon-search /></template>
                  {{ residueScans[app.name] ? t("uninstall_mgr.close_scan") : t("uninstall_mgr.residue_scan") }}
                </a-button>
                <a-button
                  size="mini"
                  status="danger"
                  :loading="uninstalling === app.name"
                  :disabled="uninstalling !== null || scanning === app.name"
                  @click="handleUninstall(app)"
                >
                  {{ t("uninstall_mgr.uninstall") }}
                </a-button>
              </div>
            </div>

            <!-- Residue scan panel (expandable) -->
            <div v-if="residueScans[app.name]" class="residue-panel">
              <a-alert
                v-if="scanErrors[app.name]"
                type="error"
                :message="scanErrors[app.name]"
              />
              <template v-else>
                <!-- Summary bar -->
                <div class="residue-summary">
                  <div class="summary-stats">
                    <span>
                      <span class="stat-num">{{ residueScans[app.name].total_items }}</span>
                      {{ t("uninstall_mgr.residues_count") }}
                    </span>
                    <span>
                      {{ t("uninstall_mgr.total_size") }}
                      <span class="stat-num">{{ formatSize(residueScans[app.name].total_size) }}</span>
                    </span>
                  </div>
                  <div class="summary-actions">
                    <a-button
                      size="mini"
                      :loading="cleaningResidues === app.name"
                      :disabled="cleaningResidues !== null || scanning !== null"
                      @click="cleanSelected(app.name)"
                    >
                      <template #icon><icon-clean /></template>
                      {{ t("uninstall_mgr.clean_selected") }}
                    </a-button>
                    <a-button
                      size="mini"
                      status="danger"
                      :loading="uninstalling === app.name"
                      :disabled="uninstalling !== null || cleaningResidues !== null"
                      @click="handleForceUninstall(app)"
                    >
                      <template #icon><icon-delete /></template>
                      {{ t("uninstall_mgr.force_uninstall") }}
                    </a-button>
                  </div>
                </div>

                <!-- Residue items grouped by category -->
                <a-empty
                  v-if="getAllItems(residueScans[app.name]).length === 0"
                  :description="t('uninstall_mgr.no_residues')"
                >
                  <template #image>
                    <icon-check-circle class="empty-ok-icon" />
                  </template>
                </a-empty>

                <div
                  v-for="key in residueKeys(residueScans[app.name])"
                  v-else
                  :key="key"
                  class="residue-group"
                >
                  <template v-if="residueScans[app.name][key] && residueScans[app.name][key].length > 0">
                    <button
                      type="button"
                      class="group-toggle"
                      @click="toggleAllInKey(app.name, key, residueScans[app.name][key])"
                    >
                      <icon-folder v-if="key === 'directories'" />
                      <icon-file v-else-if="key === 'files'" />
                      <icon-shortcut v-else-if="key === 'shortcuts'" />
                      <icon-tool v-else-if="key === 'services'" />
                      <icon-database v-else />
                      {{
                        t("residue.category_" + (key === "registry_keys" ? "registry" : key))
                      }}
                      <span class="group-count">({{ residueScans[app.name][key].length }})</span>
                    </button>
                    <div
                      v-for="item in residueScans[app.name][key]"
                      :key="item.path"
                      class="residue-item"
                    >
                      <a-checkbox
                        :model-value="isSelected(app.name, item.path)"
                        :disabled="!item.is_safe_to_delete"
                        @change="(c) => toggleItem(app.name, item.path, c)"
                      />
                      <icon-settings v-if="item.category === 'config'" class="item-icon" />
                      <icon-history v-else-if="item.category === 'cache'" class="item-icon" />
                      <icon-file v-else-if="item.category === 'log'" class="item-icon" />
                      <icon-delete v-else-if="item.category === 'temp'" class="item-icon" />
                      <icon-folder v-else-if="item.category === 'data'" class="item-icon" />
                      <icon-shortcut v-else-if="item.category === 'shortcut'" class="item-icon" />
                      <icon-tool v-else-if="item.category === 'service'" class="item-icon" />
                      <icon-database v-else-if="item.category === 'registry'" class="item-icon" />
                      <icon-file v-else class="item-icon" />
                      <span class="item-path" :title="item.path">{{ item.path }}</span>
                      <span class="item-size">{{ item.size > 0 ? formatSize(item.size) : "" }}</span>
                      <a-tag v-if="!item.is_safe_to_delete" color="orange" size="mini">
                        {{ t("uninstall_mgr.caution") }}
                      </a-tag>
                    </div>
                  </template>
                </div>
              </template>
            </div>
          </div>

          <!-- Footer count -->
          <div class="list-footer">
            <span>
              {{ filtered.length }} / {{ apps.length }} {{ t("uninstall_mgr.apps_count") }}
            </span>
          </div>
        </template>
      </a-spin>
    </a-card>
  </div>
</template>

<style scoped>
.toolbar-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}
.search-input {
  flex: 1;
}
.source-select {
  width: 180px;
}
.app-list-card {
  border-radius: var(--nx-radius-5);
}
.list-header {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  border-bottom: 1px solid var(--color-border-2);
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-3);
}
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
.app-item {
  border-bottom: 1px solid var(--color-border-2);
}
.app-item:last-child {
  border-bottom: none;
}
.app-row {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  transition: background-color 0.15s;
}
.app-row:hover {
  background-color: var(--color-fill-1);
}
.app-icon {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  object-fit: contain;
  flex-shrink: 0;
}
.app-icon-fallback {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background-color: var(--color-fill-2);
  color: var(--color-text-3);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.app-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.residue-badge {
  flex-shrink: 0;
}
.residue-panel {
  border-top: 1px solid var(--color-border-2);
  background-color: var(--color-fill-1);
  padding: 12px 16px;
}
.residue-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.summary-stats {
  display: flex;
  align-items: center;
  gap: 16px;
  font-size: 12px;
  color: var(--color-text-3);
}
.stat-num {
  font-weight: 600;
  color: var(--color-text-1);
}
.summary-actions {
  display: flex;
  gap: 8px;
}
.residue-group {
  margin-bottom: 8px;
}
.group-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-1);
  padding: 2px 0;
  margin-bottom: 4px;
}
.group-toggle:hover {
  color: rgb(var(--primary-6));
}
.group-count {
  color: var(--color-text-4);
  font-weight: 400;
}
.residue-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  border-radius: 6px;
  font-size: 12px;
}
.residue-item:hover {
  background-color: var(--color-fill-2);
}
.item-icon {
  color: var(--color-text-4);
  flex-shrink: 0;
}
.item-path {
  flex: 1;
  color: var(--color-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.item-size {
  flex-shrink: 0;
  font-family: "JetBrains Mono", monospace;
  color: var(--color-text-4);
}
.list-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 8px 16px;
  border-top: 1px solid var(--color-border-2);
  font-size: 12px;
  color: var(--color-text-3);
}
.empty-icon-img {
  font-size: 36px;
  color: var(--color-text-4);
}
.empty-ok-icon {
  font-size: 36px;
  color: rgb(var(--green-6));
}
</style>
