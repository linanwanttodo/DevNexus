<script setup>
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { showToast } from "../lib/toast.js";
import { t } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";

const activeTab = ref("export");
const environments = ref([]);
const loading = ref(true);
const error = ref(null);

// 导出
const selectedEnvs = ref([]);
const versionsMap = ref({});
const loadingVersions = ref({});
const selectedVersions = ref({});

// 导入
const importManifest = ref(null);
const importPath = ref("");
const applyVersions = ref(true);
const importing = ref(false);
const importResult = ref(null);

const versionManagedTypes = ["python", "node", "java", "go", "rust", "cpp"];

async function loadEnvironments() {
  try {
    loading.value = true;
    error.value = null;
    environments.value = await invoke("list_environments");
  } catch (err) {
    error.value = friendlyError(err);
  } finally {
    loading.value = false;
  }
}

function toggleEnv(name) {
  if (selectedEnvs.value.includes(name)) {
    selectedEnvs.value = selectedEnvs.value.filter((n) => n !== name);
    versionsMap.value[name] = undefined;
    selectedVersions.value[name] = [];
  } else {
    selectedEnvs.value = [...selectedEnvs.value, name];
    loadVersions(name);
  }
}

async function loadVersions(name) {
  const env = environments.value.find((e) => e.name === name);
  if (!env || !versionManagedTypes.includes(env.lang_type)) return;
  loadingVersions.value = { ...loadingVersions.value, [name]: true };
  try {
    const vers = await invoke("list_versions", { langType: env.lang_type });
    versionsMap.value = { ...versionsMap.value, [name]: vers || [] };
    if (!selectedVersions.value[name]) selectedVersions.value[name] = [];
  } catch (err) {
    versionsMap.value = { ...versionsMap.value, [name]: [] };
    showToast(t("migration.versions_failed").replace("{error}", friendlyError(err)), "error");
  } finally {
    loadingVersions.value = { ...loadingVersions.value, [name]: false };
  }
}

function toggleVersion(name, ver) {
  const env = environments.value.find((e) => e.name === name);
  const snap = { lang_type: env.lang_type, version: ver.version };
  const arr = [...(selectedVersions.value[name] || [])];
  const idx = arr.findIndex((v) => v.version === ver.version);
  if (idx >= 0) arr.splice(idx, 1);
  else arr.push(snap);
  selectedVersions.value = { ...selectedVersions.value, [name]: arr };
}

const selectedVersionCount = computed(() =>
  Object.values(selectedVersions.value).reduce(
    (sum, arr) => sum + (arr ? arr.length : 0),
    0
  )
);

function isVersionSelected(name, ver) {
  return (selectedVersions.value[name] || []).some((v) => v.version === ver.version);
}

async function exportMigration() {
  if (selectedEnvs.value.length === 0) {
    showToast(t("migration.select_env"), "error");
    return;
  }
  const versions = Object.values(selectedVersions.value).flat();
  try {
    const json = await invoke("export_migration", {
      selected: { environments: selectedEnvs.value, versions },
    });
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `devnexus-migration-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    showToast(t("migration.exported"));
  } catch (err) {
    showToast(t("migration.export_failed").replace("{error}", friendlyError(err)), "error");
  }
}

async function pickImportFile() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!selected) return;
    importPath.value = selected;
    importResult.value = null;
    importManifest.value = await invoke("load_migration_file", { path: importPath.value });
  } catch (err) {
    importManifest.value = null;
    showToast(t("migration.import_failed").replace("{error}", friendlyError(err)), "error");
  }
}

async function runImport() {
  if (!importManifest.value) {
    showToast(t("migration.empty_file"), "error");
    return;
  }
  importing.value = true;
  importResult.value = null;
  try {
    const json = JSON.stringify(importManifest.value);
    const result = await invoke("import_migration", {
      json,
      applyVersions: applyVersions.value,
    });
    importResult.value = result;
    showToast(
      t("migration.import_success")
        .replace("{switched}", result.switched)
        .replace("{skipped}", result.skipped)
        .replace("{failed}", result.failed)
    );
    await loadEnvironments();
  } catch (err) {
    showToast(t("migration.import_failed").replace("{error}", friendlyError(err)), "error");
  } finally {
    importing.value = false;
  }
}

onMounted(loadEnvironments);
</script>

<template>
  <div class="page migration-page">
    <!-- Header -->
    <div class="page-header">
      <div>
        <h1 class="page-title">{{ t("migration.title") }}</h1>
        <p class="page-desc">{{ t("migration.desc") }}</p>
      </div>
    </div>

    <!-- Tabs -->
    <a-tabs v-model:active-key="activeTab" type="line" class="migration-tabs">
      <a-tab-pane key="export" :title="t('migration.tab_export')">
        <div class="export-header">
          <a-button
            type="primary"
            :disabled="selectedEnvs.length === 0"
            @click="exportMigration"
          >
            <template #icon><icon-download /></template>
            {{ t("migration.export") }}
          </a-button>
        </div>

        <a-spin :loading="loading" style="width: 100%">
          <a-result v-if="error" status="error" :title="error" style="padding: 40px 0">
            <template #extra>
              <a-button type="primary" @click="loadEnvironments">{{ t("common.retry") }}</a-button>
            </template>
          </a-result>

          <a-empty
            v-else-if="environments.length === 0"
            :description="t('migration.no_envs')"
            style="padding: 40px 0"
          />

          <a-card v-else :bordered="true">
            <div class="env-list">
              <div v-for="env in environments" :key="env.name" class="env-item">
                <a-checkbox
                  :model-value="selectedEnvs.includes(env.name)"
                  @change="(v) => { if (v) { if (!selectedEnvs.includes(env.name)) toggleEnv(env.name); } else toggleEnv(env.name); }"
                  class="env-checkbox"
                >
                  <div class="env-main">
                    <div class="env-title-row">
                      <span class="env-name">{{ env.name }}</span>
                      <a-typography-text code type="secondary" style="font-size: 12px">
                        v{{ env.version }}
                      </a-typography-text>
                      <a-tag size="small">{{ env.lang_type }}</a-tag>
                    </div>
                    <div class="env-path">{{ env.path }}</div>
                  </div>
                </a-checkbox>

                <div
                  v-if="selectedEnvs.includes(env.name) && versionManagedTypes.includes(env.lang_type)"
                  class="version-select"
                >
                  <div v-if="loadingVersions[env.name]" class="version-loading">
                    <a-spin :size="14" style="margin-right: 8px" />
                    {{ t("common.loading") }}
                  </div>
                  <div v-else-if="versionsMap[env.name] && versionsMap[env.name].length > 0" class="version-pills">
                    <a-tag
                      v-for="ver in versionsMap[env.name]"
                      :key="ver.version"
                      :color="isVersionSelected(env.name, ver) ? 'arcoblue' : 'default'"
                      :checkable="true"
                      :checked="isVersionSelected(env.name, ver)"
                      @check="() => toggleVersion(env.name, ver)"
                      class="version-tag"
                    >
                      {{ ver.version }}
                    </a-tag>
                  </div>
                  <div v-else class="version-loading">{{ t("migration.no_versions") }}</div>
                </div>
              </div>
            </div>

            <template #footer>
              <div class="export-footer">
                <span class="summary-text">
                  {{
                    t("migration.summary")
                      .replace("{envs}", selectedEnvs.length)
                      .replace("{versions}", selectedVersionCount)
                  }}
                </span>
                <a-button size="small" :loading="loading" @click="loadEnvironments">
                  <template #icon><icon-refresh /></template>
                  {{ t("common.refresh") }}
                </a-button>
              </div>
            </template>
          </a-card>
        </a-spin>
      </a-tab-pane>

      <a-tab-pane key="import" :title="t('migration.tab_import')">
        <a-card :bordered="true">
          <p class="import-note">{{ t("migration.import_note") }}</p>

          <div class="import-pick-row">
            <a-button type="primary" @click="pickImportFile">
              <template #icon><icon-folder-open /></template>
              {{ t("migration.import_pick") }}
            </a-button>
            <a-typography-text v-if="importPath" code type="secondary" class="import-path">
              {{ importPath }}
            </a-typography-text>
          </div>

          <div v-if="importManifest" class="import-preview">
            <div class="preview-title">{{ t("migration.import_preview") }}</div>
            <div class="preview-grid">
              <div>{{ t("migration.exported_at") }}: {{ importManifest.meta?.exported_at || "—" }}</div>
              <div>{{ t("migration.meta_os") }}: {{ importManifest.meta?.source_os || "—" }}</div>
              <div>{{ t("migration.meta_host") }}: {{ importManifest.meta?.hostname || "—" }}</div>
              <div>DevNexus: {{ importManifest.meta?.devnexus_version || "—" }}</div>
            </div>
            <div class="preview-count">
              {{ importManifest.environments?.length || 0 }} envs ·
              {{ importManifest.versions?.length || 0 }} versions
            </div>
            <ul v-if="importManifest.environments?.length" class="preview-envs">
              <li v-for="env in importManifest.environments" :key="env.name">
                <span class="env-name">{{ env.name }}</span>
                <a-typography-text code type="secondary">{{ env.version }}</a-typography-text>
                <a-tag size="mini">{{ env.lang_type }}</a-tag>
              </li>
            </ul>
            <div v-if="importManifest.versions?.length" class="preview-versions">
              <a-tag v-for="ver in importManifest.versions" :key="ver.lang_type + '@' + ver.version" size="small">
                {{ ver.lang_type }}@{{ ver.version }}
              </a-tag>
            </div>
          </div>

          <div v-if="importManifest" class="import-actions">
            <a-checkbox v-model="applyVersions">{{ t("migration.apply_versions") }}</a-checkbox>
            <a-button type="primary" :loading="importing" @click="runImport">
              <template #icon><icon-upload /></template>
              {{ t("migration.import") }}
            </a-button>
          </div>

          <div v-if="importResult" class="import-result">
            <div class="result-title">
              switched {{ importResult.switched }} · skipped {{ importResult.skipped }} ·
              failed {{ importResult.failed }}
            </div>
            <ul class="result-list">
              <li v-for="(line, i) in importResult.details" :key="i" class="result-line">
                {{ line }}
              </li>
            </ul>
          </div>
        </a-card>
      </a-tab-pane>
    </a-tabs>
  </div>
</template>

<style scoped>
.export-header {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 12px;
}
.env-list {
  display: flex;
  flex-direction: column;
}
.env-item {
  padding: 12px 4px;
  border-bottom: 1px solid var(--color-border);
}
.env-item:last-child {
  border-bottom: none;
}
.env-checkbox {
  width: 100%;
}
.env-main {
  min-width: 0;
}
.env-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.env-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
}
.env-path {
  margin-top: 2px;
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.version-select {
  margin: 8px 0 0 28px;
}
.version-loading {
  display: flex;
  align-items: center;
  font-size: 12px;
  color: var(--color-text-3);
}
.version-pills {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.version-tag {
  cursor: pointer;
}
.export-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.summary-text {
  font-size: 12px;
  color: var(--color-text-3);
}
.import-note {
  font-size: 12px;
  color: var(--color-text-3);
  margin: 0 0 14px;
}
.import-pick-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}
.import-path {
  font-size: 12px;
  max-width: 400px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.import-preview {
  margin-top: 16px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background-color: var(--color-fill-1);
  padding: 12px;
}
.preview-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-bottom: 8px;
}
.preview-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 4px;
  font-size: 12px;
  color: var(--color-text-3);
}
.preview-count {
  margin-top: 10px;
  font-size: 12px;
  color: var(--color-text-1);
}
.preview-envs {
  max-height: 150px;
  overflow-y: auto;
  margin: 8px 0 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.preview-envs li {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}
.preview-versions {
  margin-top: 8px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.import-actions {
  margin-top: 16px;
  display: flex;
  align-items: center;
  gap: 16px;
}
.import-result {
  margin-top: 16px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 12px;
}
.result-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-bottom: 8px;
}
.result-list {
  max-height: 190px;
  overflow-y: auto;
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.result-line {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-3);
}
</style>
