<script setup>
import { ref, reactive, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { showToast } from "../lib/toast.js";
import { showConfirm } from "../lib/confirm.js";
import { t, tFormat } from "../lib/i18n.js";
import { friendlyError } from "../lib/errors.js";
import AppIcon from "../components/AppIcon.vue";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
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
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

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
      <div class="flex items-center gap-2">
        <Button variant="outline" :disabled="refreshingAll" @click="refreshAll">
          <AppIcon name="refresh" :spin="refreshingAll" class="size-4" />
          {{ t("environments.refresh") }}
        </Button>
        <Button variant="outline" @click="exportEnvironments">
          <AppIcon name="download" class="size-4" />
          {{ t("environments.export") }}
        </Button>
        <Button @click="showCreateModal = true">
          <AppIcon name="plus" class="size-4" />
          {{ t("environments.new") }}
        </Button>
      </div>
    </div>

    <!-- Loading -->
    <Card v-if="loading" class="shadow-sm">
      <CardContent class="space-y-3 py-4">
        <Skeleton class="h-10 w-full" />
        <Skeleton class="h-10 w-full" />
        <Skeleton class="h-10 w-full" />
      </CardContent>
    </Card>

    <!-- Error -->
    <Card v-else-if="error" class="shadow-sm">
      <CardContent class="py-4">
        <Alert variant="destructive">
          <AppIcon name="close-circle-fill" class="size-4" />
          <AlertTitle>{{ t("error.title") }}</AlertTitle>
          <AlertDescription>{{ error }}</AlertDescription>
        </Alert>
        <Button class="mt-3" @click="loadEnvironments">
          {{ t("common.retry") }}
        </Button>
      </CardContent>
    </Card>

    <!-- Empty -->
    <Card v-else-if="environments.length === 0" class="shadow-sm">
      <CardContent class="py-4">
        <Empty class="py-5">
          <EmptyMedia>
            <AppIcon name="code" class="size-10 text-muted-foreground/60" />
          </EmptyMedia>
          <EmptyContent>
            <EmptyDescription>
              <div>{{ t("environments.none") }}</div>
              <div class="empty-hint">{{ t("environments.none_hint") }}</div>
            </EmptyDescription>
          </EmptyContent>
        </Empty>
      </CardContent>
    </Card>

    <!-- Table -->
    <Card v-else class="shadow-sm">
      <CardContent class="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead class="w-[38%]">{{ t("environments.name") }}</TableHead>
              <TableHead>{{ t("environments.path") }}</TableHead>
              <TableHead class="w-[130px]">{{ t("environments.status") }}</TableHead>
              <TableHead class="w-[130px] text-right">{{ t("environments.actions") }}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="record in environments" :key="record.name">
              <TableCell>
                <div class="env-name-cell">
                  <span class="env-name">{{ record.name }}</span>
                  <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-xs text-muted-foreground">
                    v{{ record.version }}
                  </code>
                </div>
              </TableCell>
              <TableCell>
                <span class="env-path">{{ record.path }}</span>
              </TableCell>
              <TableCell>
                <Badge variant="secondary" class="gap-1">
                  <AppIcon name="check-circle" class="size-3.5 text-success" />
                  {{ record.status }}
                </Badge>
              </TableCell>
              <TableCell>
                <TooltipProvider>
                  <div class="actions-row">
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button variant="ghost" size="icon-sm" class="size-7" @click="addToPath(record)">
                          <AppIcon name="plus" class="size-3.5" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{{ t("environments.add_to_path") }}</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button variant="ghost" size="icon-sm" class="size-7" @click="removeFromPath(record)">
                          <AppIcon name="minus" class="size-3.5" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{{ t("environments.remove_from_path") }}</TooltipContent>
                    </Tooltip>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button variant="ghost" size="icon-sm" class="size-7" @click="viewConfig(record)">
                          <AppIcon name="file" class="size-3.5" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>{{ t("environments.view_config") }}</TooltipContent>
                    </Tooltip>
                  </div>
                </TooltipProvider>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        <div class="table-footer">
          <span class="footer-count">
            {{ tFormat("environments.count", { count: environments.length }) }}
          </span>
        </div>
      </CardContent>
    </Card>

    <!-- 版本展开面板（独立于表格下方渲染） -->
    <template v-for="env in environments" :key="env.name">
      <Card
        v-if="expanded[env.name] && versionManagedTypes.includes(env.lang_type)"
        class="version-panel shadow-sm"
      >
        <CardHeader class="flex-row items-center justify-between space-y-0 py-3">
          <CardTitle class="version-title">{{ t("environments.versions") }}</CardTitle>
          <Button
            variant="outline"
            size="sm"
            :disabled="!!refreshing[env.name]"
            @click="refreshVersions(env)"
          >
            <AppIcon name="refresh" :spin="!!refreshing[env.name]" class="size-3.5" />
            {{ t("environments.refresh") }}
          </Button>
        </CardHeader>

        <CardContent>
          <div v-if="loadingVersions[env.name]" class="version-loading">
            <Spinner class="mr-2 size-3.5" />
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
                <AppIcon v-if="ver.is_active" name="check-circle-fill" class="active-icon size-4" />
                <AppIcon v-else name="radio-button-unchecked" class="inactive-icon size-4" />
                <span class="version-mono">{{ ver.version }}</span>
                <span v-if="ver.path" class="version-path">{{ ver.path }}</span>
              </div>
              <div class="version-right">
                <span v-if="ver.is_active" class="active-label">{{ t("environments.active") }}</span>
                <Button
                  v-else
                  size="sm"
                  :disabled="!!switchingVersion[env.name]"
                  @click="switchVersion(env, ver)"
                >
                  <Spinner v-if="switchingVersion[env.name]" class="size-3.5" />
                  {{ t("environments.switch") }}
                </Button>
              </div>
            </div>
          </div>

          <Empty v-else class="py-4">
            <EmptyContent>
              <EmptyDescription>{{ t("environments.no_versions") }}</EmptyDescription>
            </EmptyContent>
          </Empty>
        </CardContent>
      </Card>
    </template>

    <!-- Create Environment Dialog -->
    <Dialog v-model:open="showCreateModal">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ `${t('environments.title')} - ${t('environments.new')}` }}</DialogTitle>
        </DialogHeader>
        <div class="space-y-3">
          <div class="grid gap-2">
            <Label for="new-env-name">{{ t("environments.name") }}</Label>
            <Input
              id="new-env-name"
              v-model="newEnvName"
              :placeholder="t('environments.name_placeholder')"
            />
          </div>
          <div class="grid gap-2">
            <Label for="new-env-path">{{ t("environments.path") }}</Label>
            <Input
              id="new-env-path"
              v-model="newEnvPath"
              :placeholder="t('environments.path_placeholder')"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" @click="showCreateModal = false">
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="!newEnvName.trim() || !newEnvPath.trim() || creating"
            @click="createEnvironment"
          >
            <Spinner v-if="creating" class="size-4" />
            {{ t("common.confirm") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<style scoped>
.empty-hint {
  font-size: 12px;
  color: var(--color-muted-foreground);
  margin-top: 4px;
}
.env-name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}
.env-name {
  font-weight: 500;
  color: var(--color-foreground);
}
.env-path {
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.actions-row {
  display: flex;
  justify-content: flex-end;
  gap: 2px;
}
.table-footer {
  padding: 10px 14px;
}
.footer-count {
  font-size: 12px;
  color: var(--color-muted-foreground);
}
.version-panel {
  margin-top: 8px;
}
.version-title {
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--color-muted-foreground);
}
.version-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  font-size: 12px;
  color: var(--color-muted-foreground);
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
  background-color: var(--color-muted);
}
.version-row.active {
  border-color: var(--color-primary);
  background-color: color-mix(in srgb, var(--color-primary) 10%, transparent);
}
.version-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.active-icon {
  color: var(--color-primary);
}
.inactive-icon {
  color: var(--color-muted-foreground);
}
.version-mono {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  color: var(--color-foreground);
}
.version-path {
  font-family: "JetBrains Mono", monospace;
  font-size: 11px;
  color: var(--color-muted-foreground);
}
.version-right {
  display: flex;
  align-items: center;
}
.active-label {
  font-size: 12px;
  color: var(--color-primary);
}
</style>