<script setup>
import { ref, reactive, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";

const environments = ref([]);
const loading = ref(true);
const error = ref(null);
const showCreateModal = ref(false);
const newEnvName = ref("");
const newEnvPath = ref("");
const creating = ref(false);
const refreshingAll = ref(false);

// 展开/版本状态
const expanded = reactive({});
const versionsMap = reactive({});
const loadingVersions = reactive({});
const switchingVersion = reactive({});
const refreshing = reactive({});

// 支持版本管理的语言类型
const versionManagedTypes = ["python", "node", "java", "go", "rust", "cpp"];

async function loadEnvironments() {
  try {
    loading.value = true;
    error.value = null;
    environments.value = await invoke("list_environments");
  } catch (err) {
    error.value = friendlyError(err);
    console.error("Error loading environments:", err);
  } finally {
    loading.value = false;
  }
}

async function exportEnvironments() {
  try {
    const filePath = await save({
      filters: [{ name: "JSON", extensions: ["json"] }],
      defaultPath: `devnexus-environments-${new Date().toISOString().slice(0, 10)}.json`,
    });
    if (!filePath) return; // 用户取消
    const msg = await invoke("save_export_file", { path: filePath });
    showToast(msg, "success");
  } catch (err) {
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)), "error");
  }
}

async function toggleExpand(env) {
  if (!expanded[env.name]) {
    expanded[env.name] = true;
    await loadVersions(env);
    const versions = versionsMap[env.name];
    if (!versions || versions.length <= 1) {
      expanded[env.name] = false;
    }
  } else {
    expanded[env.name] = false;
  }
}

async function loadVersions(env, forceRefresh = false) {
  loadingVersions[env.name] = true;
  try {
    versionsMap[env.name] = await invoke("list_versions", {
      langType: env.lang_type,
      forceRefresh: forceRefresh || undefined,
    });
  } catch (err) {
    console.error(`Error loading versions for ${env.name}:`, err);
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)));
    versionsMap[env.name] = [];
  } finally {
    loadingVersions[env.name] = false;
  }
}

async function refreshVersions(env) {
  refreshing[env.name] = true;
  try {
    await loadVersions(env, true);
    showToast(t("common.all_refreshed"));
  } finally {
    refreshing[env.name] = false;
  }
}

async function refreshAll() {
  refreshingAll.value = true;
  try {
    await loadEnvironments();
    const promises = environments.value
      .filter((env) => versionManagedTypes.includes(env.lang_type) && expanded[env.name])
      .map((env) => loadVersions(env, true));
    await Promise.all(promises);
    showToast(t("common.all_refreshed"));
  } finally {
    refreshingAll.value = false;
  }
}

async function switchVersion(env, version) {
  if (switchingVersion[env.name]) return;
  switchingVersion[env.name] = true;
  try {
    const result = await invoke("switch_version", {
      langType: env.lang_type,
      version: version.version,
    });
    showToast(result);
    await loadVersions(env, true);
    if (versionsMap[env.name]) {
      versionsMap[env.name] = versionsMap[env.name].map((v) => ({
        ...v,
        is_active: v.version === version.version,
      }));
    }
    await loadEnvironments();
  } catch (err) {
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)));
  } finally {
    switchingVersion[env.name] = false;
  }
}

async function addToPath(env) {
  try {
    const result = await invoke("add_to_path", { envName: env.name, path: env.path });
    showToast(result);
    await loadEnvironments();
  } catch (err) {
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)));
  }
}

async function removeFromPath(env) {
  if (!(await showConfirm(tFormat("environments.remove_from_path_confirm", { name: env.name })))) return;
  try {
    const result = await invoke("remove_from_path", { envName: env.name, path: env.path });
    showToast(result);
    await loadEnvironments();
  } catch (err) {
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)));
  }
}

function viewConfig(env) {
  if (env.shell_config) {
    showToast(tFormat("environments.config_file", { path: env.shell_config }));
  } else {
    showToast(t("environments.no_config"));
  }
}

async function createEnvironment() {
  if (!newEnvName.value.trim() || !newEnvPath.value.trim()) return;
  creating.value = true;
  try {
    const result = await invoke("add_to_path", {
      envName: newEnvName.value.trim(),
      path: newEnvPath.value.trim(),
    });
    showToast(result);
    showCreateModal.value = false;
    newEnvName.value = "";
    newEnvPath.value = "";
    await loadEnvironments();
  } catch (err) {
    showToast(t("common.error_msg").replace("{error}", friendlyError(err)));
  } finally {
    creating.value = false;
  }
}

onMounted(() => {
  loadEnvironments();
});
</script>

<template>
  <div class="page env-page">
    <!-- Header -->
    <div class="page-header">
      <h1 class="page-title">{{ t("environments.title") }}</h1>
      <div class="flex gap-2 items-center">
        <a-button :loading="refreshingAll" @click="refreshAll">
          <template #icon><icon-refresh /></template>
          {{ t("environments.refresh") }}
        </a-button>
        <a-button @click="exportEnvironments">
          <template #icon><icon-download /></template>
          {{ t("environments.export") }}
        </a-button>
        <a-button type="primary" @click="showCreateModal = true">
          <template #icon><icon-plus /></template>
          {{ t("environments.new") }}
        </a-button>
      </div>
    </div>

    <a-spin :loading="loading" style="width: 100%">
      <a-result v-if="error" status="error" :title="error" style="padding: 48px 0">
        <template #extra>
          <a-button type="primary" @click="loadEnvironments">{{ t("common.retry") }}</a-button>
        </template>
      </a-result>

      <a-empty
        v-else-if="environments.length === 0"
        :description="t('environments.none')"
        style="padding: 48px 0"
      >
        <template #description>
          <div>{{ t("environments.none") }}</div>
          <div class="empty-hint">{{ t("environments.none_hint") }}</div>
        </template>
      </a-empty>

      <a-card v-else :bordered="true" class="env-card">
        <a-table
          :data="environments"
          :pagination="false"
          :bordered="false"
          :row-key="'name'"
          size="small"
        >
          <template #columns>
            <a-table-column :title="t('environments.name')" data-index="name">
              <template #cell="{ record }">
                <div class="env-name-cell">
                  <span class="env-name">{{ record.name }}</span>
                  <a-typography-text code type="secondary" style="font-size: 12px">
                    v{{ record.version }}
                  </a-typography-text>
                </div>
              </template>
            </a-table-column>
            <a-table-column :title="t('environments.path')" data-index="path">
              <template #cell="{ record }">
                <span class="env-path">{{ record.path }}</span>
              </template>
            </a-table-column>
            <a-table-column :title="t('environments.status')" data-index="status" :width="140">
              <template #cell="{ record }">
                <a-tag color="green" size="small">
                  <template #icon><icon-check-circle /></template>
                  {{ record.status }}
                </a-tag>
              </template>
            </a-table-column>
            <a-table-column :title="t('environments.actions')" :width="130" align="right">
              <template #cell="{ record }">
                <div class="actions-row">
                  <a-tooltip :content="t('environments.add_to_path')">
                    <a-button type="text" size="mini" @click="addToPath(record)">
                      <template #icon><icon-plus /></template>
                    </a-button>
                  </a-tooltip>
                  <a-tooltip :content="t('environments.remove_from_path')">
                    <a-button type="text" size="mini" @click="removeFromPath(record)">
                      <template #icon><icon-minus /></template>
                    </a-button>
                  </a-tooltip>
                  <a-tooltip :content="t('environments.view_config')">
                    <a-button type="text" size="mini" @click="viewConfig(record)">
                      <template #icon><icon-file /></template>
                    </a-button>
                  </a-tooltip>
                </div>
              </template>
            </a-table-column>
          </template>
        </a-table>

        <template #footer>
          <div class="table-footer">
            <span class="footer-count">
              {{ tFormat("environments.count", { count: environments.length }) }}
            </span>
          </div>
        </template>
      </a-card>
    </a-spin>

    <!-- 版本展开面板（独立于表格下方渲染） -->
    <template v-for="env in environments" :key="env.name">
      <a-card
        v-if="expanded[env.name] && versionManagedTypes.includes(env.lang_type)"
        :bordered="true"
        class="version-panel"
      >
        <template #title>
          <div class="version-head">
            <span class="version-title">{{ t("environments.versions") }}</span>
            <a-button
              size="mini"
              :loading="!!refreshing[env.name]"
              @click="refreshVersions(env)"
            >
              <template #icon><icon-refresh /></template>
              {{ t("environments.refresh") }}
            </a-button>
          </div>
        </template>

        <div v-if="loadingVersions[env.name]" class="version-loading">
          <a-spin :size="14" style="margin-right: 8px" />
          {{ t("common.loading") }}
        </div>

        <div v-else-if="versionsMap[env.name] && versionsMap[env.name].length > 0" class="version-list">
          <div
            v-for="ver in versionsMap[env.name]"
            :key="ver.version"
            class="version-row"
            :class="{ active: ver.is_active }"
          >
            <div class="version-left">
              <icon-check-circle-fill v-if="ver.is_active" class="active-icon" />
              <icon-radio-button-unchecked v-else class="inactive-icon" />
              <span class="version-mono">{{ ver.version }}</span>
              <span v-if="ver.path" class="version-path">{{ ver.path }}</span>
            </div>
            <div class="version-right">
              <span v-if="ver.is_active" class="active-label">{{ t("environments.active") }}</span>
              <a-button
                v-else
                size="mini"
                type="primary"
                :loading="!!switchingVersion[env.name]"
                @click="switchVersion(env, ver)"
              >
                {{ t("environments.switch") }}
              </a-button>
            </div>
          </div>
        </div>

        <a-empty v-else :description="t('environments.no_versions')" style="padding: 16px 0" />
      </a-card>
    </template>

    <!-- Create Environment Modal -->
    <a-modal
      v-model:visible="showCreateModal"
      :title="`${t('environments.title')} - ${t('environments.new')}`"
      :on-before-ok="createEnvironment"
      :ok-button-props="{ disabled: !newEnvName.trim() || !newEnvPath.trim() || creating, loading: creating }"
    >
      <a-form layout="vertical">
        <a-form-item :label="t('environments.name')">
          <a-input v-model="newEnvName" :placeholder="t('environments.name_placeholder')" />
        </a-form-item>
        <a-form-item :label="t('environments.path')">
          <a-input v-model="newEnvPath" :placeholder="t('environments.path_placeholder')" />
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<style scoped>
.empty-hint {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 4px;
}
.env-name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}
.env-name {
  font-weight: 500;
  color: var(--color-text-1);
}
.env-path {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-text-2);
}
.actions-row {
  display: flex;
  justify-content: flex-end;
  gap: 2px;
}
.table-footer {
  padding: 10px 0;
}
.footer-count {
  font-size: 12px;
  color: var(--color-text-3);
}
.version-panel {
  margin-top: 8px;
  border-radius: 8px;
}
.version-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.version-title {
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-text-3);
}
.version-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  font-size: 12px;
  color: var(--color-text-3);
}
.version-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.version-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 12px;
  border-radius: 6px;
  border: 1px solid transparent;
  transition: background-color 0.15s ease, border-color 0.15s ease;
}
.version-row:hover {
  background-color: var(--color-fill-1);
}
.version-row.active {
  border-color: var(--color-primary-6);
  background-color: var(--color-primary-1);
}
.version-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.active-icon {
  color: var(--color-primary-6);
}
.inactive-icon {
  color: var(--color-text-4);
}
.version-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  color: var(--color-text-1);
}
.version-path {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-text-3);
}
.version-right {
  display: flex;
  align-items: center;
}
.active-label {
  font-size: 12px;
  color: var(--color-primary-6);
}
</style>
